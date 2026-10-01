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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionCommand {
    pub user_id: UserId,
    pub operation_id: String,
    pub expected_version: u64,
    pub requested: Option<EndpointIdentity>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuccessfulSelection {
    pub request: SelectionCommand,
    pub result: SelectionResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectionDecision {
    Replay(SelectionResult),
    Apply(SelectionResult),
    Reject(SelectionError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionError {
    Unauthorized,
    OperationIdReused,
    InvalidOperation,
    VersionConflict { current_version: u64 },
    InvalidReference,
    ArchivedTarget,
    VersionOverflow,
}

/// `recorded` is the authoritative success for this user and operation ID.
/// The caller commits the selection and success record atomically, including
/// unchanged selections, so an uncertain outcome can return its original result.
pub fn decide_selection_command(
    current: &CurrentWork,
    requested: Option<&Endpoint>,
    recorded: Option<&SuccessfulSelection>,
    authorized: bool,
    command: &SelectionCommand,
) -> SelectionDecision {
    use SelectionDecision::{Apply, Reject, Replay};
    if !authorized {
        return Reject(SelectionError::Unauthorized);
    }
    if current.user_id != command.user_id || current.user_id.0.is_empty() {
        return Reject(SelectionError::InvalidReference);
    }
    if let Some(recorded) = recorded {
        if recorded.request.user_id != command.user_id
            || recorded.request.operation_id != command.operation_id
        {
            return Reject(SelectionError::InvalidReference);
        }
        return if recorded.request == *command {
            Replay(recorded.result.clone())
        } else {
            Reject(SelectionError::OperationIdReused)
        };
    }
    if command.operation_id.is_empty() {
        return Reject(SelectionError::InvalidOperation);
    }
    if requested.map(Endpoint::identity) != command.requested {
        return Reject(SelectionError::InvalidReference);
    }
    match decide_selection(current, command.expected_version, requested, true) {
        Ok(result) => Apply(result),
        Err(error) => Reject(error),
    }
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
