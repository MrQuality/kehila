//! Versioned project and WorkItem archival decisions without lifecycle effects.

use std::collections::HashSet;

use crate::current_work::CurrentWork;
use crate::project::{decide_project_archive, ArchiveAction, ArchiveEffect, Project, ProjectError};
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
    pub result: ProjectArchiveResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectArchiveDecision {
    Replay(ProjectArchiveResult),
    Apply(ProjectArchiveResult),
    Reject(ArchiveCommandError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionClear {
    pub before: CurrentWork,
    pub after: CurrentWork,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectArchiveResult {
    pub project: Project,
    pub configuration: Configuration,
    pub effect: ArchiveEffect,
    pub selection_clears: Vec<SelectionClear>,
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
    pub selection_clears: Vec<SelectionClear>,
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

/// `selections` must include every current-work record that could point into
/// this project. The caller must commit all returned state and clears with the
/// successful operation record under one coordinated boundary.
pub fn decide_project_archive_command(
    project: &Project,
    configuration: &Configuration,
    selections: &[CurrentWork],
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
    if project.id.0.is_empty() || configuration.project_id != project.id {
        return Reject(E::InvalidReference);
    }
    if configuration.revision != project.configuration_revision
        || configuration.project_archived != project.archived
        || configuration.validate().is_err()
    {
        return Reject(E::InvalidConfiguration);
    }
    if !crate::operation::valid_operation_id(&command.operation_id) {
        return Reject(E::InvalidOperation);
    }
    match decide_project_archive(project, command.action) {
        Ok(result) => {
            let selection_clears = match plan_selection_clears(selections, |selected| {
                selected.project_id == project.id && command.action == ArchiveAction::Archive
            }) {
                Ok(clears) => clears,
                Err(error) => return Reject(error),
            };
            let mut next_configuration = configuration.clone();
            next_configuration.revision = result.project.configuration_revision;
            next_configuration.project_archived = result.project.archived;
            Apply(ProjectArchiveResult {
                project: result.project,
                configuration: next_configuration,
                effect: result.effect,
                selection_clears,
            })
        }
        Err(ProjectError::InvalidOperation) => Reject(E::InvalidOperation),
        Err(ProjectError::RevisionExhausted) => Reject(E::VersionOverflow),
        Err(_) => Reject(E::InvalidConfiguration),
    }
}

/// An item archive/restore advances only item version and archive state.
/// `selections` must include every current-work record that could select this
/// item. The caller commits every returned clear with the item and operation.
pub fn decide_item_archive(
    configuration: &Configuration,
    item: &WorkItem,
    selections: &[CurrentWork],
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
    if !crate::operation::valid_operation_id(&command.operation_id)
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
    let selection_clears = match plan_selection_clears(selections, |selected| {
        selected.project_id == item.project_id
            && selected.item_id == item.id
            && command.action == ArchiveAction::Archive
    }) {
        Ok(clears) => clears,
        Err(error) => return Reject(error),
    };
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
        selection_clears,
    })
}

fn plan_selection_clears(
    selections: &[CurrentWork],
    affected: impl Fn(&EndpointIdentity) -> bool,
) -> Result<Vec<SelectionClear>, ArchiveCommandError> {
    let mut user_ids = HashSet::new();
    let mut clears = Vec::new();
    for current in selections {
        if current.user_id.0.is_empty() || !user_ids.insert(&current.user_id) {
            return Err(ArchiveCommandError::InvalidReference);
        }
        if current.selected.as_ref().is_some_and(&affected) {
            let version = current
                .version
                .checked_add(1)
                .ok_or(ArchiveCommandError::VersionOverflow)?;
            clears.push(SelectionClear {
                before: current.clone(),
                after: CurrentWork {
                    user_id: current.user_id.clone(),
                    selected: None,
                    version,
                },
            });
        }
    }
    Ok(clears)
}
