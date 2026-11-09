//! Known administrative row blockers qualify the shared sampling query only.
use super::*;
use observe::{sample_once, Sample};

fn matches(sample: &Sample, blocker: i32, xid: &str, fingerprint: &str) -> bool {
    sample
        .backends
        .iter()
        .filter(|backend| {
            backend.state.as_deref() == Some("active")
                && backend.wait_type.as_deref() == Some("Lock")
                && backend.blocking_pids.contains(&blocker)
                && backend.backend_start.is_some()
                && backend.transaction_start.is_some()
                && backend
                    .query
                    .as_deref()
                    .is_some_and(|query| query.contains(fingerprint))
                && serde_json::from_str::<Vec<Value>>(&backend.ungranted_locks_json).is_ok_and(
                    |locks| {
                        locks.iter().any(|lock| {
                            lock["locktype"] == "transactionid" && lock["transaction_id"] == xid
                        })
                    },
                )
        })
        .count()
        == 1
}

pub(super) async fn phase(
    handle: &Handle,
    client: &Client,
    admin: &Admin,
    blocker: &Admin,
    setup: &mut Setup,
    cell: Cell,
) -> PhaseReport {
    assert_eq!(cell.index, 15);
    evidence::settled(handle).await;
    handle.live.probe.capacity.start(2, 128);
    let origin = handle.live.probe.capacity.phase_origin().unwrap();
    let mut attempts = Vec::new();
    let mut controls = Vec::new();
    let mut samples = Vec::new();
    let mut errors = Vec::new();
    for (ordinal, (target, fingerprint)) in [
        ("actor", " FROM kehila_auth.fence("),
        ("project", " FROM kehila_auth.save_item_edit("),
    ]
    .into_iter()
    .enumerate()
    {
        blocker
            .client()
            .batch_execute("BEGIN; SET LOCAL ROLE kehila_owner")
            .await
            .unwrap();
        let project = &setup.slots[0].project;
        let held = if target == "actor" {
            blocker
                .client()
                .query_one(
                    "SELECT installation_id::text FROM kehila_auth.installation \
                WHERE singleton AND installation_id='installation' FOR SHARE",
                    &[],
                )
                .await
                .unwrap();
            blocker
                .client()
                .query_one(
                    "SELECT actor_id::text,ctid::text FROM kehila.actors \
                WHERE actor_id='owner' FOR NO KEY UPDATE",
                    &[],
                )
                .await
                .unwrap()
        } else {
            blocker
                .client()
                .query_one(
                    "SELECT project_id::text,ctid::text FROM kehila.projects \
                WHERE project_id=$1::text::kehila.ident FOR NO KEY UPDATE",
                    &[project],
                )
                .await
                .unwrap()
        };
        let pid: i32 = blocker
            .client()
            .query_one("SELECT pg_backend_pid()", &[])
            .await
            .unwrap()
            .get(0);
        // SET LOCAL ROLE intentionally narrows the blocker. Read protected
        // backend metadata through the independent privileged observer.
        let identity = admin
            .client()
            .query_one(
                "SELECT pid,backend_start::text,xact_start::text, \
            backend_xid::text FROM pg_stat_activity WHERE pid=$1",
                &[&pid],
            )
            .await
            .unwrap();
        let xid: String = identity.get(3);
        let command =
            setup.slots[0].command(cell, Phase::Calibration, ordinal, &setup.texts[ordinal]);
        let dispatch_ns = nanos(origin);
        let observe_and_release = async {
            let mut recorded = Vec::new();
            // Preserve half of the original HTTP deadline for rollback/response.
            let deadline = tokio::time::Instant::now() + crate::CONNECTION_DEADLINE / 2;
            let observed = tokio::time::timeout_at(deadline, async {
                let mut timer = tokio::time::interval(Duration::from_millis(5));
                timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    timer.tick().await;
                    let sample = sample_once(admin, origin)
                        .await
                        .map_err(|error| error.to_string())?;
                    let decisive = matches(&sample, pid, &xid, fingerprint);
                    recorded.push(sample);
                    if decisive {
                        return Ok::<(), String>(());
                    }
                    if recorded.len() >= 5_000 {
                        return Err("calibration sample evidence overflow".into());
                    }
                }
            })
            .await;
            // No assertion may precede this release. Parent also retains the
            // connection and rolls it back if this child task itself panics.
            let release_ns = nanos(origin);
            let rollback = blocker.client().batch_execute("ROLLBACK").await;
            (recorded, observed, release_ns, rollback)
        };
        let (attempt, (mut recorded, observed, release_ns, rollback)) = tokio::join!(
            setup.slots[0].send(cell, command, client, &setup.browsers[ordinal % 4]),
            observe_and_release
        );
        let response_ns = nanos(origin);
        let observed_ok = matches!(observed, Ok(Ok(())));
        if !observed_ok {
            errors.push(format!("{target} known blocker not sampled: {observed:?}"));
        }
        if let Err(error) = rollback {
            errors.push(format!("{target} blocker rollback failed: {error}"));
        }
        if attempt.outcome != "success" || attempt.uncertain || attempt.integrity_error.is_some() {
            errors.push(format!("{target} original edit failed after release"));
        }
        let after = sample_once(admin, origin).await;
        let edge_gone = after.as_ref().is_ok_and(|sample| {
            sample
                .backends
                .iter()
                .all(|backend| !backend.blocking_pids.contains(&pid))
        });
        if !edge_gone {
            errors.push(format!("{target} blocking edge not released"));
        }
        // Inspect from the other connection so this check does not start a new
        // statement transaction on the blocker being inspected.
        let blocker_idle = admin
            .client()
            .query_one(
                "SELECT state='idle' AND xact_start IS NULL \
            FROM pg_stat_activity WHERE pid=$1 AND backend_start::text=$2",
                &[&pid, &identity.get::<_, String>(1)],
            )
            .await
            .is_ok_and(|row| row.get::<_, bool>(0));
        if !blocker_idle {
            errors.push(format!("{target} blocker transaction remains"));
        }
        controls.push(json!({"target":target,"target_key":held.get::<_,String>(0),"target_ctid":held.get::<_,String>(1),
            "blocker_pid":pid,"blocker_backend_start":identity.get::<_,String>(1),
            "blocker_transaction_start":identity.get::<_,String>(2),"blocker_transaction_id":xid,
            "query_fingerprint":fingerprint,"dispatch_ns":dispatch_ns,"release_ns":release_ns,
            "response_ns":response_ns,"known_blocker_observed":observed_ok,"edge_gone":edge_gone,
            "blocker_idle":blocker_idle,"sample_count":recorded.len(),"after_release":after.ok()}));
        samples.append(&mut recorded);
        attempts.push(attempt);
        if !errors.is_empty() {
            break;
        }
    }
    let request_window_ns = nanos(origin);
    evidence::settled(handle).await;
    let rows = match handle.live.probe.capacity.finish() {
        Ok(rows) => {
            errors.extend(observe::validate(&rows, &attempts));
            Some(rows)
        }
        Err(error) => {
            errors.push(error.into());
            None
        }
    };
    PhaseReport {
        phase: Phase::Calibration,
        configured_concurrency: 1,
        planned_requests: 2,
        request_window_ns,
        checkpoint_ns: nanos(origin) - request_window_ns,
        attempts,
        recoveries: Vec::new(),
        rows,
        sampling: Some(Sampling {
            interval_ns: 5_000_000,
            sample_limit: 10_001,
            samples,
            error: None,
        }),
        errors,
        controls,
    }
}

#[test]
fn calibration_requires_known_transaction_edge_query_and_backend_identity() {
    let mut sample = Sample {
        started_ns: 0,
        finished_ns: 1,
        backends: vec![observe::Backend {
            pid: 1,
            state: Some("active".into()),
            wait_type: Some("Lock".into()),
            wait_event: Some("transactionid".into()),
            blocking_pids: vec![2],
            query: Some("SELECT * FROM kehila_auth.fence(...)".into()),
            backend_start: Some("start".into()),
            transaction_start: Some("transaction".into()),
            ungranted_locks_json: r#"[{"locktype":"transactionid","transaction_id":"3"}]"#.into(),
        }],
    };
    assert!(matches(&sample, 2, "3", " FROM kehila_auth.fence("));
    assert!(!matches(&sample, 2, "4", " FROM kehila_auth.fence("));
    assert!(!matches(
        &sample,
        2,
        "3",
        " FROM kehila_auth.save_item_edit("
    ));
    sample.backends[0].backend_start = None;
    assert!(!matches(&sample, 2, "3", " FROM kehila_auth.fence("));
}
