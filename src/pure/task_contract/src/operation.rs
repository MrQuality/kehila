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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationError {
    Unauthorized,
    InvalidReference,
    InvalidOperation,
    OperationIdReused,
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
    recorded: Option<&OperationSuccess<Request, Result, Scope>>,
    new_required_grants: &Scope,
    authorize: impl FnOnce(&Scope) -> bool,
) -> ReplayDecision<Result> {
    let scope = recorded.map_or(new_required_grants, |recorded| &recorded.required_grants);
    if !authorize(scope) {
        return ReplayDecision::Reject(OperationError::Unauthorized);
    }
    if let Err(error) = key.validate() {
        return ReplayDecision::Reject(error);
    }
    let Some(recorded) = recorded else {
        return ReplayDecision::Unseen;
    };
    if recorded.key != *key {
        return ReplayDecision::Reject(OperationError::InvalidReference);
    }
    if recorded.request != *request {
        return ReplayDecision::Reject(OperationError::OperationIdReused);
    }
    ReplayDecision::Replay(recorded.result.clone())
}
