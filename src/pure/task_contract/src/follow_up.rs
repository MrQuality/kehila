//! Immutable, directed provenance between a source and a follow-up WorkItem.

use crate::relationship::EndpointIdentity;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FollowUpId(pub String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FollowUpRecord {
    pub id: FollowUpId,
    pub source: EndpointIdentity,
    pub follow_up: EndpointIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FollowUpSnapshot {
    pub item: EndpointIdentity,
    pub project_revision: u64,
    pub item_version: u64,
    pub project_archived: bool,
    pub item_archived: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FollowUpAuthorization {
    pub can_link_source: bool,
    pub can_link_follow_up: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FollowUpCommand {
    pub operation_id: String,
    /// Assigned by the trusted boundary; B-005 must keep it globally unique.
    pub id: FollowUpId,
    pub source: EndpointIdentity,
    pub follow_up: EndpointIdentity,
    pub expected_source_project_revision: u64,
    pub expected_follow_up_project_revision: u64,
    pub expected_source_item_version: u64,
    pub expected_follow_up_item_version: u64,
}

pub type SuccessfulFollowUp = (FollowUpCommand, FollowUpRecord);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FollowUpDecision {
    Replay(FollowUpRecord),
    Apply(FollowUpRecord),
    Reject(FollowUpError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FollowUpError {
    Unauthorized,
    OperationIdReused,
    ConfigurationConflict,
    ItemVersionConflict,
    InvalidReference,
    InvalidOperation,
    ArchivedTarget,
    SelfOrigin,
    AlreadyHasOrigin,
    Cycle,
}

/// `source_ancestors` must contain the complete provenance chain above source;
/// B-005 must protect it, both endpoints, the child's unique origin, and the
/// success record from concurrent writes. Provenance does not schedule work.
pub fn decide_follow_up(
    source: &FollowUpSnapshot,
    follow_up: &FollowUpSnapshot,
    authorization: FollowUpAuthorization,
    existing_for_child: Option<&FollowUpRecord>,
    source_ancestors: &[EndpointIdentity],
    previous: Option<&SuccessfulFollowUp>,
    command: &FollowUpCommand,
) -> FollowUpDecision {
    use FollowUpDecision::{Apply, Reject, Replay};
    use FollowUpError as E;
    if !authorization.can_link_source || !authorization.can_link_follow_up {
        return Reject(E::Unauthorized);
    }
    if let Some((request, result)) = previous {
        if request.operation_id != command.operation_id || request.source != command.source {
            return Reject(E::InvalidReference);
        }
        return if request == command {
            Replay(result.clone())
        } else {
            Reject(E::OperationIdReused)
        };
    }
    if !crate::operation::valid_operation_id(&command.operation_id) || command.id.0.is_empty() {
        return Reject(E::InvalidOperation);
    }
    if source.item != command.source
        || follow_up.item != command.follow_up
        || source.project_revision == 0
        || follow_up.project_revision == 0
        || source.item_version == 0
        || follow_up.item_version == 0
        || (source.item.project_id == follow_up.item.project_id
            && source.project_revision != follow_up.project_revision)
    {
        return Reject(E::InvalidReference);
    }
    if source.project_revision != command.expected_source_project_revision
        || follow_up.project_revision != command.expected_follow_up_project_revision
    {
        return Reject(E::ConfigurationConflict);
    }
    if source.item_version != command.expected_source_item_version
        || follow_up.item_version != command.expected_follow_up_item_version
    {
        return Reject(E::ItemVersionConflict);
    }
    if source.project_archived
        || source.item_archived
        || follow_up.project_archived
        || follow_up.item_archived
    {
        return Reject(E::ArchivedTarget);
    }
    if command.source == command.follow_up {
        return Reject(E::SelfOrigin);
    }
    if existing_for_child.is_some() {
        return Reject(E::AlreadyHasOrigin);
    }
    if source_ancestors.contains(&command.follow_up) {
        return Reject(E::Cycle);
    }
    Apply(FollowUpRecord {
        id: command.id.clone(),
        source: command.source.clone(),
        follow_up: command.follow_up.clone(),
    })
}
