use task_contract::{current_work::UserId, field::*, item_mutation::*, project::*, work_item::*};
use work_item_codec::*;

fn created() -> CreateResult {
    CreateResult {
        project: Project {
            id: ProjectId("p".into()),
            name: "project".into(),
            prefix: "PR".into(),
            configuration_revision: 1,
            next_sequence: 2,
            archived: false,
            estimate_unit: EstimateUnit::Hours,
            ever_estimated: false,
        },
        item: WorkItem {
            id: WorkItemId("i".into()),
            project_id: ProjectId("p".into()),
            type_id: WorkItemTypeId("task".into()),
            workflow_id: WorkflowId("wf".into()),
            status_id: StatusId("new".into()),
            version: 1,
            archived: false,
        },
        data: ItemData {
            estimate: None,
            values: vec![],
        },
        phase: Phase::New,
        readable_id: ReadableId {
            project_id: ProjectId("p".into()),
            item_id: WorkItemId("i".into()),
            prefix: "PR".into(),
            sequence: 1,
        },
    }
}

#[test]
fn independent_saved_result_and_revision_shapes_are_frozen() {
    let create = SavedResult::Create(created());
    let c = created();
    let mut item = c.item;
    item.version = 2;
    let edit = SavedResult::Edit(EditResult {
        project: c.project,
        item,
        data: c.data,
        phase: c.phase,
    });
    for (value, fixture) in [
        (
            create.clone(),
            include_bytes!("fixtures/create-result.json").as_slice(),
        ),
        (edit, include_bytes!("fixtures/edit-result.json").as_slice()),
    ] {
        assert_eq!(
            parse_json(&encode_result(&value, 1).unwrap()).unwrap(),
            parse_json(fixture).unwrap()
        );
        assert_eq!(decode_result(1, fixture).unwrap(), value);
    }
    let revision = Revision {
        actor_id: UserId("a".into()),
        configuration_revision: 1,
        project_state_version: 1,
        previous_project_operation: 4,
        result: create,
    };
    let fixture = include_bytes!("fixtures/revision.json");
    assert_eq!(
        parse_json(&encode_revision(&revision, 1).unwrap()).unwrap(),
        parse_json(fixture).unwrap()
    );
    assert_eq!(decode_revision(1, fixture).unwrap(), revision);
}

#[test]
fn individually_bounded_result_and_actor_cannot_overflow_the_revision_aggregate() {
    let mut c = created();
    c.data.values = (0..16)
        .map(|i| FieldEntry {
            id: FieldId(format!("f{i}")),
            value: FieldValue::Text("a".repeat(60 * 1024)),
        })
        .collect();
    let result = SavedResult::Create(c);
    assert!(admit_result(&result).is_ok());
    let mut revision = Revision {
        actor_id: UserId("a".into()),
        configuration_revision: 1,
        project_state_version: 1,
        previous_project_operation: 4,
        result,
    };
    assert!(admit_revision(&revision).is_ok());
    revision.actor_id.0 = "a".repeat(64 * 1024);
    assert!(encode_revision(&revision, 1).is_err());
    assert!(admit_revision(&revision).is_err());
}

#[test]
fn revision_metadata_is_strict_and_cannot_supply_its_own_database_operation_id() {
    let fixture = include_bytes!("fixtures/revision.json");
    let original = parse_json(fixture).unwrap();
    let mut with_allocated_id = original.clone();
    with_allocated_id["value"]["operation_row_id"] = serde_json::json!("9");
    assert!(decode_revision(1, &serde_json::to_vec(&with_allocated_id).unwrap()).is_err());

    for field in [
        "configuration_revision",
        "project_state_version",
        "previous_project_operation",
    ] {
        for invalid in [
            serde_json::json!(1),
            serde_json::json!("01"),
            serde_json::json!("18446744073709551616"),
            serde_json::Value::Null,
        ] {
            let mut corrupted = original.clone();
            corrupted["value"][field] = invalid;
            assert!(
                decode_revision(1, &serde_json::to_vec(&corrupted).unwrap()).is_err(),
                "accepted corrupt {field}"
            );
        }
    }
}
