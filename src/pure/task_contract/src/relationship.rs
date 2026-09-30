//! Pure identity and eligibility rules for first-class WorkItem relationships.

use crate::work_item::{ProjectId, WorkItemId};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelationshipTypeId(pub String);

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
