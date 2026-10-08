use super::*;
use task_contract::current_work::UserId;
use task_contract::field::*;
use task_contract::operation::*;
use task_contract::relationship::*;
use task_contract::work_item::*;

fn request() -> ProjectCreateCommand {
    command(&json!({"operation_id":"op","project_id":"project","name":"Project","prefix":"QA","estimate_unit":"hours"})).unwrap()
}
fn result() -> ProjectCreateResult {
    let ProjectCreateDecision::Apply(r) = decide_project_create(false, None, true, &request())
    else {
        panic!("valid creation");
    };
    r
}

#[test]
fn requests_round_trip_every_field_without_normalization() {
    let c = request();
    for name in ["Project", "é", "e\u{301}", "字\"\\", "with\0nul"] {
        for unit in [EstimateUnit::Hours, EstimateUnit::Points] {
            let mut variant = c.clone();
            variant.name = name.into();
            variant.estimate_unit = unit;
            assert_eq!(
                decode_request(&encode_request(&variant, 1).unwrap()).unwrap(),
                variant
            );
        }
    }
    let mut encodings = std::collections::HashSet::new();
    for field in 0..5 {
        let mut variant = c.clone();
        match field {
            0 => variant.operation_id.push('x'),
            1 => variant.project_id.0.push('x'),
            2 => variant.name.push('x'),
            3 => variant.prefix.push('x'),
            _ => variant.estimate_unit = EstimateUnit::Points,
        };
        assert!(encodings.insert(encode_request(&variant, 1).unwrap()));
    }
}
#[test]
fn malformed_request_is_rejected() {
    let bytes = encode_request(&request(), 1).unwrap();
    for length in 0..bytes.len() {
        assert!(decode_request(&bytes[..length]).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decode_request(&trailing).is_err());
    let mut bad = bytes.clone();
    *bad.last_mut().unwrap() = 2;
    assert!(decode_request(&bad).is_err());
    let mut utf8 = bytes;
    // Alter the first field payload, not a letter in the framing header.
    let index = b"kehila\0project_create\0\x00\x00\x00\x01".len() + 4;
    utf8[index] = 255;
    assert!(decode_request(&utf8).is_err());
    assert!(encode_request(&request(), 0).is_err());
    assert!(encode_request(&request(), 2).is_err());
}
#[test]
fn full_result_and_snapshot_round_trip_actual_types() {
    let mut r = result();
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
            assert_eq!(decode_result(&encode_result(&r)).unwrap(), r);
            assert_eq!(
                decode_snapshot(&encode_snapshot(&r.configuration)).unwrap(),
                r.configuration
            );
        }
    }
    r.configuration.relationship_types[0].direction = Direction::Symmetric;
    r.project.estimate_unit = EstimateUnit::Points;
    assert_eq!(decode_result(&encode_result(&r)).unwrap(), r);
    // These codec-only shape fixtures intentionally need not pass domain validation.
}
#[test]
fn unsigned_boundaries_and_invalid_spellings() {
    for n in [
        0,
        1,
        (1 << 53) - 1,
        1 << 53,
        (1 << 53) + 1,
        (1 << 63),
        u64::MAX,
    ] {
        let mut r = result();
        r.project.configuration_revision = n;
        r.project.next_sequence = n;
        r.configuration.revision = n;
        assert_eq!(decode_result(&encode_result(&r)).unwrap(), r);
    }
    for v in [
        json!(0),
        json!(1.5),
        json!(-1),
        Value::Null,
        json!(""),
        json!("00"),
        json!("01"),
        json!("+1"),
        json!("-1"),
        json!("1.0"),
        json!("1e0"),
        json!("18446744073709551616"),
        json!("١"),
    ] {
        assert!(unsigned(&v).is_err(), "{v}");
    }
}
#[test]
fn strict_shapes_versions_and_kinds() {
    let base = encode_result(&result());
    for version in [json!(0), json!(2), json!("1"), json!(1.0), Value::Null] {
        let mut v = base.clone();
        v["codec_version"] = version;
        assert!(decode_result(&v).is_err());
    }
    let mut missing = base.clone();
    missing["value"]["project"]
        .as_object_mut()
        .unwrap()
        .remove("next_sequence");
    assert!(decode_result(&missing).is_err());
    let mut extra = base.clone();
    extra["value"]["project"]["future"] = json!(true);
    assert!(decode_result(&extra).is_err());
    let mut wrong = base;
    wrong["value"]["configuration"]["statuses"][0]["phase"] = json!("future");
    assert!(decode_result(&wrong).is_err());
    assert!(decode_result(&encode_snapshot(&result().configuration)).is_err());
}
#[test]
fn array_order_is_typed_intent() {
    let r = result();
    let mut value = encode_result(&r);
    value["value"]["configuration"]["statuses"]
        .as_array_mut()
        .unwrap()
        .reverse();
    assert_ne!(decode_result(&value).unwrap(), r);
}
#[test]
fn replay_uses_stored_version_and_original_result() {
    let request = request();
    let result = result();
    let key = OperationKey {
        actor_id: UserId("actor".into()),
        family: CommandFamily::ProjectCreate,
        target: OperationTarget::Project(request.project_id.clone()),
        operation_id: OperationId::parse(&request.operation_id).unwrap(),
    };
    let fingerprint = RequestFingerprint {
        codec_version: 1,
        sha256: digest(&encode_request(&request, 1).unwrap()),
    };
    let retained = RetainedOperation {
        success: OperationSuccess {
            key: key.clone(),
            request: request.clone(),
            result: result.clone(),
            required_grants: (),
        },
        recorded_at_ms: 100,
        request_fingerprint: fingerprint,
    };
    let full = OperationRecord::Full(retained.clone());
    assert_eq!(
        decide_replay(&key, &request, Some(&full), &(), fingerprint, 101, |_| true),
        ReplayDecision::Replay(result)
    );
    let mut changed = request.clone();
    changed.name.push('x');
    let changed_digest = RequestFingerprint {
        codec_version: 1,
        sha256: digest(&encode_request(&changed, 1).unwrap()),
    };
    assert_eq!(
        decide_replay(
            &key,
            &changed,
            Some(&full),
            &(),
            changed_digest,
            101,
            |_| true
        ),
        ReplayDecision::Reject(OperationError::OperationIdReused)
    );
    let inconsistent = RequestFingerprint {
        codec_version: 1,
        sha256: [0; 32],
    };
    assert_eq!(
        decide_replay(&key, &request, Some(&full), &(), inconsistent, 101, |_| {
            true
        }),
        ReplayDecision::Reject(OperationError::InvalidReference)
    );
    let tombstone: OperationRecord<ProjectCreateCommand, ProjectCreateResult, ()> =
        OperationRecord::Tombstone(compact_expired(&retained, 100 + REPLAY_PERIOD_MS).unwrap());
    assert_eq!(
        decide_replay(
            &key,
            &request,
            Some(&tombstone),
            &(),
            fingerprint,
            100 + REPLAY_PERIOD_MS,
            |_| true
        ),
        ReplayDecision::Reject(OperationError::ReplayExpired)
    );
    assert_eq!(
        decide_replay(
            &key,
            &changed,
            Some(&tombstone),
            &(),
            changed_digest,
            100 + REPLAY_PERIOD_MS,
            |_| true
        ),
        ReplayDecision::Reject(OperationError::OperationIdReused)
    );
    assert!(encode_request(&request, 2).is_err()); // Unknown stored version never becomes Unseen.
}
#[test]
fn identifier_proposal_preserves_bytes_and_counts_utf8() {
    assert!(proposed_id(&"a".repeat(128)));
    assert!(!proposed_id(&"a".repeat(129)));
    assert!(proposed_id(&"é".repeat(64)));
    assert!(!proposed_id(&"é".repeat(65)));
    assert!(!proposed_id(""));
    assert!(!proposed_id("a\0b"));
    assert!(proposed_id("e\u{301}"));
}
