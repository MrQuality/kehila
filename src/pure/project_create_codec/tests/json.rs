use project_create_codec::*;
use serde_json::json;

#[test]
fn duplicates_are_detected_before_their_values_are_parsed() {
    for bytes in [
        br#"{"id":1,"id":2}"#.as_slice(),
        br#"{"id":1,"\u0069d":2}"#,
        br#"{"nested":{"id":1,"id":INVALID}}"#,
    ] {
        assert_eq!(parse_json(bytes), Err(Error::Malformed("duplicate key")));
    }
    assert_eq!(
        parse_json(br#"{"a":{"id":1},"b":{"id":2}}"#).unwrap(),
        json!({"a":{"id":1},"b":{"id":2}})
    );
    assert!(parse_json(b"{} trailing").is_err());
    assert!(parse_json(b"{\"x\":").is_err());
}

#[test]
fn every_resource_limit_has_an_acceptance_boundary() {
    let bytes = serde_json::to_vec(&"x".repeat(MAX_STRING_BYTES)).unwrap();
    assert!(parse_json(&bytes).is_ok());
    let bytes = serde_json::to_vec(&"x".repeat(MAX_STRING_BYTES + 1)).unwrap();
    assert_eq!(parse_json(&bytes), Err(Error::LimitExceeded));
    let bytes = serde_json::to_vec(&vec![0; MAX_COLLECTION_ENTRIES]).unwrap();
    assert!(parse_json(&bytes).is_ok());
    let bytes = serde_json::to_vec(&vec![0; MAX_COLLECTION_ENTRIES + 1]).unwrap();
    assert_eq!(parse_json(&bytes), Err(Error::LimitExceeded));
    let nested = |depth| format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
    assert!(parse_json(nested(MAX_DEPTH).as_bytes()).is_ok());
    assert_eq!(
        parse_json(nested(MAX_DEPTH + 1).as_bytes()),
        Err(Error::LimitExceeded)
    );
    let mut padded = b"{}".to_vec();
    padded.resize(MAX_RECORD_BYTES, b' ');
    assert!(parse_json(&padded).is_ok());
    padded.push(b' ');
    assert_eq!(parse_json(&padded), Err(Error::LimitExceeded));
    let bytes = serde_json::to_vec(&vec![vec![0; MAX_COLLECTION_ENTRIES]; 20]).unwrap();
    assert_eq!(parse_json(&bytes), Err(Error::LimitExceeded));
}

#[test]
fn aggregate_node_and_object_entry_bounds_are_exact() {
    let mut groups = vec![vec![0; MAX_COLLECTION_ENTRIES]; 15];
    groups.push(vec![0; MAX_NODES - 1 - 16 - 15 * MAX_COLLECTION_ENTRIES]);
    assert!(parse_json(&serde_json::to_vec(&groups).unwrap()).is_ok());
    groups.last_mut().unwrap().push(0);
    assert_eq!(
        parse_json(&serde_json::to_vec(&groups).unwrap()),
        Err(Error::LimitExceeded)
    );
    let mut object = serde_json::Map::new();
    for i in 0..MAX_COLLECTION_ENTRIES {
        object.insert(i.to_string(), json!(0));
    }
    assert!(parse_json(&serde_json::to_vec(&object).unwrap()).is_ok());
    object.insert("extra".into(), json!(0));
    assert_eq!(
        parse_json(&serde_json::to_vec(&object).unwrap()),
        Err(Error::LimitExceeded)
    );
    assert!(parse_json(&[b'"', 255, b'"']).is_err());
}

#[test]
fn object_key_limits_count_decoded_utf8_bytes() {
    for (encoded, decoded) in [
        ("a".repeat(MAX_STRING_BYTES), "a".repeat(MAX_STRING_BYTES)),
        (
            "\\u0061".repeat(MAX_STRING_BYTES),
            "a".repeat(MAX_STRING_BYTES),
        ),
        (
            "\\u00e9".repeat(MAX_STRING_BYTES / 2),
            "\u{e9}".repeat(MAX_STRING_BYTES / 2),
        ),
    ] {
        let bytes = format!("{{\"{encoded}\":null}}");
        let value = parse_json(bytes.as_bytes()).unwrap();
        assert_eq!(value.as_object().unwrap().keys().next().unwrap(), &decoded);
        let over_limit = format!("{{\"{encoded}x\":null}}");
        assert_eq!(parse_json(over_limit.as_bytes()), Err(Error::LimitExceeded));
    }
}

#[test]
fn oversized_keys_are_rejected_before_parsing_their_values() {
    for encoded in [
        "a".repeat(MAX_STRING_BYTES + 1),
        "\\u0061".repeat(MAX_STRING_BYTES + 1),
    ] {
        let bytes = format!("{{\"{encoded}\":INVALID}}");
        assert_eq!(parse_json(bytes.as_bytes()), Err(Error::LimitExceeded));
    }
}

#[test]
fn maximum_length_keys_preserve_escaped_duplicate_detection() {
    let literal = "a".repeat(MAX_STRING_BYTES);
    let escaped = "\\u0061".repeat(MAX_STRING_BYTES);
    let bytes = format!("{{\"{literal}\":null,\"{escaped}\":INVALID}}");
    assert_eq!(
        parse_json(bytes.as_bytes()),
        Err(Error::Malformed("duplicate key"))
    );
}
