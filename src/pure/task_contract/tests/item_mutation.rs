use task_contract::field::{
    FieldDefinition, FieldEdit, FieldEntry, FieldError, FieldId, FieldKind, FieldUsage, FieldValue,
};
use task_contract::item_mutation::{
    decide_create, decide_edit, display_name, CreateCommand, CreateDecision, EditCommand,
    EditDecision, EstimateEdit, FieldUpdate, ItemData, ItemMutationError, SuccessfulCreate,
    SuccessfulEdit,
};
use task_contract::payload::MAX_FIELD_ENTRIES;
use task_contract::project::{Estimate, EstimateUnit, Project};
use task_contract::work_item::{
    Configuration, Phase, ProjectId, Status, StatusId, WorkItemId, WorkItemType, WorkItemTypeId,
    Workflow, WorkflowId,
};

fn field(id: &str, usage: FieldUsage) -> FieldDefinition {
    FieldDefinition {
        id: FieldId(id.into()),
        owner_type: WorkItemTypeId("task".into()),
        name: id.into(),
        kind: FieldKind::Text,
        usage,
        archived: false,
        options: vec![],
    }
}

fn config() -> Configuration {
    Configuration {
        project_id: ProjectId("project".into()),
        revision: 4,
        project_archived: false,
        relationship_types: vec![],
        statuses: vec![Status {
            id: StatusId("ready".into()),
            phase: Phase::New,
            archived: false,
        }],
        workflows: vec![Workflow {
            id: WorkflowId("flow".into()),
            status_ids: vec![StatusId("ready".into())],
            initial_status_id: StatusId("ready".into()),
            permitted_phase_changes: vec![],
            archived: false,
        }],
        types: vec![WorkItemType {
            id: WorkItemTypeId("task".into()),
            permitted_workflows: vec![WorkflowId("flow".into())],
            default_workflow_id: WorkflowId("flow".into()),
            title_field_id: Some(FieldId("title".into())),
            archived: false,
        }],
        fields: vec![
            field("title", FieldUsage::Optional),
            field("summary", FieldUsage::Required),
        ],
    }
}

fn project() -> Project {
    Project {
        id: ProjectId("project".into()),
        name: "Project".into(),
        prefix: "PRJ".into(),
        configuration_revision: 4,
        next_sequence: 1,
        archived: false,
        estimate_unit: EstimateUnit::Hours,
        ever_estimated: false,
    }
}

fn create() -> CreateCommand {
    CreateCommand {
        operation_id: "create-1".into(),
        expected_configuration_revision: 4,
        item_id: WorkItemId("item-1".into()),
        type_id: WorkItemTypeId("task".into()),
        workflow_id: None,
        estimate: Some(Estimate::try_from_thousandths(0).unwrap()),
        values: vec![FieldEntry {
            id: FieldId("summary".into()),
            value: FieldValue::Text("Work to do".into()),
        }],
    }
}

#[test]
fn create_uses_default_initial_status_and_allows_an_untitled_item() {
    let CreateDecision::Apply(result) = decide_create(&project(), &config(), None, true, &create())
    else {
        panic!("valid create must apply");
    };
    assert_eq!(result.item.status_id, StatusId("ready".into()));
    assert_eq!(result.item.version, 1);
    assert_eq!(result.phase, Phase::New);
    assert_eq!(result.readable_id.display(), "PRJ-1");
    assert_eq!(
        display_name(&config().types[0], &result.readable_id, &result.data),
        "PRJ-1"
    );
    assert!(result.project.ever_estimated);
    assert_eq!(result.project.next_sequence, 2);
}

#[test]
fn create_rejects_missing_required_and_hidden_assignments() {
    let mut missing = create();
    missing.values.clear();
    assert_eq!(
        decide_create(&project(), &config(), None, true, &missing),
        CreateDecision::Reject(ItemMutationError::Field(FieldError::Required))
    );
    let mut hidden_config = config();
    hidden_config.fields[0].usage = FieldUsage::Hidden;
    let mut assigned = create();
    assigned.values.push(FieldEntry {
        id: FieldId("title".into()),
        value: FieldValue::Text("Hidden title".into()),
    });
    assert_eq!(
        decide_create(&project(), &hidden_config, None, true, &assigned),
        CreateDecision::Reject(ItemMutationError::Field(FieldError::Hidden))
    );
}

#[test]
fn edit_preserves_hidden_values_and_does_not_change_status_or_usage() {
    let CreateDecision::Apply(created) =
        decide_create(&project(), &config(), None, true, &create())
    else {
        panic!("valid create must apply");
    };
    let mut config = config();
    config.fields[0].usage = FieldUsage::Hidden;
    let current = ItemData {
        estimate: created.data.estimate,
        values: vec![
            FieldEntry {
                id: FieldId("title".into()),
                value: FieldValue::Text("Old title".into()),
            },
            created.data.values[0].clone(),
        ],
    };
    let command = EditCommand {
        operation_id: "edit-1".into(),
        expected_item_version: 1,
        expected_configuration_revision: 4,
        estimate: EstimateEdit::Keep,
        fields: vec![FieldUpdate {
            id: FieldId("summary".into()),
            edit: FieldEdit::Set(FieldValue::Text("Updated".into())),
        }],
    };
    let EditDecision::Apply(result) = decide_edit(
        &created.project,
        &config,
        &created.item,
        &current,
        None,
        true,
        &command,
    ) else {
        panic!("valid edit must apply");
    };
    assert_eq!(result.item.status_id, created.item.status_id);
    assert_eq!(result.item.version, 2);
    assert_eq!(result.data.estimate, current.estimate);
    assert_eq!(result.data.values[0], current.values[0]);
    assert_eq!(result.phase, Phase::New);
}

#[test]
fn stale_create_conflicts_but_recorded_success_replays() {
    let initial = decide_create(&project(), &config(), None, true, &create());
    let CreateDecision::Apply(result) = initial else {
        panic!("valid create must apply")
    };
    let recorded = SuccessfulCreate {
        project_id: ProjectId("project".into()),
        request: create(),
        result: result.clone(),
    };
    let mut changed = config();
    changed.revision = 5;
    assert_eq!(
        decide_create(&result.project, &changed, None, true, &create()),
        CreateDecision::Reject(ItemMutationError::ConfigurationConflict {
            current_revision: 5
        })
    );
    assert_eq!(
        decide_create(&result.project, &changed, Some(&recorded), true, &create()),
        CreateDecision::Replay(result)
    );
}

#[test]
fn explicit_hidden_field_edits_and_duplicate_field_values_fail() {
    let mut repeated = create();
    repeated.values.push(repeated.values[0].clone());
    assert_eq!(
        decide_create(&project(), &config(), None, true, &repeated),
        CreateDecision::Reject(ItemMutationError::DuplicateField)
    );
    let CreateDecision::Apply(created) =
        decide_create(&project(), &config(), None, true, &create())
    else {
        panic!("valid create must apply");
    };
    let mut hidden = config();
    hidden.fields[0].usage = FieldUsage::Hidden;
    let command = EditCommand {
        operation_id: "edit-hidden".into(),
        expected_item_version: 1,
        expected_configuration_revision: 4,
        estimate: EstimateEdit::Keep,
        fields: vec![FieldUpdate {
            id: FieldId("title".into()),
            edit: FieldEdit::Set(FieldValue::Text("Cannot change".into())),
        }],
    };
    assert_eq!(
        decide_edit(
            &created.project,
            &hidden,
            &created.item,
            &created.data,
            None,
            true,
            &command
        ),
        EditDecision::Reject(ItemMutationError::Field(FieldError::Hidden))
    );
}

#[test]
fn edit_checks_versions_and_replays_recorded_success() {
    let CreateDecision::Apply(created) =
        decide_create(&project(), &config(), None, true, &create())
    else {
        panic!("valid create must apply");
    };
    let command = EditCommand {
        operation_id: "edit-1".into(),
        expected_item_version: 1,
        expected_configuration_revision: 4,
        estimate: EstimateEdit::Clear,
        fields: vec![],
    };
    let EditDecision::Apply(result) = decide_edit(
        &created.project,
        &config(),
        &created.item,
        &created.data,
        None,
        true,
        &command,
    ) else {
        panic!("valid edit must apply");
    };
    assert_eq!(result.data.estimate, None);
    assert!(result.project.ever_estimated);
    assert_eq!(
        decide_edit(
            &result.project,
            &config(),
            &result.item,
            &result.data,
            None,
            true,
            &command
        ),
        EditDecision::Reject(ItemMutationError::ItemVersionConflict { current_version: 2 })
    );
    let recorded = SuccessfulEdit {
        item_id: created.item.id.clone(),
        request: command.clone(),
        result: result.clone(),
    };
    let mut changed = config();
    changed.revision = 5;
    assert_eq!(
        decide_edit(
            &result.project,
            &changed,
            &result.item,
            &result.data,
            Some(&recorded),
            true,
            &command
        ),
        EditDecision::Replay(result)
    );
}

#[test]
fn a_nonblank_configured_title_changes_display_but_not_identity() {
    let mut titled = create();
    titled.values.push(FieldEntry {
        id: FieldId("title".into()),
        value: FieldValue::Text("Visible name".into()),
    });
    let CreateDecision::Apply(result) = decide_create(&project(), &config(), None, true, &titled)
    else {
        panic!("valid create must apply");
    };
    assert_eq!(
        display_name(&config().types[0], &result.readable_id, &result.data),
        "Visible name"
    );
    assert_eq!(result.readable_id.display(), "PRJ-1");
}

#[test]
fn new_commands_reject_archived_project_and_ineligible_workflow() {
    let mut archived = project();
    archived.archived = true;
    assert_eq!(
        decide_create(&archived, &config(), None, true, &create()),
        CreateDecision::Reject(ItemMutationError::ArchivedProject)
    );
    let mut ineligible = create();
    ineligible.workflow_id = Some(WorkflowId("missing".into()));
    assert_eq!(
        decide_create(&project(), &config(), None, true, &ineligible),
        CreateDecision::Reject(ItemMutationError::InvalidReference)
    );
}

#[test]
fn create_and_edit_reject_oversized_field_payloads() {
    let mut oversized = create();
    oversized.values = vec![oversized.values[0].clone(); MAX_FIELD_ENTRIES + 1];
    assert_eq!(
        decide_create(&project(), &config(), None, true, &oversized),
        CreateDecision::Reject(ItemMutationError::PayloadLimitExceeded)
    );

    let CreateDecision::Apply(created) =
        decide_create(&project(), &config(), None, true, &create())
    else {
        panic!("fixture create must apply");
    };
    let edit = EditCommand {
        operation_id: "edit-oversized".into(),
        expected_item_version: created.item.version,
        expected_configuration_revision: 4,
        estimate: EstimateEdit::Keep,
        fields: vec![
            FieldUpdate {
                id: FieldId("title".into()),
                edit: FieldEdit::Keep,
            };
            MAX_FIELD_ENTRIES + 1
        ],
    };
    assert_eq!(
        decide_edit(
            &created.project,
            &config(),
            &created.item,
            &created.data,
            None,
            true,
            &edit,
        ),
        EditDecision::Reject(ItemMutationError::PayloadLimitExceeded)
    );
}

#[test]
fn create_counts_combined_text_bytes_across_distinct_valid_fields() {
    let mut configuration = config();
    let mut command = create();
    for index in 0..65 {
        let id = format!("note-{index}");
        configuration.fields.push(field(&id, FieldUsage::Optional));
        command.values.push(FieldEntry {
            id: FieldId(id),
            value: FieldValue::Text("x".repeat(16 * 1024)),
        });
    }
    assert_eq!(
        decide_create(&project(), &configuration, None, true, &command),
        CreateDecision::Reject(ItemMutationError::PayloadLimitExceeded)
    );
}

#[test]
fn recorded_success_replays_before_current_payload_limit() {
    let CreateDecision::Apply(result) = decide_create(&project(), &config(), None, true, &create())
    else {
        panic!("fixture create must apply");
    };
    let mut oversized = create();
    oversized.values = vec![oversized.values[0].clone(); MAX_FIELD_ENTRIES + 1];
    let recorded = SuccessfulCreate {
        project_id: ProjectId("project".into()),
        request: oversized.clone(),
        result: result.clone(),
    };
    assert_eq!(
        decide_create(&project(), &config(), Some(&recorded), true, &oversized),
        CreateDecision::Replay(result)
    );
}
