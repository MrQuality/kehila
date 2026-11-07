use project_create_codec::{
    composition, MAX_COLLECTION_ENTRIES, MAX_RECORD_BYTES, MAX_STRING_BYTES,
};
use serde_json::json;
use task_contract::{field::*, item_mutation::CreateCommand, work_item::*};
use work_item_codec::*;

fn text_request(total: usize) -> Request {
    let values = (0..16)
        .map(|i| {
            let length = total / 16 + usize::from(i < total % 16);
            FieldEntry {
                id: FieldId(format!("f{i}")),
                value: FieldValue::Text("a".repeat(length)),
            }
        })
        .collect();
    Request::Create {
        project_id: ProjectId("p".into()),
        command: CreateCommand {
            operation_id: "op".into(),
            expected_configuration_revision: 1,
            item_id: WorkItemId("i".into()),
            type_id: WorkItemTypeId("task".into()),
            workflow_id: None,
            estimate: None,
            values,
        },
    }
}

#[test]
fn exact_record_limit_and_jsonb_spacing_are_distinct_admission_boundaries() {
    let (mut low, mut high) = (0, MAX_RECORD_BYTES);
    while low < high {
        let mid = (low + high).div_ceil(2);
        if encode_request(&text_request(mid), 1).is_ok() {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    let request = text_request(low);
    let bytes = encode_request(&request, 1).unwrap();
    assert_eq!(bytes.len(), MAX_RECORD_BYTES);
    assert_eq!(decode_request(1, &bytes).unwrap(), request);
    assert!(encode_request(&text_request(low + 1), 1).is_err());
    assert!(
        admit_request(&request).is_err(),
        "compact bytes alone do not admit JSONB spacing"
    );
    assert!(admit_request(&text_request(low - 1024)).is_ok());
}

#[test]
fn strings_collections_escaped_growth_and_native_nul_are_checked() {
    let mut request = text_request(0);
    let Request::Create { command, .. } = &mut request else {
        unreachable!()
    };
    command.values = vec![FieldEntry {
        id: FieldId("f".into()),
        value: FieldValue::Text("a".repeat(MAX_STRING_BYTES)),
    }];
    assert!(encode_request(&request, 1).is_ok());
    let Request::Create { command, .. } = &mut request else {
        unreachable!()
    };
    command.values[0].value = FieldValue::Text("a".repeat(MAX_STRING_BYTES + 1));
    assert!(encode_request(&request, 1).is_err());
    let Request::Create { command, .. } = &mut request else {
        unreachable!()
    };
    command.values = vec![
        FieldEntry {
            id: FieldId("f".into()),
            value: FieldValue::Text("\n".repeat(MAX_STRING_BYTES))
        };
        16
    ];
    assert!(
        encode_request(&request, 1).is_err(),
        "escaped output must be bounded"
    );
    let Request::Create { command, .. } = &mut request else {
        unreachable!()
    };
    command.values = vec![FieldEntry {
        id: FieldId("f".into()),
        value: FieldValue::Text("\0".into()),
    }];
    let bytes = encode_request(&request, 1).unwrap();
    assert_eq!(decode_request(1, &bytes).unwrap(), request);
    assert!(admit_request(&request).is_err());
    let Request::Create { command, .. } = &mut request else {
        unreachable!()
    };
    command.values = vec![
        FieldEntry {
            id: FieldId("f".into()),
            value: FieldValue::Boolean(false)
        };
        MAX_COLLECTION_ENTRIES + 1
    ];
    assert!(encode_request(&request, 1).is_err());
}

#[test]
fn raw_edge_and_composed_record_obey_depth_nodes_keys_and_scalar_vocabulary() {
    let deep = format!("{}null{}", "[".repeat(17), "]".repeat(17));
    assert!(decode_request(1, deep.as_bytes()).is_err());
    assert!(composition::encode_record(&json!({"f":0.5})).is_err());
    let mut object = serde_json::Map::new();
    object.insert("x".repeat(MAX_STRING_BYTES + 1), json!(false));
    assert!(composition::encode_record(&serde_json::Value::Object(object)).is_err());
    let nodes = json!(vec![vec![false; 1024]; 16]);
    assert!(composition::encode_record(&nodes).is_err());
    for bytes in [b"null".as_slice(), b"[]", b"true", b"1", b"{}"] {
        assert!(decode_request(1, bytes).is_err());
        assert!(decode_result(1, bytes).is_err());
        assert!(decode_revision(1, bytes).is_err());
    }
}

#[test]
fn persisted_request_verification_requires_canonical_bytes_and_matching_digest() {
    let request = text_request(0);
    let bytes = encode_request(&request, 1).unwrap();
    let digest = fingerprint(&bytes);
    assert_eq!(verify_request(1, &bytes, &digest).unwrap(), request);
    assert!(verify_request(1, &bytes, &[0; 32]).is_err());
    let padded = [b" ".as_slice(), &bytes].concat();
    assert_eq!(decode_request(1, &padded).unwrap(), request);
    assert!(verify_request(1, &padded, &fingerprint(&padded)).is_err());
}
