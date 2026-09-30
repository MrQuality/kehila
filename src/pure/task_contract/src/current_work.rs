//! User-scoped current-work selection, independent of WorkItem phase.

use crate::relationship::{Endpoint, EndpointIdentity};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct UserId(pub String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentWork {
    pub user_id: UserId,
    pub selected: Option<EndpointIdentity>,
    pub version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionResult {
    pub next: CurrentWork,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionError {
    Unauthorized,
    VersionConflict { current_version: u64 },
    InvalidReference,
    ArchivedTarget,
    VersionOverflow,
}

/// Selecting work changes only user context. An unchanged selection is an
/// idempotent no-op. The caller owns authentication and durable coordination.
pub fn decide_selection(
    current: &CurrentWork,
    expected_version: u64,
    requested: Option<&Endpoint>,
    authorized: bool,
) -> Result<SelectionResult, SelectionError> {
    if !authorized {
        return Err(SelectionError::Unauthorized);
    }
    if current.user_id.0.is_empty() {
        return Err(SelectionError::InvalidReference);
    }
    if expected_version != current.version {
        return Err(SelectionError::VersionConflict {
            current_version: current.version,
        });
    }
    let target = if let Some(endpoint) = requested {
        if endpoint.project_id.0.is_empty() || endpoint.item_id.0.is_empty() {
            return Err(SelectionError::InvalidReference);
        }
        if endpoint.project_archived || endpoint.item_archived {
            return Err(SelectionError::ArchivedTarget);
        }
        Some(endpoint.identity())
    } else {
        None
    };
    if target == current.selected {
        return Ok(SelectionResult {
            next: current.clone(),
        });
    }
    let version = current
        .version
        .checked_add(1)
        .ok_or(SelectionError::VersionOverflow)?;
    Ok(SelectionResult {
        next: CurrentWork {
            user_id: current.user_id.clone(),
            selected: target,
            version,
        },
    })
}
