//! Shared M1 operation identity and replay envelope. Storage adapters must use
//! this scoped lookup before passing a recorded success to domain decisions.

use crate::current_work::UserId;
use crate::relationship::RelationshipId;
use crate::work_item::{ProjectId, WorkItemId};

pub const MAX_OPERATION_ID_BYTES: usize = 128;

pub fn valid_operation_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_OPERATION_ID_BYTES
        && value.bytes().all(|byte| byte.is_ascii_graphic())
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct OperationId(String);

impl OperationId {
    pub fn parse(value: &str) -> Result<Self, OperationError> {
        if !valid_operation_id(value) {
            return Err(OperationError::InvalidOperation);
        }
        Ok(Self(value.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CommandFamily {
    ProjectCreate,
    ProjectMetadata,
    ProjectArchive,
    ConfigurationChange,
    ChoiceOptionAdmin,
    ItemCreate,
    ItemEdit,
    ItemArchive,
    ItemTransition,
    ItemConversion,
    Selection,
    RelationshipCreate,
    KnowledgeCreate,
    KnowledgeEdit,
    FollowUpCreate,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum OperationTarget {
    Project(ProjectId),
    WorkItem {
        project_id: ProjectId,
        item_id: WorkItemId,
    },
    User(UserId),
    Relationship {
        owner_project_id: ProjectId,
        relationship_id: RelationshipId,
    },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct OperationKey {
    /// Authenticated identity supplied by B-007, never trusted from a payload.
    pub actor_id: UserId,
    pub family: CommandFamily,
    pub target: OperationTarget,
    pub operation_id: OperationId,
}

impl OperationKey {
    pub fn validate(&self) -> Result<(), OperationError> {
        use CommandFamily as F;
        use OperationTarget as T;
        let valid_target = match (&self.target, self.family) {
            (
                T::Project(id),
                F::ProjectCreate
                | F::ProjectMetadata
                | F::ProjectArchive
                | F::ConfigurationChange
                | F::ChoiceOptionAdmin,
            ) => !id.0.is_empty(),
            (
                T::WorkItem {
                    project_id,
                    item_id,
                },
                F::ItemCreate
                | F::ItemEdit
                | F::ItemArchive
                | F::ItemTransition
                | F::ItemConversion
                | F::KnowledgeCreate
                | F::KnowledgeEdit
                | F::FollowUpCreate,
            ) => !project_id.0.is_empty() && !item_id.0.is_empty(),
            (T::User(id), F::Selection) => !id.0.is_empty(),
            (
                T::Relationship {
                    owner_project_id,
                    relationship_id,
                },
                F::RelationshipCreate,
            ) => !owner_project_id.0.is_empty() && !relationship_id.0.is_empty(),
            _ => false,
        };
        if self.actor_id.0.is_empty() || !valid_target {
            return Err(OperationError::InvalidReference);
        }
        Ok(())
    }
}

/// Retain the exact typed request and originally required permission scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationSuccess<Request, Result, Scope> {
    pub key: OperationKey,
    pub request: Request,
    pub result: Result,
    pub required_grants: Scope,
}

pub const REPLAY_PERIOD_MS: u64 = 90 * 24 * 60 * 60 * 1000;

/// SHA-256 of the exact typed request under a versioned deterministic codec.
/// Supplied by a trusted adapter, never accepted from a client as evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestFingerprint {
    pub codec_version: u32,
    pub sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedOperation<Request, Result, Scope> {
    pub success: OperationSuccess<Request, Result, Scope>,
    pub committed_at_ms: u64,
    pub request_fingerprint: RequestFingerprint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationTombstone<Scope> {
    pub key: OperationKey,
    pub request_fingerprint: RequestFingerprint,
    pub required_grants: Scope,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationRecord<Request, Result, Scope> {
    Full(RetainedOperation<Request, Result, Scope>),
    Tombstone(OperationTombstone<Scope>),
}

/// Replaces only the replay record. Product state/history is a separate store.
/// B-005 must atomically replace the full record without a lookup gap and never
/// delete the tombstone or admit that operation key as new again.
pub fn compact_expired<Request, Outcome, Scope: Clone>(
    record: &RetainedOperation<Request, Outcome, Scope>,
    now_ms: u64,
) -> Result<OperationTombstone<Scope>, OperationError> {
    record.success.key.validate()?;
    if record.request_fingerprint.codec_version == 0 {
        return Err(OperationError::InvalidReference);
    }
    let deadline = record
        .committed_at_ms
        .checked_add(REPLAY_PERIOD_MS)
        .ok_or(OperationError::InvalidOperation)?;
    if now_ms < deadline {
        return Err(OperationError::InvalidOperation);
    }
    Ok(OperationTombstone {
        key: record.success.key.clone(),
        request_fingerprint: record.request_fingerprint,
        required_grants: record.success.required_grants.clone(),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationError {
    Unauthorized,
    InvalidReference,
    InvalidOperation,
    OperationIdReused,
    ReplayExpired,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplayDecision<Result> {
    /// Only this outcome permits running the new-command domain decision.
    Unseen,
    Replay(Result),
    Reject(OperationError),
}

/// `authorize` checks all current grants required by the supplied scope. B-005
/// must protect the lookup and authorization through commit. Exact typed Eq
/// includes versions, revisions, string bytes, and collection order.
pub fn decide_replay<Request: Eq, Result: Clone, Scope>(
    key: &OperationKey,
    request: &Request,
    recorded: Option<&OperationRecord<Request, Result, Scope>>,
    new_required_grants: &Scope,
    request_fingerprint: RequestFingerprint,
    now_ms: u64,
    authorize: impl FnOnce(&Scope) -> bool,
) -> ReplayDecision<Result> {
    let scope = match recorded {
        None => new_required_grants,
        Some(OperationRecord::Full(record)) => &record.success.required_grants,
        Some(OperationRecord::Tombstone(record)) => &record.required_grants,
    };
    if !authorize(scope) {
        return ReplayDecision::Reject(OperationError::Unauthorized);
    }
    if let Err(error) = key.validate() {
        return ReplayDecision::Reject(error);
    }
    if request_fingerprint.codec_version == 0 {
        return ReplayDecision::Reject(OperationError::InvalidReference);
    }
    let Some(recorded) = recorded else {
        return ReplayDecision::Unseen;
    };
    match recorded {
        OperationRecord::Full(record) => {
            if record.success.key != *key {
                return ReplayDecision::Reject(OperationError::InvalidReference);
            }
            if record.success.request != *request {
                return ReplayDecision::Reject(OperationError::OperationIdReused);
            }
            if record.request_fingerprint != request_fingerprint {
                return ReplayDecision::Reject(OperationError::InvalidReference);
            }
            let Some(deadline) = record.committed_at_ms.checked_add(REPLAY_PERIOD_MS) else {
                return ReplayDecision::Reject(OperationError::InvalidOperation);
            };
            if now_ms < record.committed_at_ms {
                return ReplayDecision::Reject(OperationError::InvalidOperation);
            }
            if now_ms >= deadline {
                return ReplayDecision::Reject(OperationError::ReplayExpired);
            }
            ReplayDecision::Replay(record.success.result.clone())
        }
        OperationRecord::Tombstone(record) => {
            if record.key != *key {
                return ReplayDecision::Reject(OperationError::InvalidReference);
            }
            if record.request_fingerprint != request_fingerprint {
                return ReplayDecision::Reject(OperationError::OperationIdReused);
            }
            ReplayDecision::Reject(OperationError::ReplayExpired)
        }
    }
}
