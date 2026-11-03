//! Actual HTTP/TLS requests queued behind controlled transaction boundaries.
//! This is one authority/serving connection, not separate database writers.
use super::*;
use crate::lifecycle::probe::Point;

fn spawn(
    handle: &Handle,
    browser: &Browser,
    path: &'static str,
    body: Value,
) -> tokio::task::JoinHandle<(u16, Value)> {
    let handle = handle.clone();
    let browser = browser.clone();
    tokio::spawn(async move { project(&handle, &browser, path, body).await })
}
async fn attempts(handle: &Handle, expected: usize) {
    tokio::time::timeout(WORK_DEADLINE, async {
        while handle.live.probe.snapshot().native_mutex_attempts < expected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
pub(super) async fn execution_wins(
    handle: &Handle,
    first: &Browser,
    second: &Browser,
    entry: &Value,
) -> Value {
    let id = entry["request_id"].as_str().unwrap().parse().unwrap();
    let gate = handle.live.probe.hold_transaction(id, Point::ExecuteLookup);
    let winner = spawn(handle, second, "execute", execute(entry));
    tokio::time::timeout(WORK_DEADLINE, gate.arrived)
        .await
        .unwrap()
        .unwrap();
    // The first transaction has acquired the actor fence and read prepared.
    let before = handle.live.probe.snapshot().native_mutex_attempts;
    let equal = spawn(handle, first, "execute", execute(entry));
    attempts(handle, before + 1).await;
    let mut changed = execute(entry);
    changed["name"] = json!("a different concurrent form");
    let changed = spawn(handle, first, "execute", changed);
    attempts(handle, before + 2).await;
    let cancel = spawn(
        handle,
        first,
        "cancel",
        json!({"request_id":entry["request_id"]}),
    );
    attempts(handle, before + 3).await;
    assert!(!winner.is_finished());
    gate.release.send(()).unwrap();
    let (status, created) = winner.await.unwrap();
    assert_eq!(status, 201);
    let (status, replayed) = equal.await.unwrap();
    assert_eq!(status, 200);
    assert_eq!(replayed["result"], created["result"]);
    assert_eq!(
        replayed["entry"]["project_id"],
        created["entry"]["project_id"]
    );
    assert_eq!(changed.await.unwrap().0, 409);
    let (status, cancelled) = cancel.await.unwrap();
    assert_eq!(status, 200);
    assert_eq!(cancelled["entry"]["state"], "completed");
    created
}
pub(super) async fn cancellation_wins(handle: &Handle, browser: &Browser, entry: &Value) -> Value {
    let id = entry["request_id"].as_str().unwrap().parse().unwrap();
    let gate = handle.live.probe.hold_transaction(id, Point::CancelCommit);
    let cancel = spawn(
        handle,
        browser,
        "cancel",
        json!({"request_id":entry["request_id"]}),
    );
    tokio::time::timeout(WORK_DEADLINE, gate.arrived)
        .await
        .unwrap()
        .unwrap();
    let before = handle.live.probe.snapshot().native_mutex_attempts;
    let execute = spawn(handle, browser, "execute", execute(entry));
    attempts(handle, before + 1).await;
    assert!(!cancel.is_finished());
    gate.release.send(()).unwrap();
    let (status, cancelled) = cancel.await.unwrap();
    assert_eq!(status, 200);
    assert_eq!(cancelled["entry"]["state"], "cancelled");
    assert_eq!(execute.await.unwrap().0, 410);
    cancelled
}

fn sealed(browser: &Browser, entry: &Value, targets: [String; 3]) -> Operation {
    let input = crate::native_ingress::decode(
        crate::native_ingress::Route::Execute,
        &serde_json::to_vec(&execute(entry)).unwrap(),
    )
    .unwrap();
    let crate::native_ingress::Input::Execute {
        request_id,
        token,
        form,
    } = input
    else {
        unreachable!()
    };
    Operation::Project {
        credential: admission::credential(browser),
        command: crate::project::Command::Execute {
            request_id,
            candidates: crate::project::Candidates::new(form, token, targets).unwrap(),
        },
    }
}
fn delivered(response: &Response, expected: u16) -> Value {
    let Output::Project(delivery) = response.result().unwrap() else {
        panic!("expected native reply")
    };
    let material = delivery.material().unwrap();
    assert_eq!(material.status, expected);
    serde_json::from_slice(&material.body).unwrap()
}
pub(super) async fn collision_reentry(handle: &Handle, browser: &Browser, existing: &Value) {
    let (status, registered) = project(
        handle,
        browser,
        "register",
        json!({"name":"Collision ordering","prefix":"COL","estimate_unit":"hours"}),
    )
    .await;
    assert_eq!(status, 201);
    let entry = &registered["entry"];
    let id = entry["request_id"].as_str().unwrap().parse().unwrap();
    let existing = existing.as_str().unwrap().to_owned();
    let first_targets = [
        existing.clone(),
        "fixture-first-fallback".into(),
        "fixture-first-last".into(),
    ];
    let winner_targets = [
        existing.clone(),
        "fixture-winner-fallback".into(),
        "fixture-winner-last".into(),
    ];
    assert!(first_targets[1..]
        .iter()
        .chain(winner_targets[1..].iter())
        .all(|id| id != &existing));
    let first = sealed(browser, entry, first_targets);
    let winner = sealed(browser, entry, winner_targets);
    let gate = handle.live.probe.hold_transaction(id, Point::Rollback);
    let first = handle.submit(first, deadline()).unwrap();
    tokio::time::timeout(WORK_DEADLINE, gate.arrived)
        .await
        .unwrap()
        .unwrap();
    // The first candidate collided and fully rolled back; its serving mutex
    // is released. A second sealed command collides too, then creates its
    // next candidate before the first request re-enters the database.
    let winner = request(handle, winner).await;
    let created = delivered(&winner, 201);
    assert_eq!(created["entry"]["project_id"], "fixture-winner-fallback");
    drop(winner);
    gate.release.send(()).unwrap();
    let first = tokio::time::timeout(WORK_DEADLINE, first)
        .await
        .unwrap()
        .unwrap();
    let replayed = delivered(&first, 200);
    assert_eq!(replayed["result"], created["result"]);
    assert_eq!(
        replayed["entry"]["project_id"],
        created["entry"]["project_id"]
    );
    // Targets are test-supplied sealed candidates; this schedule does not
    // qualify their random generation or represent a second authority.
}

pub(super) async fn lost_registration_and_cancellation(
    endpoint: &crate::postgres::Endpoint,
    installation: &str,
    attempt: &str,
) {
    let (handle, owner) = runtime(endpoint, installation, format!("{attempt}-register-loss")).await;
    let engine = Arc::downgrade(owner.engine());
    let running = tokio::spawn(owner.run());
    let browser = login(&handle).await;
    engine
        .upgrade()
        .unwrap()
        .database
        .lock()
        .await
        .lose_next_confirmation_for_test();
    let (status, unknown) = project(
        &handle,
        &browser,
        "register",
        json!({"name":"Uncertain registration","prefix":"UR","estimate_unit":"hours"}),
    )
    .await;
    assert_eq!(status, 503);
    assert_eq!(unknown["error"], "outcome_unknown");
    running.await.unwrap().unwrap();

    let (handle, owner) = runtime(endpoint, installation, format!("{attempt}-cancel-loss")).await;
    let engine = Arc::downgrade(owner.engine());
    let running = tokio::spawn(owner.run());
    let browser = login(&handle).await;
    let (status, listed) = project(&handle, &browser, "list", json!({"before":null})).await;
    assert_eq!(status, 200);
    let matches: Vec<_> = listed["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["form"]["name"] == "Uncertain registration")
        .collect();
    assert_eq!(matches.len(), 1);
    let entry = matches[0].clone();
    assert_eq!(
        entry["form"],
        json!({"name":"Uncertain registration","prefix":"UR","estimate_unit":"hours"})
    );
    assert_eq!(entry["state"], "prepared");
    engine
        .upgrade()
        .unwrap()
        .database
        .lock()
        .await
        .lose_next_confirmation_for_test();
    let (status, unknown) = project(
        &handle,
        &browser,
        "cancel",
        json!({"request_id":entry["request_id"]}),
    )
    .await;
    assert_eq!(status, 503);
    assert_eq!(unknown["error"], "outcome_unknown");
    running.await.unwrap().unwrap();

    let (handle, owner) = runtime(
        endpoint,
        installation,
        format!("{attempt}-cancel-reconcile"),
    )
    .await;
    let running = tokio::spawn(owner.run());
    let browser = login(&handle).await;
    let (status, opened) = project(
        &handle,
        &browser,
        "open",
        json!({"request_id":entry["request_id"]}),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(opened["entry"]["state"], "cancelled");
    assert_eq!(
        project(&handle, &browser, "execute", execute(&entry))
            .await
            .0,
        410
    );
    handle.stop();
    running.await.unwrap().unwrap();
}
