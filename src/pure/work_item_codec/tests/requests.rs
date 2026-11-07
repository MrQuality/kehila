use task_contract::{
    field::{FieldEdit, FieldId, FieldValue},
    item_mutation::{CreateCommand, EditCommand, EstimateEdit, FieldUpdate},
    work_item::{ProjectId, WorkItemId, WorkItemTypeId},
};
use work_item_codec::*;

fn create() -> Request {
    Request::Create {
        project_id: ProjectId("p".into()),
        command: CreateCommand {
            operation_id: "op".into(),
            expected_configuration_revision: 1,
            item_id: WorkItemId("i".into()),
            type_id: WorkItemTypeId("task".into()),
            workflow_id: None,
            estimate: None,
            values: vec![],
        },
    }
}
fn edit() -> Request {
    Request::Edit {
        project_id: ProjectId("p".into()),
        item_id: WorkItemId("i".into()),
        command: EditCommand {
            operation_id: "op".into(),
            expected_item_version: 1,
            expected_configuration_revision: 1,
            estimate: EstimateEdit::Keep,
            fields: vec![],
        },
    }
}

#[test]
fn create_request_has_independent_frozen_bytes() {
    let expected = br#"{"codec_version":1,"kind":"item_create_request","value":{"estimate":null,"expected_configuration_revision":"1","item_id":"i","operation_id":"op","project_id":"p","type_id":"task","values":[],"workflow_id":null}}"#;
    assert_eq!(encode_request(&create(), 1).unwrap(), expected);
    assert_eq!(decode_request(1, expected).unwrap(), create());
    assert_eq!(
        fingerprint(expected)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        "91aa337ae3f13cea726a24d77bc2e1fa036ee42897f33cbde71ae21cdce25880"
    );
}

#[test]
fn edit_request_has_independent_frozen_bytes() {
    let expected = br#"{"codec_version":1,"kind":"item_edit_request","value":{"estimate":{"action":"keep"},"expected_configuration_revision":"1","expected_item_version":"1","fields":[],"item_id":"i","operation_id":"op","project_id":"p"}}"#;
    assert_eq!(encode_request(&edit(), 1).unwrap(), expected);
    assert_eq!(decode_request(1, expected).unwrap(), edit());
    assert_eq!(
        fingerprint(expected)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        "b090396c8e6d30797f41d1c51547a95f7ed4e8225619e8abaa1051f034a3bf28"
    );
}

#[test]
fn equivalent_json_spellings_canonicalize_but_ordered_actions_do_not() {
    let alternate = br#"{"kind":"item_create_request","value":{"workflow_id":null,"values":[],"type_id":"task","project_id":"p","operation_id":"op","item_id":"\u0069","expected_configuration_revision":"1","estimate":null},"codec_version":1}"#;
    let decoded = decode_request(1, alternate).unwrap();
    assert_eq!(
        encode_request(&decoded, 1).unwrap(),
        encode_request(&create(), 1).unwrap()
    );
    let mut first = edit();
    let Request::Edit { command, .. } = &mut first else {
        unreachable!()
    };
    command.fields = vec![
        FieldUpdate {
            id: FieldId("a".into()),
            edit: FieldEdit::Keep,
        },
        FieldUpdate {
            id: FieldId("b".into()),
            edit: FieldEdit::Set(FieldValue::Text("λ".into())),
        },
    ];
    let before = encode_request(&first, 1).unwrap();
    let Request::Edit { command, .. } = &mut first else {
        unreachable!()
    };
    command.fields.reverse();
    assert_ne!(
        fingerprint(&before),
        fingerprint(&encode_request(&first, 1).unwrap())
    );
}

#[test]
fn explicit_keep_clear_and_omission_have_distinct_identity() {
    let mut request = edit();
    let omitted = encode_request(&request, 1).unwrap();
    let Request::Edit { command, .. } = &mut request else {
        unreachable!()
    };
    command.fields.push(FieldUpdate {
        id: FieldId("f".into()),
        edit: FieldEdit::Keep,
    });
    let keep = encode_request(&request, 1).unwrap();
    let Request::Edit { command, .. } = &mut request else {
        unreachable!()
    };
    command.fields[0].edit = FieldEdit::Clear;
    let clear = encode_request(&request, 1).unwrap();
    assert_ne!(keep, omitted);
    assert_ne!(keep, clear);
    assert_ne!(omitted, clear);
}

#[test]
fn historical_requests_do_not_apply_current_business_validation() {
    let mut historical = create();
    let Request::Create { command, .. } = &mut historical else {
        unreachable!()
    };
    command.operation_id.clear();
    command.expected_configuration_revision = 0;
    command.type_id.0 = "no longer configured".into();
    let bytes = encode_request(&historical, 1).unwrap();
    assert_eq!(decode_request(1, &bytes).unwrap(), historical);
}

#[test]
fn raw_duplicate_and_noncanonical_version_fields_are_rejected() {
    let canonical = encode_request(&edit(), 1).unwrap();
    let text = String::from_utf8(canonical).unwrap();
    let duplicate = text.replacen(
        "\"codec_version\":1",
        "\"codec_version\":1,\"codec_\\u0076ersion\":1",
        1,
    );
    assert!(decode_request(1, duplicate.as_bytes()).is_err());
    for bad in ["01", "+1", "-0", "1.0", "18446744073709551616"] {
        let corrupt = text.replace(
            "\"expected_item_version\":\"1\"",
            &format!("\"expected_item_version\":\"{bad}\""),
        );
        assert!(decode_request(1, corrupt.as_bytes()).is_err());
    }
    assert!(decode_request(2, text.as_bytes()).is_err());
}

#[test]
fn scope_versions_workflow_tokens_and_estimate_actions_are_exact_identity() {
    use std::collections::HashSet;
    use task_contract::{project::Estimate, work_item::WorkflowId};
    let original = edit();
    let mut variants = vec![original.clone(), create()];
    for change in 0..8 {
        let mut request = original.clone();
        let Request::Edit {
            project_id,
            item_id,
            command,
        } = &mut request
        else {
            unreachable!()
        };
        match change {
            0 => project_id.0 = "other".into(),
            1 => item_id.0 = "other".into(),
            2 => command.operation_id = "OP".into(),
            3 => command.expected_item_version = u64::MAX,
            4 => command.expected_configuration_revision = u64::MAX,
            5 => command.estimate = EstimateEdit::Clear,
            6 => command.estimate = EstimateEdit::Set(Estimate::try_from_thousandths(0).unwrap()),
            _ => {
                command.estimate =
                    EstimateEdit::Set(Estimate::try_from_thousandths(999_999_999).unwrap())
            }
        }
        variants.push(request);
    }
    let mut explicit_workflow = create();
    let Request::Create { command, .. } = &mut explicit_workflow else {
        unreachable!()
    };
    command.workflow_id = Some(WorkflowId("default".into()));
    variants.push(explicit_workflow);
    let mut zero_estimate = create();
    let Request::Create { command, .. } = &mut zero_estimate else {
        unreachable!()
    };
    command.estimate = Some(Estimate::try_from_thousandths(0).unwrap());
    variants.push(zero_estimate);
    let digests = variants
        .iter()
        .map(|request| {
            let bytes = encode_request(request, 1).unwrap();
            assert_eq!(decode_request(1, &bytes).unwrap(), *request);
            fingerprint(&bytes)
        })
        .collect::<HashSet<_>>();
    assert_eq!(digests.len(), variants.len());
}
