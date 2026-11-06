//! Closed response policy. Facts remain in the report, never JSON diagnostics.
use super::*;
use crate::postgres::transaction::ItemDisposition;
use task_contract::{item_mutation::ItemMutationError as Mutation, project::ProjectError};

// One declaration fixes vocabulary, ordinal and catalogue order together.
macro_rules! notices{
 ($( $name:ident => ($status:literal,$code:literal)),+ $(,)?)=>{
  #[repr(usize)]
  #[derive(Clone,Copy,Debug,Eq,PartialEq)]
  pub(in crate::coordinator) enum Notice{$($name),+}
  impl Notice{
   const ALL:&'static[Self]=&[$(Self::$name),+];
   fn value(self)->Reply{match self{$(Self::$name=>reply($status,json!({"error":$code}))),+}}
  }
 }
}
notices! {
 Forbidden=>(403,"forbidden"),
 NotFound=>(404,"not_found"),
 PendingMissing=>(404,"pending_not_found"),
 OperationMissing=>(404,"operation_not_found"),
 PendingCancelled=>(410,"pending_cancelled"),
 PendingExpired=>(410,"pending_expired"),
 ReplayExpired=>(410,"replay_expired"),
 OperationConflict=>(409,"operation_conflict"),
 ConfigurationConflict=>(409,"configuration_conflict"),
 ItemConflict=>(409,"item_conflict"),
 InvalidMutation=>(422,"invalid_mutation"),
 Archived=>(409,"archived_target"),
 RepresentationRejected=>(422,"representation_rejected"),
 StateExhausted=>(409,"mutation_state_exhausted"),
 AttemptExhausted=>(503,"attempt_exhausted"),
}
const NOTICE_BYTES: usize = 128;
const COUNT: usize = Notice::ALL.len();
const _: () = assert!(COUNT <= 16 && NOTICE_BYTES * COUNT <= 2048);

pub(super) struct Catalogue {
    replies: [EncodedReply; COUNT],
}
impl Catalogue {
    pub(super) fn admit() -> Result<Self, Error> {
        let replies = Notice::ALL
            .iter()
            .copied()
            .map(|kind| {
                let encoded = EncodedReply::admit(kind.value())?;
                if encoded.encoded_len() > NOTICE_BYTES {
                    return Err(Error::Unavailable);
                }
                Ok(encoded)
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let replies = replies.try_into().map_err(|_| Error::Unavailable)?;
        Ok(Self { replies })
    }
    pub(super) fn reply(&self, kind: Notice) -> EncodedReply {
        // Macro-generated enum and ALL share exactly the same order/cardinality.
        // Selection only clones an admitted Arc; it cannot encode/fail after COMMIT.
        self.replies[kind as usize].clone()
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::coordinator) enum Policy {
    Committed,
    Replayed,
    Historical,
    Scoped(Notice),
    Uncertain,
    Unauthorized,
    Busy,
    Unavailable,
}
impl Policy {
    // Only fixed nonidentifying errors use the generic channel. Scoped notices
    // require current scope plus final NativeDelivery session/deadline eligibility.
    pub(in crate::coordinator) fn generic_error(self) -> Option<Error> {
        match self {
            Self::Uncertain => Some(Error::Project(crate::project::Failure::OutcomeUnknown)),
            Self::Unauthorized => Some(Error::Unauthorized),
            Self::Busy => Some(Error::Busy),
            Self::Unavailable => Some(Error::Unavailable),
            Self::Committed | Self::Replayed | Self::Historical | Self::Scoped(_) => None,
        }
    }
}
fn owner(error: Error) -> Policy {
    match error {
        Error::Unauthorized | Error::UnknownCredential => Policy::Unauthorized,
        Error::Busy => Policy::Busy,
        Error::AuditUnconfirmed | Error::OtherAuditUnconfirmed => Policy::Uncertain,
        Error::Project(reason) => project(reason),
        Error::Invalid => Policy::Scoped(Notice::InvalidMutation),
        Error::Unavailable | Error::PendingIdentityExhausted => Policy::Unavailable,
    }
}
fn project(error: crate::project::Failure) -> Policy {
    use crate::project::Failure;
    match error {
        Failure::Forbidden => Policy::Scoped(Notice::Forbidden),
        Failure::Absent => Policy::Scoped(Notice::NotFound),
        Failure::Conflict => Policy::Scoped(Notice::OperationConflict),
        // Bare expiry carries no pending-versus-replay provenance.
        Failure::Expired => Policy::Unavailable,
        Failure::Cancelled => Policy::Scoped(Notice::PendingCancelled),
        Failure::OutcomeUnknown => Policy::Uncertain,
    }
}
fn native(error: &NativeError) -> Policy {
    match error {
        NativeError::Rejected(reason) => project(*reason),
        NativeError::Database(DbError::UnknownCommit) => Policy::Uncertain,
        NativeError::Database(DbError::Rejected(code)) => match code.as_str() {
            "42501" => Policy::Scoped(Notice::Forbidden),
            // This status also covers a trusted clock before the replay origin.
            // Only typed owner/fence provenance can establish authentication failure.
            "55000" => Policy::Unavailable,
            "54000" => Policy::Busy,
            _ => Policy::Unavailable,
        },
        NativeError::Database(_) | NativeError::Integrity => Policy::Unavailable,
    }
}
fn domain(error: &Mutation) -> Policy {
    use ProjectError as Project;
    match error {
        Mutation::ConfigurationConflict { .. } => Policy::Scoped(Notice::ConfigurationConflict),
        Mutation::ItemVersionConflict { .. } => Policy::Scoped(Notice::ItemConflict),
        Mutation::OperationIdReused => Policy::Scoped(Notice::OperationConflict),
        Mutation::Unauthorized => Policy::Scoped(Notice::Forbidden),
        Mutation::VersionOverflow
        | Mutation::Project(Project::SequenceExhausted | Project::RevisionExhausted) => {
            Policy::Scoped(Notice::StateExhausted)
        }
        Mutation::ArchivedProject
        | Mutation::ArchivedItem
        | Mutation::ArchivedTarget
        | Mutation::Project(Project::Archived) => Policy::Scoped(Notice::Archived),
        Mutation::InvalidConfiguration
        | Mutation::Project(Project::InvalidPrefix | Project::InvalidIdentity) => {
            Policy::Unavailable
        }
        Mutation::InvalidReference
        | Mutation::InvalidOperation
        | Mutation::DuplicateField
        | Mutation::PayloadLimitExceeded
        | Mutation::Field(_)
        | Mutation::Project(
            Project::InvalidOperation | Project::InvalidEstimate | Project::FieldMigrationRequired,
        ) => Policy::Scoped(Notice::InvalidMutation),
    }
}
fn admission(error: &AdmissionFailure) -> Policy {
    match error {
        AdmissionFailure::Representation
        | AdmissionFailure::Item(ItemAdmissionError::Representation(_)) => {
            Policy::Scoped(Notice::RepresentationRejected)
        }
        AdmissionFailure::Item(ItemAdmissionError::Domain(error)) => domain(error),
        AdmissionFailure::Item(ItemAdmissionError::StateExhausted) => {
            Policy::Scoped(Notice::StateExhausted)
        }
        AdmissionFailure::Item(
            ItemAdmissionError::InvalidProof
            | ItemAdmissionError::Expired
            | ItemAdmissionError::Unavailable,
        ) => Policy::Unavailable,
    }
}
pub(super) fn failure(error: &AttemptFailure) -> Policy {
    match error {
        AttemptFailure::CommittedUndeliverable(_)
        | AttemptFailure::Commit(DbError::UnknownCommit) => Policy::Uncertain,
        AttemptFailure::Owner(error) => owner(*error),
        AttemptFailure::Eligibility {
            primary,
            rollback: None,
        } => owner(*primary),
        AttemptFailure::Eligibility {
            rollback: Some(_), ..
        }
        | AttemptFailure::Cleanup(_) => Policy::Unavailable,
        AttemptFailure::Preparation { primary, rollback }
        | AttemptFailure::Write(ItemWriteFailure::Operation { primary, rollback })
        | AttemptFailure::Release(
            crate::postgres::transaction::ItemHistoryReleaseFailure::Operation {
                primary,
                rollback,
            },
        ) => {
            let policy = native(primary);
            if policy == Policy::Uncertain {
                policy
            } else if rollback.is_some() {
                Policy::Unavailable
            } else {
                policy
            }
        }
        AttemptFailure::Admission(error) => admission(error),
        AttemptFailure::Write(ItemWriteFailure::Rollback(_))
        | AttemptFailure::Commit(_)
        | AttemptFailure::Release(
            crate::postgres::transaction::ItemHistoryReleaseFailure::Rollback(_),
        ) => Policy::Unavailable,
        AttemptFailure::Exhausted => Policy::Scoped(Notice::AttemptExhausted),
    }
}
pub(super) fn conflict(value: ItemWriteConflict) -> Policy {
    Policy::Scoped(match value {
        ItemWriteConflict::Configuration => Notice::ConfigurationConflict,
        ItemWriteConflict::Item => Notice::ItemConflict,
    })
}
fn observed(value: ItemDisposition, has_operation: bool) -> Policy {
    match (value, has_operation) {
        // Finding an operation never manufactures a payload or fresh execution.
        (ItemDisposition::Replay, true) => Policy::Historical,
        (ItemDisposition::Expired, true) => Policy::Scoped(Notice::ReplayExpired),
        (ItemDisposition::Expired, false) => Policy::Scoped(Notice::PendingExpired),
        (ItemDisposition::Cancelled, false) => Policy::Scoped(Notice::PendingCancelled),
        (ItemDisposition::Missing, false) => Policy::Scoped(Notice::PendingMissing),
        (ItemDisposition::Conflict, _) => Policy::Scoped(Notice::OperationConflict),
        _ => Policy::Unavailable,
    }
}
pub(super) fn resolution(value: &ItemResolution) -> Policy {
    observed(value.outcome(), value.operation().is_some())
}
#[cfg(test)]
mod tests;
