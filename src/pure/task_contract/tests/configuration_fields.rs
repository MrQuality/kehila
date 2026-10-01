use task_contract::field::{FieldDefinition, FieldId, FieldKind, FieldUsage};
use task_contract::payload::MAX_FIELD_ENTRIES;
use task_contract::relationship::{Direction, RelationshipType, RelationshipTypeId};
use task_contract::work_item::{
    Configuration, Error, Phase, ProjectId, Status, StatusId, WorkItemType, WorkItemTypeId,
    Workflow, WorkflowId,
};

fn config() -> Configuration {
    Configuration {
        project_id: ProjectId("project".into()),
        revision: 1,
        project_archived: false,
        status_groups: vec![],
        relationship_types: vec![],
        statuses: vec![Status {
            id: StatusId("new".into()),
            name: "Status".into(),
            group_id: None,
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
            title_field_id: Some(FieldId("title".into())),
            archived: false,
        }],
        fields: vec![FieldDefinition {
            id: FieldId("title".into()),
            owner_type: WorkItemTypeId("task".into()),
            name: "Title".into(),
            kind: FieldKind::Text,
            usage: FieldUsage::Optional,
            archived: false,
            options: vec![],
        }],
    }
}

#[test]
fn title_role_must_reference_a_text_field_of_the_same_type() {
    let good = config();
    assert_eq!(good.validate(), Ok(()));
    let mut wrong_kind = good.clone();
    wrong_kind.fields[0].kind = FieldKind::Boolean;
    assert_eq!(wrong_kind.validate(), Err(Error::InvalidConfiguration));
    let mut wrong_owner = good.clone();
    wrong_owner.fields[0].owner_type = WorkItemTypeId("other".into());
    assert_eq!(wrong_owner.validate(), Err(Error::InvalidConfiguration));
    let mut missing = good;
    missing.types[0].title_field_id = Some(FieldId("missing".into()));
    assert_eq!(missing.validate(), Err(Error::InvalidConfiguration));
}

#[test]
fn field_definitions_are_unique_in_one_configuration_revision() {
    let mut duplicate = config();
    duplicate.fields.push(duplicate.fields[0].clone());
    assert_eq!(duplicate.validate(), Err(Error::InvalidConfiguration));
}

#[test]
fn display_names_reject_control_characters() {
    let mut configuration = config();
    configuration.fields[0].name = "Title\0hidden".into();
    assert_eq!(configuration.validate(), Err(Error::InvalidConfiguration));

    let mut configuration = config();
    configuration.fields[0].kind = FieldKind::SingleChoice;
    configuration.types[0].title_field_id = None;
    configuration.fields[0].options = vec![task_contract::field::ChoiceOption {
        id: task_contract::field::OptionId("option".into()),
        name: "Option\nother".into(),
        archived: false,
    }];
    assert_eq!(configuration.validate(), Err(Error::InvalidConfiguration));

    let mut configuration = config();
    configuration.relationship_types.push(RelationshipType {
        owner_project_id: configuration.project_id.clone(),
        id: RelationshipTypeId("related".into()),
        name: "Related\0hidden".into(),
        direction: Direction::Directed,
        archived: false,
    });
    assert_eq!(configuration.validate(), Err(Error::InvalidConfiguration));
}

#[test]
fn required_fields_cannot_exceed_one_create_payload() {
    let mut configuration = config();
    for index in 0..MAX_FIELD_ENTRIES {
        configuration.fields.push(FieldDefinition {
            id: FieldId(format!("required-{index}")),
            owner_type: WorkItemTypeId("task".into()),
            name: format!("Required {index}"),
            kind: FieldKind::Boolean,
            usage: FieldUsage::Required,
            archived: false,
            options: vec![],
        });
    }
    assert_eq!(configuration.validate(), Ok(()));
    configuration.fields.push(FieldDefinition {
        id: FieldId("one-too-many".into()),
        owner_type: WorkItemTypeId("task".into()),
        name: "One too many".into(),
        kind: FieldKind::Boolean,
        usage: FieldUsage::Required,
        archived: false,
        options: vec![],
    });
    assert_eq!(configuration.validate(), Err(Error::InvalidConfiguration));
}
