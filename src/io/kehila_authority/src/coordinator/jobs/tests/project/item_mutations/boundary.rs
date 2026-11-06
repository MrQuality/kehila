//! Disposable DB-clock and post-handoff eligibility controls, not host time.
use super::*;
use crate::lifecycle::probe::{ItemReplayObservation as Observation, Point};
use crate::postgres::Endpoint;
const INSTALL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/integration/native_clock_install.sql"
));
const RESTORE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/integration/native_clock_restore.sql"
));
async fn at(admin: &Admin, value: u64) {
    assert_eq!(
        admin
            .client()
            .execute(
                "UPDATE kehila_native_fixture.clock SET value=$1::text::numeric WHERE singleton",
                &[&value.to_string()]
            )
            .await
            .unwrap(),
        1
    );
}
type Reply = (u16, Value, Option<String>);
type Exchanges = Arc<std::sync::Mutex<tokio::task::JoinSet<()>>>;
struct Held {
    response: tokio::sync::oneshot::Receiver<Reply>,
    release: std::sync::mpsc::Sender<()>,
    started: Instant,
}
impl Held {
    async fn finish(self) -> (u16, Value) {
        self.release.send(()).unwrap();
        let (status, value, _) = tokio::time::timeout(WORK_DEADLINE, self.response)
            .await
            .unwrap()
            .unwrap();
        assert!(
            self.started.elapsed() < WORK_DEADLINE,
            "control exhausted original request window"
        );
        (status, value)
    }
}
async fn prepared(
    handle: &Handle,
    browser: &Browser,
    admin: &Admin,
    edit: Value,
    exchanges: &Exchanges,
) -> Held {
    handle.live.probe.take_item_replay();
    let loaded = handle
        .live
        .probe
        .hold_transaction(0, Point::ItemHistoryLoaded);
    let jobs = handle.clone();
    let browser = browser.clone();
    let started = Instant::now();
    let (sent, mut response) = tokio::sync::oneshot::channel();
    // The parent owns the task through panic/receiver timeout. Dropping this
    // receiver can discard a reply, but cannot detach the exchange's owner.
    exchanges.lock().unwrap().spawn(async move {
        let reply = call(&jobs, Some(&browser), "/items/reconcile", Some(edit)).await;
        let _ = sent.send(reply);
    });
    tokio::time::timeout(WORK_DEADLINE, loaded.arrived)
        .await
        .unwrap()
        .unwrap();
    let engine = handle.engine.upgrade().unwrap();
    let thread = engine.hash.lock().await.hold_next_thread_exit_for_test();
    loaded.release.send(()).unwrap();
    tokio::time::timeout(WORK_DEADLINE, thread.published)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        handle.live.probe.take_item_replay(),
        Some(Observation::Prepared),
        "worker did not publish successful complete response preparation"
    );
    assert!(
        matches!(
            response.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ),
        "historical response escaped before worker join"
    );
    assert_eq!(handle.capacity.available_permits(), CAPACITY - 1);
    let database = tokio::time::timeout(WORK_DEADLINE, engine.database.lock())
        .await
        .unwrap();
    drop(database);
    evidence::fences_released(admin).await;
    Held {
        response,
        release: thread.release,
        started,
    }
}
async fn submitted(
    handle: &Handle,
    browser: &Browser,
    route: crate::item_mutation_ingress::Route,
    body: &Value,
    deadline: Instant,
) -> tokio::sync::oneshot::Receiver<Response> {
    let input =
        crate::item_mutation_ingress::decode(route, &serde_json::to_vec(body).unwrap()).unwrap();
    let prepared = handle
        .prepare_item_mutation(admission::credential(browser), input, deadline)
        .unwrap();
    let prepared = tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), prepared)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    handle.submit_prepared(prepared).unwrap()
}
async fn owned_response(
    handle: &Handle,
    browser: &Browser,
    edit: &Value,
    deadline: Instant,
) -> Response {
    let reply = submitted(
        handle,
        browser,
        crate::item_mutation_ingress::Route::ReconcileEdit,
        edit,
        deadline,
    )
    .await;
    tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), reply)
        .await
        .unwrap()
        .unwrap()
}
fn delivery(response: &Response) -> &NativeDelivery {
    let Output::Item(value) = response.result().unwrap() else {
        panic!("historical response lost item delivery")
    };
    value
}
async fn cases(
    handle: &Handle,
    browser: &Browser,
    second: &Browser,
    admin: &Admin,
    edit: &Value,
    expected: &Value,
    exchanges: &Exchanges,
) {
    let row=admin.client().query_one("SELECT replay_origin_ms::text,replay_deadline_ms::text FROM kehila.operations WHERE command_family='item_edit' AND target_project_id=$1::text::kehila.ident AND target_item_id=$2::text::kehila.ident AND token=$3::text::kehila.operation_token",
        &[&edit["project_id"].as_str().unwrap(),&edit["item_id"].as_str().unwrap(),&edit["operation_id"].as_str().unwrap()]).await.unwrap();
    let origin = row.get::<_, String>(0).parse::<u64>().unwrap();
    let end = row.get::<_, String>(1).parse::<u64>().unwrap();
    drop(row);
    assert!(origin > 0);
    assert_eq!(end - origin, task_contract::operation::REPLAY_PERIOD_MS);
    at(admin, origin + 1).await;
    let held = prepared(handle, browser, admin, edit.clone(), exchanges).await;
    at(admin, end).await;
    let (status, value) = held.finish().await;
    assert_eq!(status, 410);
    assert_eq!(value, json!({"error":"replay_expired"}));
    assert_eq!(
        handle.live.probe.take_item_replay(),
        Some(Observation::Expired)
    );
    assert!(handle.live.is_live());
    // The initial lookup was eligible; only the final sample sees before-origin.
    at(admin, origin + 1).await;
    let held = prepared(handle, browser, admin, edit.clone(), exchanges).await;
    at(admin, origin - 1).await;
    let (status, value) = held.finish().await;
    assert_eq!(status, 503);
    assert!(value.get("result").is_none());
    assert_eq!(
        handle.live.probe.take_item_replay(),
        Some(Observation::BeforeOrigin)
    );
    assert!(handle.live.is_live());
    at(admin, origin + 1).await;
    let current = request(
        handle,
        browser,
        "/items/read",
        json!({"project_id":edit["project_id"],"item_id":edit["item_id"]}),
        200,
    )
    .await;
    let SavedResult::Edit(current) = decoded(&current) else {
        panic!()
    };
    assert_eq!(current.item.version, 3);
    // The owned response has crossed synchronous job->transport handoff.
    // A later DB-clock jump cannot retract its already completed authorization.
    at(admin, end - 1).await;
    handle.live.probe.take_item_replay();
    let mut response = owned_response(handle, browser, edit, Instant::now() + WORK_DEADLINE).await;
    assert_eq!(
        handle.live.probe.take_item_replay(),
        Some(Observation::Authorized)
    );
    let before = delivery(&response).material().unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(before.body.as_slice()).unwrap(),
        json!({"outcome":"replayed","result":expected})
    );
    at(admin, end + 1).await;
    let after = delivery(&response).material().unwrap();
    assert!(Arc::ptr_eq(&before.body, &after.body));
    assert_eq!(before.body.as_slice(), after.body.as_slice());
    response.confirm_exchange().unwrap();
    drop(before);
    drop(after);
    drop(response);
    // Continuing session eligibility is independent of that one DB-clock approval.
    at(admin, origin + 1).await;
    let mut response = owned_response(handle, second, edit, Instant::now() + WORK_DEADLINE).await;
    delivery(&response).material().unwrap();
    let engine = handle.engine.upgrade().unwrap();
    engine
        .candidate(&admission::credential(second))
        .unwrap()
        .retire();
    drop(engine);
    assert!(matches!(
        delivery(&response).material(),
        Err(Error::Unauthorized)
    ));
    assert!(response.confirm_exchange().is_err());
    drop(response);
    // The original request deadline is never renewed by replay authorization.
    let deadline = Instant::now() + WORK_DEADLINE;
    let mut response = owned_response(handle, browser, edit, deadline).await;
    delivery(&response).material().unwrap();
    tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
    assert!(matches!(response.result(), Err(Error::Unavailable)));
    assert!(response.confirm_exchange().is_err());
    drop(response);
    assert!(handle.live.is_live());
}
async fn stop_runtime(
    handle: &Handle,
    running: &mut tokio::task::JoinHandle<Result<(), Error>>,
    expected_failure: bool,
) -> (bool, Result<Result<(), Error>, tokio::task::JoinError>) {
    handle.stop();
    let observed = tokio::time::timeout(Duration::from_secs(10), &mut *running)
        .await
        .expect("runtime cleanup unobserved; clock must remain isolated for fixture disposal");
    let valid = matches!(
        (&observed, expected_failure),
        (Ok(Ok(())), false) | (Ok(Err(Error::Unavailable)), true)
    );
    (valid, observed)
}
pub(super) struct ReplayInputs {
    pub(super) edit: Value,
    pub(super) clear: Value,
    pub(super) creation: Value,
    pub(super) expected: Value,
}
pub(super) async fn exercise(
    handle: &Handle,
    browser: &Browser,
    admin: &Admin,
    endpoint: &Endpoint,
    inputs: ReplayInputs,
    running: &mut tokio::task::JoinHandle<Result<(), Error>>,
) {
    let second = login(handle).await;
    let before = evidence::snapshot(admin).await;
    let case_admin = Admin::connect(endpoint).await;
    let maintenance = Arc::new(std::sync::Mutex::new(vec![
        Admin::connect(endpoint).await,
        Admin::connect(endpoint).await,
    ]));
    let installed = admin.client().batch_execute(INSTALL).await;
    if installed.is_err() {
        case_admin.close().await;
        let unused = std::mem::take(&mut *maintenance.lock().unwrap());
        for owner in unused {
            owner.close().await;
        }
        let (stopped, root_result) = stop_runtime(handle, running, false).await;
        evidence::runtime_released(admin).await;
        let restored = admin.client().batch_execute(RESTORE).await;
        panic!("disposable clock installation failed; runtime_ok={stopped}, root={root_result:?}, restoration_ok={}",restored.is_ok());
    }
    let jobs = handle.clone();
    let browser = browser.clone();
    let exchanges: Exchanges = Arc::new(std::sync::Mutex::new(tokio::task::JoinSet::new()));
    let registered = Arc::clone(&exchanges);
    let owners = Arc::clone(&maintenance);
    // Observe the assertion-prone child, retaining all spawned exchanges in the
    // parent. Sender drops release its gates even when that child panics.
    let outcome = tokio::spawn(async move {
        cases(
            &jobs,
            &browser,
            &second,
            &case_admin,
            &inputs.edit,
            &inputs.expected,
            &registered,
        )
        .await;
        retirement::cases(&jobs, &browser, &case_admin, &inputs, &owners, &registered).await;
        rollback::case(&jobs, &browser, &case_admin, &inputs.creation).await;
        case_admin.close().await;
        true
    })
    .await;
    let unused = std::mem::take(&mut *maintenance.lock().unwrap());
    for owner in unused {
        owner.close().await;
    }
    let permits = tokio::time::timeout(
        WORK_DEADLINE,
        handle.capacity.clone().acquire_many_owned(CAPACITY as u32),
    )
    .await;
    let settled = matches!(&permits, Ok(Ok(_)));
    drop(permits);
    // These controls run last. Stop and observe the runtime owner before any
    // clock restoration, including assertion and exchange failure paths.
    let expected_failure = matches!(&outcome, Ok(true));
    let (runtime_ok, root_result) = stop_runtime(handle, running, expected_failure).await;
    let mutex = Arc::try_unwrap(exchanges)
        .unwrap_or_else(|_| panic!("exchange registry still has an unobserved owner"));
    let (mut exchanges, poisoned) = match mutex.into_inner() {
        Ok(value) => (value, false),
        Err(error) => (error.into_inner(), true),
    };
    let mut exchange_failed = poisoned;
    let joined = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(result) = exchanges.join_next().await {
            exchange_failed |= result.is_err();
        }
    })
    .await;
    if joined.is_err() {
        exchange_failed = true;
        exchanges.abort_all();
        let drained = tokio::time::timeout(Duration::from_secs(5), async {
            while exchanges.join_next().await.is_some() {}
        })
        .await;
        assert!(
            drained.is_ok(),
            "exchange cleanup unobserved; clock must remain isolated for fixture disposal"
        );
    }
    evidence::runtime_released(admin).await;
    let restored = admin.client().batch_execute(RESTORE).await;
    assert!(restored.is_ok(),"clock restoration failed; cases_ok={}, settled={settled}, runtime_ok={runtime_ok}, root={root_result:?}, exchange_failed={exchange_failed}",outcome.is_ok());
    assert!(outcome.is_ok()&&settled&&runtime_ok&&!exchange_failed,
        "replay boundary failed; cases_ok={}, settled={settled}, runtime_ok={runtime_ok}, root={root_result:?}, exchange_failed={exchange_failed}",outcome.is_ok());
    assert_eq!(
        retirement::normalize(evidence::snapshot(admin).await),
        retirement::expected(before),
        "retention changed unexpected durable artifacts"
    );
}

mod controls;
mod retirement;
mod rollback;
