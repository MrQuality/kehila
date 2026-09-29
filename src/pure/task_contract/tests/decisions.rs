use task_contract::{decide, Decision, Mutation, SuccessfulOperation};

fn mutation() -> Mutation {
    Mutation {
        operation_id: "operation".into(),
        expected_version: 0,
        title: "Task".into(),
        status: "Open".into(),
    }
}

#[test]
fn recorded_result_wins_over_current_version_and_changed_rules() {
    let original = mutation();
    let previous = SuccessfulOperation {
        request: original.clone(),
        result_version: 1,
    };
    assert_eq!(
        decide(9, Some(&previous), &original),
        Decision::Replay { version: 1 }
    );
    let changed = Mutation {
        title: "".into(),
        ..original
    };
    assert_eq!(
        decide(9, Some(&previous), &changed),
        Decision::OperationIdReused
    );
}

#[test]
fn unrecorded_intentions_require_a_valid_matching_version() {
    let original = mutation();
    assert_eq!(decide(0, None, &original), Decision::Apply { version: 1 });
    assert_eq!(
        decide(1, None, &original),
        Decision::VersionConflict { current_version: 1 }
    );
    let invalid = Mutation {
        expected_version: i64::MAX,
        ..original
    };
    assert_eq!(
        decide(i64::MAX, None, &invalid),
        Decision::Invalid("invalid_mutation")
    );
}

#[test]
fn changed_identity_is_a_new_intention_and_matching_update_advances_once() {
    let original = mutation();
    let update = Mutation {
        operation_id: "next-operation".into(),
        expected_version: 1,
        ..original.clone()
    };
    assert_eq!(decide(1, None, &update), Decision::Apply { version: 2 });
    let stale = Mutation {
        expected_version: 0,
        ..update
    };
    assert_eq!(
        decide(1, None, &stale),
        Decision::VersionConflict { current_version: 1 }
    );
}

#[test]
fn new_mutations_validate_version_title_and_status() {
    for expected_version in [-1, i64::MAX] {
        assert_eq!(
            decide(
                0,
                None,
                &Mutation {
                    expected_version,
                    ..mutation()
                }
            ),
            Decision::Invalid("invalid_mutation")
        );
    }
    for title in [String::new(), "x".repeat(129)] {
        assert_eq!(
            decide(
                0,
                None,
                &Mutation {
                    title,
                    ..mutation()
                }
            ),
            Decision::Invalid("invalid_mutation")
        );
    }
    assert_eq!(
        decide(
            0,
            None,
            &Mutation {
                status: "Unknown".into(),
                ..mutation()
            }
        ),
        Decision::Invalid("invalid_status")
    );
}
