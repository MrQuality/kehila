use project_create_codec::Error;
use project_create_codec::*;
use serde_json::{json, Value};

fn frozen() -> Value {
    serde_json::from_str(include_str!(
        "../../../../experiments/SP-003/fixtures/result-v1.json"
    ))
    .unwrap()
}

#[test]
fn frozen_result_and_configuration_reconstruct_exactly() {
    let fixture = frozen();
    let result = decode_result(1, &serde_json::to_vec(&fixture["result"]).unwrap()).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&encode_result(&result).unwrap()).unwrap(),
        fixture["result"]
    );
    let configuration =
        decode_snapshot(1, &serde_json::to_vec(&fixture["snapshot"]).unwrap()).unwrap();
    assert_eq!(configuration, result.configuration);
    assert_eq!(
        serde_json::from_slice::<Value>(&encode_snapshot(&configuration).unwrap()).unwrap(),
        fixture["snapshot"]
    );
}

#[test]
fn raw_json_rejects_duplicates_before_value_conversion() {
    let original = serde_json::to_string(&frozen()["result"]).unwrap();
    for (needle, replacement) in [
        (
            "\"codec_version\":1",
            "\"codec_version\":1,\"codec_version\":1",
        ),
        (
            "\"next_sequence\":\"1\"",
            "\"next_sequence\":\"1\",\"next_sequence\":\"2\"",
        ),
        ("\"project\":{", "\"project\":{\"\\u0069d\":\"duplicate\","),
    ] {
        assert!(original.contains(needle));
        let bad = original.replacen(needle, replacement, 1);
        assert_eq!(
            decode_result(1, bad.as_bytes()),
            Err(Error::Malformed("duplicate key"))
        );
    }
}

#[test]
fn stored_version_kind_and_decimal_corruption_are_explicit() {
    let fixture = frozen();
    let bytes = serde_json::to_vec(&fixture["result"]).unwrap();
    assert_eq!(decode_result(2, &bytes), Err(Error::UnsupportedVersion(2)));
    for value in [
        json!(0),
        json!(1.5),
        json!("01"),
        json!("+1"),
        json!("18446744073709551616"),
    ] {
        let mut bad = fixture["result"].clone();
        bad["value"]["project"]["next_sequence"] = value;
        assert!(decode_result(1, &serde_json::to_vec(&bad).unwrap()).is_err());
    }
    for version in [0, 2] {
        let mut bad = fixture["result"].clone();
        bad["codec_version"] = json!(version);
        assert!(decode_result(1, &serde_json::to_vec(&bad).unwrap()).is_err());
    }
    for bad in [b"{} trailing".as_slice(), b"{", b"null"] {
        assert!(decode_result(1, bad).is_err());
    }
    let mut bad = fixture["result"].clone();
    bad["extra"] = json!(true);
    assert!(decode_result(1, &serde_json::to_vec(&bad).unwrap()).is_err());
    assert!(decode_snapshot(1, &bytes).is_err());
}

#[test]
fn historical_values_do_not_reapply_current_domain_rules() {
    let mut stored = frozen()["result"].clone();
    stored["value"]["project"]["id"] = json!("");
    stored["value"]["project"]["name"] = json!("historical\u{0}name");
    stored["value"]["project"]["next_sequence"] = json!(u64::MAX.to_string());
    let result = decode_result(1, &serde_json::to_vec(&stored).unwrap()).unwrap();
    assert_eq!(result.project.next_sequence, u64::MAX);
    assert_eq!(
        serde_json::from_slice::<Value>(&encode_result(&result).unwrap()).unwrap(),
        stored
    );
}

#[test]
fn resource_limits_reject_before_shape_mapping() {
    assert_eq!(
        decode_result(1, &vec![b' '; MAX_RECORD_BYTES + 1]),
        Err(Error::LimitExceeded)
    );
    let too_deep = format!(
        "{}0{}",
        "[".repeat(MAX_DEPTH + 2),
        "]".repeat(MAX_DEPTH + 2)
    );
    assert_eq!(
        decode_result(1, too_deep.as_bytes()),
        Err(Error::LimitExceeded)
    );
    let too_wide = serde_json::to_vec(&vec![0; MAX_COLLECTION_ENTRIES + 1]).unwrap();
    assert_eq!(decode_result(1, &too_wide), Err(Error::LimitExceeded));
    let too_many = serde_json::to_vec(&vec![vec![0; MAX_COLLECTION_ENTRIES]; 20]).unwrap();
    assert_eq!(decode_result(1, &too_many), Err(Error::LimitExceeded));
    let too_long = serde_json::to_vec(&"x".repeat(MAX_STRING_BYTES + 1)).unwrap();
    assert_eq!(decode_result(1, &too_long), Err(Error::LimitExceeded));
}

#[test]
fn encoding_bounds_typed_inputs_and_escaped_output() {
    let result = decode_result(1, &serde_json::to_vec(&frozen()["result"]).unwrap()).unwrap();
    let mut too_wide = result.clone();
    too_wide.configuration.statuses =
        vec![result.configuration.statuses[0].clone(); MAX_COLLECTION_ENTRIES + 1];
    assert_eq!(encode_result(&too_wide), Err(Error::LimitExceeded));
    let mut too_long = result.clone();
    too_long.project.name = "x".repeat(MAX_STRING_BYTES + 1);
    assert_eq!(encode_result(&too_long), Err(Error::LimitExceeded));
    let mut escaped = result;
    let mut status = escaped.configuration.statuses[0].clone();
    status.name = "\u{0}".repeat(MAX_STRING_BYTES);
    escaped.configuration.statuses = vec![status; 4];
    assert_eq!(encode_result(&escaped), Err(Error::LimitExceeded));
}

use task_contract::field::*;
use task_contract::project::EstimateUnit;
use task_contract::relationship::*;
use task_contract::work_item::*;

#[test]
fn full_result_and_snapshot_round_trip_actual_types() {
    let mut r = decode_result(1, &serde_json::to_vec(&frozen()["result"]).unwrap()).unwrap();
    r.project.archived = true;
    r.project.ever_estimated = true;
    r.configuration.project_archived = true;
    r.configuration.statuses[0].group_id = Some(StatusGroupId("group".into()));
    r.configuration.status_groups.push(StatusGroup {
        id: StatusGroupId("group".into()),
        name: "Group".into(),
        archived: true,
    });
    r.configuration.types[0].title_field_id = None;
    r.configuration.relationship_types.push(RelationshipType {
        owner_project_id: r.project.id.clone(),
        id: RelationshipTypeId("rel".into()),
        name: "Related".into(),
        direction: Direction::Directed,
        archived: true,
    });
    for kind in [
        FieldKind::Text,
        FieldKind::Number,
        FieldKind::Boolean,
        FieldKind::Date,
        FieldKind::SingleChoice,
    ] {
        for usage in [
            FieldUsage::Hidden,
            FieldUsage::Optional,
            FieldUsage::Required,
        ] {
            r.configuration.fields[0].kind = kind;
            r.configuration.fields[0].usage = usage;
            r.configuration.fields[0].origin = FieldOrigin::Project;
            r.configuration.fields[0].archived = true;
            r.configuration.fields[0].options = vec![ChoiceOption {
                id: OptionId("choice".into()),
                name: "Choice".into(),
                archived: true,
            }];
            assert_eq!(decode_result(1, &encode_result(&r).unwrap()).unwrap(), r);
            assert_eq!(
                decode_snapshot(1, &encode_snapshot(&r.configuration).unwrap()).unwrap(),
                r.configuration
            );
        }
    }
    r.configuration.relationship_types[0].direction = Direction::Symmetric;
    r.project.estimate_unit = EstimateUnit::Points;
    assert_eq!(decode_result(1, &encode_result(&r).unwrap()).unwrap(), r);
    // These codec-only shape fixtures intentionally need not pass domain validation.
}
#[test]
fn numeric_extremes_and_vector_order_remain_exact() {
    let mut r = decode_result(1, &serde_json::to_vec(&frozen()["result"]).unwrap()).unwrap();
    r.configuration.statuses.reverse();
    r.configuration.fields.reverse();
    for n in [0, 1, (1 << 53) + 1, 1 << 63, u64::MAX] {
        r.project.next_sequence = n;
        r.project.configuration_revision = n;
        r.configuration.revision = n;
        assert_eq!(decode_result(1, &encode_result(&r).unwrap()).unwrap(), r);
    }
}

#[test]
fn aggregate_shape_bound_applies_to_encoding_before_writing() {
    let mut r = decode_result(1, &serde_json::to_vec(&frozen()["result"]).unwrap()).unwrap();
    r.configuration.statuses = vec![r.configuration.statuses[0].clone(); MAX_COLLECTION_ENTRIES];
    r.configuration.fields = vec![r.configuration.fields[0].clone(); MAX_COLLECTION_ENTRIES];
    r.configuration.types = vec![r.configuration.types[0].clone(); MAX_COLLECTION_ENTRIES];
    assert_eq!(encode_result(&r), Err(Error::LimitExceeded));
}

#[test]
fn saved_result_rejects_unknown_enums_and_missing_nested_fields() {
    let template = frozen()["result"].clone();
    for pointer in [
        "/value/seed_profile",
        "/value/project/estimate_unit",
        "/value/configuration/statuses/0/phase",
        "/value/configuration/fields/0/kind",
        "/value/configuration/fields/0/origin",
        "/value/configuration/fields/0/usage",
    ] {
        let mut bad = template.clone();
        *bad.pointer_mut(pointer).unwrap() = json!("unknown");
        assert!(decode_result(1, &serde_json::to_vec(&bad).unwrap()).is_err());
    }
    let mut bad = template;
    bad["value"]["project"]
        .as_object_mut()
        .unwrap()
        .remove("id");
    assert!(decode_result(1, &serde_json::to_vec(&bad).unwrap()).is_err());
}
