//! Consumed item-write attempts and separately authorized historical delivery.
use super::native_delivery::EncodedReply;
use super::*;
use crate::postgres::transaction::{
    AdmittedItem, CanonicalItem, ItemAdmissionError, ItemDraft, ItemPreparation, ItemResolution,
    ItemRetryIdentity, ItemSaveOutcome, ItemWriteConflict, ItemWriteFailure, NativeError,
};
use serde_json::json;
mod notice;
mod replay;
mod replies;
use notice::{NoticeOrigin, ScopedNotice};
use replies::Catalogue;
pub(super) use replies::Policy;

// Admission of every scoped notice precedes SQL entry. Neither a report nor
// its encoded notice grants scope/session authority or releases transport data.
pub(super) struct AttemptReport {
    result: Result<AttemptOutcome, AttemptFailure>,
    notices: Catalogue,
    origin: NoticeOrigin,
}
impl AttemptReport {
    pub(super) fn policy(&self) -> Policy {
        match &self.result {
            Ok(AttemptOutcome::Committed(_)) => Policy::Committed,
            Ok(AttemptOutcome::Replayed(_)) => Policy::Replayed,
            Ok(AttemptOutcome::Unseen) => Policy::Scoped(replies::Notice::OperationMissing),
            Ok(AttemptOutcome::Resolved(value)) => replies::resolution(value),
            Ok(AttemptOutcome::Conflict(value)) => replies::conflict(*value),
            Err(error) => error.policy(),
        }
    }
    pub(super) fn notice(&self) -> Option<ScopedNotice> {
        match self.policy() {
            Policy::Scoped(kind) => Some(ScopedNotice::from_catalogue(
                kind,
                self.origin.clone(),
                &self.notices,
            )),
            _ => None,
        }
    }
    pub(super) fn into_result(self) -> Result<AttemptOutcome, AttemptFailure> {
        self.result
    }
}

// Provisional containment, not measured capacity. Initial entries count.
const PREPARATIONS: u8 = 3;
const WRITES: u8 = 3;
#[derive(Default)]
struct Entries {
    preparations: u8,
    writes: u8,
}
impl Entries {
    fn preparation(&mut self) -> Result<(), AttemptFailure> {
        if self.preparations == PREPARATIONS {
            return Err(AttemptFailure::Exhausted);
        }
        self.preparations += 1;
        Ok(())
    }
    fn write(&mut self) -> Result<(), AttemptFailure> {
        if self.writes == WRITES {
            return Err(AttemptFailure::Exhausted);
        }
        self.writes += 1;
        Ok(())
    }
}
#[derive(Clone)]
pub(super) enum Intent {
    Create { project: String, handle: String },
    Edit(Arc<CanonicalItem>),
}
impl Intent {
    fn project(&self) -> &str {
        match self {
            Self::Create { project, .. } => project,
            Self::Edit(command) => command.project_id(),
        }
    }
}
// Prepared by the tracked ingress owner; no caller can replace a sealed intent.
pub(crate) struct Command {
    action: crate::item_mutation_ingress::Action,
    intent: Intent,
}
impl Command {
    pub(super) fn new(action: crate::item_mutation_ingress::Action, intent: Intent) -> Self {
        Self { action, intent }
    }
}
enum Next {
    Initial(Intent),
    Retry(ItemRetryIdentity),
}
#[derive(Debug)]
pub(crate) enum AdmissionFailure {
    Item(ItemAdmissionError),
    Representation,
}
// Only this module can bind the consumed packet to its preencoded reply.
pub(crate) struct Prepared {
    packet: AdmittedItem,
    reply: EncodedReply,
}
impl Prepared {
    pub(crate) fn admit(draft: ItemDraft) -> Result<Self, AdmissionFailure> {
        let packet = draft.admit().map_err(AdmissionFailure::Item)?;
        let (status, outcome) = match &packet.revision().result {
            work_item_codec::SavedResult::Create(_) => (201, "created"),
            work_item_codec::SavedResult::Edit(_) => (200, "edited"),
        };
        let value = project_create_codec::parse_json(packet.result_bytes())
            .map_err(|_| AdmissionFailure::Representation)?;
        // Complete outer envelope: bytes, strings, nodes, depth and collections,
        // including metadata and punctuation. No result is regenerated after save.
        let reply = EncodedReply::admit(reply(status, json!({"outcome":outcome,"result":value})))
            .map_err(|_| AdmissionFailure::Representation)?;
        Ok(Self { packet, reply })
    }
}
pub(super) enum AttemptFailure {
    Owner(Error),
    Eligibility {
        primary: Error,
        rollback: Option<DbError>,
    },
    Preparation {
        primary: NativeError,
        rollback: Option<DbError>,
    },
    Cleanup(DbError),
    Admission(AdmissionFailure),
    Write(ItemWriteFailure),
    Release(crate::postgres::transaction::ItemHistoryReleaseFailure),
    Commit(DbError),
    // SQL acceptance is known; egress eligibility was lost afterward. The
    // adapter must not translate this into a claim of rejection or rollback.
    CommittedUndeliverable(Error),
    Exhausted,
}
impl std::fmt::Debug for AttemptFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Owner(error) => f.debug_tuple("Owner").field(error).finish(),
            Self::Eligibility { primary, rollback } => f
                .debug_struct("Eligibility")
                .field("primary", primary)
                .field("rollback", rollback)
                .finish(),
            Self::Preparation { primary, rollback } => f
                .debug_struct("Preparation")
                .field("primary", primary)
                .field("rollback", rollback)
                .finish(),
            Self::Cleanup(error) => f.debug_tuple("Cleanup").field(error).finish(),
            Self::Admission(error) => f.debug_tuple("Admission").field(error).finish(),
            Self::Write(error) => f.debug_tuple("Write").field(error).finish(),
            Self::Release(error) => f.debug_tuple("Release").field(error).finish(),
            Self::Commit(error) => f.debug_tuple("Commit").field(error).finish(),
            Self::CommittedUndeliverable(error) => f
                .debug_tuple("CommittedUndeliverable")
                .field(error)
                .finish(),
            Self::Exhausted => f.write_str("Exhausted"),
        }
    }
}
impl AttemptFailure {
    pub(super) fn policy(&self) -> Policy {
        replies::failure(self)
    }
}
pub(super) enum AttemptOutcome {
    Committed(NativeDelivery),
    Replayed(NativeDelivery),
    Unseen,
    // No SavedResult bytes or delivery capability; the owner must separately
    // authorize historical release at the accepted database-clock boundary.
    Resolved(ItemResolution),
    Conflict(ItemWriteConflict),
}
enum Written {
    Committed {
        reply: EncodedReply,
        fresh: Snapshot,
    },
    Retry(ItemRetryIdentity),
    Resolved(ItemResolution),
    Conflict(ItemWriteConflict),
}
impl Engine {
    pub(super) async fn execute_item_mutation(
        &self,
        credential: Credential,
        command: Command,
        execution: &jobs::Execution,
    ) -> Result<Output, Error> {
        let report = match command.action {
            crate::item_mutation_ingress::Action::Execute => {
                self.item_attempt(&credential, command.intent, execution)
                    .await?
            }
            crate::item_mutation_ingress::Action::Reconcile => {
                self.item_history(&credential, command.intent, execution)
                    .await?
            }
        };
        // Native success selection is synchronous through the job's response send.
        // Only fixed scoped failures enter another authorization step; saved payload
        // bodies never pass through a second queue or encoder after final release.
        if let Some(notice) = report.notice() {
            return self
                .deliver_item_notice(&credential, notice)
                .await
                .map(Output::Item)
                .map_err(|error| error.generic_error());
        }
        if let Some(error) = report.policy().generic_error() {
            return Err(error);
        }
        match report.into_result() {
            Ok(AttemptOutcome::Committed(delivery) | AttemptOutcome::Replayed(delivery)) => {
                Ok(Output::Item(delivery))
            }
            _ => Err(Error::Unavailable),
        }
    }
    fn item_native_failure(&self, error: &NativeError) {
        if matches!(
            error,
            NativeError::Integrity | NativeError::Database(DbError::Constraint { .. })
        ) {
            self.live
                .fail(crate::lifecycle::FatalCause::NativeIntegrity);
        }
    }
    fn item_write_failure(&self, error: ItemWriteFailure) -> AttemptFailure {
        match &error {
            ItemWriteFailure::Operation { primary, rollback } => {
                self.item_native_failure(primary);
                if rollback.is_some() {
                    self.live.fail(crate::lifecycle::FatalCause::OwnedExecution)
                }
            }
            ItemWriteFailure::Rollback(_) => {
                self.live.fail(crate::lifecycle::FatalCause::OwnedExecution)
            }
        }
        AttemptFailure::Write(error)
    }
    pub(super) async fn item_attempt(
        &self,
        credential: &Credential,
        intent: Intent,
        execution: &jobs::Execution,
    ) -> Result<AttemptReport, Error> {
        let deadline = execution.deadline();
        self.check(deadline)?;
        let entry = self.candidate(credential)?;
        let origin = NoticeOrigin::capture(&intent, Arc::clone(&entry), deadline)?;
        let notices = self
            .hash(deadline, |worker| {
                worker.owned_preparation(deadline, execution.admission(), Catalogue::admit)
            })
            .await??;
        // Keep the original locator/exact edit Arc through historical resolution.
        // Never reconstruct submitted edit bytes from metadata returned by SQL.
        let result = match self
            .item_attempt_inner(Arc::clone(&entry), credential, intent.clone(), execution)
            .await
        {
            Ok(AttemptOutcome::Resolved(value))
                if replies::resolution(&value) == Policy::Historical =>
            {
                self.item_history_inner(entry, credential, intent, execution)
                    .await
            }
            other => other,
        };
        Ok(AttemptReport {
            result,
            notices,
            origin,
        })
    }
    async fn item_attempt_inner(
        &self,
        entry: Arc<Session>,
        credential: &Credential,
        intent: Intent,
        execution: &jobs::Execution,
    ) -> Result<AttemptOutcome, AttemptFailure> {
        let deadline = execution.deadline();
        self.check(deadline).map_err(AttemptFailure::Owner)?;
        let mut entries = Entries::default();
        #[cfg(test)]
        let watched = match &intent {
            Intent::Create { project, handle } => self.live.probe.begin_item_attempt(
                project,
                handle,
                deadline,
                &execution.admission(),
            ),
            Intent::Edit(_) => false,
        };
        let mut next = Next::Initial(intent);
        loop {
            self.check(deadline).map_err(AttemptFailure::Owner)?;
            entries.preparation()?;
            #[cfg(test)]
            capacity::event(capacity::Stage::ItemPreparationAttempt);
            #[cfg(test)]
            if watched {
                self.live.probe.item_attempt_entry(
                    crate::lifecycle::probe::AttemptEntry::Preparation,
                    deadline,
                    &execution.admission(),
                );
            }
            // This block can yield only owned data. The RR rollback and mutex guard
            // are both gone before waiting for or entering the CPU lane.
            let preparation = {
                #[cfg(test)]
                let wait = capacity::span(capacity::Stage::PrepareMutex);
                #[cfg(test)]
                self.live
                    .probe
                    .record(crate::lifecycle::probe::Stage::NativeMutex);
                let mut database = tokio::time::timeout_at(
                    tokio::time::Instant::from_std(deadline),
                    self.database.lock(),
                )
                .await
                .map_err(|_| AttemptFailure::Owner(Error::Unavailable))?;
                #[cfg(test)]
                if let Some(wait) = wait {
                    wait.complete();
                }
                #[cfg(test)]
                let service = capacity::span(capacity::Stage::PrepareService);
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
                let result = match next {
                    Next::Initial(Intent::Create { project, handle }) => {
                        transaction.prepare_item_create(&project, &handle).await
                    }
                    Next::Initial(Intent::Edit(command)) => {
                        transaction.prepare_item_edit(command).await
                    }
                    Next::Retry(identity) => transaction.prepare_item_retry(identity).await,
                };
                let rollback = transaction.rollback().await.err();
                if rollback.is_some() {
                    self.live.fail(crate::lifecycle::FatalCause::OwnedExecution)
                }
                let preparation = match (result, rollback) {
                    (Err(primary), rollback) => {
                        self.item_native_failure(&primary);
                        return Err(AttemptFailure::Preparation { primary, rollback });
                    }
                    (Ok(_), Some(error)) => return Err(AttemptFailure::Cleanup(error)),
                    (Ok(preparation), None) => preparation,
                };
                #[cfg(test)]
                if let Some(service) = service {
                    service.complete();
                }
                preparation
            };
            let draft = match preparation {
                ItemPreparation::Resolved(value) => return Ok(AttemptOutcome::Resolved(value)),
                ItemPreparation::Ready(draft) => *draft,
            };
            let admitted = self
                .hash(deadline, |worker| {
                    worker.owned_preparation(deadline, execution.admission(), move || {
                        Prepared::admit(draft)
                    })
                })
                .await
                .map_err(AttemptFailure::Owner)?;
            let Prepared { packet, reply } = admitted.map_err(|error| {
                if matches!(
                    &error,
                    AdmissionFailure::Item(ItemAdmissionError::InvalidProof)
                ) {
                    self.live
                        .fail(crate::lifecycle::FatalCause::NativeIntegrity);
                }
                AttemptFailure::Admission(error)
            })?;
            // The owned CPU preparation has returned and joined before this fixture
            // point. It retains only the admitted packet/reply and original reservation.
            #[cfg(test)]
            self.live
                .probe
                .transaction_point(
                    0,
                    crate::lifecycle::probe::Point::ItemMutationPrepared,
                    deadline,
                )
                .await;
            self.check(deadline).map_err(AttemptFailure::Owner)?;
            #[cfg(test)]
            if watched {
                self.live.probe.item_attempt_bytes(packet.request_bytes());
                self.live
                    .probe
                    .transaction_point(
                        0,
                        crate::lifecycle::probe::Point::ItemAttemptPrepared,
                        deadline,
                    )
                    .await;
            }
            entries.write()?;
            #[cfg(test)]
            capacity::event(capacity::Stage::ItemWriteAttempt);
            #[cfg(test)]
            if watched {
                self.live.probe.item_attempt_entry(
                    crate::lifecycle::probe::AttemptEntry::Write,
                    deadline,
                    &execution.admission(),
                );
            }
            let written = {
                #[cfg(test)]
                let wait = capacity::span(capacity::Stage::WriteMutex);
                #[cfg(test)]
                self.live
                    .probe
                    .record(crate::lifecycle::probe::Stage::NativeMutex);
                let mut database = tokio::time::timeout_at(
                    tokio::time::Instant::from_std(deadline),
                    self.database.lock(),
                )
                .await
                .map_err(|_| AttemptFailure::Owner(Error::Unavailable))?;
                #[cfg(test)]
                if let Some(wait) = wait {
                    wait.complete();
                }
                #[cfg(test)]
                let service = capacity::span(capacity::Stage::WriteService);
                let binding = &entry.binding;
                let transaction = database
                    .fenced_item_write(
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
                let transition = match transaction
                    .save_item(packet)
                    .await
                    .map_err(|error| self.item_write_failure(error))?
                {
                    ItemSaveOutcome::Ready(write) => {
                        #[cfg(test)]
                        self.live
                            .probe
                            .transaction_point(
                                0,
                                crate::lifecycle::probe::Point::ItemBeforeCommit,
                                deadline,
                            )
                            .await;
                        // This is the last owner eligibility check before consuming COMMIT.
                        let eligible = self
                            .check(deadline)
                            .and_then(|_| self.project_eligibility(&entry, credential, &fresh));
                        if let Err(primary) = eligible {
                            let rollback = write.rollback().await.err();
                            if rollback.is_some() {
                                self.live.fail(crate::lifecycle::FatalCause::OwnedExecution)
                            }
                            return Err(AttemptFailure::Eligibility { primary, rollback });
                        }
                        write.commit().await.map_err(AttemptFailure::Commit)?;
                        Written::Committed { reply, fresh }
                    }
                    ItemSaveOutcome::Reprepare(identity) => {
                        #[cfg(test)]
                        capacity::event(capacity::Stage::Reprepare);
                        #[cfg(test)]
                        if watched {
                            self.live.probe.item_attempt_entry(
                                crate::lifecycle::probe::AttemptEntry::Reprepare,
                                deadline,
                                &execution.admission(),
                            );
                        }
                        #[cfg(test)]
                        self.live.probe.item_reprepared();
                        drop(reply);
                        Written::Retry(identity)
                    }
                    ItemSaveOutcome::Resolved(value) => {
                        drop(reply);
                        Written::Resolved(value)
                    }
                    ItemSaveOutcome::Conflict(value) => {
                        drop(reply);
                        Written::Conflict(value)
                    }
                };
                #[cfg(test)]
                if let Some(service) = service {
                    service.complete();
                }
                transition
            };
            // No old transaction, packet, reply or mutex guard survives a retry.
            match written {
                Written::Committed { reply, fresh } => {
                    return self
                        .native_delivery_encoded(Arc::clone(&entry), fresh, reply, deadline)
                        .map(AttemptOutcome::Committed)
                        .map_err(AttemptFailure::CommittedUndeliverable)
                }
                Written::Resolved(value) => return Ok(AttemptOutcome::Resolved(value)),
                Written::Conflict(value) => return Ok(AttemptOutcome::Conflict(value)),
                Written::Retry(identity) => next = Next::Retry(identity),
            }
        }
    }
}
