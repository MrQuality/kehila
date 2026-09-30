//! Pure identity and eligibility rules for first-class WorkItem relationships.

use crate::work_item::{ProjectId, WorkItemId};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelationshipTypeId(pub String);

/// Opaque identity of one persisted link, independent of its uniqueness key.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelationshipId(pub String);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Directed,
    Symmetric,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationshipType {
    pub owner_project_id: ProjectId,
    pub id: RelationshipTypeId,
    pub direction: Direction,
    pub archived: bool,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EndpointIdentity {
    pub project_id: ProjectId,
    pub item_id: WorkItemId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Endpoint {
    pub project_id: ProjectId,
    pub item_id: WorkItemId,
    pub project_archived: bool,
    pub item_archived: bool,
}

impl Endpoint {
    pub fn identity(&self) -> EndpointIdentity {
        EndpointIdentity {
            project_id: self.project_id.clone(),
            item_id: self.item_id.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RelationshipKey {
    pub owner_project_id: ProjectId,
    pub type_id: RelationshipTypeId,
    pub from: EndpointIdentity,
    pub to: EndpointIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationshipRecord {
    pub id: RelationshipId,
    pub key: RelationshipKey,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewDirection {
    Outgoing,
    Incoming,
    Symmetric,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationshipView {
    pub id: RelationshipId,
    pub other_endpoint: EndpointIdentity,
    pub direction: ViewDirection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationshipError {
    Unauthorized,
    InvalidReference,
    UnrelatedTypeOwner,
    SelfLink,
    Duplicate,
    ArchivedTarget,
    ArchivedType,
}

/// Authorization results supplied by the access layer for link creation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinkAuthorization {
    pub can_use_type: bool,
    pub can_link_from: bool,
    pub can_link_to: bool,
}

/// Derive one stored identity. Directed inverse presentation does not create
/// another stored relationship; symmetric links sort their endpoint identities.
pub fn canonical_key(
    kind: &RelationshipType,
    from: &Endpoint,
    to: &Endpoint,
) -> Result<RelationshipKey, RelationshipError> {
    if kind.owner_project_id.0.is_empty()
        || kind.id.0.is_empty()
        || from.project_id.0.is_empty()
        || from.item_id.0.is_empty()
        || to.project_id.0.is_empty()
        || to.item_id.0.is_empty()
    {
        return Err(RelationshipError::InvalidReference);
    }
    let mut from = from.identity();
    let mut to = to.identity();
    if from == to {
        return Err(RelationshipError::SelfLink);
    }
    if from.project_id != kind.owner_project_id && to.project_id != kind.owner_project_id {
        return Err(RelationshipError::UnrelatedTypeOwner);
    }
    if kind.direction == Direction::Symmetric && from > to {
        std::mem::swap(&mut from, &mut to);
    }
    Ok(RelationshipKey {
        owner_project_id: kind.owner_project_id.clone(),
        type_id: kind.id.clone(),
        from,
        to,
    })
}

/// `existing` must come from an authoritative uniqueness lookup. B-005 must
/// enforce the same canonical key under concurrent creation.
pub fn decide_relationship(
    kind: &RelationshipType,
    from: &Endpoint,
    to: &Endpoint,
    authorization: LinkAuthorization,
    existing: &[RelationshipKey],
) -> Result<RelationshipKey, RelationshipError> {
    if !authorization.can_use_type || !authorization.can_link_from || !authorization.can_link_to {
        return Err(RelationshipError::Unauthorized);
    }
    if kind.archived {
        return Err(RelationshipError::ArchivedType);
    }
    if from.project_archived || from.item_archived || to.project_archived || to.item_archived {
        return Err(RelationshipError::ArchivedTarget);
    }
    let key = canonical_key(kind, from, to)?;
    if existing.contains(&key) {
        return Err(RelationshipError::Duplicate);
    }
    Ok(key)
}

/// Project one stored link for an endpoint. The inverse of a directed link is
/// a view of the same record; it is never another persisted relationship.
/// The caller must authorize the read before presenting this view.
pub fn view_from(
    record: &RelationshipRecord,
    kind: &RelationshipType,
    viewer: &EndpointIdentity,
) -> Result<RelationshipView, RelationshipError> {
    if record.id.0.is_empty()
        || record.key.owner_project_id != kind.owner_project_id
        || record.key.type_id != kind.id
    {
        return Err(RelationshipError::InvalidReference);
    }
    let (other_endpoint, direction) = if *viewer == record.key.from {
        (
            record.key.to.clone(),
            match kind.direction {
                Direction::Directed => ViewDirection::Outgoing,
                Direction::Symmetric => ViewDirection::Symmetric,
            },
        )
    } else if *viewer == record.key.to {
        (
            record.key.from.clone(),
            match kind.direction {
                Direction::Directed => ViewDirection::Incoming,
                Direction::Symmetric => ViewDirection::Symmetric,
            },
        )
    } else {
        return Err(RelationshipError::InvalidReference);
    };
    Ok(RelationshipView {
        id: record.id.clone(),
        other_endpoint,
        direction,
    })
}
