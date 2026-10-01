use task_contract::error_code::{Code, StableCode};
use task_contract::field::{
    apply_edit, ChoiceOption, FieldDefinition, FieldEdit, FieldError, FieldId, FieldKind,
    FieldUsage, FieldValue, OptionId,
};
use task_contract::field_admin::{
    decide_option_admin, OptionAction, OptionAdminCommand, OptionAdminDecision, OptionAdminError,
    SuccessfulOptionAdmin,
};
use task_contract::work_item::{
    Configuration, Phase, ProjectId, Status, StatusId, WorkItemType, WorkItemTypeId, Workflow,
    WorkflowId, MAX_OPTIONS_PER_FIELD,
};

fn config() -> Configuration {
    Configuration {
        project_id: ProjectId("project".into()),
        revision: 4,
        project_archived: false,
        relationship_types: vec![],
        statuses: vec![Status {
            id: StatusId("new".into()),
            phase: Phase::New,
            archived: false,
        }],
        workflows: vec![Workflow {
            id: WorkflowId("flow".into()),
            status_ids: vec![StatusId("new".into())],
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
        fields: vec![FieldDefinition {
            id: FieldId("priority".into()),
            owner_type: WorkItemTypeId("task".into()),
            name: "Priority".into(),
            kind: FieldKind::SingleChoice,
            usage: FieldUsage::Optional,
            archived: false,
            options: vec![ChoiceOption {
                id: OptionId("high".into()),
                name: "High".into(),
                archived: false,
            }],
        }],
    }
}

fn command(action: OptionAction) -> OptionAdminCommand {
    OptionAdminCommand {
        operation_id: "option-1".into(),
        expected_configuration_revision: 4,
        field_id: FieldId("priority".into()),
        action,
    }
}

#[test]
fn option_identity_survives_rename_and_archival() {
    let renamed = command(OptionAction::Rename {
        id: OptionId("high".into()),
        name: "Urgent".into(),
    });
    let OptionAdminDecision::Apply(renamed) = decide_option_admin(&config(), None, true, &renamed)
    else {
        panic!("valid rename must apply")
    };
    assert_eq!(renamed.fields[0].options[0].id, OptionId("high".into()));
    assert_eq!(renamed.fields[0].options[0].name, "Urgent");
    assert_eq!(renamed.revision, 5);
    let archive = OptionAdminCommand {
        operation_id: "option-2".into(),
        expected_configuration_revision: 5,
        action: OptionAction::Archive {
            id: OptionId("high".into()),
        },
        ..command(OptionAction::Archive {
            id: OptionId("high".into()),
        })
    };
    let OptionAdminDecision::Apply(archived) = decide_option_admin(&renamed, None, true, &archive)
    else {
        panic!("valid archive must apply")
    };
    let field = &archived.fields[0];
    let old = FieldValue::SingleChoice(OptionId("high".into()));
    assert_eq!(
        apply_edit(field, &field.owner_type, Some(&old), FieldEdit::Keep),
        Ok(Some(old.clone()))
    );
    assert_eq!(
        apply_edit(field, &field.owner_type, None, FieldEdit::Set(old)),
        Err(FieldError::ArchivedOption)
    );
    let restore = OptionAdminCommand {
        operation_id: "option-3".into(),
        expected_configuration_revision: 6,
        action: OptionAction::Restore {
            id: OptionId("high".into()),
        },
        ..archive
    };
    let OptionAdminDecision::Apply(restored) = decide_option_admin(&archived, None, true, &restore)
    else {
        panic!("valid restore must apply")
    };
    assert!(!restored.fields[0].options[0].archived);
    assert_eq!(restored.fields[0].options[0].id, OptionId("high".into()));
}

#[test]
fn add_rejects_duplicate_identity_and_invalid_display_name() {
    let duplicate = command(OptionAction::Add {
        id: OptionId("high".into()),
        name: "Other".into(),
    });
    assert_eq!(
        decide_option_admin(&config(), None, true, &duplicate),
        OptionAdminDecision::Reject(OptionAdminError::DuplicateOption)
    );
    let invalid = command(OptionAction::Add {
        id: OptionId("low".into()),
        name: "   ".into(),
    });
    assert_eq!(
        decide_option_admin(&config(), None, true, &invalid),
        OptionAdminDecision::Reject(OptionAdminError::InvalidName)
    );
}

#[test]
fn option_admin_checks_revision_and_replays_before_changed_configuration() {
    let request = command(OptionAction::Add {
        id: OptionId("low".into()),
        name: "Low".into(),
    });
    let OptionAdminDecision::Apply(result) = decide_option_admin(&config(), None, true, &request)
    else {
        panic!("valid add must apply")
    };
    let previous = SuccessfulOptionAdmin {
        project_id: ProjectId("project".into()),
        request: request.clone(),
        result: result.clone(),
    };
    assert_eq!(
        decide_option_admin(&result, None, true, &request),
        OptionAdminDecision::Reject(OptionAdminError::ConfigurationConflict {
            current_revision: 5
        })
    );
    assert_eq!(
        decide_option_admin(&result, Some(&previous), true, &request),
        OptionAdminDecision::Replay(result)
    );
}

#[test]
fn archived_project_or_field_blocks_new_option_administration() {
    let request = command(OptionAction::Add {
        id: OptionId("low".into()),
        name: "Low".into(),
    });
    let mut archived_project = config();
    archived_project.project_archived = true;
    assert_eq!(
        decide_option_admin(&archived_project, None, true, &request),
        OptionAdminDecision::Reject(OptionAdminError::ArchivedProject)
    );
    let mut archived_field = config();
    archived_field.fields[0].archived = true;
    assert_eq!(
        decide_option_admin(&archived_field, None, true, &request),
        OptionAdminDecision::Reject(OptionAdminError::ArchivedField)
    );
}

#[test]
fn adding_an_option_past_the_configuration_limit_reports_a_payload_error() {
    let mut full = config();
    full.fields[0].options = (0..MAX_OPTIONS_PER_FIELD)
        .map(|index| ChoiceOption {
            id: OptionId(format!("option-{index}")),
            name: format!("Option {index}"),
            archived: false,
        })
        .collect();
    assert_eq!(full.validate(), Ok(()));
    let add = command(OptionAction::Add {
        id: OptionId("one-more".into()),
        name: "One more".into(),
    });
    let decision = decide_option_admin(&full, None, true, &add);
    assert_eq!(
        decision,
        OptionAdminDecision::Reject(OptionAdminError::PayloadLimitExceeded)
    );
    assert_eq!(
        OptionAdminError::PayloadLimitExceeded.code(),
        Code::PayloadLimitExceeded
    );
}
