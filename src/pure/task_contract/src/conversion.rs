//! Complete destination validation for one WorkItem type conversion.

use std::collections::HashSet;

pub use crate::field::FieldEntry;
use crate::field::{apply_edit, FieldEdit, FieldError};
use crate::work_item::{
    classify_transition, Configuration, Error, LifecycleEffect, Phase, StatusId, WorkItem,
    WorkItemId, WorkItemTypeId, WorkflowId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversionCommand {
    pub operation_id: String,
    pub expected_item_version: u64,
    pub expected_configuration_revision: u64,
    pub destination_type: WorkItemTypeId,
    pub destination_workflow: WorkflowId,
    pub destination_status: StatusId,
    /// Complete current-value set for the destination type, not a patch.
    pub destination_values: Vec<FieldEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversionResult {
    pub next_item: WorkItem,
    pub phase: Phase,
    pub effect: LifecycleEffect,
    pub current_values: Vec<FieldEntry>,
    /// Independent history payload; persistence must retain this after replay expiry.
    pub source_values: Vec<FieldEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuccessfulConversion {
    pub item_id: WorkItemId,
    pub request: ConversionCommand,
    pub result: ConversionResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConversionError {
    Unauthorized,
    MigrationUnauthorized,
    OperationIdReused,
    ConfigurationConflict { current_revision: u64 },
    ItemVersionConflict { current_version: u64 },
    InvalidReference,
    InvalidConfiguration,
    ArchivedProject,
    ArchivedItem,
    ArchivedTarget,
    InvalidOperation,
    DuplicateField,
    Field(FieldError),
    ProhibitedPhaseChange,
    VersionOverflow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConversionDecision {
    Replay(ConversionResult),
    Apply(ConversionResult),
    Reject(ConversionError),
}

/// The caller supplies authoritative configuration, source values, and replay
/// lookup, then commits the returned item, values, history, and operation atomically.
pub fn decide_conversion(
    configuration: &Configuration,
    item: &WorkItem,
    source_values: &[FieldEntry],
    previous: Option<&SuccessfulConversion>,
    authorized: bool,
    may_migrate: bool,
    command: &ConversionCommand,
) -> ConversionDecision {
    use ConversionDecision::{Apply, Reject, Replay};
    use ConversionError as C;
    let definitions = &configuration.fields;

    if !authorized {
        return Reject(C::Unauthorized);
    }
    if let Some(previous) = previous {
        if previous.item_id != item.id || previous.request.operation_id != command.operation_id {
            return Reject(C::InvalidReference);
        }
        return if previous.request == *command {
            Replay(previous.result.clone())
        } else {
            Reject(C::OperationIdReused)
        };
    }
    if command.expected_configuration_revision != configuration.revision {
        return Reject(C::ConfigurationConflict {
            current_revision: configuration.revision,
        });
    }
    if configuration.validate().is_err() {
        return Reject(C::InvalidConfiguration);
    }
    if command.expected_item_version != item.version {
        return Reject(C::ItemVersionConflict {
            current_version: item.version,
        });
    }
    if item.project_id != configuration.project_id || item.id.0.is_empty() {
        return Reject(C::InvalidReference);
    }
    if configuration.project_archived {
        return Reject(C::ArchivedProject);
    }
    if item.archived {
        return Reject(C::ArchivedItem);
    }
    if command.operation_id.is_empty() || item.type_id == command.destination_type {
        return Reject(C::InvalidOperation);
    }
    let Some(source_type) = configuration
        .types
        .iter()
        .find(|entry| entry.id == item.type_id)
    else {
        return Reject(C::InvalidReference);
    };
    let Some(source_workflow) = configuration
        .workflows
        .iter()
        .find(|entry| entry.id == item.workflow_id)
    else {
        return Reject(C::InvalidReference);
    };
    let Some(source_status) = configuration
        .statuses
        .iter()
        .find(|entry| entry.id == item.status_id)
    else {
        return Reject(C::InvalidReference);
    };
    if !source_type.permitted_workflows.contains(&item.workflow_id)
        || !source_workflow.status_ids.contains(&item.status_id)
    {
        return Reject(C::InvalidReference);
    }
    let Some(destination_type) = configuration
        .types
        .iter()
        .find(|entry| entry.id == command.destination_type)
    else {
        return Reject(C::InvalidReference);
    };
    let Some(destination_workflow) = configuration
        .workflows
        .iter()
        .find(|entry| entry.id == command.destination_workflow)
    else {
        return Reject(C::InvalidReference);
    };
    let Some(destination_status) = configuration
        .statuses
        .iter()
        .find(|entry| entry.id == command.destination_status)
    else {
        return Reject(C::InvalidReference);
    };
    if !destination_type
        .permitted_workflows
        .contains(&command.destination_workflow)
        || !destination_workflow
            .status_ids
            .contains(&command.destination_status)
    {
        return Reject(C::InvalidReference);
    }
    if destination_type.archived || destination_workflow.archived || destination_status.archived {
        return Reject(C::ArchivedTarget);
    }
    let migration = item.workflow_id != command.destination_workflow;
    if migration && !may_migrate {
        return Reject(C::MigrationUnauthorized);
    }
    let effect = match classify_transition(
        source_workflow,
        destination_workflow,
        source_status.phase,
        destination_status.phase,
        migration,
    ) {
        Ok(effect) => effect,
        Err(Error::ProhibitedPhaseChange) => return Reject(C::ProhibitedPhaseChange),
        Err(_) => return Reject(C::InvalidConfiguration),
    };
    let Some(version) = item.version.checked_add(1) else {
        return Reject(C::VersionOverflow);
    };
    let mut ids = HashSet::new();
    for field in definitions {
        if !ids.insert(&field.id) || field.validate().is_err() {
            return Reject(C::InvalidConfiguration);
        }
    }
    ids.clear();
    for entry in source_values {
        if !ids.insert(&entry.id) {
            return Reject(C::DuplicateField);
        }
    }
    ids.clear();
    for entry in &command.destination_values {
        if !ids.insert(&entry.id) {
            return Reject(C::DuplicateField);
        }
        if !definitions
            .iter()
            .any(|field| field.id == entry.id && field.owner_type == command.destination_type)
        {
            return Reject(C::InvalidReference);
        }
    }
    for field in definitions
        .iter()
        .filter(|field| field.owner_type == command.destination_type)
    {
        let value = command
            .destination_values
            .iter()
            .find(|entry| entry.id == field.id);
        let edit = match value {
            Some(entry) => FieldEdit::Set(entry.value.clone()),
            None => FieldEdit::Keep,
        };
        if let Err(error) = apply_edit(field, &command.destination_type, None, edit) {
            return Reject(C::Field(error));
        }
    }
    let mut next_item = item.clone();
    next_item.type_id = command.destination_type.clone();
    next_item.workflow_id = command.destination_workflow.clone();
    next_item.status_id = command.destination_status.clone();
    next_item.version = version;
    Apply(ConversionResult {
        next_item,
        phase: destination_status.phase,
        effect,
        current_values: command.destination_values.clone(),
        source_values: source_values.to_vec(),
    })
}
