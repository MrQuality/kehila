use super::*;
use serde_json::json;
fn edit() -> Value {
    json!({"project_id":"p","item_id":"i","operation_id":"a".repeat(32),
    "expected_item_version":"18446744073709551615","expected_configuration_revision":"1",
    "estimate":{"action":"keep"},"fields":[{"id":"title","action":"set","value":"  exact\r\n\u{e9}  "}]})
}
fn encoded(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}
#[test]
fn execution_and_reconciliation_keep_the_identical_edit_identity() {
    let bytes = encoded(&edit());
    let execute = decode(Route::Edit, &bytes).unwrap();
    let reconcile = decode(Route::ReconcileEdit, &bytes).unwrap();
    assert_eq!(execute.action, Action::Execute);
    assert_eq!(reconcile.action, Action::Reconcile);
    assert_eq!(execute.request, reconcile.request);
    let Request::Edit { command, .. } = execute.request else {
        panic!()
    };
    assert_eq!(command.expected_item_version, u64::MAX);
    assert_eq!(
        command.fields[0].edit,
        FieldEdit::Set(FieldValue::Text("  exact\r\n\u{e9}  ".into()))
    );
}
#[test]
fn creation_locator_accepts_no_request_authority_or_new_target() {
    let mut value = json!({"project_id":"p","intent_handle":"b".repeat(32)});
    assert!(decode(Route::ExecuteCreate, &encoded(&value)).is_ok());
    assert_eq!(
        decode(Route::ExecuteCreate, &encoded(&value))
            .unwrap()
            .request,
        decode(Route::ReconcileCreate, &encoded(&value))
            .unwrap()
            .request
    );
    for field in [
        "item_id",
        "operation_id",
        "actor_id",
        "installation_id",
        "result",
    ] {
        value[field] = json!("forged");
        assert!(decode(Route::ExecuteCreate, &encoded(&value)).is_err());
        value.as_object_mut().unwrap().remove(field);
    }
}
#[test]
fn sparse_actions_are_closed_and_only_set_accepts_text() {
    for estimate in [
        json!({"action":"keep"}),
        json!({"action":"clear"}),
        json!({"action":"set","value":"0"}),
        json!({"action":"set","value":"999999999"}),
    ] {
        let mut value = edit();
        value["estimate"] = estimate;
        assert!(decode(Route::Edit, &encoded(&value)).is_ok());
    }
    for field in [
        json!({"id":"title","action":"keep"}),
        json!({"id":"title","action":"clear"}),
        json!({"id":"title","action":"set","value":""}),
    ] {
        let mut value = edit();
        value["fields"] = json!([field]);
        assert!(decode(Route::Edit, &encoded(&value)).is_ok());
    }
    for field in [
        json!({"id":"title","action":"keep","value":"x"}),
        json!({"id":"title","action":"set","value":1}),
        json!({"id":"title","action":"set","value":{"kind":"text","value":"x"}}),
        json!({"id":"title","action":"set"}),
    ] {
        let mut value = edit();
        value["fields"] = json!([field]);
        assert!(decode(Route::Edit, &encoded(&value)).is_err());
    }
}
#[test]
fn identity_and_expected_versions_are_exact_and_have_no_defaults() {
    for field in ["expected_item_version", "expected_configuration_revision"] {
        for bad in [
            json!(1),
            json!("0"),
            json!("01"),
            json!("+1"),
            json!("18446744073709551616"),
            Value::Null,
        ] {
            let mut value = edit();
            value[field] = bad;
            assert!(decode(Route::Edit, &encoded(&value)).is_err());
        }
    }
    for field in [
        "item_id",
        "project_id",
        "operation_id",
        "estimate",
        "fields",
    ] {
        let mut value = edit();
        value.as_object_mut().unwrap().remove(field);
        assert!(decode(Route::Edit, &encoded(&value)).is_err());
    }
    for bad in ["a".repeat(31), "A".repeat(32), "g".repeat(32)] {
        let mut value = edit();
        value["operation_id"] = json!(bad);
        assert!(decode(Route::Edit, &encoded(&value)).is_err());
    }
}
#[test]
fn duplicate_fields_unknown_keys_nul_and_original_byte_overflow_are_rejected() {
    let mut value = edit();
    let field = value["fields"][0].clone();
    value["fields"] = json!([field.clone(), field]);
    assert!(decode(Route::Edit, &encoded(&value)).is_err());
    for field in ["actor_id", "target_kind", "request_codec_version"] {
        let mut value = edit();
        value[field] = json!("forged");
        assert!(decode(Route::Edit, &encoded(&value)).is_err());
    }
    let mut value = edit();
    value["fields"][0]["value"] = json!("bad\0text");
    assert!(decode(Route::Edit, &encoded(&value)).is_err());
    assert!(decode(Route::Edit, &vec![b' '; Route::Edit.body_limit() + 1]).is_err());
    assert!(decode(Route::ExecuteCreate,br#"{"project_id":"p","project_id":"q","intent_handle":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#).is_err());
}
