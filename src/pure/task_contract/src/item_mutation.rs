//! Pure creation and field/estimate editing against one project configuration.

use std::collections::HashSet;

use crate::field::{apply_edit, FieldEdit, FieldEntry, FieldError, FieldId, FieldValue};
use crate::payload::within_field_payload_limit;
use crate::project::{
    allocate_readable_id, record_estimate, Estimate, Project, ProjectError, ReadableId,
};
use crate::work_item::{
    Configuration, Phase, ProjectId, WorkItem, WorkItemId, WorkItemType, WorkItemTypeId, WorkflowId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemData {
    pub estimate: Option<Estimate>,
    pub values: Vec<FieldEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateCommand {
    pub operation_id: String,
    pub expected_configuration_revision: u64,
    /// Stable internal ID assigned by the trusted command boundary.
    pub item_id: WorkItemId,
    pub type_id: WorkItemTypeId,
    /// None selects the type's current default workflow.
    pub workflow_id: Option<WorkflowId>,
    pub estimate: Option<Estimate>,
    /// Full initial values; absent optional fields remain absent.
    pub values: Vec<FieldEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateResult {
    pub project: Project,
    pub readable_id: ReadableId,
    pub item: WorkItem,
    pub data: ItemData,
    pub phase: Phase,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuccessfulCreate {
    pub project_id: ProjectId,
    pub request: CreateCommand,
    pub result: CreateResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateDecision {
    Replay(CreateResult),
    Apply(CreateResult),
    Reject(ItemMutationError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EstimateEdit {
    Keep,
    Set(Estimate),
    Clear,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldUpdate {
    pub id: FieldId,
    pub edit: FieldEdit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditCommand {
    pub operation_id: String,
    pub expected_item_version: u64,
    pub expected_configuration_revision: u64,
    pub estimate: EstimateEdit,
    /// Sparse edits; an omitted field is kept, including hidden values.
    pub fields: Vec<FieldUpdate>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditResult {
    pub project: Project,
    pub item: WorkItem,
    pub data: ItemData,
    pub phase: Phase,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuccessfulEdit {
    pub item_id: WorkItemId,
    pub request: EditCommand,
    pub result: EditResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EditDecision {
    Replay(EditResult),
    Apply(EditResult),
    Reject(ItemMutationError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ItemMutationError {
    Unauthorized,
    OperationIdReused,
    ConfigurationConflict { current_revision: u64 },
    ItemVersionConflict { current_version: u64 },
    InvalidConfiguration,
    InvalidReference,
    InvalidOperation,
    ArchivedProject,
    ArchivedItem,
    ArchivedTarget,
    DuplicateField,
    PayloadLimitExceeded,
    Field(FieldError),
    Project(ProjectError),
    VersionOverflow,
}

/// Allocation, item write, estimate-unit lock, and operation record must commit
/// as one accepted result. The caller provides the authoritative replay lookup.
pub fn decide_create(
    project: &Project,
    configuration: &Configuration,
    previous: Option<&SuccessfulCreate>,
    authorized: bool,
    command: &CreateCommand,
) -> CreateDecision {
    use CreateDecision::{Apply, Reject, Replay};
    use ItemMutationError as E;

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
    if command.expected_configuration_revision != configuration.revision {
        return Reject(E::ConfigurationConflict {
            current_revision: configuration.revision,
        });
    }
    if project.id != configuration.project_id
        || project.configuration_revision != configuration.revision
        || configuration.validate().is_err()
    {
        return Reject(E::InvalidConfiguration);
    }
    if project.archived || configuration.project_archived {
        return Reject(E::ArchivedProject);
    }
    if command.operation_id.is_empty() || command.item_id.0.is_empty() {
        return Reject(E::InvalidOperation);
    }
    if !within_field_payload_limit(
        command.values.len(),
        command.values.iter().map(|entry| &entry.value),
    ) {
        return Reject(E::PayloadLimitExceeded);
    }
    let Some(item_type) = configuration
        .types
        .iter()
        .find(|entry| entry.id == command.type_id)
    else {
        return Reject(E::InvalidReference);
    };
    if item_type.archived {
        return Reject(E::ArchivedTarget);
    }
    let workflow_id = command
        .workflow_id
        .as_ref()
        .unwrap_or(&item_type.default_workflow_id);
    let Some(workflow) = configuration
        .workflows
        .iter()
        .find(|entry| entry.id == *workflow_id)
    else {
        return Reject(E::InvalidReference);
    };
    if !item_type.permitted_workflows.contains(workflow_id) {
        return Reject(E::InvalidReference);
    }
    if workflow.archived {
        return Reject(E::ArchivedTarget);
    }
    let Some(status) = configuration
        .statuses
        .iter()
        .find(|entry| entry.id == workflow.initial_status_id)
    else {
        return Reject(E::InvalidConfiguration);
    };
    let values = match decide_values(configuration, &command.type_id, &[], &command.values, &[]) {
        Ok(values) => values,
        Err(error) => return Reject(error),
    };
    let (mut next_project, readable_id) =
        match allocate_readable_id(project, command.item_id.clone()) {
            Ok(result) => result,
            Err(error) => return Reject(E::Project(error)),
        };
    next_project = match record_estimate(&next_project, command.estimate) {
        Ok(project) => project,
        Err(error) => return Reject(E::Project(error)),
    };
    Apply(CreateResult {
        project: next_project,
        readable_id,
        item: WorkItem {
            id: command.item_id.clone(),
            project_id: project.id.clone(),
            type_id: command.type_id.clone(),
            workflow_id: workflow_id.clone(),
            status_id: status.id.clone(),
            version: 1,
            archived: false,
        },
        data: ItemData {
            estimate: command.estimate,
            values,
        },
        phase: status.phase,
    })
}

/// Ordinary editing preserves type, workflow, status, and derived phase.
/// No usage, reservation, or scheduling effect is emitted.
pub fn decide_edit(
    project: &Project,
    configuration: &Configuration,
    item: &WorkItem,
    data: &ItemData,
    previous: Option<&SuccessfulEdit>,
    authorized: bool,
    command: &EditCommand,
) -> EditDecision {
    use EditDecision::{Apply, Reject, Replay};
    use ItemMutationError as E;

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
    if project.id != configuration.project_id
        || project.configuration_revision != configuration.revision
        || configuration.validate().is_err()
    {
        return Reject(E::InvalidConfiguration);
    }
    if command.expected_item_version != item.version {
        return Reject(E::ItemVersionConflict {
            current_version: item.version,
        });
    }
    if item.project_id != project.id {
        return Reject(E::InvalidReference);
    }
    if project.archived || configuration.project_archived {
        return Reject(E::ArchivedProject);
    }
    if item.archived {
        return Reject(E::ArchivedItem);
    }
    if command.operation_id.is_empty() {
        return Reject(E::InvalidOperation);
    }
    if !within_field_payload_limit(
        command.fields.len(),
        command
            .fields
            .iter()
            .filter_map(|update| match &update.edit {
                FieldEdit::Set(value) => Some(value),
                FieldEdit::Keep | FieldEdit::Clear => None,
            }),
    ) {
        return Reject(E::PayloadLimitExceeded);
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
    let Some(status) = configuration
        .statuses
        .iter()
        .find(|entry| entry.id == item.status_id)
    else {
        return Reject(E::InvalidReference);
    };
    if !item_type.permitted_workflows.contains(&item.workflow_id)
        || !workflow.status_ids.contains(&item.status_id)
    {
        return Reject(E::InvalidReference);
    }
    let values = match decide_values(
        configuration,
        &item.type_id,
        &data.values,
        &[],
        &command.fields,
    ) {
        Ok(values) => values,
        Err(error) => return Reject(error),
    };
    let estimate = match command.estimate {
        EstimateEdit::Keep => data.estimate,
        EstimateEdit::Set(value) => Some(value),
        EstimateEdit::Clear => None,
    };
    let next_project = match record_estimate(project, estimate) {
        Ok(project) => project,
        Err(error) => return Reject(E::Project(error)),
    };
    let Some(version) = item.version.checked_add(1) else {
        return Reject(E::VersionOverflow);
    };
    let mut next_item = item.clone();
    next_item.version = version;
    Apply(EditResult {
        project: next_project,
        item: next_item,
        data: ItemData { estimate, values },
        phase: status.phase,
    })
}

fn decide_values(
    configuration: &Configuration,
    item_type: &WorkItemTypeId,
    current: &[FieldEntry],
    initial: &[FieldEntry],
    updates: &[FieldUpdate],
) -> Result<Vec<FieldEntry>, ItemMutationError> {
    use ItemMutationError as E;
    let mut ids = HashSet::new();
    for entry in current.iter().chain(initial) {
        if !ids.insert(&entry.id) {
            return Err(E::DuplicateField);
        }
        if !configuration
            .fields
            .iter()
            .any(|field| field.id == entry.id && field.owner_type == *item_type)
        {
            return Err(E::InvalidReference);
        }
    }
    ids.clear();
    for update in updates {
        if !ids.insert(&update.id) {
            return Err(E::DuplicateField);
        }
        if !configuration
            .fields
            .iter()
            .any(|field| field.id == update.id && field.owner_type == *item_type)
        {
            return Err(E::InvalidReference);
        }
    }
    let mut result = Vec::new();
    for field in configuration
        .fields
        .iter()
        .filter(|field| field.owner_type == *item_type)
    {
        let old = current.iter().find(|entry| entry.id == field.id);
        let edit = if let Some(entry) = initial.iter().find(|entry| entry.id == field.id) {
            FieldEdit::Set(entry.value.clone())
        } else if let Some(update) = updates.iter().find(|update| update.id == field.id) {
            update.edit.clone()
        } else {
            FieldEdit::Keep
        };
        let next =
            apply_edit(field, item_type, old.map(|entry| &entry.value), edit).map_err(E::Field)?;
        if let Some(value) = next {
            result.push(FieldEntry {
                id: field.id.clone(),
                value,
            });
        }
    }
    Ok(result)
}

/// A configured nonblank title is display-only; readable ID is the fallback.
pub fn display_name(item_type: &WorkItemType, readable_id: &ReadableId, data: &ItemData) -> String {
    if let Some(title_field_id) = &item_type.title_field_id {
        if let Some(FieldEntry {
            value: FieldValue::Text(title),
            ..
        }) = data.values.iter().find(|entry| entry.id == *title_field_id)
        {
            if !title.trim().is_empty() {
                return title.clone();
            }
        }
    }
    readable_id.display()
}
