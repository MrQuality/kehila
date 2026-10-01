//! Pure compatibility rules for one project-configuration revision change.

use std::collections::HashSet;

use crate::field::{apply_edit, FieldEdit, FieldEntry, FieldId, FieldOrigin, FieldUsage};
use crate::relationship::RelationshipTypeId;
use crate::work_item::{Configuration, WorkItem, WorkItemId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemSnapshot {
    pub item: WorkItem,
    pub field_values: Vec<FieldEntry>,
}

/// Complete authoritative current items and evidence of historical field use.
/// B-005 must read and protect this evidence through the configuration commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigurationSnapshot {
    pub items: Vec<ItemSnapshot>,
    pub historically_used_field_ids: Vec<FieldId>,
    pub historically_used_relationship_type_ids: Vec<RelationshipTypeId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigurationChangeCommand {
    pub operation_id: String,
    pub expected_revision: u64,
    /// Complete proposed configuration, not a partial patch.
    pub proposed: Configuration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuccessfulConfigurationChange {
    pub project_id: crate::work_item::ProjectId,
    pub request: ConfigurationChangeCommand,
    pub result: Configuration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigurationChangeDecision {
    Replay(Configuration),
    Apply(Configuration),
    Reject(ConfigurationChangeError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigurationChangeError {
    Unauthorized,
    OperationIdReused,
    RevisionConflict { current_revision: u64 },
    ArchivedProject,
    InvalidConfiguration,
    InvalidReference,
    InvalidOperation,
    PayloadLimitExceeded,
    RevisionOverflow,
    MigrationRequired { item_id: WorkItemId },
    FieldMigrationRequired { field_id: FieldId },
    RelationshipMigrationRequired { type_id: RelationshipTypeId },
}

/// A complete configuration replacement is one versioned, replayable command.
/// B-005 must supply authoritative snapshot and replay lookup, then commit the
/// new revision, history, and success record under one consistency boundary.
pub fn decide_configuration_change(
    previous: &Configuration,
    snapshot: &ConfigurationSnapshot,
    recorded: Option<&SuccessfulConfigurationChange>,
    authorized: bool,
    command: &ConfigurationChangeCommand,
) -> ConfigurationChangeDecision {
    use ConfigurationChangeDecision::{Apply, Reject, Replay};
    use ConfigurationChangeError as E;

    if !authorized {
        return Reject(E::Unauthorized);
    }
    if let Some(recorded) = recorded {
        if recorded.project_id != previous.project_id
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
    if command.expected_revision != previous.revision {
        return Reject(E::RevisionConflict {
            current_revision: previous.revision,
        });
    }
    if command.operation_id.is_empty() {
        return Reject(E::InvalidOperation);
    }
    if !command.proposed.within_limits() {
        return Reject(E::PayloadLimitExceeded);
    }
    if let Err(error) = validate_change(
        previous,
        &command.proposed,
        command.expected_revision,
        snapshot,
        true,
    ) {
        return Reject(error);
    }
    let mut comparable = command.proposed.clone();
    comparable.revision = previous.revision;
    if comparable == *previous {
        return Reject(E::InvalidOperation);
    }
    Apply(command.proposed.clone())
}

/// `snapshot` must contain every current item and its values, plus all field IDs
/// ever used in accepted history. The caller must serialize validation and
/// commit with item writes and historical-use changes.
pub fn validate_change(
    previous: &Configuration,
    proposed: &Configuration,
    expected_revision: u64,
    snapshot: &ConfigurationSnapshot,
    authorized: bool,
) -> Result<(), ConfigurationChangeError> {
    if !authorized {
        return Err(ConfigurationChangeError::Unauthorized);
    }
    if expected_revision != previous.revision {
        return Err(ConfigurationChangeError::RevisionConflict {
            current_revision: previous.revision,
        });
    }
    if previous.project_archived {
        return Err(ConfigurationChangeError::ArchivedProject);
    }
    let next_revision = previous
        .revision
        .checked_add(1)
        .ok_or(ConfigurationChangeError::RevisionOverflow)?;
    if proposed.project_id != previous.project_id
        || proposed.revision != next_revision
        || proposed.project_archived != previous.project_archived
        || proposed.validate().is_err()
    {
        return Err(ConfigurationChangeError::InvalidConfiguration);
    }
    let mut item_ids = HashSet::new();
    let historical_ids: HashSet<_> = snapshot.historically_used_field_ids.iter().collect();
    let used_relationship_ids: HashSet<_> = snapshot
        .historically_used_relationship_type_ids
        .iter()
        .collect();
    for old_type in &previous.relationship_types {
        let replacement = proposed
            .relationship_types
            .iter()
            .find(|candidate| candidate.id == old_type.id);
        if used_relationship_ids.contains(&old_type.id)
            && !matches!(replacement, Some(candidate) if candidate.direction == old_type.direction)
        {
            return Err(ConfigurationChangeError::RelationshipMigrationRequired {
                type_id: old_type.id.clone(),
            });
        }
    }
    for old_field in &previous.fields {
        let new_field = proposed
            .fields
            .iter()
            .find(|field| field.id == old_field.id);
        if old_field.origin == FieldOrigin::Application
            && !matches!(new_field, Some(field) if field.origin == FieldOrigin::Application
                && !field.archived && field.kind == old_field.kind)
        {
            return Err(ConfigurationChangeError::InvalidConfiguration);
        }
        let changes_historical_kind = match new_field {
            Some(field) => field.kind != old_field.kind,
            None => true,
        };
        if historical_ids.contains(&old_field.id) && changes_historical_kind {
            return Err(ConfigurationChangeError::FieldMigrationRequired {
                field_id: old_field.id.clone(),
            });
        }
        if let Some(field) = new_field {
            if field.owner_type != old_field.owner_type || field.origin != old_field.origin {
                return Err(ConfigurationChangeError::InvalidConfiguration);
            }
            if field.kind == old_field.kind && !old_field.options.is_empty() {
                let retained_ids: HashSet<_> =
                    field.options.iter().map(|option| &option.id).collect();
                if old_field
                    .options
                    .iter()
                    .any(|option| !retained_ids.contains(&option.id))
                {
                    return Err(ConfigurationChangeError::InvalidConfiguration);
                }
            }
        }
    }
    if proposed.fields.iter().any(|field| {
        field.origin == FieldOrigin::Application
            && !previous.fields.iter().any(|old| old.id == field.id)
    }) {
        return Err(ConfigurationChangeError::InvalidConfiguration);
    }
    for snapshot_item in &snapshot.items {
        let item = &snapshot_item.item;
        if !item_ids.insert(&item.id) {
            return Err(ConfigurationChangeError::InvalidConfiguration);
        }
        if item.project_id != previous.project_id {
            return Err(ConfigurationChangeError::InvalidConfiguration);
        }
        let Some(item_type) = proposed.types.iter().find(|entry| entry.id == item.type_id) else {
            return Err(ConfigurationChangeError::MigrationRequired {
                item_id: item.id.clone(),
            });
        };
        let Some(workflow) = proposed
            .workflows
            .iter()
            .find(|entry| entry.id == item.workflow_id)
        else {
            return Err(ConfigurationChangeError::MigrationRequired {
                item_id: item.id.clone(),
            });
        };
        let Some(status) = proposed
            .statuses
            .iter()
            .find(|entry| entry.id == item.status_id)
        else {
            return Err(ConfigurationChangeError::MigrationRequired {
                item_id: item.id.clone(),
            });
        };
        if !item_type.permitted_workflows.contains(&item.workflow_id)
            || !workflow.status_ids.contains(&item.status_id)
        {
            return Err(ConfigurationChangeError::MigrationRequired {
                item_id: item.id.clone(),
            });
        }
        let old_status = previous
            .statuses
            .iter()
            .find(|entry| entry.id == item.status_id);
        if !matches!(old_status, Some(old) if old.phase == status.phase) {
            return Err(ConfigurationChangeError::MigrationRequired {
                item_id: item.id.clone(),
            });
        }
        let mut value_ids = HashSet::new();
        for entry in &snapshot_item.field_values {
            if !value_ids.insert(&entry.id)
                || !previous
                    .fields
                    .iter()
                    .any(|field| field.id == entry.id && field.owner_type == item.type_id)
            {
                return Err(ConfigurationChangeError::InvalidConfiguration);
            }
            let Some(definition) = proposed.fields.iter().find(|field| field.id == entry.id) else {
                return Err(ConfigurationChangeError::MigrationRequired {
                    item_id: item.id.clone(),
                });
            };
            if apply_edit(
                definition,
                &item.type_id,
                Some(&entry.value),
                FieldEdit::Keep,
            )
            .is_err()
            {
                return Err(ConfigurationChangeError::MigrationRequired {
                    item_id: item.id.clone(),
                });
            }
        }
        for definition in proposed.fields.iter().filter(|field| {
            field.owner_type == item.type_id
                && !field.archived
                && field.usage == FieldUsage::Required
        }) {
            let current = snapshot_item
                .field_values
                .iter()
                .find(|entry| entry.id == definition.id)
                .map(|entry| &entry.value);
            if apply_edit(definition, &item.type_id, current, FieldEdit::Keep).is_err() {
                return Err(ConfigurationChangeError::MigrationRequired {
                    item_id: item.id.clone(),
                });
            }
        }
    }
    Ok(())
}
