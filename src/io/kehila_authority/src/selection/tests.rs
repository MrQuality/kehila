use super::*;
use task_contract::{
    current_work::{CurrentWork, SelectionCommand, SelectionError, UserId},
    relationship::{Endpoint, EndpointIdentity},
    work_item::{ProjectId, WorkItemId},
};

fn target(item: &str) -> Endpoint {
    Endpoint {
        project_id: ProjectId("p".into()),
        item_id: WorkItemId(item.into()),
        project_archived: false,
        item_archived: false,
    }
}
fn command(expected: u64, requested: Option<EndpointIdentity>) -> Canonical {
    Canonical::admit(
        "installation",
        "owner",
        SelectionCommand {
            user_id: UserId("owner".into()),
            operation_id: "a".repeat(32),
            expected_version: expected,
            requested,
        },
    )
    .unwrap()
}
fn current(version: u64, selected: Option<EndpointIdentity>) -> CurrentWork {
    CurrentWork {
        user_id: UserId("owner".into()),
        version,
        selected,
    }
}

#[test]
fn noop_keeps_selection_version_but_admits_distinct_next_event() {
    let endpoint = target("a");
    let canonical = command(9, Some(endpoint.identity()));
    let packet = canonical
        .prepare(&current(9, Some(endpoint.identity())), 20, Some(&endpoint))
        .unwrap();
    assert_eq!(packet.event.ordinal, 21);
    assert_eq!(packet.event.result.next.version, 9);
    assert_eq!(
        selection_codec::decode_event(1, packet.event_bytes.as_bytes()).unwrap(),
        packet.event
    );
    assert_eq!(
        selection_codec::decode_result(1, &packet.result_bytes).unwrap(),
        packet.event.result
    );
    let response: serde_json::Value = serde_json::from_slice(&packet.accepted_reply).unwrap();
    assert_eq!(
        response,
        serde_json::json!({"codec_version":1,"outcome":"accepted",
        "selection":{"user_id":"owner","version":"9","requested":{"project_id":"p","item_id":"a"}}})
    );
}

#[test]
fn changed_selection_and_clear_each_advance_exactly_once() {
    let endpoint = target("a");
    let packet = command(0, Some(endpoint.identity()))
        .prepare(&current(0, None), 0, Some(&endpoint))
        .unwrap();
    assert_eq!(
        packet.event.result.next,
        current(1, Some(endpoint.identity()))
    );
    let clear = command(1, None)
        .prepare(&packet.event.result.next, 1, None)
        .unwrap();
    assert_eq!(clear.event.result.next, current(2, None));
    assert_eq!(clear.event.ordinal, 2);
}

#[test]
fn permanent_event_verification_binds_owner_and_original_fingerprint_without_payload() {
    let endpoint = target("a");
    let canonical = command(0, Some(endpoint.identity()));
    let packet = canonical
        .prepare(&current(0, None), 0, Some(&endpoint))
        .unwrap();
    assert_eq!(
        verify_retained_event(
            "installation",
            "owner",
            1,
            packet.event_bytes.as_bytes(),
            &canonical.digest
        )
        .unwrap(),
        packet.event
    );
    assert!(verify_retained_event(
        "installation",
        "another-owner",
        1,
        packet.event_bytes.as_bytes(),
        &canonical.digest
    )
    .is_err());
    assert!(verify_retained_event(
        "installation",
        "owner",
        1,
        packet.event_bytes.as_bytes(),
        &[0; 32]
    )
    .is_err());
    let empty = command(0, None);
    let noop = empty.prepare(&current(0, None), 0, None).unwrap();
    assert_eq!(
        verify_retained_event(
            "installation",
            "owner",
            1,
            noop.event_bytes.as_bytes(),
            &empty.digest
        )
        .unwrap()
        .result
        .next
        .version,
        0
    );
}

#[test]
fn changed_version_remains_conflict_even_for_unchanged_target() {
    let endpoint = target("a");
    assert!(matches!(
        command(8, Some(endpoint.identity())).prepare(
            &current(9, Some(endpoint.identity())),
            20,
            Some(&endpoint)
        ),
        Err(PreparationError::Domain(SelectionError::VersionConflict {
            current_version: 9
        }))
    ));
}

#[test]
fn subject_target_and_ordinal_boundaries_cannot_be_substituted() {
    let mut request = command(0, None).request;
    request.user_id = UserId("another-owner".into());
    assert!(Canonical::admit("installation", "owner", request).is_err());
    for invalid in ["", "bad\0id"] {
        let request = command(0, None).request;
        assert!(Canonical::admit(invalid, "owner", request.clone()).is_err());
        assert!(Canonical::admit("installation", invalid, request).is_err());
    }
    let canonical = command(0, Some(target("a").identity()));
    assert!(canonical
        .prepare(&current(0, None), 0, Some(&target("b")))
        .is_err());
    assert!(matches!(
        command(0, None).prepare(&current(0, None), u64::MAX, None),
        Err(PreparationError::StateExhausted)
    ));
    assert!(matches!(
        command(u64::MAX, None).prepare(
            &current(u64::MAX, Some(target("a").identity())),
            u64::MAX,
            None
        ),
        Err(PreparationError::Domain(SelectionError::VersionOverflow))
    ));
}

#[test]
fn historical_reconstruction_preserves_original_without_current_requalification() {
    let endpoint = target("a");
    let canonical = command(0, Some(endpoint.identity()));
    let packet = canonical
        .prepare(&current(0, None), 0, Some(&endpoint))
        .unwrap();
    let historical = canonical
        .reconstruct(1, packet.event_bytes.as_bytes(), 1, &packet.result_bytes)
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&historical.reply).unwrap(),
        serde_json::json!({"codec_version":1,"outcome":"replayed",
            "selection":{"user_id":"owner","version":"1","requested":{"project_id":"p","item_id":"a"}}})
    );
    let changed = command(1, None);
    assert!(changed
        .reconstruct(1, packet.event_bytes.as_bytes(), 1, &packet.result_bytes)
        .is_err());
    let mut event = packet.event.clone();
    event.result.next.user_id = UserId("another-owner".into());
    let bytes = selection_codec::encode_event(&event, 1).unwrap();
    assert!(canonical
        .reconstruct(1, &bytes, 1, &packet.result_bytes)
        .is_err());
}

#[test]
fn historical_identity_and_transition_checks_are_independent_of_payload_equality() {
    let endpoint = target("a");
    let canonical = command(0, Some(endpoint.identity()));
    let packet = canonical
        .prepare(&current(0, None), 0, Some(&endpoint))
        .unwrap();
    for changed in 0..5 {
        let mut event = packet.event.clone();
        match changed {
            0 => event.operation_id = "b".repeat(32),
            1 => event.before.user_id = UserId("another-owner".into()),
            2 => event.before.version = 8,
            3 => event.result.next.selected = Some(target("b").identity()),
            4 => event.result.next.version = 2,
            _ => unreachable!(),
        }
        let history = selection_codec::encode_event(&event, 1).unwrap();
        let matching_result = selection_codec::encode_result(&event.result, 1).unwrap();
        assert!(
            canonical
                .reconstruct(1, &history, 1, &matching_result)
                .is_err(),
            "case {changed}"
        );
    }
    let different = selection_codec::encode_result(
        &task_contract::current_work::SelectionResult {
            next: current(1, Some(target("b").identity())),
        },
        1,
    )
    .unwrap();
    assert!(canonical
        .reconstruct(1, packet.event_bytes.as_bytes(), 1, &different)
        .is_err());
    let endpoint = target("a");
    let canonical = command(1, Some(endpoint.identity()));
    let mut event = canonical
        .prepare(&current(1, Some(endpoint.identity())), 1, Some(&endpoint))
        .unwrap()
        .event;
    event.result.next.version = 2;
    let history = selection_codec::encode_event(&event, 1).unwrap();
    let result = selection_codec::encode_result(&event.result, 1).unwrap();
    assert!(canonical.reconstruct(1, &history, 1, &result).is_err());
}

#[test]
fn zero_version_target_is_invalid_even_after_later_empty_noops() {
    let endpoint = target("a");
    assert!(command(0, Some(endpoint.identity()))
        .prepare(&current(0, Some(endpoint.identity())), 8, Some(&endpoint))
        .is_err());
}

#[test]
fn retained_zero_version_target_is_not_validated_by_matching_payload_alone() {
    let endpoint = target("a");
    let mut event = command(1, Some(endpoint.identity()))
        .prepare(&current(1, Some(endpoint.identity())), 8, Some(&endpoint))
        .unwrap()
        .event;
    event.before.version = 0;
    event.result.next.version = 0;
    let history = selection_codec::encode_event(&event, 1).unwrap();
    let result = selection_codec::encode_result(&event.result, 1).unwrap();
    assert!(command(0, Some(endpoint.identity()))
        .reconstruct(1, &history, 1, &result)
        .is_err());
}

#[test]
fn retained_event_checks_structure_even_with_a_matching_request_fingerprint() {
    let endpoint = target("a");
    let original = command(1, Some(endpoint.identity()))
        .prepare(&current(1, Some(endpoint.identity())), 8, Some(&endpoint))
        .unwrap()
        .event;
    for changed in 0..3 {
        let mut event = original.clone();
        match changed {
            0 => event.ordinal = event.before.version,
            1 => event.result.next.version = 2, // An unchanged choice cannot advance the version.
            2 => {
                event.before.version = 0;
                event.result.next.version = 0;
            }
            _ => unreachable!(),
        }
        let canonical = command(event.before.version, event.result.next.selected.clone());
        let history = selection_codec::encode_event(&event, 1).unwrap();
        assert!(
            verify_retained_event("installation", "owner", 1, &history, &canonical.digest).is_err(),
            "invalid retained structure {changed} cannot be admitted by a matching fingerprint"
        );
    }
}
