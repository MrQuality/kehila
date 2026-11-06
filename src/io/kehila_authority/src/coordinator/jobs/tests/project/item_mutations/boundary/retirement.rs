//! Whole-protocol ordering, including the shared installation/actor fence.
use super::*;
type Owners = Arc<std::sync::Mutex<Vec<Admin>>>;

fn maintenance(
    owners: &Owners,
    installation: &str,
    operation: i64,
    exchanges: &Exchanges,
) -> controls::Maintenance {
    let admin = { owners.lock().unwrap().pop() };
    controls::retire(
        admin.expect("one preconnected owner per retirement"),
        installation.into(),
        operation,
        exchanges,
    )
}
async fn window(admin: &Admin, edit: &Value) -> (i64, u64, u64) {
    let row=admin.client().query_one("SELECT operation_row_id,replay_origin_ms::text,replay_deadline_ms::text FROM kehila.operations WHERE command_family='item_edit' AND target_project_id=$1::text::kehila.ident AND target_item_id=$2::text::kehila.ident AND token=$3::text::kehila.operation_token",
        &[&edit["project_id"].as_str().unwrap(),&edit["item_id"].as_str().unwrap(),&edit["operation_id"].as_str().unwrap()]).await.unwrap();
    (
        row.get(0),
        row.get::<_, String>(1).parse().unwrap(),
        row.get::<_, String>(2).parse().unwrap(),
    )
}
fn exchange(
    handle: &Handle,
    browser: &Browser,
    edit: Value,
    exchanges: &Exchanges,
) -> tokio::sync::oneshot::Receiver<Reply> {
    let jobs = handle.clone();
    let browser = browser.clone();
    let (sent, response) = tokio::sync::oneshot::channel();
    exchanges.lock().unwrap().spawn(async move {
        let reply = call(&jobs, Some(&browser), "/items/reconcile", Some(edit)).await;
        let _ = sent.send(reply);
    });
    response
}
async fn committed(value: controls::Maintenance) {
    assert!(tokio::time::timeout(WORK_DEADLINE, value.staged)
        .await
        .unwrap()
        .unwrap());
    value.commit.send(()).unwrap();
    tokio::time::timeout(WORK_DEADLINE, value.finished)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}
async fn retired(admin: &Admin, operation: i64) {
    let row=admin.client().query_one("SELECT o.payload_retired,NOT EXISTS(SELECT FROM kehila.operation_payloads p WHERE p.operation_row_id=o.operation_row_id) FROM kehila.operations o WHERE o.operation_row_id=$1",&[&operation]).await.unwrap();
    assert!(row.get::<_, bool>(0) && row.get::<_, bool>(1));
}
pub(super) async fn cases(
    handle: &Handle,
    browser: &Browser,
    admin: &Admin,
    edit: &Value,
    clear: &Value,
    expected: &Value,
    owners: &Owners,
    exchanges: &Exchanges,
) {
    let installation: String = admin
        .client()
        .query_one(
            "SELECT installation_id::text FROM kehila_auth.installation WHERE singleton",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    let (operation, _, end) = window(admin, edit).await;
    at(admin, end - 1).await;
    handle.live.probe.take_item_replay();
    let gate = handle
        .live
        .probe
        .hold_transaction(0, Point::ItemReplayCleanup);
    let reply = exchange(handle, browser, edit.clone(), exchanges);
    tokio::time::timeout(WORK_DEADLINE, gate.arrived)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        handle.live.probe.take_item_replay(),
        Some(Observation::Authorized)
    );
    let serving = controls::authorized(admin).await;
    at(admin, end + 1).await;
    let mut retire = maintenance(owners, &installation, operation, exchanges);
    let pid = tokio::time::timeout(WORK_DEADLINE, &mut retire.started)
        .await
        .unwrap()
        .unwrap();
    controls::blocked_by(admin, pid, &serving).await;
    // The common actor fence can block retirement before its operation-row lock.
    // This observes serialization of the complete protocol, not one lock alone.
    gate.release.send(()).unwrap();
    committed(retire).await;
    let (status, value, _) = tokio::time::timeout(WORK_DEADLINE, reply)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status, 200);
    assert_eq!(value, json!({"outcome":"replayed","result":expected}));
    retired(admin, operation).await;

    let (operation, origin, end) = window(admin, clear).await;
    at(admin, origin + 1).await;
    let held = prepared(handle, browser, admin, clear.clone(), exchanges).await;
    at(admin, end + 1).await;
    let mut retire = maintenance(owners, &installation, operation, exchanges);
    let pid = tokio::time::timeout(WORK_DEADLINE, &mut retire.started)
        .await
        .unwrap()
        .unwrap();
    assert!(tokio::time::timeout(WORK_DEADLINE, &mut retire.staged)
        .await
        .unwrap()
        .unwrap());
    // Retirement has executed but remains uncommitted while the actual prepared
    // response leaves its tracked worker and requests a new fence.
    let gate = handle
        .live
        .probe
        .hold_transaction(0, Point::ItemReplayFenced);
    held.release.send(()).unwrap();
    let waiter = controls::fence_waiter(admin, pid).await;
    assert_ne!(waiter.pid(), pid);
    retire.commit.send(()).unwrap();
    tokio::time::timeout(WORK_DEADLINE, retire.finished)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    tokio::time::timeout(WORK_DEADLINE, gate.arrived)
        .await
        .unwrap()
        .unwrap();
    // Commit retirement while expired, then reverse only after the waiting
    // constructor acquired its fresh fence and before fixed authorization SQL.
    at(admin, origin + 1).await;
    gate.release.send(()).unwrap();
    let (status, value, _) = tokio::time::timeout(WORK_DEADLINE, held.response)
        .await
        .unwrap()
        .unwrap();
    assert!(held.started.elapsed() < WORK_DEADLINE);
    assert_eq!(status, 410);
    assert_eq!(value, json!({"error":"replay_expired"}));
    assert_eq!(
        handle.live.probe.take_item_replay(),
        Some(Observation::Expired)
    );
    retired(admin, operation).await;
    for path in ["/items/edit", "/items/reconcile"] {
        assert_eq!(
            request(handle, browser, path, clear.clone(), 410).await,
            json!({"error":"replay_expired"})
        );
    }
    let mut changed = clear.clone();
    changed["estimate"] = json!({"action":"keep"});
    assert_eq!(
        request(handle, browser, "/items/reconcile", changed, 409).await,
        json!({"error":"operation_conflict"})
    );
    let current = request(
        handle,
        browser,
        "/items/read",
        json!({"project_id":clear["project_id"],"item_id":clear["item_id"]}),
        200,
    )
    .await;
    let SavedResult::Edit(current) = decoded(&current) else {
        panic!("retirement removed permanent history")
    };
    assert_eq!(current.item.version, 3);
    assert!(current.data.estimate.is_none());
    assert!(current.data.values.is_empty());
    assert!(handle.live.is_live());
}

pub(super) fn expected(mut before: Vec<(String, String)>) -> Vec<(String, String)> {
    // Derive exact expected retention changes from the original snapshot. All
    // other rows/columns, including pending bytes and permanent history, remain.
    let operations = before
        .iter_mut()
        .find(|(name, _)| name == "kehila.operations")
        .unwrap();
    let mut rows: Vec<Value> = serde_json::from_str(&operations.1).unwrap();
    let mut retired = Vec::new();
    for row in &mut rows {
        if row["command_family"] == "item_edit"
            && ["c".repeat(32), "d".repeat(32)]
                .iter()
                .any(|token| row["token"].as_str() == Some(token.as_str()))
        {
            assert_eq!(row["payload_retired"], false);
            retired.push(row["operation_row_id"].clone());
            row["payload_retired"] = json!(true);
        }
    }
    assert_eq!(retired.len(), 2);
    rows.sort_by_key(Value::to_string);
    operations.1 = serde_json::to_string(&rows).unwrap();
    let payloads = before
        .iter_mut()
        .find(|(name, _)| name == "kehila.operation_payloads")
        .unwrap();
    let mut rows: Vec<Value> = serde_json::from_str(&payloads.1).unwrap();
    rows.retain(|row| !retired.contains(&row["operation_row_id"]));
    rows.sort_by_key(Value::to_string);
    payloads.1 = serde_json::to_string(&rows).unwrap();
    normalize(before)
}
pub(super) fn normalize(mut snapshot: Vec<(String, String)>) -> Vec<(String, String)> {
    for (name, encoded) in &mut snapshot {
        if name == "kehila.operations" || name == "kehila.operation_payloads" {
            let mut rows: Vec<Value> = serde_json::from_str(encoded).unwrap();
            rows.sort_by_key(Value::to_string);
            *encoded = serde_json::to_string(&rows).unwrap();
        }
    }
    snapshot
}
