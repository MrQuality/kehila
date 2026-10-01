//! Stable single-choice option administration within one configuration revision.

use crate::field::{FieldId, FieldKind, OptionId};
use crate::work_item::{Configuration, ProjectId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptionAction {
    Add { id: OptionId, name: String },
    Rename { id: OptionId, name: String },
    Archive { id: OptionId },
    Restore { id: OptionId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OptionAdminCommand {
    pub operation_id: String,
    pub expected_configuration_revision: u64,
    pub field_id: FieldId,
    pub action: OptionAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuccessfulOptionAdmin {
    pub project_id: ProjectId,
    pub request: OptionAdminCommand,
    pub result: Configuration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptionAdminDecision {
    Replay(Configuration),
    Apply(Configuration),
    Reject(OptionAdminError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptionAdminError {
    Unauthorized,
    OperationIdReused,
    ConfigurationConflict { current_revision: u64 },
    InvalidConfiguration,
    InvalidReference,
    InvalidOperation,
    PayloadLimitExceeded,
    ArchivedProject,
    ArchivedField,
    NotChoiceField,
    DuplicateOption,
    UnknownOption,
    InvalidName,
    RevisionOverflow,
}

/// Option IDs are permanent. Archiving blocks new assignments but existing
/// values retain their reference. The caller commits the complete revision and
/// operation record under the same configuration concurrency boundary.
pub fn decide_option_admin(
    configuration: &Configuration,
    previous: Option<&SuccessfulOptionAdmin>,
    authorized: bool,
    command: &OptionAdminCommand,
) -> OptionAdminDecision {
    use OptionAdminDecision::{Apply, Reject, Replay};
    use OptionAdminError as E;

    if !authorized {
        return Reject(E::Unauthorized);
    }
    if let Some(previous) = previous {
        if previous.project_id != configuration.project_id
            || previous.request.operation_id != command.operation_id
        {
            return Reject(E::InvalidReference);
        }
        return if previous.request == *command {
            Replay(previous.result.clone())
        } else {
            Reject(E::OperationIdReused)
        };
    }
    if command.expected_configuration_revision != configuration.revision {
        return Reject(E::ConfigurationConflict {
            current_revision: configuration.revision,
        });
    }
    if configuration.validate().is_err() {
        return Reject(E::InvalidConfiguration);
    }
    if configuration.project_archived {
        return Reject(E::ArchivedProject);
    }
    if !crate::operation::valid_operation_id(&command.operation_id) {
        return Reject(E::InvalidOperation);
    }
    let mut next = configuration.clone();
    let Some(field) = next
        .fields
        .iter_mut()
        .find(|field| field.id == command.field_id)
    else {
        return Reject(E::InvalidReference);
    };
    if field.kind != FieldKind::SingleChoice {
        return Reject(E::NotChoiceField);
    }
    if field.archived {
        return Reject(E::ArchivedField);
    }
    match &command.action {
        OptionAction::Add { id, name } => {
            if id.0.is_empty() {
                return Reject(E::InvalidReference);
            }
            if field.options.iter().any(|option| option.id == *id) {
                return Reject(E::DuplicateOption);
            }
            if !valid_name(name) {
                return Reject(E::InvalidName);
            }
            field.options.push(crate::field::ChoiceOption {
                id: id.clone(),
                name: name.clone(),
                archived: false,
            });
        }
        OptionAction::Rename { id, name } => {
            let Some(option) = field.options.iter_mut().find(|option| option.id == *id) else {
                return Reject(E::UnknownOption);
            };
            if !valid_name(name) {
                return Reject(E::InvalidName);
            }
            if option.name == *name {
                return Reject(E::InvalidOperation);
            }
            option.name.clone_from(name);
        }
        OptionAction::Archive { id } | OptionAction::Restore { id } => {
            let Some(option) = field.options.iter_mut().find(|option| option.id == *id) else {
                return Reject(E::UnknownOption);
            };
            let archived = matches!(command.action, OptionAction::Archive { .. });
            if option.archived == archived {
                return Reject(E::InvalidOperation);
            }
            option.archived = archived;
        }
    }
    let Some(revision) = configuration.revision.checked_add(1) else {
        return Reject(E::RevisionOverflow);
    };
    next.revision = revision;
    if !next.within_limits() {
        return Reject(E::PayloadLimitExceeded);
    }
    if next.validate().is_err() {
        return Reject(E::InvalidConfiguration);
    }
    Apply(next)
}

fn valid_name(name: &str) -> bool {
    !name.trim().is_empty() && name.len() <= 256 && !name.chars().any(char::is_control)
}
