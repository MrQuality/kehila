//! Owned replay pipeline. The response/packet pair is private and consumed.
use super::*;
use crate::postgres::transaction::{
    ItemHistoricalAccepted, ItemHistoricalDraft, ItemHistoryAuthorization, ItemHistoryPreparation,
    ItemHistoryReleaseFailure, ItemReplayRequest,
};

struct PreparedReplay {
    packet: ItemHistoricalAccepted,
    reply: EncodedReply,
}
impl PreparedReplay {
    fn admit(draft: ItemHistoricalDraft) -> Result<Self, AdmissionFailure> {
        let packet = draft.admit().map_err(AdmissionFailure::Item)?;
        let value = project_create_codec::parse_json(packet.result_bytes())
            .map_err(|_| AdmissionFailure::Representation)?;
        let reply = EncodedReply::admit(reply(200, json!({"outcome":"replayed","result":value})))
            .map_err(|_| AdmissionFailure::Representation)?;
        Ok(Self { packet, reply })
    }
}
impl Engine {
    fn item_release_failure(&self, error: ItemHistoryReleaseFailure) -> AttemptFailure {
        match &error {
            ItemHistoryReleaseFailure::Operation { primary, rollback } => {
                self.item_native_failure(primary);
                if rollback.is_some() {
                    self.live.fail(crate::lifecycle::FatalCause::OwnedExecution)
                }
            }
            ItemHistoryReleaseFailure::Rollback(_) => {
                self.live.fail(crate::lifecycle::FatalCause::OwnedExecution)
            }
        }
        AttemptFailure::Release(error)
    }
    // A historical-only entry for reconciliation. It cannot execute unseen work.
    pub(in crate::coordinator) async fn item_history(
        &self,
        credential: &Credential,
        intent: Intent,
        execution: &jobs::Execution,
    ) -> Result<AttemptReport, Error> {
        let deadline = execution.deadline();
        self.check(deadline)?;
        let entry = self.candidate(&credential)?;
        let origin = NoticeOrigin::capture(&intent, Arc::clone(&entry), deadline)?;
        let notices = self
            .hash(deadline, |worker| {
                worker.owned_preparation(deadline, execution.admission(), Catalogue::admit)
            })
            .await??;
        let result = self
            .item_history_inner(entry, &credential, intent, execution)
            .await;
        Ok(AttemptReport {
            result,
            notices,
            origin,
        })
    }
    pub(super) async fn item_history_inner(
        &self,
        entry: Arc<Session>,
        credential: &Credential,
        intent: Intent,
        execution: &jobs::Execution,
    ) -> Result<AttemptOutcome, AttemptFailure> {
        let deadline = execution.deadline();
        self.check(deadline).map_err(AttemptFailure::Owner)?;
        let request = match intent {
            Intent::Create { project, handle } => ItemReplayRequest::Create { project, handle },
            Intent::Edit(command) => ItemReplayRequest::Edit(command),
        };
        let preparation = {
            let mut database = tokio::time::timeout_at(
                tokio::time::Instant::from_std(deadline),
                self.database.lock(),
            )
            .await
            .map_err(|_| AttemptFailure::Owner(Error::Unavailable))?;
            let binding = &entry.binding;
            let mut transaction = database
                .read_snapshot(
                    &binding.installation,
                    binding.runtime_generation,
                    binding.auth_generation,
                    deadline,
                )
                .await
                .map_err(|error| AttemptFailure::Owner(fence_error(error)))?;
            let fresh = snapshot(transaction.authority());
            if let Err(primary) = self.project_eligibility(&entry, credential, &fresh) {
                let rollback = transaction.rollback().await.err();
                if rollback.is_some() {
                    self.live.fail(crate::lifecycle::FatalCause::OwnedExecution)
                }
                return Err(AttemptFailure::Eligibility { primary, rollback });
            }
            let checked = transaction.prepare_item_history(request).await;
            let rollback = transaction.rollback().await.err();
            if rollback.is_some() {
                self.live.fail(crate::lifecycle::FatalCause::OwnedExecution)
            }
            match (checked, rollback) {
                (Err(primary), rollback) => {
                    self.item_native_failure(&primary);
                    return Err(AttemptFailure::Preparation { primary, rollback });
                }
                (Ok(_), Some(error)) => return Err(AttemptFailure::Cleanup(error)),
                (Ok(value), None) => value,
            }
        };
        let draft = match preparation {
            ItemHistoryPreparation::Absent => return Ok(AttemptOutcome::Unseen),
            ItemHistoryPreparation::Resolved(value) => return Ok(AttemptOutcome::Resolved(value)),
            ItemHistoryPreparation::Ready(value) => *value,
        };
        // Confirmed RR rollback and mutex release precede all decode/encoding.
        #[cfg(test)]
        self.live
            .probe
            .transaction_point(
                0,
                crate::lifecycle::probe::Point::ItemHistoryLoaded,
                deadline,
            )
            .await;
        #[cfg(test)]
        let observed = Arc::clone(&self.live);
        let prepared = self
            .hash(deadline, |worker| {
                worker.owned_preparation(deadline, execution.admission(), move || {
                    let value = PreparedReplay::admit(draft);
                    #[cfg(test)]
                    if value.is_ok() {
                        observed.probe.record_item_replay(
                            crate::lifecycle::probe::ItemReplayObservation::Prepared,
                        )
                    }
                    value
                })
            })
            .await
            .map_err(AttemptFailure::Owner)?;
        let PreparedReplay { packet, reply } = prepared.map_err(|error| {
            if matches!(
                &error,
                AdmissionFailure::Item(ItemAdmissionError::InvalidProof)
            ) {
                self.live
                    .fail(crate::lifecycle::FatalCause::NativeIntegrity);
            }
            AttemptFailure::Admission(error)
        })?;
        self.check(deadline).map_err(AttemptFailure::Owner)?;
        let fresh = {
            let mut database = tokio::time::timeout_at(
                tokio::time::Instant::from_std(deadline),
                self.database.lock(),
            )
            .await
            .map_err(|_| AttemptFailure::Owner(Error::Unavailable))?;
            let binding = &entry.binding;
            let transaction = database
                .fenced_item_replay(
                    &binding.installation,
                    binding.runtime_generation,
                    binding.auth_generation,
                    deadline,
                )
                .await
                .map_err(|error| AttemptFailure::Owner(fence_error(error)))?;
            let fresh = snapshot(transaction.authority());
            if let Err(primary) = self.project_eligibility(&entry, credential, &fresh) {
                let rollback = transaction.rollback().await.err();
                if rollback.is_some() {
                    self.live.fail(crate::lifecycle::FatalCause::OwnedExecution)
                }
                return Err(AttemptFailure::Eligibility { primary, rollback });
            }
            let authorized = transaction
                .authorize_item_history(packet)
                .await
                .map_err(|error| self.item_release_failure(error))?;
            match authorized {
                ItemHistoryAuthorization::Expired(value) => {
                    return Ok(AttemptOutcome::Resolved(value))
                }
                ItemHistoryAuthorization::Released(release) => snapshot(release.authority()),
            }
        };
        // Final DB-clock authorization and confirmed rollback are complete.
        // No further CPU queue/encoding/database query precedes this handoff.
        self.check(deadline).map_err(AttemptFailure::Owner)?;
        self.project_eligibility(&entry, credential, &fresh)
            .map_err(|primary| AttemptFailure::Eligibility {
                primary,
                rollback: None,
            })?;
        self.native_delivery_encoded(entry, fresh, reply, deadline)
            .map(AttemptOutcome::Replayed)
            .map_err(AttemptFailure::Owner)
    }
}
