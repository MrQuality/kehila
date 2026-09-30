//! Versioned project and WorkItem archival decisions without lifecycle effects.

use crate::project::{
    decide_project_archive, ArchiveAction, ArchiveDecision, Project, ProjectError,
};
use crate::relationship::EndpointIdentity;
use crate::work_item::{Configuration, ProjectId, WorkItem, WorkItemId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectArchiveCommand {
    pub operation_id: String,
    pub expected_configuration_revision: u64,
    pub action: ArchiveAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuccessfulProjectArchive {
    pub project_id: ProjectId,
    pub request: ProjectArchiveCommand,
    pub result: ArchiveDecision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectArchiveDecision {
    Replay(ArchiveDecision),
    Apply(ArchiveDecision),
    Reject(ArchiveCommandError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemArchiveCommand {
    pub operation_id: String,
    pub expected_item_version: u64,
    pub expected_configuration_revision: u64,
    pub action: ArchiveAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ItemArchiveEffect {
    None,
    ClearCurrentWorkForItem(EndpointIdentity),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemArchiveResult {
    pub item: WorkItem,
    pub effect: ItemArchiveEffect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuccessfulItemArchive {
    pub item_id: WorkItemId,
    pub request: ItemArchiveCommand,
    pub result: ItemArchiveResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ItemArchiveDecision {
    Replay(ItemArchiveResult),
    Apply(ItemArchiveResult),
    Reject(ArchiveCommandError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveCommandError {
    Unauthorized,
    OperationIdReused,
    ConfigurationConflict { current_revision: u64 },
    ItemVersionConflict { current_version: u64 },
    InvalidConfiguration,
    InvalidReference,
    InvalidOperation,
    ArchivedProject,
    VersionOverflow,
}

/// The caller must commit project state, configuration revision, selection
/// clearing, and the operation record as one accepted archive/restore outcome.
pub fn decide_project_archive_command(
    project: &Project,
    previous: Option<&SuccessfulProjectArchive>,
    authorized: bool,
    command: &ProjectArchiveCommand,
) -> ProjectArchiveDecision {
    use ArchiveCommandError as E;
    use ProjectArchiveDecision::{Apply, Reject, Replay};

    if !authorized {
        return Reject(E::Unauthorized);
    }
    if let Some(previous) = previous {
        if previous.project_id != project.id
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
    if command.expected_configuration_revision != project.configuration_revision {
        return Reject(E::ConfigurationConflict {
            current_revision: project.configuration_revision,
        });
    }
    if project.id.0.is_empty() {
        return Reject(E::InvalidReference);
    }
    if command.operation_id.is_empty() {
        return Reject(E::InvalidOperation);
    }
    match decide_project_archive(project, command.action) {
        Ok(result) => Apply(result),
        Err(ProjectError::InvalidOperation) => Reject(E::InvalidOperation),
        Err(ProjectError::RevisionExhausted) => Reject(E::VersionOverflow),
        Err(_) => Reject(E::InvalidConfiguration),
    }
}

/// An item archive/restore advances only item version and archive state.
/// Clearing every affected user's selection must be coordinated with commit.
pub fn decide_item_archive(
    configuration: &Configuration,
    item: &WorkItem,
    previous: Option<&SuccessfulItemArchive>,
    authorized: bool,
    command: &ItemArchiveCommand,
) -> ItemArchiveDecision {
    use ArchiveCommandError as E;
    use ItemArchiveDecision::{Apply, Reject, Replay};

    if !authorized {
        return Reject(E::Unauthorized);
    }
    if let Some(previous) = previous {
        if previous.item_id != item.id || previous.request.operation_id != command.operation_id {
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
    if command.expected_item_version != item.version {
        return Reject(E::ItemVersionConflict {
            current_version: item.version,
        });
    }
    if item.id.0.is_empty() || item.project_id != configuration.project_id {
        return Reject(E::InvalidReference);
    }
    let Some(item_type) = configuration
        .types
        .iter()
        .find(|entry| entry.id == item.type_id)
    else {
        return Reject(E::InvalidReference);
    };
    let Some(workflow) = configuration
        .workflows
        .iter()
        .find(|entry| entry.id == item.workflow_id)
    else {
        return Reject(E::InvalidReference);
    };
    if !configuration
        .statuses
        .iter()
        .any(|status| status.id == item.status_id)
        || !item_type.permitted_workflows.contains(&item.workflow_id)
        || !workflow.status_ids.contains(&item.status_id)
    {
        return Reject(E::InvalidReference);
    }
    if configuration.project_archived {
        return Reject(E::ArchivedProject);
    }
    if command.operation_id.is_empty()
        || item.archived == (command.action == ArchiveAction::Archive)
    {
        return Reject(E::InvalidOperation);
    }
    let Some(version) = item.version.checked_add(1) else {
        return Reject(E::VersionOverflow);
    };
    let mut next = item.clone();
    next.version = version;
    next.archived = command.action == ArchiveAction::Archive;
    Apply(ItemArchiveResult {
        item: next,
        effect: if command.action == ArchiveAction::Archive {
            ItemArchiveEffect::ClearCurrentWorkForItem(EndpointIdentity {
                project_id: item.project_id.clone(),
                item_id: item.id.clone(),
            })
        } else {
            ItemArchiveEffect::None
        },
    })
}
