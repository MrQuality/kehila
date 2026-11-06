//! Controlled competing requests through the single serving authority.
use super::*;
use crate::lifecycle::probe::Point;
type Reply = (u16, Value, Option<String>);
type Exchanges = Arc<std::sync::Mutex<tokio::task::JoinSet<()>>>;
pub(super) struct Commands {
    pub(super) milestone: Value,
    pub(super) edit: Value,
    pub(super) clear: Value,
    pub(super) cancel_form: Value,
}
fn exchange(
    handle: &Handle,
    browser: &Browser,
    path: &'static str,
    body: Value,
    exchanges: &Exchanges,
) -> tokio::sync::oneshot::Receiver<Reply> {
    let jobs = handle.clone();
    let browser = browser.clone();
    let (sent, response) = tokio::sync::oneshot::channel();
    exchanges.lock().unwrap().spawn(async move {
        let reply = call(&jobs, Some(&browser), path, Some(body)).await;
        let _ = sent.send(reply);
    });
    response
}
async fn response(reply: tokio::sync::oneshot::Receiver<Reply>, expected: u16) -> Value {
    let (status, value, _) = tokio::time::timeout(WORK_DEADLINE, reply)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status, expected);
    value
}
async fn attempts(handle: &Handle, expected: usize) {
    tokio::time::timeout(WORK_DEADLINE, async {
        while handle.live.probe.snapshot().native_mutex_attempts < expected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        handle.live.probe.snapshot().native_mutex_attempts,
        expected,
        "another source advanced the intended blocked-entry schedule"
    );
}
struct Identity {
    id: i64,
    token: String,
    target: String,
    codec: i64,
    digest: Vec<u8>,
    bytes: Vec<u8>,
}
async fn identity(admin: &Admin, locator: &Value) -> Identity {
    let row=admin.client().query_one("SELECT request_id,token::text,target_item_id::text,request_codec_version,request_sha256,request_bytes FROM kehila_auth.pending_items WHERE project_id=$1::text::kehila.ident AND intent_handle=$2::text::kehila_auth.intent_key AND state='prepared'",
        &[&locator["project_id"].as_str().unwrap(),&locator["intent_handle"].as_str().unwrap()]).await.unwrap();
    Identity {
        id: row.get(0),
        token: row.get(1),
        target: row.get(2),
        codec: row.get(3),
        digest: row.get(4),
        bytes: row.get(5),
    }
}
async fn creation_identity(admin: &Admin, locator: &Value, original: &Identity, saved: &Value) {
    let row=admin.client().query_one("SELECT p.request_id,p.token::text,p.target_item_id::text,p.request_codec_version,p.request_sha256,p.request_bytes,o.token::text,o.target_item_id::text,o.request_codec_version,o.request_sha256,b.request_bytes,(SELECT count(*) FROM kehila.items i WHERE i.creation_request_id=p.request_id),(SELECT count(*) FROM kehila.item_revisions r WHERE r.operation_row_id=o.operation_row_id) FROM kehila_auth.pending_items p JOIN kehila.operations o USING(operation_row_id) JOIN kehila.operation_payloads b USING(operation_row_id) WHERE p.project_id=$1::text::kehila.ident AND p.intent_handle=$2::text::kehila_auth.intent_key AND p.state='completed' AND o.command_family='item_create'",
        &[&locator["project_id"].as_str().unwrap(),&locator["intent_handle"].as_str().unwrap()]).await.unwrap();
    assert_eq!(row.get::<_, i64>(0), original.id);
    assert_eq!(row.get::<_, String>(1), original.token);
    assert_eq!(row.get::<_, String>(2), original.target);
    assert_eq!(row.get::<_, i64>(3), original.codec);
    assert_eq!(row.get::<_, Vec<u8>>(4), original.digest);
    assert_eq!(row.get::<_, Vec<u8>>(5), original.bytes);
    assert_eq!(row.get::<_, String>(6), original.token);
    assert_eq!(row.get::<_, String>(7), original.target);
    assert_eq!(row.get::<_, i64>(8), original.codec);
    assert_eq!(row.get::<_, Vec<u8>>(9), original.digest);
    assert_eq!(row.get::<_, Vec<u8>>(10), original.bytes);
    assert_eq!(row.get::<_, i64>(11), 1);
    assert_eq!(row.get::<_, i64>(12), 1);
    let SavedResult::Create(result) = decoded(saved) else {
        panic!("creation result family lost")
    };
    assert_eq!(result.item.id.0, original.target);
    assert_eq!(result.readable_id.sequence, 2);
}
async fn cases(
    handle: &Handle,
    browser: &Browser,
    admin: &Admin,
    commands: Commands,
    exchanges: &Exchanges,
) -> (Value, Value, Value) {
    let Commands {
        milestone,
        edit,
        clear,
        cancel_form,
    } = commands;
    let original = identity(admin, &milestone).await;
    let count = handle.live.probe.item_repreparations();
    let prepared = handle
        .live
        .probe
        .hold_transaction(0, Point::ItemMutationPrepared);
    let winner = exchange(
        handle,
        browser,
        "/items/pending/execute",
        milestone.clone(),
        exchanges,
    );
    tokio::time::timeout(WORK_DEADLINE, prepared.arrived)
        .await
        .unwrap()
        .unwrap();
    // The Milestone owns a complete packet, but neither a transaction/mutex nor
    // the tracked CPU lane. Edit c must commit without waiting for that permit.
    lose_network_reply(handle, browser, "/items/edit", edit.clone()).await;
    let changed = request(handle, browser, "/items/reconcile", edit.clone(), 200).await;
    assert_eq!(changed["outcome"], "replayed");
    let SavedResult::Edit(changed_result) = decoded(&changed) else {
        panic!("edit result family lost")
    };
    assert_eq!(changed_result.item.version, 2);
    assert_eq!(handle.live.probe.item_repreparations(), count);
    let commit = handle
        .live
        .probe
        .hold_transaction(0, Point::ItemBeforeCommit);
    prepared.release.send(()).unwrap();
    tokio::time::timeout(WORK_DEADLINE, commit.arrived)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        handle.live.probe.item_repreparations(),
        count + 1,
        "Milestone must consume exactly one actual coordinator reprepare"
    );
    let baseline = handle.live.probe.snapshot().native_mutex_attempts;
    // Start one follower at a time. Its completed CPU admission and observed
    // lock attempt exclude an accidental try-lock Busy race from this control.
    let duplicate = exchange(
        handle,
        browser,
        "/items/pending/execute",
        milestone.clone(),
        exchanges,
    );
    attempts(handle, baseline + 1).await;
    let cancel = exchange(
        handle,
        browser,
        "/items/pending/cancel",
        milestone.clone(),
        exchanges,
    );
    attempts(handle, baseline + 2).await;
    commit.release.send(()).unwrap();
    let second = response(winner, 201).await;
    assert_eq!(second["outcome"], "created");
    let equal = response(duplicate, 200).await;
    assert_eq!(equal["outcome"], "replayed");
    assert_eq!(equal["result"], second["result"]);
    let cancelled = response(cancel, 200).await;
    assert_eq!(cancelled["entry"]["state"], "completed");
    assert_eq!(cancelled["entry"]["target_item_id"], original.target);
    assert_eq!(
        cancelled["entry"]["intent_handle"],
        milestone["intent_handle"]
    );
    creation_identity(admin, &milestone, &original, &second).await;
    assert_eq!(handle.live.probe.item_repreparations(), count + 1);

    let commit = handle
        .live
        .probe
        .hold_transaction(0, Point::ItemBeforeCommit);
    let winner = exchange(handle, browser, "/items/edit", clear.clone(), exchanges);
    tokio::time::timeout(WORK_DEADLINE, commit.arrived)
        .await
        .unwrap()
        .unwrap();
    let baseline = handle.live.probe.snapshot().native_mutex_attempts;
    let duplicate = exchange(handle, browser, "/items/edit", clear.clone(), exchanges);
    attempts(handle, baseline + 1).await;
    let mut altered = clear.clone();
    altered["estimate"] = json!({"action":"keep"});
    let changed_echo = exchange(handle, browser, "/items/edit", altered, exchanges);
    attempts(handle, baseline + 2).await;
    let mut stale = clear.clone();
    stale["operation_id"] = json!("e".repeat(32));
    let loser = exchange(handle, browser, "/items/edit", stale, exchanges);
    attempts(handle, baseline + 3).await;
    commit.release.send(()).unwrap();
    let cleared = response(winner, 200).await;
    assert_eq!(cleared["outcome"], "edited");
    let equal = response(duplicate, 200).await;
    assert_eq!(equal["outcome"], "replayed");
    assert_eq!(equal["result"], cleared["result"]);
    assert_eq!(
        response(changed_echo, 409).await,
        json!({"error":"operation_conflict"})
    );
    assert_eq!(response(loser, 409).await, json!({"error":"item_conflict"}));
    assert!(!admin.client().query_one("SELECT EXISTS(SELECT FROM kehila.operations WHERE command_family='item_edit' AND target_project_id=$1::text::kehila.ident AND target_item_id=$2::text::kehila.ident AND token=repeat('e',32)) OR EXISTS(SELECT FROM kehila.item_revisions WHERE token=repeat('e',32))",
        &[&clear["project_id"].as_str().unwrap(),&clear["item_id"].as_str().unwrap()]).await.unwrap().get::<_,bool>(0));

    let registration = request(handle, browser, "/items/pending/register", cancel_form, 201).await;
    let locator = json!({"project_id":registration["entry"]["project_id"],"intent_handle":registration["entry"]["intent_handle"]});
    let original = identity(admin, &locator).await;
    let before = evidence::snapshot(admin).await;
    let commit = handle
        .live
        .probe
        .hold_transaction(original.id, Point::PendingCommit);
    let cancel = exchange(
        handle,
        browser,
        "/items/pending/cancel",
        locator.clone(),
        exchanges,
    );
    tokio::time::timeout(WORK_DEADLINE, commit.arrived)
        .await
        .unwrap()
        .unwrap();
    let baseline = handle.live.probe.snapshot().native_mutex_attempts;
    let execute = exchange(
        handle,
        browser,
        "/items/pending/execute",
        locator.clone(),
        exchanges,
    );
    attempts(handle, baseline + 1).await;
    commit.release.send(()).unwrap();
    let cancelled = response(cancel, 200).await;
    assert_eq!(cancelled["entry"]["state"], "cancelled");
    assert_eq!(cancelled["entry"]["target_item_id"], original.target);
    assert_eq!(
        response(execute, 410).await,
        json!({"error":"pending_cancelled"})
    );
    // Only this pending row may change. Its immutable identity survives while
    // cancellation clears the executable request bytes and creates no domain row.
    let mut expected = before;
    let pending = expected
        .iter_mut()
        .find(|(name, _)| name == "kehila_auth.pending_items")
        .unwrap();
    let mut rows: Vec<Value> = serde_json::from_str(&pending.1).unwrap();
    let selected = rows
        .iter_mut()
        .find(|row| row["request_id"] == json!(original.id))
        .unwrap();
    assert_eq!(selected["state"], "prepared");
    assert!(!selected["request_bytes"].is_null());
    selected["state"] = json!("cancelled");
    selected["request_bytes"] = Value::Null;
    pending.1 = serde_json::to_string(&rows).unwrap();
    let normalize = |mut snapshot: Vec<(String, String)>| {
        let pending = snapshot
            .iter_mut()
            .find(|(name, _)| name == "kehila_auth.pending_items")
            .unwrap();
        let mut rows: Vec<Value> = serde_json::from_str(&pending.1).unwrap();
        rows.sort_by_key(Value::to_string);
        pending.1 = serde_json::to_string(&rows).unwrap();
        snapshot
    };
    assert_eq!(
        normalize(evidence::snapshot(admin).await),
        normalize(expected),
        "cancellation changed data beyond the exact selected pending state/bytes"
    );
    let row=admin.client().query_one("SELECT token::text,target_item_id::text,request_codec_version,request_sha256,request_bytes IS NULL,state,operation_row_id IS NULL FROM kehila_auth.pending_items WHERE request_id=$1",&[&original.id]).await.unwrap();
    assert_eq!(row.get::<_, String>(0), original.token);
    assert_eq!(row.get::<_, String>(1), original.target);
    assert_eq!(row.get::<_, i64>(2), original.codec);
    assert_eq!(row.get::<_, Vec<u8>>(3), original.digest);
    assert!(row.get::<_, bool>(4));
    assert_eq!(row.get::<_, String>(5), "cancelled");
    assert!(row.get::<_, bool>(6));
    assert!(handle.live.is_live());
    (second, changed, cleared)
}

pub(super) async fn exercise(
    handle: &Handle,
    browser: &Browser,
    admin: Admin,
    commands: Commands,
    running: &mut tokio::task::JoinHandle<Result<(), Error>>,
) -> (Value, Value, Value, Admin) {
    // Parent retains both administrative and exchange owners across child panic.
    let admin = Arc::new(tokio::sync::Mutex::new(admin));
    let owned_admin = Arc::clone(&admin);
    let exchanges: Exchanges = Arc::new(std::sync::Mutex::new(tokio::task::JoinSet::new()));
    let registered = Arc::clone(&exchanges);
    let jobs = handle.clone();
    let browser = browser.clone();
    let outcome = tokio::spawn(async move {
        let admin = owned_admin.lock().await;
        cases(&jobs, &browser, &admin, commands, &registered).await
    })
    .await;
    let mut stopped = false;
    let mut root_result = None;
    if outcome.is_err() {
        handle.stop();
        root_result = Some(
            tokio::time::timeout(Duration::from_secs(10), &mut *running)
                .await
                .expect("contention root cleanup unobserved"),
        );
        stopped = true;
    }
    let mutex = Arc::try_unwrap(exchanges)
        .unwrap_or_else(|_| panic!("unobserved contention registry owner"));
    let (mut exchanges, mut failed) = match mutex.into_inner() {
        Ok(value) => (value, false),
        Err(error) => (error.into_inner(), true),
    };
    let joined = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(result) = exchanges.join_next().await {
            failed |= result.is_err();
        }
    })
    .await;
    if joined.is_err() {
        failed = true;
        exchanges.abort_all();
        tokio::time::timeout(Duration::from_secs(5), async {
            while exchanges.join_next().await.is_some() {}
        })
        .await
        .expect("contention exchange cleanup unobserved");
    }
    let permits = tokio::time::timeout(
        WORK_DEADLINE,
        handle.capacity.clone().acquire_many_owned(CAPACITY as u32),
    )
    .await;
    let settled = matches!(&permits, Ok(Ok(_)));
    drop(permits);
    let admin = Arc::try_unwrap(admin)
        .unwrap_or_else(|_| panic!("unobserved contention administrative owner"))
        .into_inner();
    if outcome.is_err() || failed || !settled {
        if !stopped {
            handle.stop();
            root_result = Some(
                tokio::time::timeout(Duration::from_secs(10), &mut *running)
                    .await
                    .expect("contention root cleanup unobserved"),
            );
        }
        evidence::runtime_released(&admin).await;
        admin.close().await;
        panic!("contention failed: cases_ok={}, exchange_failed={failed}, settled={settled}, root={root_result:?}",outcome.is_ok());
    }
    let (second, changed, cleared) = outcome.unwrap();
    (second, changed, cleared, admin)
}
