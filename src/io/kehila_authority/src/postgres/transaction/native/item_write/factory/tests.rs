//! Synthetic owned-draft boundaries, not database authority qualification.
use super::*;
use task_contract::{item_mutation::CreateCommand, project::EstimateUnit, work_item::WorkItemId};
pub(super) fn draft() -> Draft {
    let seed = crate::project::Prepared::new(
        crate::native_ingress::Form {
            name: "Project".into(),
            prefix: "PR".into(),
            estimate_unit: EstimateUnit::Hours,
        },
        "project".into(),
        "0".repeat(32),
    )
    .unwrap();
    let seed = seed.result();
    let observed = Observed {
        scope: Scope {
            installation: "installation".into(),
            actor: "owner".into(),
        },
        project: seed.project.clone(),
        configuration: seed.configuration.clone(),
        configuration_project: seed.project.clone(),
        configuration_operation: 1,
        state_version: 0,
        state_operation: 1,
        existing: None,
    };
    let command = CreateCommand {
        operation_id: "f".repeat(32),
        expected_configuration_revision: 1,
        item_id: WorkItemId("item".into()),
        type_id: seed.configuration.types[0].id.clone(),
        workflow_id: None,
        estimate: None,
        values: vec![],
    };
    let request = Request::Create {
        project_id: seed.project.id.clone(),
        command,
    };
    let bytes = work_item_codec::admit_request(&request)
        .unwrap()
        .as_bytes()
        .to_vec();
    Draft {
        observed,
        source: Source::Retained(StoredRequest { request, bytes }),
        identity: Identity::Create {
            handle: "e".repeat(32),
            pending: 17,
            expires: 86400000,
        },
        epoch: Epoch {
            runtime: 1,
            authentication: 2,
            revision: 3,
        },
        live: RuntimeLiveness::new(),
        deadline: Instant::now() + std::time::Duration::from_secs(5),
    }
}
#[test]
fn admission_preserves_original_scope_dependencies_identity_and_deadline() {
    let draft = draft();
    let epoch = draft.epoch;
    let deadline = draft.deadline;
    let live = draft.live.clone();
    let Source::Retained(stored) = &draft.source else {
        panic!()
    };
    let bytes = stored.bytes.clone();
    let admitted = draft.admit().unwrap();
    assert_eq!(admitted.request_bytes(), bytes);
    assert_eq!(admitted.original.deadline, deadline);
    assert_eq!(admitted.original.epoch, epoch);
    assert!(Arc::ptr_eq(&admitted.original.live, &live));
    assert_eq!(admitted.packet.observed.project.next_sequence, 1);
    assert_eq!(admitted.packet.observed.state_version, 0);
    let work_item_codec::SavedResult::Create(result) = &admitted.revision().result else {
        panic!()
    };
    assert_eq!(result.project.next_sequence, 2);
    assert_eq!(admitted.revision().project_state_version, 1);
    match &admitted.original.identity {
        Identity::Create {
            handle,
            pending,
            expires,
        } => {
            assert_eq!(handle, &"e".repeat(32));
            assert_eq!(*pending, 17);
            assert_eq!(*expires, 86400000)
        }
        _ => panic!(),
    }
}
#[test]
fn exact_retained_bytes_cannot_be_replaced_by_semantically_equal_json() {
    let mut draft = draft();
    let Source::Retained(stored) = &mut draft.source else {
        panic!()
    };
    stored.bytes.insert(0, b' ');
    // A trusted-factory corruption control: the ordinary pending decoder would
    // reject this noncanonical spelling before it could become a real draft.
    assert!(matches!(draft.admit(), Err(PreparationError::InvalidProof)));
}
#[test]
fn elapsed_original_deadline_cannot_mint_an_admitted_attempt() {
    let mut draft = draft();
    draft.deadline = Instant::now();
    assert!(matches!(draft.admit(), Err(PreparationError::Expired)));
}
#[test]
fn stopped_original_liveness_cannot_mint_an_admitted_attempt() {
    let draft = draft();
    draft.live.stop();
    assert!(matches!(draft.admit(), Err(PreparationError::Unavailable)));
}

fn authority() -> Authority {
    Authority {
        installation: "installation".into(),
        owner: "owner".into(),
        runtime: 1,
        authentication: 2,
        revision: 3,
    }
}
#[test]
fn write_binding_checks_every_authority_field_original_liveness_and_exact_deadline() {
    let admitted = draft().admit().unwrap();
    let now = Instant::now();
    assert!(admitted
        .check_owner(
            &authority(),
            &admitted.original.live,
            admitted.original.deadline,
            now
        )
        .is_ok());
    for field in 0..5 {
        let mut a = authority();
        match field {
            0 => a.installation = "other".into(),
            1 => a.owner = "other".into(),
            2 => a.runtime += 1,
            3 => a.authentication += 1,
            _ => a.revision += 1,
        };
        assert!(matches!(
            admitted.check_owner(&a, &admitted.original.live, admitted.original.deadline, now),
            Err(NativeError::Integrity)
        ));
    }
    assert!(matches!(
        admitted.check_owner(
            &authority(),
            &RuntimeLiveness::new(),
            admitted.original.deadline,
            now
        ),
        Err(NativeError::Integrity)
    ));
    assert!(matches!(
        admitted.check_owner(
            &authority(),
            &admitted.original.live,
            admitted.original.deadline + std::time::Duration::from_millis(1),
            now
        ),
        Err(NativeError::Integrity)
    ));
}
#[test]
fn original_write_deadline_and_stop_state_cannot_be_renewed() {
    let admitted = draft().admit().unwrap();
    assert!(matches!(
        admitted.check_owner(
            &authority(),
            &admitted.original.live,
            admitted.original.deadline,
            admitted.original.deadline
        ),
        Err(NativeError::Database(crate::postgres::DbError::Expired))
    ));
    admitted.original.live.stop();
    assert!(matches!(
        admitted.check_owner(
            &authority(),
            &admitted.original.live,
            admitted.original.deadline,
            Instant::now()
        ),
        Err(NativeError::Database(crate::postgres::DbError::Unavailable))
    ));
}

#[test]
fn a_misbound_family_cannot_cross_the_write_boundary() {
    let mut draft = draft();
    draft.identity = Identity::Edit;
    let admitted = draft.admit().unwrap();
    assert!(matches!(
        admitted.check_owner(
            &authority(),
            &admitted.original.live,
            admitted.original.deadline,
            Instant::now()
        ),
        Err(NativeError::Integrity)
    ));
}

pub(super) fn edit_draft() -> Draft {
    let (observed, command) = super::super::tests::edit_fixture();
    let request = Request::Edit {
        project_id: observed.project.id.clone(),
        item_id: WorkItemId("item".into()),
        command,
    };
    let canonical = Arc::new(Canonical::new(observed.scope.clone(), request).unwrap());
    Draft {
        observed,
        source: Source::Direct(canonical),
        identity: Identity::Edit,
        epoch: Epoch {
            runtime: 1,
            authentication: 2,
            revision: 3,
        },
        live: RuntimeLiveness::new(),
        deadline: Instant::now() + std::time::Duration::from_secs(5),
    }
}
fn original(draft: Draft) -> RetryIdentity {
    let Admitted { packet, original } = draft.admit().unwrap();
    drop(packet);
    original
}
fn refreshed(original: &RetryIdentity) -> Draft {
    let mut fresh = match &original.identity {
        Identity::Create { .. } => draft(),
        Identity::Edit => edit_draft(),
    };
    fresh.live = Arc::clone(&original.live);
    fresh.deadline = original.deadline;
    fresh.epoch = original.epoch;
    if matches!(&original.identity, Identity::Edit) {
        fresh.source = Source::Direct(Arc::clone(&original.command));
    }
    fresh
}
fn historical(original: &RetryIdentity, outcome: Disposition) -> Preparation {
    let (target, token) = match &original.command.request {
        Request::Create { command, .. } => {
            (command.item_id.0.clone(), command.operation_id.clone())
        }
        Request::Edit {
            item_id, command, ..
        } => (item_id.0.clone(), command.operation_id.clone()),
    };
    let (pending, pending_expires) = match &original.identity {
        Identity::Create {
            pending, expires, ..
        } => (Some(*pending), Some(*expires)),
        Identity::Edit => (None, None),
    };
    let success = outcome != Disposition::Cancelled;
    Preparation::Resolved(Resolved {
        outcome,
        data: Metadata {
            operation: success.then_some(9),
            target: Some(target),
            token: Some(token),
            codec: Some(1),
            pending,
            pending_expires,
            origin: success.then_some(100),
            end: success.then_some(100 + task_contract::operation::REPLAY_PERIOD_MS),
        },
    })
}

#[test]
fn allocation_retry_reuses_original_bytes_arc_and_lifetime() {
    let original = original(draft());
    let canonical = Arc::clone(&original.command);
    let deadline = original.deadline;
    let live = Arc::clone(&original.live);
    let mut fresh = refreshed(&original);
    fresh.observed.project.next_sequence = 2;
    fresh.observed.state_version = 1;
    fresh.observed.state_operation = 2;
    let prep = original
        .bind_preparation(Preparation::Ready(Box::new(fresh)))
        .unwrap_or_else(|_| panic!("valid retry binding"));
    let Preparation::Ready(fresh) = prep else {
        panic!()
    };
    let admitted = (*fresh).admit().unwrap();
    assert!(Arc::ptr_eq(&admitted.original.command, &canonical));
    assert_eq!(admitted.request_bytes(), canonical.bytes());
    assert_eq!(admitted.original.deadline, deadline);
    assert!(Arc::ptr_eq(&admitted.original.live, &live));
    let work_item_codec::SavedResult::Create(result) = &admitted.revision().result else {
        panic!()
    };
    assert_eq!(result.item.id.0, "item");
    assert_eq!(result.readable_id.sequence, 2);
    assert_eq!(result.project.next_sequence, 3);
}

#[test]
fn retry_cannot_repair_changed_snapshot_owner_pending_identity_or_content() {
    for field in 0..8 {
        let original = original(draft());
        let mut fresh = refreshed(&original);
        match field {
            0 => fresh.epoch.runtime += 1,
            1 => fresh.deadline += std::time::Duration::from_millis(1),
            2 => fresh.live = RuntimeLiveness::new(),
            3 => {
                let Identity::Create { handle, .. } = &mut fresh.identity else {
                    panic!()
                };
                *handle = "other".into();
            }
            4 => {
                let Identity::Create { pending, .. } = &mut fresh.identity else {
                    panic!()
                };
                *pending += 1;
            }
            5 => {
                let Identity::Create { expires, .. } = &mut fresh.identity else {
                    panic!()
                };
                *expires += 1;
            }
            6 => fresh.observed.scope.actor = "other".into(),
            _ => {
                let Source::Retained(stored) = &mut fresh.source else {
                    panic!()
                };
                stored.bytes.insert(0, b' ');
            }
        }
        // The byte corruption is a trusted-boundary control, not ingress reachability.
        assert!(matches!(
            original.bind_preparation(Preparation::Ready(Box::new(fresh))),
            Err(NativeError::Integrity)
        ));
    }
}

#[test]
fn historical_creation_preserves_the_complete_available_immutable_binding() {
    for outcome in [
        Disposition::Replay,
        Disposition::Expired,
        Disposition::Cancelled,
    ] {
        let original = original(draft());
        let prep = historical(&original, outcome);
        assert!(matches!(
            original.bind_preparation(prep),
            Ok(Preparation::Resolved(_))
        ));
    }
    for field in 0..5 {
        let original = original(draft());
        let Preparation::Resolved(mut resolved) = historical(&original, Disposition::Replay) else {
            panic!()
        };
        match field {
            0 => resolved.data.pending = Some(18),
            1 => resolved.data.pending_expires = Some(86400001),
            2 => resolved.data.target = Some("other".into()),
            3 => resolved.data.token = Some("other".into()),
            _ => resolved.data.codec = Some(2),
        };
        assert!(matches!(
            original.bind_preparation(Preparation::Resolved(resolved)),
            Err(NativeError::Integrity)
        ));
    }
}

#[test]
fn missing_creation_is_terminal_without_a_replacement_identity() {
    let original = original(draft());
    let prep = Preparation::Resolved(Resolved {
        outcome: Disposition::Missing,
        data: Metadata {
            operation: None,
            target: None,
            codec: None,
            token: None,
            pending: None,
            pending_expires: None,
            origin: None,
            end: None,
        },
    });
    assert!(matches!(
        original.bind_preparation(prep),
        Ok(Preparation::Resolved(Resolved {
            outcome: Disposition::Missing,
            ..
        }))
    ));
}

#[test]
fn retry_never_rebases_original_expected_item_version() {
    let original = original(edit_draft());
    let accepted = refreshed(&original).admit().unwrap();
    let mut fresh = refreshed(&original);
    fresh.observed.project = super::super::project_from(&accepted.revision().result).clone();
    fresh.observed.state_version = accepted.revision().project_state_version;
    fresh.observed.state_operation = 3;
    let readable = fresh.observed.existing.as_ref().unwrap().readable.clone();
    fresh.observed.existing = Some(Previous {
        operation: 3,
        revision: accepted.revision().clone(),
        readable,
    });
    drop(accepted);
    let prep = original
        .bind_preparation(Preparation::Ready(Box::new(fresh)))
        .unwrap_or_else(|_| panic!("same identity must bind"));
    let Preparation::Ready(fresh) = prep else {
        panic!()
    };
    assert!(matches!(
        (*fresh).admit(),
        Err(PreparationError::Domain(
            task_contract::item_mutation::ItemMutationError::ItemVersionConflict {
                current_version: 2
            }
        ))
    ));
}

#[test]
fn historical_edit_codec_conflict_does_not_become_replay() {
    for outcome in [
        Disposition::Conflict,
        Disposition::Replay,
        Disposition::Expired,
    ] {
        let original = original(edit_draft());
        let Preparation::Resolved(mut resolved) = historical(&original, outcome) else {
            panic!()
        };
        resolved.data.codec = Some(2);
        let result = original.bind_preparation(Preparation::Resolved(resolved));
        if outcome == Disposition::Conflict {
            assert!(matches!(
                result,
                Ok(Preparation::Resolved(Resolved {
                    outcome: Disposition::Conflict,
                    ..
                }))
            ));
        } else {
            assert!(matches!(result, Err(NativeError::Integrity)));
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn item_cpu_admission_retains_original_allowance_through_published_or_dropped_result() {
    use crate::password::worker::{Worker, WorkerError};
    for abandon in [false, true] {
        let draft = draft();
        let deadline = draft.deadline;
        let mut worker = Worker::new(Arc::clone(&draft.live));
        let capacity = Arc::new(tokio::sync::Semaphore::new(1));
        let permit = Arc::new(capacity.clone().try_acquire_owned().unwrap());
        let original = Arc::downgrade(&permit);
        let gate = worker.hold_next_thread_exit_for_test();
        let pending = worker
            .owned_preparation(deadline, permit, move || {
                crate::coordinator::item_write::Prepared::admit(draft)
            })
            .unwrap();
        tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), gate.published)
            .await
            .unwrap()
            .unwrap();
        if abandon {
            drop(pending)
        } else {
            let result = pending.result().await.unwrap().unwrap();
            assert!(result.is_ok(), "valid item failed nested admission");
            drop(result);
        }
        assert!(
            original.upgrade().is_some(),
            "CPU owner replaced or released the original allowance"
        );
        assert!(!worker.finished());
        assert_eq!(worker.join_completed(), Err(WorkerError::Busy));
        assert!(capacity.clone().try_acquire_owned().is_err());
        gate.release.send(()).unwrap();
        tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), async {
            while !worker.finished() {
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            capacity.available_permits(),
            0,
            "thread exit alone released unjoined allowance"
        );
        worker.join_completed().unwrap();
        assert!(original.upgrade().is_none());
        assert_eq!(capacity.available_permits(), 1);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn item_domain_rejection_stays_distinct_from_worker_execution_failure() {
    use crate::coordinator::item_write::AdmissionFailure;
    use crate::password::worker::Worker;
    let mut draft = draft();
    let Source::Retained(stored) = &mut draft.source else {
        panic!()
    };
    let Request::Create { command, .. } = &mut stored.request else {
        panic!()
    };
    command.expected_configuration_revision += 1;
    stored.bytes = work_item_codec::admit_request(&stored.request)
        .unwrap()
        .as_bytes()
        .to_vec();
    let deadline = draft.deadline;
    let mut worker = Worker::new(Arc::clone(&draft.live));
    let capacity = Arc::new(tokio::sync::Semaphore::new(1));
    let permit = Arc::new(capacity.clone().try_acquire_owned().unwrap());
    let pending = worker
        .owned_preparation(deadline, permit, move || {
            crate::coordinator::item_write::Prepared::admit(draft)
        })
        .unwrap();
    let outcome = pending.result().await.unwrap().unwrap();
    assert!(matches!(
        outcome,
        Err(AdmissionFailure::Item(PreparationError::Domain(
            task_contract::item_mutation::ItemMutationError::ConfigurationConflict {
                current_revision: 1
            }
        )))
    ));
    tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), async {
        while !worker.finished() {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
    worker.join_completed().unwrap();
    assert_eq!(capacity.available_permits(), 1);
}
