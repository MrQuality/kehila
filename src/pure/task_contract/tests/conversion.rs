use task_contract::conversion::{
    decide_conversion, ConversionCommand, ConversionDecision, ConversionError, FieldEntry,
    SourceSnapshotError, SuccessfulConversion,
};
use task_contract::field::{
    FieldDefinition, FieldError, FieldId, FieldKind, FieldUsage, FieldValue,
};
use task_contract::payload::MAX_FIELD_ENTRIES;
use task_contract::work_item::{
    Configuration, LifecycleEffect, Phase, PhaseChange, ProjectId, Status, StatusId, WorkItem,
    WorkItemId, WorkItemType, WorkItemTypeId, Workflow, WorkflowId,
};

fn config() -> Configuration {
    let statuses = [
        ("new", Phase::New),
        ("active", Phase::Active),
        ("done", Phase::Done),
    ]
    .into_iter()
    .map(|(id, phase)| Status {
        id: StatusId(id.into()),
        name: id.into(),
        group_id: None,
        phase,
        archived: false,
    })
    .collect();
    let workflow = |id: &str| Workflow {
        id: WorkflowId(id.into()),
        status_ids: ["new", "active", "done"]
            .into_iter()
            .map(|id| StatusId(id.into()))
            .collect(),
        initial_status_id: StatusId("new".into()),
        permitted_phase_changes: vec![
            PhaseChange::new(Phase::New, Phase::Active),
            PhaseChange::new(Phase::Active, Phase::Done),
            PhaseChange::new(Phase::Done, Phase::Active),
        ],
        archived: false,
    };
    Configuration {
        project_id: ProjectId("project".into()),
        revision: 4,
        project_archived: false,
        status_groups: vec![],
        relationship_types: vec![],
        statuses,
        workflows: vec![workflow("task-flow"), workflow("milestone-flow")],
        types: vec![
            WorkItemType {
                id: WorkItemTypeId("task".into()),
                permitted_workflows: vec![WorkflowId("task-flow".into())],
                default_workflow_id: WorkflowId("task-flow".into()),
                title_field_id: None,
                archived: false,
            },
            WorkItemType {
                id: WorkItemTypeId("milestone".into()),
                permitted_workflows: vec![WorkflowId("milestone-flow".into())],
                default_workflow_id: WorkflowId("milestone-flow".into()),
                title_field_id: None,
                archived: false,
            },
        ],
        fields: definitions(),
    }
}

fn item(status: &str) -> WorkItem {
    WorkItem {
        id: WorkItemId("item".into()),
        project_id: ProjectId("project".into()),
        type_id: WorkItemTypeId("task".into()),
        workflow_id: WorkflowId("task-flow".into()),
        status_id: StatusId(status.into()),
        version: 2,
        archived: false,
    }
}

fn definitions() -> Vec<FieldDefinition> {
    vec![
        FieldDefinition {
            id: FieldId("milestone-goal".into()),
            owner_type: WorkItemTypeId("milestone".into()),
            name: "Goal".into(),
            kind: FieldKind::Text,
            origin: task_contract::field::FieldOrigin::Project,
            usage: FieldUsage::Required,
            archived: false,
            options: vec![],
        },
        FieldDefinition {
            id: FieldId("task-note".into()),
            owner_type: WorkItemTypeId("task".into()),
            name: "Note".into(),
            kind: FieldKind::Text,
            origin: task_contract::field::FieldOrigin::Project,
            usage: FieldUsage::Optional,
            archived: false,
            options: vec![],
        },
    ]
}

#[test]
fn conversion_rejects_malformed_source_snapshots_with_field_details() {
    for (id, value, reason) in [
        (
            "missing",
            FieldValue::Text("text".into()),
            SourceSnapshotError::UnknownField,
        ),
        (
            "milestone-goal",
            FieldValue::Text("text".into()),
            SourceSnapshotError::WrongOwner,
        ),
        (
            "task-note",
            FieldValue::Boolean(true),
            SourceSnapshotError::InvalidValue(FieldError::WrongValueKind),
        ),
    ] {
        let value = FieldEntry {
            id: FieldId(id.into()),
            value,
        };
        assert_eq!(
            decide_conversion(
                &config(),
                &item("active"),
                &[value],
                None,
                true,
                true,
                &command("active")
            ),
            ConversionDecision::Reject(ConversionError::InvalidSourceSnapshot {
                field_id: FieldId(id.into()),
                reason
            })
        );
    }
    let mut configuration = config();
    configuration.fields[1].usage = FieldUsage::Required;
    assert_eq!(
        decide_conversion(
            &configuration,
            &item("active"),
            &[],
            None,
            true,
            true,
            &command("active")
        ),
        ConversionDecision::Reject(ConversionError::InvalidSourceSnapshot {
            field_id: FieldId("task-note".into()),
            reason: SourceSnapshotError::InvalidValue(FieldError::Required)
        })
    );
}

#[test]
fn conversion_preserves_valid_hidden_and_archived_source_values() {
    let mut configuration = config();
    configuration.fields[1].usage = FieldUsage::Hidden;
    configuration.fields[1].archived = true;
    let values = vec![FieldEntry {
        id: FieldId("task-note".into()),
        value: FieldValue::Text("Retained note".into()),
    }];
    let ConversionDecision::Apply(result) = decide_conversion(
        &configuration,
        &item("active"),
        &values,
        None,
        true,
        true,
        &command("active"),
    ) else {
        panic!("valid source history should survive")
    };
    assert_eq!(result.source_values, values);
}

fn command(status: &str) -> ConversionCommand {
    ConversionCommand {
        operation_id: "convert-1".into(),
        expected_item_version: 2,
        expected_configuration_revision: 4,
        destination_type: WorkItemTypeId("milestone".into()),
        destination_workflow: WorkflowId("milestone-flow".into()),
        destination_status: StatusId(status.into()),
        destination_values: vec![FieldEntry {
            id: FieldId("milestone-goal".into()),
            value: FieldValue::Text("Deliver the release".into()),
        }],
    }
}

#[test]
fn conversion_rejects_oversized_destination_payload() {
    let mut oversized = command("active");
    oversized.destination_values =
        vec![oversized.destination_values[0].clone(); MAX_FIELD_ENTRIES + 1];
    assert_eq!(
        decide_conversion(
            &config(),
            &item("active"),
            &[],
            None,
            true,
            true,
            &oversized
        ),
        ConversionDecision::Reject(ConversionError::PayloadLimitExceeded)
    );
}

#[test]
fn conversion_checks_fresh_versions_live_targets_and_operation_identity() {
    let configuration = config();
    let source = item("active");
    let mut request = command("active");
    request.expected_configuration_revision -= 1;
    assert_eq!(
        decide_conversion(&configuration, &source, &[], None, true, true, &request),
        ConversionDecision::Reject(ConversionError::ConfigurationConflict {
            current_revision: 4
        })
    );
    request = command("active");
    request.expected_item_version -= 1;
    assert_eq!(
        decide_conversion(&configuration, &source, &[], None, true, true, &request),
        ConversionDecision::Reject(ConversionError::ItemVersionConflict { current_version: 2 })
    );
    request = command("active");
    request.operation_id.clear();
    assert_eq!(
        decide_conversion(&configuration, &source, &[], None, true, true, &request),
        ConversionDecision::Reject(ConversionError::InvalidOperation)
    );
    let mut archived_project = configuration.clone();
    archived_project.project_archived = true;
    assert_eq!(
        decide_conversion(
            &archived_project,
            &source,
            &[],
            None,
            true,
            true,
            &command("active")
        ),
        ConversionDecision::Reject(ConversionError::ArchivedProject)
    );
    let mut archived_item = source;
    archived_item.archived = true;
    assert_eq!(
        decide_conversion(
            &configuration,
            &archived_item,
            &[],
            None,
            true,
            true,
            &command("active")
        ),
        ConversionDecision::Reject(ConversionError::ArchivedItem)
    );
}

#[test]
fn conversion_requires_a_complete_compatible_destination_and_migration_permission() {
    let config = config();
    let source = item("active");
    let mut incomplete = command("active");
    incomplete.destination_workflow = WorkflowId("task-flow".into());
    assert_eq!(
        decide_conversion(&config, &source, &[], None, true, true, &incomplete),
        ConversionDecision::Reject(ConversionError::InvalidReference)
    );
    assert_eq!(
        decide_conversion(&config, &source, &[], None, true, false, &command("active")),
        ConversionDecision::Reject(ConversionError::MigrationUnauthorized)
    );
    let mut missing = command("active");
    missing.destination_values.clear();
    assert_eq!(
        decide_conversion(&config, &source, &[], None, true, true, &missing),
        ConversionDecision::Reject(ConversionError::Field(FieldError::Required))
    );
}

#[test]
fn conversion_can_retain_existing_archived_workflow_and_status() {
    let mut configuration = config();
    configuration.types[0]
        .permitted_workflows
        .push(WorkflowId("milestone-flow".into()));
    configuration.types[0].default_workflow_id = WorkflowId("milestone-flow".into());
    configuration.types[1]
        .permitted_workflows
        .push(WorkflowId("task-flow".into()));
    configuration.workflows[0].archived = true;
    configuration.statuses[1].archived = true;
    assert_eq!(configuration.validate(), Ok(()));
    let source = item("active");
    let mut request = command("active");
    request.destination_workflow = source.workflow_id.clone();
    let ConversionDecision::Apply(result) =
        decide_conversion(&configuration, &source, &[], None, true, false, &request)
    else {
        panic!("retained references should remain valid")
    };
    assert_eq!(result.next_item.workflow_id, source.workflow_id);
    assert_eq!(result.next_item.status_id, source.status_id);
    assert_eq!(result.effect, LifecycleEffect::None);
    request.destination_status = StatusId("done".into());
    assert!(matches!(
        decide_conversion(&configuration, &source, &[], None, true, false, &request),
        ConversionDecision::Apply(_)
    ));
    configuration.statuses[2].archived = true;
    assert_eq!(
        decide_conversion(&configuration, &source, &[], None, true, false, &request),
        ConversionDecision::Reject(ConversionError::ArchivedTarget)
    );
}

#[test]
fn conversion_cannot_migrate_into_a_different_archived_workflow() {
    let mut configuration = config();
    configuration.types[1]
        .permitted_workflows
        .push(WorkflowId("task-flow".into()));
    configuration.types[1].default_workflow_id = WorkflowId("task-flow".into());
    configuration.workflows[1].archived = true;
    assert_eq!(configuration.validate(), Ok(()));
    assert_eq!(
        decide_conversion(
            &configuration,
            &item("active"),
            &[],
            None,
            true,
            true,
            &command("active")
        ),
        ConversionDecision::Reject(ConversionError::ArchivedTarget)
    );
}

#[test]
fn conversion_uses_both_workflows_phase_rules_and_forbids_done_to_new() {
    let config = config();
    let source = item("done");
    assert_eq!(
        decide_conversion(&config, &source, &[], None, true, true, &command("new")),
        ConversionDecision::Reject(ConversionError::ProhibitedPhaseChange)
    );
    let mut restricted = config.clone();
    restricted.workflows[1].permitted_phase_changes.clear();
    assert_eq!(
        decide_conversion(
            &restricted,
            &source,
            &[],
            None,
            true,
            true,
            &command("active")
        ),
        ConversionDecision::Reject(ConversionError::ProhibitedPhaseChange)
    );
    let ConversionDecision::Apply(result) =
        decide_conversion(&config, &source, &[], None, true, true, &command("active"))
    else {
        panic!("valid reopening conversion must apply");
    };
    assert_eq!(result.phase, Phase::Active);
    assert_eq!(result.effect, LifecycleEffect::Reopening);
    assert_eq!(result.next_item.version, 3);
    assert_eq!(result.next_item.id, source.id);
}

#[test]
fn conversion_retains_source_values_in_history_independent_of_current_values() {
    let source = item("active");
    let old_value = FieldEntry {
        id: FieldId("task-note".into()),
        value: FieldValue::Text("Original note".into()),
    };
    let ConversionDecision::Apply(result) = decide_conversion(
        &config(),
        &source,
        std::slice::from_ref(&old_value),
        None,
        true,
        true,
        &command("active"),
    ) else {
        panic!("valid conversion must apply");
    };
    assert_eq!(result.source_values, vec![old_value]);
    assert_eq!(result.current_values, command("active").destination_values);
    assert_eq!(source.type_id, WorkItemTypeId("task".into()));
}

#[test]
fn identical_recorded_conversion_replays_before_new_rules_and_versions() {
    let source = item("active");
    let request = command("active");
    let ConversionDecision::Apply(result) =
        decide_conversion(&config(), &source, &[], None, true, true, &request)
    else {
        panic!("initial conversion must apply");
    };
    let recorded = SuccessfulConversion {
        item_id: source.id.clone(),
        request: request.clone(),
        result: result.clone(),
    };
    let mut changed = config();
    changed.revision = 5;
    changed.types[1].archived = true;
    assert_eq!(
        decide_conversion(
            &changed,
            &result.next_item,
            &[],
            Some(&recorded),
            true,
            true,
            &request
        ),
        ConversionDecision::Replay(result.clone())
    );
    assert_eq!(
        decide_conversion(
            &changed,
            &result.next_item,
            &[],
            Some(&recorded),
            true,
            false,
            &request
        ),
        ConversionDecision::Reject(ConversionError::MigrationUnauthorized)
    );
    let mut reused = request.clone();
    reused.destination_values.clear();
    assert_eq!(
        decide_conversion(&changed, &source, &[], Some(&recorded), true, true, &reused),
        ConversionDecision::Reject(ConversionError::OperationIdReused)
    );
}

#[test]
fn conversion_without_workflow_migration_does_not_add_a_replay_migration_grant() {
    let source = item("active");
    let mut configuration = config();
    configuration.types[1]
        .permitted_workflows
        .push(source.workflow_id.clone());
    let mut request = command("active");
    request.destination_workflow = source.workflow_id.clone();
    let ConversionDecision::Apply(result) =
        decide_conversion(&configuration, &source, &[], None, true, false, &request)
    else {
        panic!("no migration grant is needed")
    };
    assert!(!result.requires_migration_permission);
    let recorded = SuccessfulConversion {
        item_id: source.id.clone(),
        request: request.clone(),
        result: result.clone(),
    };
    assert_eq!(
        decide_conversion(
            &configuration,
            &result.next_item,
            &[],
            Some(&recorded),
            true,
            false,
            &request
        ),
        ConversionDecision::Replay(result)
    );
}
