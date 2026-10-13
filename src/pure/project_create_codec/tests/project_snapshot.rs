use project_create_codec::*;
use serde_json::{json, Value};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/project-snapshot-v1.json")).unwrap()
}

#[test]
fn frozen_project_mapping_matches_saved_results() {
    let expected = fixture();
    let p = decode_project_snapshot(1, &serde_json::to_vec(&expected).unwrap()).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&encode_project_snapshot(&p).unwrap()).unwrap(),
        expected
    );
    let saved: Value = serde_json::from_str(include_str!(
        "../../../../experiments/SP-003/fixtures/result-v1.json"
    ))
    .unwrap();
    let r = decode_result(1, &serde_json::to_vec(&saved["result"]).unwrap()).unwrap();
    assert_eq!(p, r.project);
}

#[test]
fn historical_project_preserves_exact_values_and_all_unsigned_extremes() {
    let mut p =
        decode_project_snapshot(1, include_bytes!("fixtures/project-snapshot-v1.json")).unwrap();
    p.id.0 = " old\0id\n ".into();
    p.name = "".into();
    p.prefix = "unrestricted historical prefix".into();
    p.archived = true;
    p.ever_estimated = true;
    p.estimate_unit = task_contract::project::EstimateUnit::Points;
    for n in [
        0,
        1,
        (1 << 53) - 1,
        1 << 53,
        (1 << 53) + 1,
        1 << 63,
        u64::MAX,
    ] {
        p.configuration_revision = n;
        p.next_sequence = n;
        assert_eq!(
            decode_project_snapshot(1, &encode_project_snapshot(&p).unwrap()),
            Ok(p.clone())
        );
    }
}

#[test]
fn project_envelope_shape_versions_and_decimals_fail_closed() {
    let bytes = include_bytes!("fixtures/project-snapshot-v1.json");
    assert_eq!(
        decode_project_snapshot(2, bytes),
        Err(Error::UnsupportedVersion(2))
    );
    for path in ["codec_version", "kind", "value"] {
        let mut bad = fixture();
        bad.as_object_mut().unwrap().remove(path);
        assert!(decode_project_snapshot(1, &serde_json::to_vec(&bad).unwrap()).is_err());
    }
    for key in fixture()["value"].as_object().unwrap().keys() {
        let mut bad = fixture();
        bad["value"].as_object_mut().unwrap().remove(key);
        assert!(decode_project_snapshot(1, &serde_json::to_vec(&bad).unwrap()).is_err());
    }
    for (key, value) in [
        ("codec_version", json!("1")),
        ("codec_version", json!(1.0)),
        ("codec_version", json!(2)),
        ("kind", json!("configuration_snapshot")),
        ("unknown", json!(true)),
        ("value", json!([])),
    ] {
        let mut bad = fixture();
        bad[key] = value;
        assert!(decode_project_snapshot(1, &serde_json::to_vec(&bad).unwrap()).is_err());
    }
    for (key, value) in [
        ("unknown", json!(true)),
        ("archived", json!("false")),
        ("ever_estimated", json!(0)),
        ("estimate_unit", json!("days")),
        ("name", json!(null)),
        ("id", json!(1)),
        ("prefix", json!(false)),
    ] {
        let mut bad = fixture();
        bad["value"][key] = value;
        assert!(decode_project_snapshot(1, &serde_json::to_vec(&bad).unwrap()).is_err());
    }
    for key in ["configuration_revision", "next_sequence"] {
        for value in [
            json!(0),
            json!(1.5),
            json!(""),
            json!("01"),
            json!("+1"),
            json!("-1"),
            json!("1.0"),
            json!("1e0"),
            json!("18446744073709551616"),
        ] {
            let mut bad = fixture();
            bad["value"][key] = value;
            assert!(decode_project_snapshot(1, &serde_json::to_vec(&bad).unwrap()).is_err());
        }
    }
    for bytes in [b"null".as_slice(), b"{} trailing", b"{"] {
        assert!(decode_project_snapshot(1, bytes).is_err());
    }
}

#[test]
fn project_duplicates_are_rejected_before_their_next_value() {
    for bytes in [
        br#"{"codec_version":1,"codec_version": invalid}"#.as_slice(),
        br#"{"value":{"id":"first","\u0069d": invalid}}"#,
    ] {
        assert_eq!(
            decode_project_snapshot(1, bytes),
            Err(Error::Malformed("duplicate key"))
        );
    }
}

#[test]
fn project_preflight_and_escaped_record_limits_are_enforced() {
    let mut p =
        decode_project_snapshot(1, include_bytes!("fixtures/project-snapshot-v1.json")).unwrap();
    for field in [0, 1, 2] {
        let mut p = p.clone();
        let text = match field {
            0 => &mut p.id.0,
            1 => &mut p.name,
            _ => &mut p.prefix,
        };
        *text = "é".repeat(MAX_STRING_BYTES / 2);
        assert!(encode_project_snapshot(&p).is_ok());
        match field {
            0 => p.id.0.push('x'),
            1 => p.name.push('x'),
            _ => p.prefix.push('x'),
        }
        assert_eq!(encode_project_snapshot(&p), Err(Error::LimitExceeded));
    }
    p.id.0 = "\x01".repeat(MAX_STRING_BYTES);
    p.name = p.id.0.clone();
    p.prefix = p.id.0.clone();
    assert_eq!(encode_project_snapshot(&p), Err(Error::LimitExceeded));
    assert_eq!(
        decode_project_snapshot(1, &vec![b' '; MAX_RECORD_BYTES + 1]),
        Err(Error::LimitExceeded)
    );
}
