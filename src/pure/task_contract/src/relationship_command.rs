//! Pure command boundary for creating a relationship across one or two projects.

use crate::relationship::{
    decide_relationship, Endpoint, EndpointIdentity, LinkAuthorization, RelationshipError,
    RelationshipId, RelationshipRecord, RelationshipType, RelationshipTypeId,
};
use crate::work_item::{ProjectId, WorkItemId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EndpointSnapshot {
    pub endpoint: Endpoint,
    pub configuration_revision: u64,
    pub item_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationshipCreateCommand {
    pub operation_id: String,
    /// Assigned by the trusted command boundary; B-005 must prevent reuse.
    pub relationship_id: RelationshipId,
    pub owner_project_id: ProjectId,
    pub type_id: RelationshipTypeId,
    pub from: EndpointIdentity,
    pub to: EndpointIdentity,
    pub expected_from_project_revision: u64,
    pub expected_to_project_revision: u64,
    pub expected_from_item_version: u64,
    pub expected_to_item_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuccessfulRelationshipCreate {
    pub request: RelationshipCreateCommand,
    pub result: RelationshipRecord,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelationshipCreateDecision {
    Replay(RelationshipRecord),
    Apply(RelationshipRecord),
    Reject(RelationshipCreateError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelationshipCreateError {
    Unauthorized,
    OperationIdReused,
    ConfigurationConflict {
        project_id: ProjectId,
        current_revision: u64,
    },
    ItemVersionConflict {
        item_id: WorkItemId,
        current_version: u64,
    },
    InvalidReference,
    InvalidOperation,
    Relationship(RelationshipError),
}

/// `from`, `to`, `kind`, `existing`, and `previous` must come from authoritative
/// reads. B-005 must protect both endpoint states, the owner configuration,
/// canonical uniqueness, ID allocation, and operation recording through commit.
pub fn decide_create_relationship(
    kind: &RelationshipType,
    from: &EndpointSnapshot,
    to: &EndpointSnapshot,
    authorization: LinkAuthorization,
    existing: &[RelationshipRecord],
    previous: Option<&SuccessfulRelationshipCreate>,
    command: &RelationshipCreateCommand,
) -> RelationshipCreateDecision {
    use RelationshipCreateDecision::{Apply, Reject, Replay};
    use RelationshipCreateError as E;

    if !authorization.can_use_type || !authorization.can_link_from || !authorization.can_link_to {
        return Reject(E::Unauthorized);
    }
    if let Some(previous) = previous {
        if previous.request.operation_id != command.operation_id
            || previous.request.owner_project_id != command.owner_project_id
        {
            return Reject(E::InvalidReference);
        }
        return if previous.request == *command {
            Replay(previous.result.clone())
        } else {
            Reject(E::OperationIdReused)
        };
    }
    if from.endpoint.project_id == to.endpoint.project_id
        && from.configuration_revision != to.configuration_revision
    {
        return Reject(E::InvalidReference);
    }
    if command.expected_from_project_revision != from.configuration_revision {
        return Reject(E::ConfigurationConflict {
            project_id: from.endpoint.project_id.clone(),
            current_revision: from.configuration_revision,
        });
    }
    if command.expected_to_project_revision != to.configuration_revision {
        return Reject(E::ConfigurationConflict {
            project_id: to.endpoint.project_id.clone(),
            current_revision: to.configuration_revision,
        });
    }
    if command.expected_from_item_version != from.item_version {
        return Reject(E::ItemVersionConflict {
            item_id: from.endpoint.item_id.clone(),
            current_version: from.item_version,
        });
    }
    if command.expected_to_item_version != to.item_version {
        return Reject(E::ItemVersionConflict {
            item_id: to.endpoint.item_id.clone(),
            current_version: to.item_version,
        });
    }
    if command.operation_id.is_empty() || command.relationship_id.0.is_empty() {
        return Reject(E::InvalidOperation);
    }
    if from.configuration_revision == 0
        || to.configuration_revision == 0
        || from.item_version == 0
        || to.item_version == 0
        || command.owner_project_id != kind.owner_project_id
        || command.type_id != kind.id
        || command.from != from.endpoint.identity()
        || command.to != to.endpoint.identity()
    {
        return Reject(E::InvalidReference);
    }
    let existing_keys = existing
        .iter()
        .map(|record| record.key.clone())
        .collect::<Vec<_>>();
    let key = match decide_relationship(
        kind,
        &from.endpoint,
        &to.endpoint,
        authorization,
        &existing_keys,
    ) {
        Ok(key) => key,
        Err(error) => return Reject(E::Relationship(error)),
    };
    Apply(RelationshipRecord {
        id: command.relationship_id.clone(),
        key,
    })
}
