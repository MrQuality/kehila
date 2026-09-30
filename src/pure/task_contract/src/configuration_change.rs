//! Pure compatibility rules for one project-configuration revision change.

use crate::work_item::{Configuration, WorkItem, WorkItemId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigurationChangeError {
    Unauthorized,
    RevisionConflict { current_revision: u64 },
    ArchivedProject,
    InvalidConfiguration,
    RevisionOverflow,
    MigrationRequired { item_id: WorkItemId },
}

/// `affected_items` must be the complete authoritative set of items in the
/// project. The caller must serialize its validation and commit with item writes.
pub fn validate_change(
    previous: &Configuration,
    proposed: &Configuration,
    expected_revision: u64,
    affected_items: &[WorkItem],
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
    for item in affected_items {
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
    }
    Ok(())
}
