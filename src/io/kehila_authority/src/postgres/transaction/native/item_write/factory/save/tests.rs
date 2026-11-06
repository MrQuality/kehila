//! Closed save-response classification only; no database execution evidence.
use super::*;

fn admitted() -> Admitted {
    super::super::tests::draft().admit().unwrap()
}
fn metadata() -> Metadata {
    Metadata {
        operation: Some(7),
        target: Some("item".into()),
        codec: Some(1),
        token: Some("f".repeat(32)),
        pending: Some(17),
        pending_expires: Some(86400000),
        origin: Some(100),
        end: Some(100 + task_contract::operation::REPLAY_PERIOD_MS),
    }
}
fn unseen() -> Metadata {
    let mut data = metadata();
    data.operation = None;
    data.origin = None;
    data.end = None;
    data
}

#[test]
fn fresh_and_historical_results_have_disjoint_ownership() {
    let admitted = admitted();
    assert!(matches!(
        classify("accepted", metadata(), &admitted),
        Ok(Stage::Accepted(FreshAcceptance { operation: 7, .. }))
    ));
    for outcome in ["replay", "expired"] {
        assert!(matches!(
            classify(outcome, metadata(), &admitted),
            Ok(Stage::Resolved(_))
        ));
    }
}

#[test]
fn allocation_retry_does_not_rebase_item_or_configuration_conflicts() {
    let admitted = admitted();
    assert!(matches!(
        classify("reprepare", unseen(), &admitted),
        Ok(Stage::Reprepare)
    ));
    assert!(matches!(
        classify("configuration_conflict", unseen(), &admitted),
        Ok(Stage::Conflict(WriteConflict::Configuration))
    ));
    assert!(matches!(
        classify("item_conflict", unseen(), &admitted),
        Ok(Stage::Conflict(WriteConflict::Item))
    ));
    assert!(matches!(
        classify("reprepare", metadata(), &admitted),
        Err(NativeError::Integrity)
    ));
}

#[test]
fn returned_request_and_pending_identity_cannot_be_substituted() {
    let admitted = admitted();
    for field in 0..5 {
        let mut data = metadata();
        match field {
            0 => data.target = Some("other".into()),
            1 => data.token = Some("e".repeat(32)),
            2 => data.codec = Some(2),
            3 => data.pending = Some(18),
            _ => data.pending_expires = Some(86400001),
        }
        assert!(matches!(
            classify("accepted", data, &admitted),
            Err(NativeError::Integrity)
        ));
    }
}

#[test]
fn a_creation_echo_conflict_without_success_never_becomes_replay_or_acceptance() {
    let admitted = admitted();
    assert!(matches!(
        classify("conflict", unseen(), &admitted),
        Ok(Stage::Resolved(Resolved {
            outcome: Disposition::Conflict,
            ..
        }))
    ));
    assert!(matches!(
        classify("expired", unseen(), &admitted),
        Ok(Stage::Resolved(Resolved {
            outcome: Disposition::Expired,
            ..
        }))
    ));
    assert!(matches!(
        classify("replay", unseen(), &admitted),
        Err(NativeError::Integrity)
    ));
}

#[test]
fn lookup_or_unknown_status_cannot_authorize_a_write_completion() {
    let admitted = admitted();
    for outcome in ["prepared", "unseen", "accepted_again", ""] {
        assert!(matches!(
            classify(outcome, metadata(), &admitted),
            Err(NativeError::Integrity)
        ));
    }
}

#[test]
fn historical_edit_conflict_may_report_a_different_stored_codec() {
    let admitted = super::super::tests::edit_draft().admit().unwrap();
    for outcome in ["conflict", "accepted", "replay", "expired"] {
        let mut data = metadata();
        data.pending = None;
        data.pending_expires = None;
        data.token = Some("3".repeat(32));
        data.codec = Some(2);
        let result = classify(outcome, data, &admitted);
        if outcome == "conflict" {
            assert!(matches!(
                result,
                Ok(Stage::Resolved(Resolved {
                    outcome: Disposition::Conflict,
                    ..
                }))
            ));
        } else {
            assert!(matches!(result, Err(NativeError::Integrity)));
        }
    }
}
