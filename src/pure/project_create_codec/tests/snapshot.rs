use project_create_codec::*;
use serde_json::{json, Value};

fn frozen() -> Value {
    serde_json::from_str(include_str!(
        "../../../../experiments/SP-003/fixtures/result-v1.json"
    ))
    .unwrap()
}

#[test]
fn frozen_configuration_snapshot_round_trips_with_order_and_precision() {
    let expected = frozen()["snapshot"].clone();
    let mut c = decode_snapshot(1, &serde_json::to_vec(&expected).unwrap()).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&encode_snapshot(&c).unwrap()).unwrap(),
        expected
    );
    c.statuses.reverse();
    c.fields.reverse();
    c.revision = u64::MAX;
    assert_eq!(
        decode_snapshot(1, &encode_snapshot(&c).unwrap()).unwrap(),
        c
    );
}

#[test]
fn configuration_shape_kind_version_and_limits_are_enforced() {
    let fixture = frozen();
    let bytes = serde_json::to_vec(&fixture["snapshot"]).unwrap();
    assert_eq!(
        decode_snapshot(2, &bytes),
        Err(Error::UnsupportedVersion(2))
    );
    for value in [json!(0), json!("01"), json!("18446744073709551616")] {
        let mut bad = fixture["snapshot"].clone();
        bad["value"]["revision"] = value;
        assert!(decode_snapshot(1, &serde_json::to_vec(&bad).unwrap()).is_err());
    }
    let mut bad = fixture["snapshot"].clone();
    bad["kind"] = json!("project_create_result");
    assert!(decode_snapshot(1, &serde_json::to_vec(&bad).unwrap()).is_err());
    let mut bad = fixture["snapshot"].clone();
    bad["codec_version"] = json!(2);
    assert!(decode_snapshot(1, &serde_json::to_vec(&bad).unwrap()).is_err());
    let mut bad = fixture["snapshot"].clone();
    bad["value"]["fields"][0]["unknown"] = json!(true);
    assert!(decode_snapshot(1, &serde_json::to_vec(&bad).unwrap()).is_err());
    let mut c = decode_snapshot(1, &bytes).unwrap();
    c.statuses = vec![c.statuses[0].clone(); MAX_COLLECTION_ENTRIES + 1];
    assert_eq!(encode_snapshot(&c), Err(Error::LimitExceeded));
}

#[test]
fn historical_snapshot_ids_are_not_new_write_admission() {
    let mut stored = frozen()["snapshot"].clone();
    stored["value"]["project_id"] = json!("");
    stored["value"]["statuses"][0]["id"] = json!("old\u{0}id");
    let c = decode_snapshot(1, &serde_json::to_vec(&stored).unwrap()).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&encode_snapshot(&c).unwrap()).unwrap(),
        stored
    );
}
