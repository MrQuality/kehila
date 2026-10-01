//! Trusted project initialization, independent of storage and grant provisioning.

use crate::field::{FieldDefinition, FieldId, FieldKind, FieldOrigin, FieldUsage};
use crate::project::{valid_prefix, EstimateUnit, Project};
use crate::work_item::{
    Configuration, Phase, PhaseChange, ProjectId, Status, StatusId, WorkItemType, WorkItemTypeId,
    Workflow, WorkflowId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectCreateCommand {
    pub operation_id: String,
    /// Allocated by the trusted identity service, not a reusable display prefix.
    pub project_id: ProjectId,
    pub name: String,
    pub prefix: String,
    pub estimate_unit: EstimateUnit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeedProfile {
    M1V1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectCreateResult {
    pub project: Project,
    pub configuration: Configuration,
    pub seed_profile: SeedProfile,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuccessfulProjectCreate {
    pub request: ProjectCreateCommand,
    pub result: ProjectCreateResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectCreateDecision {
    Replay(ProjectCreateResult),
    Apply(ProjectCreateResult),
    Reject(ProjectCreateError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectCreateError {
    Unauthorized,
    OperationIdReused,
    InvalidReference,
    InvalidOperation,
    InvalidName,
    InvalidPrefix,
    InvalidConfiguration,
    ProjectAlreadyExists,
}

/// `project_exists` and `recorded` come from authoritative identity/operation
/// lookups. B-005 must commit project, seed, history, and success coherently;
/// B-007 supplies current project-creation authorization and initial access.
pub fn decide_project_create(
    project_exists: bool,
    recorded: Option<&SuccessfulProjectCreate>,
    authorized: bool,
    command: &ProjectCreateCommand,
) -> ProjectCreateDecision {
    use ProjectCreateDecision::{Apply, Reject, Replay};
    use ProjectCreateError as E;
    if !authorized {
        return Reject(E::Unauthorized);
    }
    if let Some(recorded) = recorded {
        if recorded.request.project_id != command.project_id
            || recorded.request.operation_id != command.operation_id
        {
            return Reject(E::InvalidReference);
        }
        return if recorded.request == *command {
            Replay(recorded.result.clone())
        } else {
            Reject(E::OperationIdReused)
        };
    }
    if command.project_id.0.is_empty() {
        return Reject(E::InvalidReference);
    }
    if command.operation_id.is_empty() {
        return Reject(E::InvalidOperation);
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
    if project_exists {
        return Reject(E::ProjectAlreadyExists);
    }
    let configuration = m1_seed(command.project_id.clone());
    if configuration.validate().is_err() {
        return Reject(E::InvalidConfiguration);
    }
    Apply(ProjectCreateResult {
        project: Project {
            id: command.project_id.clone(),
            name: command.name.clone(),
            prefix: command.prefix.clone(),
            configuration_revision: 1,
            next_sequence: 1,
            archived: false,
            estimate_unit: command.estimate_unit,
            ever_estimated: false,
        },
        configuration,
        seed_profile: SeedProfile::M1V1,
    })
}

/// Only initialization may install new application-origin fields. Subsequent
/// revisions must pass the protected-field rules in `configuration_change`.
fn m1_seed(project_id: ProjectId) -> Configuration {
    let workflow_id = WorkflowId("default".into());
    let statuses: Vec<_> = [
        ("new", "New", Phase::New),
        ("active", "Active", Phase::Active),
        ("done", "Done", Phase::Done),
    ]
    .into_iter()
    .map(|(id, name, phase)| Status {
        id: StatusId(id.into()),
        name: name.into(),
        group_id: None,
        phase,
        archived: false,
    })
    .collect();
    let mut types = Vec::new();
    let mut fields = Vec::new();
    for id in ["task", "milestone"] {
        let type_id = WorkItemTypeId(id.into());
        let title_id = FieldId(format!("{id}-title"));
        types.push(WorkItemType {
            id: type_id.clone(),
            permitted_workflows: vec![workflow_id.clone()],
            default_workflow_id: workflow_id.clone(),
            title_field_id: Some(title_id),
            archived: false,
        });
        for (suffix, name) in [("title", "Title"), ("description", "Description")] {
            fields.push(FieldDefinition {
                id: FieldId(format!("{id}-{suffix}")),
                owner_type: type_id.clone(),
                name: name.into(),
                kind: FieldKind::Text,
                origin: FieldOrigin::Application,
                usage: FieldUsage::Optional,
                archived: false,
                options: vec![],
            });
        }
    }
    Configuration {
        project_id,
        revision: 1,
        project_archived: false,
        workflows: vec![Workflow {
            id: workflow_id,
            status_ids: statuses.iter().map(|status| status.id.clone()).collect(),
            initial_status_id: StatusId("new".into()),
            permitted_phase_changes: vec![
                PhaseChange::new(Phase::New, Phase::Active),
                PhaseChange::new(Phase::New, Phase::Done),
                PhaseChange::new(Phase::Active, Phase::New),
                PhaseChange::new(Phase::Active, Phase::Done),
                PhaseChange::new(Phase::Done, Phase::Active),
            ],
            archived: false,
        }],
        statuses,
        status_groups: vec![],
        types,
        fields,
        relationship_types: vec![],
    }
}
