use task_contract::{
    current_work::UserId,
    operation::{
        compact_expired, decide_replay, CommandFamily, OperationError, OperationId, OperationKey,
        OperationRecord, OperationSuccess, OperationTarget, ReplayDecision, RequestFingerprint,
        RetainedOperation, REPLAY_PERIOD_MS,
    },
    work_item::{ProjectId, WorkItemId},
};

#[path = "../../../../tests/support/seeded.rs"]
mod seeded;

#[test]
fn generated_replay_retirement_and_clock_sequences_never_resurrect_original_identity() {
    for seed in [7, 0x5eed, 0xdecaf] {
        let mut generator = seeded::Generator::new(seed);
        for scenario in 0..128 {
            let key = OperationKey {
                actor_id: UserId(format!("owner-{scenario}")),
                family: CommandFamily::ItemEdit,
                target: OperationTarget::WorkItem {
                    project_id: ProjectId("project".into()),
                    item_id: WorkItemId(format!("item-{scenario}")),
                },
                operation_id: OperationId::parse(&format!("op-{}", generator.next())).unwrap(),
            };
            let original_request = generator.next();
            let original_result = generator.next();
            let fingerprint = RequestFingerprint {
                codec_version: 1,
                sha256: [42; 32],
            };
            let full = RetainedOperation {
                success: OperationSuccess {
                    key: key.clone(),
                    request: original_request,
                    result: original_result,
                    required_grants: 17u8,
                },
                recorded_at_ms: 100,
                request_fingerprint: fingerprint,
            };
            // Keep the identity valid and vary authorization independently of
            // request reuse for both retained representations.
            let mut changed_fingerprint = fingerprint;
            changed_fingerprint.sha256[0] ^= 1;
            for retained in [
                OperationRecord::Full(full.clone()),
                OperationRecord::Tombstone(compact_expired(&full, 100 + REPLAY_PERIOD_MS).unwrap()),
            ] {
                for authorized in [false, true] {
                    assert_eq!(
                        decide_replay(
                            &key,
                            &(original_request ^ 1),
                            Some(&retained),
                            &99,
                            changed_fingerprint,
                            100,
                            |scope| {
                                assert_eq!(*scope, 17);
                                authorized
                            },
                        ),
                        ReplayDecision::Reject(if authorized {
                            OperationError::OperationIdReused
                        } else {
                            OperationError::Unauthorized
                        }),
                    );
                }
            }
            let mut record = OperationRecord::Full(full.clone());
            let mut retired = false;
            for step in 0..64 {
                let action = generator.next() % 5;
                if action == 0 {
                    record = OperationRecord::Tombstone(
                        compact_expired(&full, 100 + REPLAY_PERIOD_MS).unwrap(),
                    );
                    retired = true;
                }
                let authorized = action != 1;
                // Returning to original time after retirement must not restore replay.
                let now = if action == 2 {
                    100 + REPLAY_PERIOD_MS
                } else {
                    100
                };
                let mut supplied_fingerprint = fingerprint;
                let request = if action == 3 {
                    supplied_fingerprint.sha256[0] ^= 1;
                    original_request ^ 1
                } else {
                    original_request
                };
                let observed = decide_replay(
                    &key,
                    &request,
                    Some(&record),
                    &99,
                    supplied_fingerprint,
                    now,
                    |scope| {
                        assert_eq!(*scope, 17, "must use original authority scope");
                        authorized
                    },
                );
                let expected = if !authorized {
                    ReplayDecision::Reject(OperationError::Unauthorized)
                } else if action == 3 {
                    ReplayDecision::Reject(OperationError::OperationIdReused)
                } else if retired || action == 2 {
                    ReplayDecision::Reject(OperationError::ReplayExpired)
                } else {
                    ReplayDecision::Replay(original_result)
                };
                assert_eq!(
                    observed, expected,
                    "seed {seed} scenario {scenario} step {step}"
                );
                assert_ne!(observed, ReplayDecision::Unseen);
            }
        }
    }
}
