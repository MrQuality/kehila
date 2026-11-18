//! Selection reads release snapshot ownership before bounded CPU admission.
use super::native_delivery::EncodedReply;
use super::*;
use crate::lifecycle::{FailureDetail, FatalCause};
use crate::postgres::transaction::{
    NativeError, SelectionDraft, SelectionFailure, SelectionIdentity, SelectionPacket,
    SelectionPacketKind, SelectionResolution, SelectionSaveOutcome,
};
use serde_json::json;

fn notice(value: SelectionResolution) -> Result<EncodedReply, Error> {
    use SelectionResolution::*;
    let (status, value) = match value {
        Absent => (200, json!({"codec_version":1,"outcome":"absent"})),
        Expired => (410, json!({"codec_version":1,"outcome":"replay_expired"})),
        Reused => (
            409,
            json!({"codec_version":1,"outcome":"rejected","code":"operation_id_reused"}),
        ),
        Conflict(version) => (
            409,
            json!({"codec_version":1,"outcome":"rejected","code":"selection_version_conflict","current_version":version.to_string()}),
        ),
        Archived => (
            409,
            json!({"codec_version":1,"outcome":"rejected","code":"archived_target"}),
        ),
        InvalidTarget => (
            404,
            json!({"codec_version":1,"outcome":"rejected","code":"invalid_reference"}),
        ),
        Exhausted => (503, json!({"error":"preparation_unavailable"})),
    };
    bounded_reply(status, value)
}
fn bounded_reply(status: u16, value: serde_json::Value) -> Result<EncodedReply, Error> {
    let encoded = EncodedReply::admit(reply(status, value))?;
    if encoded.encoded_len() > selection_codec::MAX_WIRE_BYTES {
        return Err(Error::Unavailable);
    }
    Ok(encoded)
}
// This owner alone pairs the consumed storage packet with its preadmitted
// receipt. No success encoding or payload lookup is permitted after release.
struct PreparedCommand {
    packet: SelectionPacket,
    success: Option<EncodedReply>,
    observed: Option<EncodedReply>,
    expired: EncodedReply,
    reused: EncodedReply,
    exhausted: EncodedReply,
}
impl PreparedCommand {
    fn admit(draft: SelectionDraft, live: &RuntimeLiveness) -> Result<Self, Error> {
        let packet = draft
            .admit()
            .map_err(|error| super::project::native_error(live, error))?;
        let success = packet
            .reply()
            .map(|bytes| {
                let value =
                    project_create_codec::parse_json(bytes).map_err(|_| Error::Unavailable)?;
                bounded_reply(200, value)
            })
            .transpose()?;
        let observed = match packet.kind() {
            SelectionPacketKind::Notice(value) => Some(notice(value)?),
            _ => None,
        };
        Ok(Self {
            packet,
            success,
            observed,
            expired: notice(SelectionResolution::Expired)?,
            reused: notice(SelectionResolution::Reused)?,
            exhausted: notice(SelectionResolution::Exhausted)?,
        })
    }
}
enum Written {
    Committed(Snapshot),
    Reprepare(SelectionIdentity, Snapshot),
    Historical(SelectionIdentity),
    Notice(SelectionResolution, Snapshot),
}

fn database_detail(error: &DbError) -> FailureDetail {
    let (kind, code, name) = match error {
        DbError::Invalid => ("invalid database request", None, None),
        DbError::Busy => ("database busy", None, None),
        DbError::Expired => ("database deadline", None, None),
        DbError::Unavailable => ("database unavailable", None, None),
        DbError::UnknownCommit => ("database outcome unknown", None, None),
        DbError::Rejected(code) => ("database rejection", Some(code.as_str()), None),
        DbError::Constraint { code, name } => (
            "database constraint",
            Some(code.as_str()),
            Some(name.as_str()),
        ),
    };
    FailureDetail::new(kind, code, name)
}
fn native_detail(error: &crate::postgres::transaction::NativeError) -> FailureDetail {
    use crate::postgres::transaction::NativeError;
    match error {
        NativeError::Database(error) => database_detail(error),
        NativeError::Integrity => FailureDetail::new("native integrity", None, None),
        NativeError::Rejected(reason) => FailureDetail::new(
            match reason {
                crate::project::Failure::Forbidden => "native forbidden",
                crate::project::Failure::Absent => "native absent",
                crate::project::Failure::Conflict => "native conflict",
                crate::project::Failure::Expired => "native expired",
                crate::project::Failure::Cancelled => "native cancelled",
                crate::project::Failure::OutcomeUnknown => "native outcome unknown",
            },
            None,
            None,
        ),
    }
}

impl Engine {
    fn selection_result<T>(
        &self,
        primary: Result<T, NativeError>,
        cleanup: Result<(), DbError>,
    ) -> Result<T, Error> {
        if let Err(cleanup) = cleanup {
            self.live.fail(FatalCause::NativeRollback {
                primary: primary.as_ref().err().map(native_detail),
                cleanup: database_detail(&cleanup),
            });
            return Err(Error::Unavailable);
        }
        primary.map_err(|error| super::project::native_error(&self.live, error))
    }
    fn selection_eligibility_cleanup(&self, primary: Error, cleanup: Result<(), DbError>) -> Error {
        match cleanup {
            Ok(()) => primary,
            Err(cleanup) => {
                self.live.fail(FatalCause::NativeRollback {
                    primary: Some(FailureDetail::new(
                        if primary == Error::Unauthorized {
                            "session unauthorized"
                        } else {
                            "session eligibility rejected"
                        },
                        None,
                        None,
                    )),
                    cleanup: database_detail(&cleanup),
                });
                Error::Unavailable
            }
        }
    }
    fn selection_failure(&self, failure: SelectionFailure) -> Error {
        match failure {
            SelectionFailure::Operation { primary, rollback } => self
                .selection_result::<()>(Err(primary), rollback.map_or(Ok(()), Err))
                .unwrap_err(),
            SelectionFailure::Rollback(error) => {
                self.selection_result(Ok(()), Err(error)).unwrap_err()
            }
        }
    }
    pub(super) async fn selection_command(
        &self,
        credential: Credential,
        input: crate::selection_ingress::Input,
        execution: &jobs::Execution,
    ) -> Result<Output, Error> {
        let crate::selection_ingress::Input::Command {
            reconcile,
            token,
            expected_version,
            requested,
        } = input
        else {
            return Err(Error::Invalid);
        };
        let deadline = execution.deadline();
        self.check(deadline)?;
        let entry = self.candidate(&credential)?;
        self.project_eligibility(
            &entry,
            &credential,
            &Snapshot {
                binding: entry.binding.clone(),
                actor_active: true,
            },
        )?;
        let installation = entry.binding.installation.clone();
        let actor = entry.binding.owner.clone();
        let canonical = self
            .hash(deadline, |worker| {
                worker.owned_preparation(deadline, execution.admission(), move || {
                    crate::selection::Canonical::admit(
                        &installation,
                        &actor,
                        task_contract::current_work::SelectionCommand {
                            user_id: task_contract::current_work::UserId(actor.clone()),
                            operation_id: token,
                            expected_version,
                            requested,
                        },
                    )
                })
            })
            .await?
            .map_err(|_| Error::Invalid)?;
        let canonical = Arc::new(canonical);
        let mut original = None;
        let mut historical_only = reconcile;
        let mut additional = 0;
        #[cfg(test)]
        let mut selected_commit = None;
        loop {
            self.check(deadline)?;
            let (draft, observed) = {
                #[cfg(test)]
                self.live
                    .probe
                    .record(crate::lifecycle::probe::Stage::NativeMutex);
                let mut database = tokio::time::timeout_at(
                    tokio::time::Instant::from_std(deadline),
                    self.database.lock(),
                )
                .await
                .map_err(|_| Error::Unavailable)?;
                let b = &entry.binding;
                let mut transaction = database
                    .read_snapshot(
                        &b.installation,
                        b.runtime_generation,
                        b.auth_generation,
                        deadline,
                    )
                    .await
                    .map_err(fence_error)?;
                let observed = snapshot(transaction.authority());
                if let Err(primary) = self.project_eligibility(&entry, &credential, &observed) {
                    return Err(
                        self.selection_eligibility_cleanup(primary, transaction.rollback().await)
                    );
                }
                let draft = transaction
                    .prepare_selection(Arc::clone(&canonical), original.take(), historical_only)
                    .await;
                #[cfg(test)]
                if matches!(&draft, Err(NativeError::Database(DbError::Rejected(code))) if code == "42501")
                {
                    self.live
                        .probe
                        .transaction_point(
                            0,
                            crate::lifecycle::probe::Point::SelectionPreparationCleanup,
                            deadline,
                        )
                        .await;
                }
                let cleanup = transaction.rollback().await;
                drop(database);
                (self.selection_result(draft, cleanup)?, observed)
            };
            let live = Arc::clone(&self.live);
            let prepared = self
                .hash(deadline, |worker| {
                    worker.owned_preparation(deadline, execution.admission(), move || {
                        PreparedCommand::admit(draft, &live)
                    })
                })
                .await??;
            match prepared.packet.kind() {
                SelectionPacketKind::Notice(_) => {
                    let body = prepared.observed.ok_or(Error::Unavailable)?;
                    self.project_eligibility(&entry, &credential, &observed)?;
                    return self
                        .native_delivery_encoded(entry, observed, body, deadline)
                        .map(Output::Selection);
                }
                SelectionPacketKind::Historical => {
                    #[cfg(test)]
                    self.live
                        .probe
                        .transaction_point(
                            0,
                            crate::lifecycle::probe::Point::SelectionHistoryPrepared,
                            deadline,
                        )
                        .await;
                    let PreparedCommand {
                        packet,
                        success,
                        expired,
                        reused,
                        ..
                    } = prepared;
                    let (outcome, fresh) = self
                        .selection_history_release(&entry, &credential, packet, deadline)
                        .await?;
                    let body = match outcome {
                        None => success.ok_or(Error::Unavailable)?,
                        Some(SelectionResolution::Expired) => expired,
                        Some(SelectionResolution::Reused) => reused,
                        _ => {
                            self.live.fail(FatalCause::NativeIntegrity);
                            return Err(Error::Unavailable);
                        }
                    };
                    self.project_eligibility(&entry, &credential, &fresh)?;
                    return self
                        .native_delivery_encoded(entry, fresh, body, deadline)
                        .map(Output::Selection);
                }
                SelectionPacketKind::Fresh => {
                    #[cfg(test)]
                    if selected_commit.is_none() {
                        selected_commit =
                            self.live.probe.select_selection_commit(canonical.bytes());
                    }
                    #[cfg(test)]
                    self.live
                        .probe
                        .transaction_point(
                            0,
                            crate::lifecycle::probe::Point::SelectionPrepared,
                            deadline,
                        )
                        .await;
                    let PreparedCommand {
                        packet,
                        success,
                        exhausted,
                        ..
                    } = prepared;
                    match self
                        .selection_write(
                            &entry,
                            &credential,
                            packet,
                            deadline,
                            #[cfg(test)]
                            &mut selected_commit,
                        )
                        .await?
                    {
                        Written::Committed(fresh) => {
                            self.project_eligibility(&entry, &credential, &fresh)?;
                            return self
                                .native_delivery_encoded(
                                    entry,
                                    fresh,
                                    success.ok_or(Error::Unavailable)?,
                                    deadline,
                                )
                                .map(Output::Selection);
                        }
                        Written::Reprepare(identity, fresh) if additional == 2 => {
                            drop(identity);
                            self.project_eligibility(&entry, &credential, &fresh)?;
                            return self
                                .native_delivery_encoded(entry, fresh, exhausted, deadline)
                                .map(Output::Selection);
                        }
                        Written::Reprepare(identity, _) => {
                            additional += 1;
                            original = Some(identity);
                        }
                        Written::Historical(identity) => {
                            original = Some(identity);
                            historical_only = true;
                        }
                        Written::Notice(value, fresh) => {
                            // This is the fresh save's bounded observation, not
                            // a historical result or a claim about later state.
                            let body = self
                                .hash(deadline, |worker| {
                                    worker.owned_preparation(
                                        deadline,
                                        execution.admission(),
                                        move || notice(value),
                                    )
                                })
                                .await??;
                            self.project_eligibility(&entry, &credential, &fresh)?;
                            return self
                                .native_delivery_encoded(entry, fresh, body, deadline)
                                .map(Output::Selection);
                        }
                    }
                }
            }
        }
    }
    async fn selection_write(
        &self,
        entry: &Arc<Session>,
        credential: &Credential,
        packet: SelectionPacket,
        deadline: Instant,
        #[cfg(test)] selected_commit: &mut Option<crate::lifecycle::probe::SelectedCommit>,
    ) -> Result<Written, Error> {
        self.check(deadline)?;
        let mut database = tokio::time::timeout_at(
            tokio::time::Instant::from_std(deadline),
            self.database.lock(),
        )
        .await
        .map_err(|_| Error::Unavailable)?;
        let b = &entry.binding;
        let transaction = database
            .fenced_selection_write(
                &b.installation,
                b.runtime_generation,
                b.auth_generation,
                deadline,
            )
            .await
            .map_err(fence_error)?;
        let fresh = snapshot(transaction.authority());
        if let Err(primary) = self.project_eligibility(entry, credential, &fresh) {
            return Err(self.selection_eligibility_cleanup(primary, transaction.rollback().await));
        }
        let outcome = transaction
            .save_selection(packet)
            .await
            .map_err(|error| self.selection_failure(error))?;
        match outcome {
            SelectionSaveOutcome::Ready(write) => {
                #[cfg(test)]
                if let Some(selected) = selected_commit.take() {
                    selected.before_commit(deadline).await;
                }
                if let Err(primary) = self
                    .check(deadline)
                    .and_then(|_| self.project_eligibility(entry, credential, &fresh))
                {
                    return Err(self.selection_eligibility_cleanup(primary, write.rollback().await));
                }
                #[cfg(test)]
                self.live
                    .probe
                    .transaction_point(
                        0,
                        crate::lifecycle::probe::Point::SelectionBeforeCommit,
                        deadline,
                    )
                    .await;
                let authority = write.commit().await.map_err(|error| {
                    super::project::native_error(&self.live, NativeError::Database(error))
                })?;
                Ok(Written::Committed(snapshot(&authority)))
            }
            SelectionSaveOutcome::Reprepare(identity) => Ok(Written::Reprepare(identity, fresh)),
            SelectionSaveOutcome::Historical(identity) => Ok(Written::Historical(identity)),
            SelectionSaveOutcome::Notice(value) => Ok(Written::Notice(value, fresh)),
        }
    }
    async fn selection_history_release(
        &self,
        entry: &Arc<Session>,
        credential: &Credential,
        packet: SelectionPacket,
        deadline: Instant,
    ) -> Result<(Option<SelectionResolution>, Snapshot), Error> {
        self.check(deadline)?;
        let mut database = tokio::time::timeout_at(
            tokio::time::Instant::from_std(deadline),
            self.database.lock(),
        )
        .await
        .map_err(|_| Error::Unavailable)?;
        let b = &entry.binding;
        let transaction = database
            .fenced_selection_replay(
                &b.installation,
                b.runtime_generation,
                b.auth_generation,
                deadline,
            )
            .await
            .map_err(fence_error)?;
        let fresh = snapshot(transaction.authority());
        if let Err(primary) = self.project_eligibility(entry, credential, &fresh) {
            return Err(self.selection_eligibility_cleanup(primary, transaction.rollback().await));
        }
        let result = transaction
            .release_selection(packet)
            .await
            .map_err(|error| self.selection_failure(error))?;
        Ok((result, fresh))
    }
    pub(super) async fn selection_read(
        &self,
        credential: Credential,
        execution: &jobs::Execution,
    ) -> Result<Output, Error> {
        let deadline = execution.deadline();
        self.check(deadline)?;
        let entry = self.candidate(&credential)?;
        #[cfg(test)]
        self.live
            .probe
            .record(crate::lifecycle::probe::Stage::NativeMutex);
        let mut database = tokio::time::timeout_at(
            tokio::time::Instant::from_std(deadline),
            self.database.lock(),
        )
        .await
        .map_err(|_| Error::Unavailable)?;
        let binding = &entry.binding;
        let mut transaction = database
            .read_snapshot(
                &binding.installation,
                binding.runtime_generation,
                binding.auth_generation,
                deadline,
            )
            .await
            .map_err(fence_error)?;
        let fresh = snapshot(transaction.authority());
        if let Err(primary) = self.project_eligibility(&entry, &credential, &fresh) {
            if let Err(cleanup) = transaction.rollback().await {
                self.live.fail(FatalCause::NativeRollback {
                    primary: Some(FailureDetail::new(
                        if primary == Error::Unauthorized {
                            "session unauthorized"
                        } else {
                            "session eligibility rejected"
                        },
                        None,
                        None,
                    )),
                    cleanup: database_detail(&cleanup),
                });
                return Err(Error::Unavailable);
            }
            return Err(primary);
        }
        let outcome = transaction.selection_read().await;
        // A target-specific grant denial has already ended the snapshot's SQL
        // work. Redacted data cannot leave until rollback is confirmed as well.
        let cleanup = transaction.rollback().await;
        drop(database);
        if let Err(cleanup) = cleanup {
            self.live.fail(FatalCause::NativeRollback {
                primary: outcome.as_ref().err().map(native_detail),
                cleanup: database_detail(&cleanup),
            });
            return Err(Error::Unavailable);
        }
        let draft = outcome.map_err(|error| super::project::native_error(&self.live, error))?;
        let live = self.live.clone();
        let permit = execution.admission();
        let encoded = self
            .hash(deadline, |worker| {
                worker.owned_preparation(deadline, permit, move || {
                    let value = draft
                        .admit()
                        .map_err(|error| super::project::native_error(&live, error))?;
                    let encoded = EncodedReply::admit(reply(200, value))?;
                    if encoded.encoded_len() > selection_codec::MAX_WIRE_BYTES {
                        return Err(Error::Unavailable);
                    }
                    Ok(encoded)
                })
            })
            .await??;
        Ok(Output::Selection(self.native_delivery_encoded(
            entry, fresh, encoded, deadline,
        )?))
    }
}
