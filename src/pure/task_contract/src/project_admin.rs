//! Replayable project metadata administration under one configuration revision.

use crate::project::{valid_prefix, EstimateUnit, Project};
use crate::work_item::{Configuration, ProjectId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectMetadataCommand {
    pub operation_id: String,
    pub expected_revision: u64,
    pub project_id: ProjectId,
    pub name: String,
    pub prefix: String,
    pub estimate_unit: EstimateUnit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectMetadataResult {
    pub project: Project,
    pub configuration: Configuration,
}

pub type SuccessfulProjectMetadata = (ProjectMetadataCommand, ProjectMetadataResult);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectMetadataDecision {
    Replay(ProjectMetadataResult),
    Apply(ProjectMetadataResult),
    Reject(ProjectMetadataError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectMetadataError {
    Unauthorized,
    OperationIdReused,
    ConfigurationConflict { current_revision: u64 },
    InvalidReference,
    InvalidOperation,
    InvalidName,
    InvalidPrefix,
    ArchivedProject,
    EstimateUnitLocked,
    RevisionExhausted,
}

/// B-005 must commit the project, configuration revision/history, and success
/// record atomically. Existing readable IDs retain their issued prefixes.
pub fn decide_project_metadata(
    project: &Project,
    configuration: &Configuration,
    recorded: Option<&SuccessfulProjectMetadata>,
    authorized: bool,
    command: &ProjectMetadataCommand,
) -> ProjectMetadataDecision {
    use ProjectMetadataDecision::{Apply, Reject, Replay};
    use ProjectMetadataError as E;
    if !authorized {
        return Reject(E::Unauthorized);
    }
    if let Some((request, result)) = recorded {
        if request.operation_id != command.operation_id || request.project_id != command.project_id
        {
            return Reject(E::InvalidReference);
        }
        return if request == command {
            Replay(result.clone())
        } else {
            Reject(E::OperationIdReused)
        };
    }
    if command.expected_revision != project.configuration_revision {
        return Reject(E::ConfigurationConflict {
            current_revision: project.configuration_revision,
        });
    }
    if !crate::operation::valid_operation_id(&command.operation_id) {
        return Reject(E::InvalidOperation);
    }
    if command.project_id != project.id
        || configuration.project_id != project.id
        || configuration.revision != project.configuration_revision
        || configuration.project_archived != project.archived
        || configuration.validate().is_err()
    {
        return Reject(E::InvalidReference);
    }
    if project.archived {
        return Reject(E::ArchivedProject);
    }
    if command.name.trim().is_empty()
        || command.name.len() > 256
        || command.name.chars().any(char::is_control)
    {
        return Reject(E::InvalidName);
    }
    if !valid_prefix(&command.prefix) {
        return Reject(E::InvalidPrefix);
    }
    if project.estimate_unit != command.estimate_unit && project.ever_estimated {
        return Reject(E::EstimateUnitLocked);
    }
    if project.name == command.name
        && project.prefix == command.prefix
        && project.estimate_unit == command.estimate_unit
    {
        return Reject(E::InvalidOperation);
    }
    let Some(next_revision) = project.configuration_revision.checked_add(1) else {
        return Reject(E::RevisionExhausted);
    };
    let mut next_project = project.clone();
    next_project.name = command.name.clone();
    next_project.prefix = command.prefix.clone();
    next_project.estimate_unit = command.estimate_unit;
    next_project.configuration_revision = next_revision;
    let mut next_configuration = configuration.clone();
    next_configuration.revision = next_revision;
    Apply(ProjectMetadataResult {
        project: next_project,
        configuration: next_configuration,
    })
}
