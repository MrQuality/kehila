use task_contract::{current_work::UserId, field::*, item_mutation::*, project::*, work_item::*};
use work_item_codec::*;

fn result(unit: EstimateUnit) -> SavedResult {
    SavedResult::Create(CreateResult {
        project: Project {
            id: ProjectId("p".into()),
            name: "é project".into(),
            prefix: "P1".into(),
            configuration_revision: u64::MAX,
            next_sequence: u64::MAX,
            archived: false,
            estimate_unit: unit,
            ever_estimated: true,
        },
        readable_id: ReadableId {
            project_id: ProjectId("p".into()),
            item_id: WorkItemId("i".into()),
            prefix: "OLD".into(),
            sequence: u64::MAX,
        },
        item: WorkItem {
            id: WorkItemId("i".into()),
            project_id: ProjectId("p".into()),
            type_id: WorkItemTypeId("task".into()),
            workflow_id: WorkflowId("wf".into()),
            status_id: StatusId("done".into()),
            version: u64::MAX,
            archived: true,
        },
        data: ItemData {
            estimate: Some(Estimate::try_from_thousandths(0).unwrap()),
            values: vec![
                FieldEntry {
                    id: FieldId("text".into()),
                    value: FieldValue::Text("".into()),
                },
                FieldEntry {
                    id: FieldId("number".into()),
                    value: FieldValue::Number(i64::MIN),
                },
                FieldEntry {
                    id: FieldId("boolean".into()),
                    value: FieldValue::Boolean(false),
                },
                FieldEntry {
                    id: FieldId("date".into()),
                    value: FieldValue::Date(CivilDate::new(9999, 12, 31).unwrap()),
                },
                FieldEntry {
                    id: FieldId("choice".into()),
                    value: FieldValue::SingleChoice(OptionId("archived choice".into())),
                },
            ],
        },
        phase: Phase::Done,
    })
}

#[test]
fn complete_saved_results_and_permanent_revisions_preserve_typed_history() {
    for unit in [EstimateUnit::Hours, EstimateUnit::Points] {
        let original = result(unit);
        let bytes = encode_result(&original, 1).unwrap();
        assert_eq!(decode_result(1, &bytes).unwrap(), original);
        let admitted = admit_result(&original).unwrap();
        assert_eq!(decode_result(1, admitted.as_bytes()).unwrap(), original);
        let history = Revision {
            actor_id: UserId("actor".into()),
            configuration_revision: u64::MAX,
            project_state_version: u64::MAX,
            previous_project_operation: u64::MAX - 1,
            result: original,
        };
        let bytes = encode_revision(&history, 1).unwrap();
        assert_eq!(decode_revision(1, &bytes).unwrap(), history);
        assert_eq!(
            decode_revision(1, admit_revision(&history).unwrap().as_bytes()).unwrap(),
            history
        );
    }
}

#[test]
fn clearing_last_estimate_retains_project_history_and_does_not_invent_a_value() {
    let SavedResult::Create(created) = result(EstimateUnit::Hours) else {
        unreachable!()
    };
    let mut data = created.data;
    data.estimate = None;
    let edited = SavedResult::Edit(EditResult {
        project: created.project,
        item: created.item,
        data,
        phase: created.phase,
    });
    let bytes = encode_result(&edited, 1).unwrap();
    let SavedResult::Edit(decoded) = decode_result(1, &bytes).unwrap() else {
        panic!("wrong family")
    };
    assert!(decoded.project.ever_estimated);
    assert_eq!(decoded.data.estimate, None);
    assert_eq!(SavedResult::Edit(decoded), edited);
}

#[test]
fn native_nul_rejection_does_not_rewrite_historical_decoding() {
    let mut original = result(EstimateUnit::Hours);
    let SavedResult::Create(created) = &mut original else {
        unreachable!()
    };
    created.project.name = "historical\0text".into();
    let bytes = encode_result(&original, 1).unwrap();
    assert_eq!(decode_result(1, &bytes).unwrap(), original);
    assert!(admit_result(&original).is_err());
}

#[test]
fn corrupt_numbers_dates_shape_and_versions_never_default() {
    let bytes = encode_result(&result(EstimateUnit::Hours), 1).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    for (old, new) in [
        ("-9223372036854775808", "-0"),
        ("9999-12-31", "9999-02-30"),
        ("\"estimate\":\"0\"", "\"estimate\":0"),
        ("\"phase\":\"done\"", "\"phase\":\"unknown\""),
        ("\"codec_version\":1", "\"codec_version\":2"),
        ("\"archived\":true", "\"archived\":true,\"extra\":false"),
    ] {
        assert!(text.contains(old), "fixture must reach {old}");
        assert!(
            decode_result(1, text.replacen(old, new, 1).as_bytes()).is_err(),
            "accepted {new}"
        );
    }
}
