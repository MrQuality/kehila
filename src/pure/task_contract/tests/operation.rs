use task_contract::current_work::UserId;
use task_contract::operation::{
    compact_expired, OperationRecord, RequestFingerprint, RetainedOperation, REPLAY_PERIOD_MS,
};
use task_contract::operation::{
    decide_replay as decide_retained_replay, CommandFamily, OperationError, OperationId,
    OperationKey, OperationSuccess, OperationTarget, ReplayDecision,
};
use task_contract::work_item::{ProjectId, WorkItemId};

fn key() -> OperationKey {
    OperationKey {
        actor_id: UserId("actor".into()),
        family: CommandFamily::ItemEdit,
        target: OperationTarget::WorkItem {
            project_id: ProjectId("project".into()),
            item_id: WorkItemId("item".into()),
        },
        operation_id: OperationId::parse("operation").unwrap(),
    }
}

#[test]
fn tokens_are_bounded_case_sensitive_and_not_normalized() {
    for bad in [
        "".to_string(),
        "x".repeat(129),
        "a b".into(),
        "a\n".into(),
        "é".into(),
    ] {
        assert_eq!(
            OperationId::parse(&bad),
            Err(OperationError::InvalidOperation)
        );
    }
    assert!(OperationId::parse(&"x".repeat(128)).is_ok());
    assert_ne!(OperationId::parse("Case"), OperationId::parse("case"));
}

#[test]
fn lookup_is_bound_to_actor_family_target_and_exact_ordered_content() {
    let request = vec!["first", "second"];
    let record = OperationSuccess {
        key: key(),
        request: request.clone(),
        result: 7,
        required_grants: vec!["edit"],
    };
    assert_eq!(
        decide_replay(&key(), &request, Some(&record), &vec![], |_| true),
        ReplayDecision::Replay(7)
    );
    assert_eq!(
        decide_replay(
            &key(),
            &vec!["second", "first"],
            Some(&record),
            &vec![],
            |_| true
        ),
        ReplayDecision::Reject(OperationError::OperationIdReused)
    );
    for field in 0..3 {
        let mut changed = key();
        match field {
            0 => changed.actor_id = UserId("other".into()),
            1 => changed.family = CommandFamily::ItemCreate,
            _ => {
                changed.target = OperationTarget::WorkItem {
                    project_id: ProjectId("other".into()),
                    item_id: WorkItemId("item".into()),
                }
            }
        }
        assert_eq!(
            decide_replay(&changed, &request, Some(&record), &vec![], |_| true),
            ReplayDecision::Reject(OperationError::InvalidReference)
        );
    }
}

#[test]
fn replay_authorizes_the_retained_scope_and_unseen_authorizes_the_new_scope() {
    let record = OperationSuccess {
        key: key(),
        request: 1,
        result: 7,
        required_grants: vec!["edit", "migrate"],
    };
    assert_eq!(
        decide_replay(&key(), &1, Some(&record), &vec!["edit"], |scope| !scope
            .contains(&"migrate")),
        ReplayDecision::Reject(OperationError::Unauthorized)
    );
    assert_eq!(
        decide_replay::<_, i32, _>(&key(), &1, None, &vec!["edit"], |scope| scope
            == &vec!["edit"]),
        ReplayDecision::Unseen
    );
}

fn decide_replay<Request: Eq + Clone, Result: Clone, Scope: Clone>(
    key: &OperationKey,
    request: &Request,
    recorded: Option<&OperationSuccess<Request, Result, Scope>>,
    new_required_grants: &Scope,
    authorize: impl FnOnce(&Scope) -> bool,
) -> ReplayDecision<Result> {
    let recorded = recorded.cloned().map(|success| {
        OperationRecord::Full(RetainedOperation {
            success,
            committed_at_ms: 1,
            request_fingerprint: RequestFingerprint {
                codec_version: 1,
                sha256: [1; 32],
            },
        })
    });
    decide_retained_replay(
        key,
        request,
        recorded.as_ref(),
        new_required_grants,
        RequestFingerprint {
            codec_version: 1,
            sha256: [1; 32],
        },
        1,
        authorize,
    )
}

#[test]
fn expiry_boundary_and_tombstones_never_become_unseen_operations() {
    let full = RetainedOperation {
        success: OperationSuccess {
            key: key(),
            request: 1,
            result: 7,
            required_grants: vec!["edit"],
        },
        committed_at_ms: 100,
        request_fingerprint: RequestFingerprint {
            codec_version: 1,
            sha256: [1; 32],
        },
    };
    let deadline = 100 + REPLAY_PERIOD_MS;
    let record = OperationRecord::Full(full.clone());
    assert_eq!(
        decide_retained_replay(
            &key(),
            &1,
            Some(&record),
            &vec![],
            RequestFingerprint {
                codec_version: 1,
                sha256: [1; 32]
            },
            deadline - 1,
            |_| true
        ),
        ReplayDecision::Replay(7)
    );
    assert_eq!(
        decide_retained_replay(
            &key(),
            &1,
            Some(&record),
            &vec![],
            RequestFingerprint {
                codec_version: 1,
                sha256: [1; 32]
            },
            deadline,
            |_| true
        ),
        ReplayDecision::Reject(OperationError::ReplayExpired)
    );
    assert_eq!(
        compact_expired(&full, deadline - 1),
        Err(OperationError::InvalidOperation)
    );
    let tombstone: OperationRecord<i32, i32, Vec<&str>> =
        OperationRecord::Tombstone(compact_expired(&full, deadline).unwrap());
    for now in [0, deadline, u64::MAX] {
        assert_eq!(
            decide_retained_replay(
                &key(),
                &1,
                Some(&tombstone),
                &vec![],
                RequestFingerprint {
                    codec_version: 1,
                    sha256: [1; 32]
                },
                now,
                |_| true
            ),
            ReplayDecision::Reject(OperationError::ReplayExpired)
        );
    }
    assert_eq!(
        decide_retained_replay(
            &key(),
            &2,
            Some(&tombstone),
            &vec![],
            RequestFingerprint {
                codec_version: 1,
                sha256: [2; 32]
            },
            deadline,
            |_| true
        ),
        ReplayDecision::Reject(OperationError::OperationIdReused)
    );
    assert_eq!(
        decide_retained_replay(
            &key(),
            &1,
            Some(&tombstone),
            &vec![],
            RequestFingerprint {
                codec_version: 1,
                sha256: [1; 32]
            },
            deadline,
            |_| false
        ),
        ReplayDecision::Reject(OperationError::Unauthorized)
    );
    let mut foreign = key();
    foreign.actor_id = UserId("other".into());
    assert_eq!(
        decide_retained_replay(
            &foreign,
            &1,
            Some(&tombstone),
            &vec![],
            RequestFingerprint {
                codec_version: 1,
                sha256: [1; 32]
            },
            deadline,
            |_| true
        ),
        ReplayDecision::Reject(OperationError::InvalidReference)
    );
}

#[test]
fn retention_rejects_timestamp_overflow_and_incoherent_fingerprints() {
    let mut full = RetainedOperation {
        success: OperationSuccess {
            key: key(),
            request: 1,
            result: 7,
            required_grants: vec!["edit"],
        },
        committed_at_ms: u64::MAX,
        request_fingerprint: RequestFingerprint {
            codec_version: 1,
            sha256: [1; 32],
        },
    };
    assert_eq!(
        compact_expired(&full, u64::MAX),
        Err(OperationError::InvalidOperation)
    );
    let record = OperationRecord::Full(full.clone());
    assert_eq!(
        decide_retained_replay(
            &key(),
            &1,
            Some(&record),
            &vec![],
            RequestFingerprint {
                codec_version: 1,
                sha256: [1; 32]
            },
            u64::MAX,
            |_| true
        ),
        ReplayDecision::Reject(OperationError::InvalidOperation)
    );
    full.committed_at_ms = 1;
    let record = OperationRecord::Full(full);
    assert_eq!(
        decide_retained_replay(
            &key(),
            &1,
            Some(&record),
            &vec![],
            RequestFingerprint {
                codec_version: 1,
                sha256: [2; 32]
            },
            1,
            |_| true
        ),
        ReplayDecision::Reject(OperationError::InvalidReference)
    );
}
