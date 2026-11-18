use selection_codec::{decode_event, decode_result, encode_event, encode_result, Event};
use task_contract::{
    current_work::{CurrentWork, SelectionResult, UserId},
    relationship::EndpointIdentity,
    work_item::{ProjectId, WorkItemId},
};

fn state(version: u64, item: Option<&str>) -> CurrentWork {
    CurrentWork {
        user_id: UserId("owner".into()),
        selected: item.map(|item| EndpointIdentity {
            project_id: ProjectId("project".into()),
            item_id: WorkItemId(item.into()),
        }),
        version,
    }
}

#[test]
fn result_is_original_state_not_recomputed_from_a_current_selection() {
    for original in [state(0, None), state(1, Some("a")), state(u64::MAX, None)] {
        let result = SelectionResult { next: original };
        let bytes = encode_result(&result, 1).unwrap();
        let later = state(4, Some("b"));
        assert_ne!(result.next, later);
        assert_eq!(decode_result(1, &bytes), Ok(result));
    }
}

#[test]
fn result_envelope_freezes_field_names_and_decimal_versions() {
    let result = SelectionResult {
        next: state(u64::MAX, Some("a")),
    };
    let value: serde_json::Value =
        serde_json::from_slice(&encode_result(&result, 1).unwrap()).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "codec_version":1,"kind":"selection_result","value":{
                "user_id":"owner","version":"18446744073709551615",
                "requested":{"project_id":"project","item_id":"a"}
            }
        })
    );
}

#[test]
fn successful_noop_has_a_new_event_ordinal_but_an_unchanged_selection_version() {
    let event = Event {
        ordinal: 8,
        operation_id: "0123456789abcdef0123456789abcdef".into(),
        before: state(3, Some("a")),
        result: SelectionResult {
            next: state(3, Some("a")),
        },
    };
    let bytes = encode_event(&event, 1).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "codec_version":1,"kind":"selection_event","value":{
                "ordinal":"8","operation_id":"0123456789abcdef0123456789abcdef",
                "before":{"user_id":"owner","version":"3",
                    "requested":{"project_id":"project","item_id":"a"}},
                "result":{"codec_version":1,"kind":"selection_result","value":{
                    "user_id":"owner","version":"3",
                    "requested":{"project_id":"project","item_id":"a"}
                }}
            }
        })
    );
    assert_eq!(decode_event(1, &bytes), Ok(event));
}

#[test]
fn event_envelope_rejects_duplicate_extra_wrong_version_and_missing_fields() {
    let event = Event {
        ordinal: 1,
        operation_id: "0123456789abcdef0123456789abcdef".into(),
        before: state(0, None),
        result: SelectionResult {
            next: state(0, None),
        },
    };
    let baseline: serde_json::Value =
        serde_json::from_slice(&encode_event(&event, 1).unwrap()).unwrap();
    for field in ["ordinal", "operation_id", "before", "result"] {
        let mut value = baseline.clone();
        value["value"].as_object_mut().unwrap().remove(field);
        assert!(decode_event(1, &serde_json::to_vec(&value).unwrap()).is_err());
    }
    for (path, replacement) in [
        ("ordinal", serde_json::json!(1)),
        ("ordinal", serde_json::json!("01")),
        ("before", serde_json::Value::Null),
        ("result", serde_json::Value::Null),
        ("unexpected", serde_json::Value::Null),
    ] {
        let mut value = baseline.clone();
        value["value"][path] = replacement;
        assert!(decode_event(1, &serde_json::to_vec(&value).unwrap()).is_err());
    }
    let mut value = baseline;
    value["codec_version"] = serde_json::json!(2);
    assert!(decode_event(1, &serde_json::to_vec(&value).unwrap()).is_err());
    let duplicate = br#"{"codec_version":1,"kind":"selection_event","value":{"ordinal":"1","ordinal":"2","operation_id":"0123456789abcdef0123456789abcdef","before":{"user_id":"owner","version":"0","requested":null},"result":{"codec_version":1,"kind":"selection_result","value":{"user_id":"owner","version":"0","requested":null}}}}"#;
    assert!(decode_event(1, duplicate).is_err());
}

#[test]
fn initial_empty_clear_and_actual_change_have_distinct_valid_events() {
    for (before, after) in [
        (state(0, None), state(0, None)),
        (state(0, None), state(1, Some("a"))),
        (state(1, Some("a")), state(2, None)),
    ] {
        let event = Event {
            ordinal: 1,
            operation_id: "0123456789abcdef0123456789abcdef".into(),
            before,
            result: SelectionResult { next: after },
        };
        assert_eq!(
            decode_event(1, &encode_event(&event, 1).unwrap()),
            Ok(event)
        );
    }
}

#[test]
fn malformed_stored_shapes_never_coerce_versions_or_ignore_extra_keys() {
    let baseline: serde_json::Value = serde_json::from_slice(
        &encode_result(
            &SelectionResult {
                next: state(1, Some("a")),
            },
            1,
        )
        .unwrap(),
    )
    .unwrap();
    for replacement in [
        serde_json::json!(1),
        serde_json::json!("01"),
        serde_json::json!("+1"),
        serde_json::json!("18446744073709551616"),
    ] {
        let mut value = baseline.clone();
        value["value"]["version"] = replacement;
        assert!(decode_result(1, &serde_json::to_vec(&value).unwrap()).is_err());
    }
    let mut value = baseline.clone();
    value["value"]["unexpected"] = serde_json::Value::Null;
    assert!(decode_result(1, &serde_json::to_vec(&value).unwrap()).is_err());
    let mut value = baseline;
    value["codec_version"] = serde_json::json!(2);
    assert!(decode_result(1, &serde_json::to_vec(&value).unwrap()).is_err());
    let duplicate = br#"{"codec_version":1,"kind":"selection_result","value":{"user_id":"owner","version":"1","version":"2","requested":null}}"#;
    assert!(decode_result(1, duplicate).is_err());
}

#[test]
fn historical_event_reconstruction_does_not_reapply_current_domain_acceptance() {
    // Event transition consistency is verified against authoritative SQL facts.
    // The historical codec preserves an old representation independently.
    let event = Event {
        ordinal: u64::MAX,
        operation_id: "0123456789abcdef0123456789abcdef".into(),
        before: state(7, Some("a")),
        result: SelectionResult {
            next: state(9, Some("b")),
        },
    };
    assert_eq!(
        decode_event(1, &encode_event(&event, 1).unwrap()),
        Ok(event)
    );
    assert!(decode_event(2, b"{}").is_err());
    assert!(decode_result(1, &vec![b' '; 16385]).is_err());
}
