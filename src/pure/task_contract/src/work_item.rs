//! Pure status and workflow command rules for the B-003 work-item contract.
//!
//! The caller owns authentication, authoritative configuration loading,
//! operation lookup, and atomic persistence. `authorized` is the result of
//! the caller's access check and is deliberately checked before replay.

use std::collections::HashSet;

use crate::field::FieldUsage;
use crate::field::{FieldDefinition, FieldId, FieldKind};
use crate::payload::MAX_FIELD_ENTRIES;
use crate::relationship::RelationshipType;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(pub String);
    };
}

id_type!(ProjectId);
id_type!(WorkItemId);
id_type!(WorkItemTypeId);
id_type!(WorkflowId);
id_type!(StatusId);
id_type!(StatusGroupId);

pub const MAX_CONFIGURATION_TEXT_BYTES: usize = 1024 * 1024;
pub const MAX_STATUSES: usize = 256;
pub const MAX_STATUS_GROUPS: usize = 256;
pub const MAX_WORKFLOWS: usize = 128;
pub const MAX_ITEM_TYPES: usize = 128;
pub const MAX_FIELDS: usize = 512;
pub const MAX_RELATIONSHIP_TYPES: usize = 128;
pub const MAX_OPTIONS_PER_FIELD: usize = 256;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Phase {
    New,
    Active,
    Done,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhaseChange {
    pub from: Phase,
    pub to: Phase,
}

impl PhaseChange {
    pub const fn new(from: Phase, to: Phase) -> Self {
        Self { from, to }
    }

    /// The system graph bounds every workflow's configurable subset.
    pub const fn is_system_allowed(self) -> bool {
        matches!(
            (self.from, self.to),
            (Phase::New, Phase::Active)
                | (Phase::New, Phase::Done)
                | (Phase::Active, Phase::New)
                | (Phase::Active, Phase::Done)
                | (Phase::Done, Phase::Active)
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Status {
    pub id: StatusId,
    pub name: String,
    pub group_id: Option<StatusGroupId>,
    pub phase: Phase,
    pub archived: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusGroup {
    pub id: StatusGroupId,
    pub name: String,
    pub archived: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Workflow {
    pub id: WorkflowId,
    pub status_ids: Vec<StatusId>,
    pub initial_status_id: StatusId,
    pub permitted_phase_changes: Vec<PhaseChange>,
    pub archived: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkItemType {
    pub id: WorkItemTypeId,
    pub permitted_workflows: Vec<WorkflowId>,
    pub default_workflow_id: WorkflowId,
    /// Optional configured text field used for display; identity never depends on it.
    pub title_field_id: Option<FieldId>,
    pub archived: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Configuration {
    pub project_id: ProjectId,
    pub revision: u64,
    pub project_archived: bool,
    pub statuses: Vec<Status>,
    pub status_groups: Vec<StatusGroup>,
    pub workflows: Vec<Workflow>,
    pub types: Vec<WorkItemType>,
    pub fields: Vec<FieldDefinition>,
    pub relationship_types: Vec<RelationshipType>,
}

impl Configuration {
    /// Logical command bound; transport must reject oversized encoded bodies
    /// before decoding. This counts all supplied string occurrences in one
    /// complete configuration, including repeated references.
    pub fn within_limits(&self) -> bool {
        if self.statuses.len() > MAX_STATUSES
            || self.status_groups.len() > MAX_STATUS_GROUPS
            || self.workflows.len() > MAX_WORKFLOWS
            || self.types.len() > MAX_ITEM_TYPES
            || self.fields.len() > MAX_FIELDS
            || self.relationship_types.len() > MAX_RELATIONSHIP_TYPES
            || self
                .fields
                .iter()
                .any(|field| field.options.len() > MAX_OPTIONS_PER_FIELD)
        {
            return false;
        }
        let mut total = self.project_id.0.len();
        for status in &self.statuses {
            total = total
                .saturating_add(status.id.0.len())
                .saturating_add(status.name.len());
            if let Some(group_id) = &status.group_id {
                total = total.saturating_add(group_id.0.len());
            }
        }
        for group in &self.status_groups {
            total = total
                .saturating_add(group.id.0.len())
                .saturating_add(group.name.len());
        }
        for workflow in &self.workflows {
            total = total
                .saturating_add(workflow.id.0.len())
                .saturating_add(workflow.initial_status_id.0.len());
            for status_id in &workflow.status_ids {
                total = total.saturating_add(status_id.0.len());
            }
        }
        for item_type in &self.types {
            total = total
                .saturating_add(item_type.id.0.len())
                .saturating_add(item_type.default_workflow_id.0.len());
            if let Some(title) = &item_type.title_field_id {
                total = total.saturating_add(title.0.len());
            }
            for workflow_id in &item_type.permitted_workflows {
                total = total.saturating_add(workflow_id.0.len());
            }
        }
        for field in &self.fields {
            total = total
                .saturating_add(field.id.0.len())
                .saturating_add(field.owner_type.0.len())
                .saturating_add(field.name.len());
            for option in &field.options {
                total = total
                    .saturating_add(option.id.0.len())
                    .saturating_add(option.name.len());
            }
        }
        for kind in &self.relationship_types {
            total = total
                .saturating_add(kind.owner_project_id.0.len())
                .saturating_add(kind.id.0.len())
                .saturating_add(kind.name.len());
        }
        total <= MAX_CONFIGURATION_TEXT_BYTES
    }

    /// Check the structural rules needed to interpret item references uniquely.
    /// Administration commands must validate their proposed complete revision.
    pub fn validate(&self) -> Result<(), Error> {
        if self.project_id.0.is_empty() || self.revision == 0 || !self.within_limits() {
            return Err(Error::InvalidConfiguration);
        }
        let mut relationship_ids = HashSet::new();
        for relationship_type in &self.relationship_types {
            if relationship_type.owner_project_id != self.project_id
                || relationship_type.id.0.is_empty()
                || relationship_type.name.trim().is_empty()
                || relationship_type.name.len() > 256
                || relationship_type.name.chars().any(char::is_control)
                || !relationship_ids.insert(&relationship_type.id)
            {
                return Err(Error::InvalidConfiguration);
            }
        }
        let mut ids = HashSet::new();
        for group in &self.status_groups {
            if group.id.0.is_empty()
                || group.name.trim().is_empty()
                || group.name.len() > 256
                || group.name.chars().any(char::is_control)
                || !ids.insert(&group.id)
            {
                return Err(Error::InvalidConfiguration);
            }
        }
        let mut ids = HashSet::new();
        for status in &self.statuses {
            if status.id.0.is_empty()
                || status.name.trim().is_empty()
                || status.name.len() > 256
                || status.name.chars().any(char::is_control)
                || !ids.insert(&status.id)
                || status.group_id.as_ref().is_some_and(|group_id| {
                    !self.status_groups.iter().any(|group| group.id == *group_id)
                })
            {
                return Err(Error::InvalidConfiguration);
            }
        }
        let mut ids = HashSet::new();
        for workflow in &self.workflows {
            if workflow.id.0.is_empty() || !ids.insert(&workflow.id) {
                return Err(Error::InvalidConfiguration);
            }
            let Some(initial) = self
                .statuses
                .iter()
                .find(|status| status.id == workflow.initial_status_id)
            else {
                return Err(Error::InvalidConfiguration);
            };
            if initial.archived || initial.phase != Phase::New {
                return Err(Error::InvalidConfiguration);
            }
            let mut members = HashSet::new();
            for member in &workflow.status_ids {
                if !members.insert(member)
                    || !self.statuses.iter().any(|status| status.id == *member)
                {
                    return Err(Error::InvalidConfiguration);
                }
            }
            if !members.contains(&workflow.initial_status_id) {
                return Err(Error::InvalidConfiguration);
            }
            let mut changes = HashSet::new();
            for change in &workflow.permitted_phase_changes {
                if !changes.insert((change.from, change.to)) || !change.is_system_allowed() {
                    return Err(Error::InvalidConfiguration);
                }
            }
        }
        let mut ids = HashSet::new();
        for item_type in &self.types {
            if item_type.id.0.is_empty() || !ids.insert(&item_type.id) {
                return Err(Error::InvalidConfiguration);
            }
            if !item_type
                .permitted_workflows
                .contains(&item_type.default_workflow_id)
            {
                return Err(Error::InvalidConfiguration);
            }
            let Some(default_workflow) = self
                .workflows
                .iter()
                .find(|workflow| workflow.id == item_type.default_workflow_id)
            else {
                return Err(Error::InvalidConfiguration);
            };
            if !item_type.archived && default_workflow.archived {
                return Err(Error::InvalidConfiguration);
            }
            let mut permitted = HashSet::new();
            for workflow_id in &item_type.permitted_workflows {
                if !permitted.insert(workflow_id)
                    || !self
                        .workflows
                        .iter()
                        .any(|workflow| workflow.id == *workflow_id)
                {
                    return Err(Error::InvalidConfiguration);
                }
            }
            if let Some(title_field_id) = &item_type.title_field_id {
                let Some(field) = self.fields.iter().find(|field| field.id == *title_field_id)
                else {
                    return Err(Error::InvalidConfiguration);
                };
                if field.owner_type != item_type.id || field.kind != FieldKind::Text {
                    return Err(Error::InvalidConfiguration);
                }
            }
        }
        let mut field_ids = HashSet::new();
        for field in &self.fields {
            if !field_ids.insert(&field.id)
                || field.validate().is_err()
                || !self
                    .types
                    .iter()
                    .any(|item_type| item_type.id == field.owner_type)
            {
                return Err(Error::InvalidConfiguration);
            }
        }
        for item_type in &self.types {
            let required = self
                .fields
                .iter()
                .filter(|field| {
                    field.owner_type == item_type.id
                        && !field.archived
                        && field.usage == FieldUsage::Required
                })
                .count();
            if required > MAX_FIELD_ENTRIES {
                return Err(Error::InvalidConfiguration);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkItem {
    pub id: WorkItemId,
    pub project_id: ProjectId,
    pub type_id: WorkItemTypeId,
    pub workflow_id: WorkflowId,
    pub status_id: StatusId,
    pub version: u64,
    pub archived: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    ChangeStatus {
        target: StatusId,
    },
    MigrateWorkflow {
        destination_workflow: WorkflowId,
        destination_status: StatusId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Command {
    pub operation_id: String,
    pub expected_item_version: u64,
    pub expected_configuration_revision: u64,
    pub action: Action,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleEffect {
    None,
    Completion,
    Reopening,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkItemChange {
    pub workflow_id: WorkflowId,
    pub status_id: StatusId,
    pub phase: Phase,
    pub version: u64,
    pub effect: LifecycleEffect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuccessfulCommand {
    pub item_id: WorkItemId,
    pub request: Command,
    pub result: WorkItemChange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Unauthorized,
    OperationIdReused,
    ConfigurationConflict { current_revision: u64 },
    ItemVersionConflict { current_version: u64 },
    InvalidReference,
    InvalidConfiguration,
    ArchivedTarget,
    ArchivedProject,
    ProhibitedPhaseChange,
    InvalidOperation,
    VersionOverflow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Decision {
    Replay(WorkItemChange),
    Apply(WorkItemChange),
    Reject(Error),
}

/// Decide one item status or workflow command without changing the supplied state.
///
/// `previous` must come from an authoritative lookup by item and operation ID.
/// A matching success wins over later configuration, versions, and rules.
pub fn decide(
    configuration: &Configuration,
    item: &WorkItem,
    previous: Option<&SuccessfulCommand>,
    authorized: bool,
    command: &Command,
) -> Decision {
    if !authorized {
        return Decision::Reject(Error::Unauthorized);
    }
    if let Some(previous) = previous {
        if previous.item_id != item.id || previous.request.operation_id != command.operation_id {
            return Decision::Reject(Error::InvalidReference);
        }
        return if previous.request == *command {
            Decision::Replay(previous.result.clone())
        } else {
            Decision::Reject(Error::OperationIdReused)
        };
    }
    if command.expected_configuration_revision != configuration.revision {
        return Decision::Reject(Error::ConfigurationConflict {
            current_revision: configuration.revision,
        });
    }
    if configuration.validate().is_err() {
        return Decision::Reject(Error::InvalidConfiguration);
    }
    if command.expected_item_version != item.version {
        return Decision::Reject(Error::ItemVersionConflict {
            current_version: item.version,
        });
    }
    if item.project_id != configuration.project_id {
        return Decision::Reject(Error::InvalidReference);
    }
    if configuration.project_archived {
        return Decision::Reject(Error::ArchivedProject);
    }
    if item.archived {
        return Decision::Reject(Error::ArchivedTarget);
    }
    if command.operation_id.is_empty() {
        return Decision::Reject(Error::InvalidOperation);
    }

    let Some(item_type) = configuration
        .types
        .iter()
        .find(|entry| entry.id == item.type_id)
    else {
        return Decision::Reject(Error::InvalidReference);
    };
    let Some(source_workflow) = configuration
        .workflows
        .iter()
        .find(|entry| entry.id == item.workflow_id)
    else {
        return Decision::Reject(Error::InvalidReference);
    };
    let Some(source_status) = configuration
        .statuses
        .iter()
        .find(|entry| entry.id == item.status_id)
    else {
        return Decision::Reject(Error::InvalidReference);
    };
    if !item_type.permitted_workflows.contains(&item.workflow_id)
        || !source_workflow.status_ids.contains(&item.status_id)
    {
        return Decision::Reject(Error::InvalidReference);
    }

    let (target_workflow_id, target_status_id, migration) = match &command.action {
        Action::ChangeStatus { target } => (&item.workflow_id, target, false),
        Action::MigrateWorkflow {
            destination_workflow,
            destination_status,
        } => {
            if destination_workflow == &item.workflow_id {
                return Decision::Reject(Error::InvalidOperation);
            }
            (destination_workflow, destination_status, true)
        }
    };
    if !migration && target_status_id == &item.status_id {
        return Decision::Reject(Error::InvalidOperation);
    }
    let Some(target_workflow) = configuration
        .workflows
        .iter()
        .find(|entry| entry.id == *target_workflow_id)
    else {
        return Decision::Reject(Error::InvalidReference);
    };
    let Some(target_status) = configuration
        .statuses
        .iter()
        .find(|entry| entry.id == *target_status_id)
    else {
        return Decision::Reject(Error::InvalidReference);
    };
    if !item_type.permitted_workflows.contains(target_workflow_id)
        || !target_workflow.status_ids.contains(target_status_id)
    {
        return Decision::Reject(Error::InvalidReference);
    }
    if (migration && target_workflow.archived) || target_status.archived {
        return Decision::Reject(Error::ArchivedTarget);
    }

    let from = source_status.phase;
    let to = target_status.phase;
    let effect = match classify_transition(source_workflow, target_workflow, from, to, migration) {
        Ok(effect) => effect,
        Err(error) => return Decision::Reject(error),
    };
    let Some(version) = item.version.checked_add(1) else {
        return Decision::Reject(Error::VersionOverflow);
    };
    Decision::Apply(WorkItemChange {
        workflow_id: target_workflow_id.clone(),
        status_id: target_status_id.clone(),
        phase: to,
        version,
        effect,
    })
}

pub(crate) fn classify_transition(
    source_workflow: &Workflow,
    target_workflow: &Workflow,
    from: Phase,
    to: Phase,
    migration: bool,
) -> Result<LifecycleEffect, Error> {
    if from != to {
        let phase_change = PhaseChange::new(from, to);
        if !phase_change.is_system_allowed()
            || !source_workflow
                .permitted_phase_changes
                .contains(&phase_change)
            || (migration
                && !target_workflow
                    .permitted_phase_changes
                    .contains(&phase_change))
        {
            return Err(Error::ProhibitedPhaseChange);
        }
    }
    Ok(match (from, to) {
        (Phase::New | Phase::Active, Phase::Done) => LifecycleEffect::Completion,
        (Phase::Done, Phase::Active) => LifecycleEffect::Reopening,
        _ => LifecycleEffect::None,
    })
}
