use task_contract::configuration_change::{
    validate_change, ConfigurationChangeError, ConfigurationSnapshot, ItemSnapshot,
};
use task_contract::field::{
    FieldDefinition, FieldEntry, FieldId, FieldKind, FieldUsage, FieldValue,
};
use task_contract::work_item::{
    Configuration, Phase, ProjectId, Status, StatusId, WorkItem, WorkItemId, WorkItemType,
    WorkItemTypeId, Workflow, WorkflowId,
};

fn config() -> Configuration {
    Configuration {
        project_id: ProjectId("project".into()),
        revision: 3,
        project_archived: false,
        statuses: vec![
            Status {
                id: StatusId("new".into()),
                phase: Phase::New,
                archived: false,
            },
            Status {
                id: StatusId("ready".into()),
                phase: Phase::New,
                archived: false,
            },
            Status {
                id: StatusId("done".into()),
                phase: Phase::Done,
                archived: false,
            },
        ],
        workflows: vec![Workflow {
            id: WorkflowId("flow".into()),
            status_ids: vec![
                StatusId("new".into()),
                StatusId("ready".into()),
                StatusId("done".into()),
            ],
            initial_status_id: StatusId("new".into()),
            permitted_phase_changes: vec![],
            archived: false,
        }],
        types: vec![WorkItemType {
            id: WorkItemTypeId("task".into()),
            permitted_workflows: vec![WorkflowId("flow".into())],
            default_workflow_id: WorkflowId("flow".into()),
            title_field_id: None,
            archived: false,
        }],
        fields: vec![],
    }
}

fn item(status: &str) -> WorkItem {
    WorkItem {
        id: WorkItemId("item".into()),
        project_id: ProjectId("project".into()),
        type_id: WorkItemTypeId("task".into()),
        workflow_id: WorkflowId("flow".into()),
        status_id: StatusId(status.into()),
        version: 1,
        archived: false,
    }
}

fn next(previous: &Configuration) -> Configuration {
    let mut proposed = previous.clone();
    proposed.revision += 1;
    proposed
}

fn snapshot(items: Vec<WorkItem>) -> ConfigurationSnapshot {
    ConfigurationSnapshot {
        items: items
            .into_iter()
            .map(|item| ItemSnapshot {
                item,
                field_values: vec![],
            })
            .collect(),
        historically_used_field_ids: vec![],
    }
}

fn field(usage: FieldUsage) -> FieldDefinition {
    FieldDefinition {
        id: FieldId("summary".into()),
        owner_type: WorkItemTypeId("task".into()),
        name: "Summary".into(),
        kind: FieldKind::Text,
        usage,
        archived: false,
        options: vec![],
    }
}

#[test]
fn removing_membership_or_type_permission_requires_prior_item_migration() {
    let previous = config();
    let mut proposed = next(&previous);
    proposed.workflows[0]
        .status_ids
        .retain(|id| id.0 != "ready");
    assert_eq!(
        validate_change(
            &previous,
            &proposed,
            3,
            &snapshot(vec![item("ready")]),
            true
        ),
        Err(ConfigurationChangeError::MigrationRequired {
            item_id: WorkItemId("item".into())
        })
    );
    assert_eq!(
        validate_change(&previous, &proposed, 3, &snapshot(vec![item("new")]), true),
        Ok(())
    );

    let mut proposed = next(&previous);
    let mut alternative = proposed.workflows[0].clone();
    alternative.id = WorkflowId("other".into());
    proposed.workflows.push(alternative);
    proposed.types[0].permitted_workflows = vec![WorkflowId("other".into())];
    proposed.types[0].default_workflow_id = WorkflowId("other".into());
    assert_eq!(
        validate_change(&previous, &proposed, 3, &snapshot(vec![item("new")]), true),
        Err(ConfigurationChangeError::MigrationRequired {
            item_id: WorkItemId("item".into())
        })
    );
}

#[test]
fn archiving_initial_status_or_default_workflow_requires_replacement() {
    let previous = config();
    let mut proposed = next(&previous);
    proposed.statuses[0].archived = true;
    assert_eq!(
        validate_change(&previous, &proposed, 3, &snapshot(vec![]), true),
        Err(ConfigurationChangeError::InvalidConfiguration)
    );
    proposed.workflows[0].initial_status_id = StatusId("ready".into());
    assert_eq!(
        validate_change(&previous, &proposed, 3, &snapshot(vec![]), true),
        Ok(())
    );

    let mut proposed = next(&previous);
    proposed.workflows[0].archived = true;
    assert_eq!(
        validate_change(&previous, &proposed, 3, &snapshot(vec![]), true),
        Err(ConfigurationChangeError::InvalidConfiguration)
    );
    proposed.types[0].archived = true;
    assert_eq!(
        validate_change(&previous, &proposed, 3, &snapshot(vec![]), true),
        Ok(())
    );
}

#[test]
fn an_in_use_status_cannot_be_reinterpreted_as_a_different_phase() {
    let previous = config();
    let mut proposed = next(&previous);
    proposed.statuses[2].phase = Phase::New;
    assert_eq!(
        validate_change(&previous, &proposed, 3, &snapshot(vec![item("done")]), true),
        Err(ConfigurationChangeError::MigrationRequired {
            item_id: WorkItemId("item".into())
        })
    );
}

#[test]
fn configuration_edits_require_authorization_revision_and_active_project() {
    let previous = config();
    let proposed = next(&previous);
    assert_eq!(
        validate_change(&previous, &proposed, 3, &snapshot(vec![]), false),
        Err(ConfigurationChangeError::Unauthorized)
    );
    assert_eq!(
        validate_change(&previous, &proposed, 2, &snapshot(vec![]), true),
        Err(ConfigurationChangeError::RevisionConflict {
            current_revision: 3
        })
    );
    let mut archived = previous.clone();
    archived.project_archived = true;
    assert_eq!(
        validate_change(&archived, &proposed, 3, &snapshot(vec![]), true),
        Err(ConfigurationChangeError::ArchivedProject)
    );
}

#[test]
fn requiring_a_field_checks_every_applicable_item_value() {
    let mut previous = config();
    previous.fields.push(field(FieldUsage::Optional));
    let mut proposed = next(&previous);
    proposed.fields[0].usage = FieldUsage::Required;
    let missing = snapshot(vec![item("new")]);
    assert_eq!(
        validate_change(&previous, &proposed, 3, &missing, true),
        Err(ConfigurationChangeError::MigrationRequired {
            item_id: WorkItemId("item".into())
        })
    );
    let mut populated = snapshot(vec![item("new")]);
    populated.items[0].field_values.push(FieldEntry {
        id: FieldId("summary".into()),
        value: FieldValue::Text("Ready".into()),
    });
    assert_eq!(
        validate_change(&previous, &proposed, 3, &populated, true),
        Ok(())
    );
    populated.items[0].field_values[0].value = FieldValue::Text("   ".into());
    assert_eq!(
        validate_change(&previous, &proposed, 3, &populated, true),
        Err(ConfigurationChangeError::MigrationRequired {
            item_id: WorkItemId("item".into())
        })
    );
}

#[test]
fn historical_use_blocks_kind_reinterpretation_even_after_value_is_cleared() {
    let mut previous = config();
    previous.fields.push(field(FieldUsage::Optional));
    let mut proposed = next(&previous);
    proposed.fields[0].kind = FieldKind::Number;
    let mut evidence = snapshot(vec![item("new")]);
    evidence
        .historically_used_field_ids
        .push(FieldId("summary".into()));
    assert_eq!(
        validate_change(&previous, &proposed, 3, &evidence, true),
        Err(ConfigurationChangeError::FieldMigrationRequired {
            field_id: FieldId("summary".into())
        })
    );
    let mut removed = next(&previous);
    removed.fields.clear();
    assert_eq!(
        validate_change(&previous, &removed, 3, &evidence, true),
        Err(ConfigurationChangeError::FieldMigrationRequired {
            field_id: FieldId("summary".into())
        })
    );
    assert_eq!(
        validate_change(&previous, &proposed, 3, &snapshot(vec![item("new")]), true),
        Ok(())
    );
}

#[test]
fn rename_hide_and_archive_preserve_existing_field_values() {
    let mut previous = config();
    previous.fields.push(field(FieldUsage::Optional));
    let mut proposed = next(&previous);
    proposed.fields[0].name = "Work summary".into();
    proposed.fields[0].usage = FieldUsage::Hidden;
    proposed.fields[0].archived = true;
    let mut evidence = snapshot(vec![item("new")]);
    evidence.items[0].field_values.push(FieldEntry {
        id: FieldId("summary".into()),
        value: FieldValue::Text("Preserved".into()),
    });
    assert_eq!(
        validate_change(&previous, &proposed, 3, &evidence, true),
        Ok(())
    );
}

#[test]
fn archiving_a_required_field_does_not_require_missing_current_values() {
    let mut previous = config();
    previous.fields.push(field(FieldUsage::Required));
    let mut proposed = next(&previous);
    proposed.fields[0].archived = true;
    assert_eq!(
        validate_change(&previous, &proposed, 3, &snapshot(vec![item("new")]), true),
        Ok(())
    );
}
