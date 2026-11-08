//! Real repeated head invalidation, bounded exhaustion and explicit identity recovery.
use super::*;
use crate::lifecycle::probe::{AttemptEntry, Point};
use crate::postgres::tests::fixture_admin::Admin;
use work_item_codec::SavedResult;

type Exchanges = Arc<std::sync::Mutex<tokio::task::JoinSet<()>>>;
async fn request(
    handle: &Handle,
    browser: &Browser,
    path: &str,
    body: Value,
    expected: u16,
) -> Value {
    let (status, value, _) = call(handle, Some(browser), path, Some(body)).await;
    assert_eq!(status, expected, "{path}: {value}");
    value
}
fn decoded(value: &Value) -> SavedResult {
    work_item_codec::decode_result(1, &serde_json::to_vec(&value["result"]).unwrap()).unwrap()
}
async fn pending(admin: &Admin, locator: &Value) -> String {
    admin.client().query_one(
        "SELECT to_jsonb(p)::text FROM kehila_auth.pending_items p WHERE project_id=$1::text::kehila.ident AND intent_handle=$2::text::kehila_auth.intent_key",
        &[&locator["project_id"].as_str().unwrap(), &locator["intent_handle"].as_str().unwrap()],
    ).await.unwrap().get(0)
}
async fn unaccepted(admin: &Admin, locator: &Value, original: &str) {
    assert_eq!(
        pending(admin, locator).await,
        original,
        "victim pending identity/state/bytes changed"
    );
    let row = admin.client().query_one(
        "SELECT EXISTS(SELECT FROM kehila.items i WHERE i.creation_request_id=p.request_id), EXISTS(SELECT FROM kehila.operations o WHERE o.target_project_id=p.project_id AND o.token=p.token), EXISTS(SELECT FROM kehila.item_revisions r WHERE r.project_id=p.project_id AND r.item_id=p.target_item_id) FROM kehila_auth.pending_items p WHERE project_id=$1::text::kehila.ident AND intent_handle=$2::text::kehila_auth.intent_key",
        &[&locator["project_id"].as_str().unwrap(), &locator["intent_handle"].as_str().unwrap()],
    ).await.unwrap();
    assert!(
        !row.get::<_, bool>(0) && !row.get::<_, bool>(1) && !row.get::<_, bool>(2),
        "victim was accepted during invalidation"
    );
}
async fn competitor(admin: &Admin, item: &str, round: i64, token: &str) {
    let row = admin.client().query_one(
        "SELECT i.current_version::text, h.state_version::text, r.token::text, r.item_version::text, o.command_family, o.target_item_id::text, o.token::text, o.request_sha256=sha256(b.request_bytes), b.result=r.history#>'{value,result}', (SELECT count(*) FROM kehila.operations), (SELECT count(*) FROM kehila.item_revisions), (SELECT count(*) FROM kehila.items) FROM kehila.items i JOIN kehila.project_operational_heads h USING(project_id) JOIN kehila.item_revisions r ON r.operation_row_id=h.operation_row_id JOIN kehila.operations o ON o.operation_row_id=r.operation_row_id JOIN kehila.operation_payloads b ON b.operation_row_id=o.operation_row_id WHERE i.item_id=$1::text::kehila.ident",
        &[&item],
    ).await.unwrap();
    assert_eq!(row.get::<_, String>(0), (round + 1).to_string());
    assert_eq!(row.get::<_, String>(1), (round + 1).to_string());
    assert_eq!(row.get::<_, String>(2), token);
    assert_eq!(row.get::<_, String>(3), (round + 1).to_string());
    assert_eq!(row.get::<_, String>(4), "item_edit");
    assert_eq!(row.get::<_, String>(5), item);
    assert_eq!(row.get::<_, String>(6), token);
    assert!(row.get::<_, bool>(7) && row.get::<_, bool>(8));
    assert_eq!(row.get::<_, i64>(9), round + 2); // project, task, competing edits
    assert_eq!(row.get::<_, i64>(10), round + 1);
    assert_eq!(row.get::<_, i64>(11), 1);
}
async fn cases(handle: &Handle, browser: &Browser, admin: &Admin, exchanges: &Exchanges) {
    let (status, registered) = project(
        handle,
        browser,
        "register",
        json!({
        "name":"Retry exhaustion", "prefix":"RTE", "estimate_unit":"hours"}),
    )
    .await;
    assert_eq!(status, 201);
    let (status, created) =
        project(handle, browser, "execute", execute(&registered["entry"])).await;
    assert_eq!(status, 201);
    let created =
        project_create_codec::decode_result(1, &serde_json::to_vec(&created["result"]).unwrap())
            .unwrap();
    let project_id = &created.project.id.0;
    let form = |kind: usize| {
        json!({"project_id":project_id,
        "type_id":created.configuration.types[kind].id.0,"workflow_id":null,
        "expected_configuration_revision":"1","estimate":null,"values":[]})
    };
    let task = request(handle, browser, "/items/pending/register", form(0), 201).await;
    let task_locator =
        json!({"project_id":project_id,"intent_handle":task["entry"]["intent_handle"]});
    let task = request(handle, browser, "/items/pending/execute", task_locator, 201).await;
    let SavedResult::Create(task) = decoded(&task) else {
        panic!("wrong result family")
    };
    let victim = request(handle, browser, "/items/pending/register", form(1), 201).await;
    let locator = json!({"project_id":project_id,"intent_handle":victim["entry"]["intent_handle"]});
    let original = pending(admin, &locator).await;
    let original_bytes: Vec<u8> = admin.client().query_one(
        "SELECT request_bytes FROM kehila_auth.pending_items WHERE project_id=$1::text::kehila.ident AND intent_handle=$2::text::kehila_auth.intent_key",
        &[&project_id.as_str(), &locator["intent_handle"].as_str().unwrap()],
    ).await.unwrap().get(0);
    let opened = request(handle, browser, "/items/pending/open", locator.clone(), 200).await;
    evidence::settled(handle).await;
    handle.live.probe.watch_item_attempt(
        project_id,
        locator["intent_handle"].as_str().unwrap(),
        original_bytes,
    );
    let mut gate = handle
        .live
        .probe
        .hold_transaction(0, Point::ItemAttemptPrepared);
    let jobs = handle.clone();
    let client = browser.clone();
    let command = locator.clone();
    let (sent, response) = tokio::sync::oneshot::channel();
    exchanges.lock().unwrap().spawn(async move {
        let result = call(
            &jobs,
            Some(&client),
            "/items/pending/execute",
            Some(command),
        )
        .await;
        let _ = sent.send(result);
    });
    let mut expected = evidence::snapshot(admin).await;
    let mut last_edit = Value::Null;
    for round in 1..=3 {
        tokio::time::timeout(WORK_DEADLINE, &mut gate.arrived)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            evidence::snapshot(admin).await,
            expected,
            "victim retry changed committed artifacts"
        );
        assert_eq!(
            handle.capacity.available_permits(),
            CAPACITY - 1,
            "victim did not retain exactly one admission"
        );
        evidence::fences_released(admin).await;
        let token = format!("{round:032x}");
        let edit = json!({"project_id":project_id,"item_id":task.item.id.0,"operation_id":token,
            "expected_item_version":round.to_string(),"expected_configuration_revision":"1",
            "estimate":{"action":"set","value":round.to_string()},"fields":[]});
        let edited = request(handle, browser, "/items/edit", edit.clone(), 200).await;
        assert_eq!(edited["outcome"], "edited");
        competitor(admin, &task.item.id.0, round, &token).await;
        unaccepted(admin, &locator, &original).await;
        expected = evidence::snapshot(admin).await;
        // The next gate must exist before this preparation is released. Its
        // victim-only point cannot capture the competitor's preparation.
        let next = (round < 3).then(|| {
            handle
                .live
                .probe
                .hold_transaction(0, Point::ItemAttemptPrepared)
        });
        gate.release.send(()).unwrap();
        last_edit = edit;
        if let Some(next) = next {
            gate = next;
        } else {
            break;
        }
    }
    let (status, exhausted, _) = tokio::time::timeout(WORK_DEADLINE, response)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status, 503);
    assert_eq!(exhausted, json!({"error":"attempt_exhausted"}));
    evidence::settled(handle).await;
    assert!(handle.live.is_live(), "exhaustion retired the authority");
    assert_eq!(evidence::snapshot(admin).await, expected);
    unaccepted(admin, &locator, &original).await;
    evidence::fences_released(admin).await;
    // Observe through terminal transport/permit cleanup, before removing watch.
    let observed = handle.live.probe.take_item_attempt();
    assert_eq!(
        observed.entries,
        [
            AttemptEntry::Preparation,
            AttemptEntry::Write,
            AttemptEntry::Reprepare
        ]
        .repeat(3)
    );
    assert!(
        Instant::now() < observed.deadline.unwrap(),
        "503 used an expired request"
    );
    assert!(
        observed.admission.upgrade().is_none(),
        "original admission still owned"
    );
    assert_eq!(
        request(
            handle,
            browser,
            "/items/pending/reconcile",
            locator.clone(),
            404
        )
        .await,
        json!({"error":"operation_not_found"})
    );
    assert_eq!(
        request(handle, browser, "/items/pending/open", locator.clone(), 200).await,
        opened
    );
    assert_eq!(
        evidence::snapshot(admin).await,
        expected,
        "reconciliation/open executed unseen work"
    );
    let recovered = request(
        handle,
        browser,
        "/items/pending/execute",
        locator.clone(),
        201,
    )
    .await;
    assert_eq!(recovered["outcome"], "created");
    let SavedResult::Create(result) = decoded(&recovered) else {
        panic!("recovery result family changed")
    };
    assert_eq!(result.item.id.0, victim["entry"]["target_item_id"]);
    assert_eq!(result.readable_id.sequence, 2);
    let original: Value = serde_json::from_str(&original).unwrap();
    let completed: Value = serde_json::from_str(&pending(admin, &locator).await).unwrap();
    for field in [
        "request_id",
        "project_id",
        "actor_id",
        "intent_handle",
        "token",
        "target_item_id",
        "request_codec_version",
        "request_sha256",
        "request_bytes",
        "registered_ms",
        "expires_ms",
    ] {
        assert!(
            !original[field].is_null(),
            "missing original identity field {field}"
        );
        assert_eq!(
            completed[field], original[field],
            "recovery replaced {field}"
        );
    }
    assert_eq!(completed["state"], "completed");
    let committed = evidence::snapshot(admin).await;
    let replay = request(
        handle,
        browser,
        "/items/pending/execute",
        locator.clone(),
        200,
    )
    .await;
    assert_eq!(replay["outcome"], "replayed");
    assert_eq!(replay["result"], recovered["result"]);
    // Preserve exact-content/stale-version conflict contracts in this fixture.
    // Round three is an already accepted operation, now stale against version four.
    let mut altered = last_edit.clone();
    altered["estimate"] = json!({"action":"keep"});
    assert_eq!(
        request(handle, browser, "/items/edit", altered, 409).await,
        json!({"error":"operation_conflict"})
    );
    last_edit["operation_id"] = json!("e".repeat(32));
    assert_eq!(
        request(handle, browser, "/items/edit", last_edit, 409).await,
        json!({"error":"item_conflict"})
    );
    assert_eq!(evidence::snapshot(admin).await, committed);
    let current = request(
        handle,
        browser,
        "/items/read",
        json!({"project_id":project_id,"item_id":task.item.id.0}),
        200,
    )
    .await;
    let SavedResult::Edit(current) = decoded(&current) else {
        panic!("competitor liveness read failed")
    };
    assert_eq!(current.item.version, 4);
    assert!(handle.live.is_live());
}

#[test]
#[ignore = "requires a fresh explicitly provisioned product PostgreSQL TLS fixture"]
fn native_item_retry_exhaustion_and_recovery() {
    let (endpoint, installation, attempt, controls) = fixture();
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let (handle, owner) = runtime(&endpoint, &installation, format!("{attempt}-retry")).await;
        let mut running = tokio::spawn(owner.run());
        let admin = Arc::new(tokio::sync::Mutex::new(Admin::connect(&endpoint).await));
        let exchanges: Exchanges = Arc::new(std::sync::Mutex::new(tokio::task::JoinSet::new()));
        let jobs = handle.clone();
        let owned_admin = Arc::clone(&admin);
        let registered = Arc::clone(&exchanges);
        // Child assertion failures cannot detach administrative or exchange owners.
        let outcome = tokio::spawn(async move {
            let browser = login(&jobs).await;
            let admin = owned_admin.lock().await;
            cases(&jobs, &browser, &admin, &registered).await;
        }).await;
        handle.stop();
        let root = tokio::time::timeout(Duration::from_secs(10), &mut running).await;
        let mut exchanges = Arc::try_unwrap(exchanges).unwrap_or_else(|_| panic!("unjoined exchange registry"))
            .into_inner().unwrap_or_else(|e| e.into_inner());
        let mut exchange_failed = false;
        let joined = tokio::time::timeout(Duration::from_secs(10), async {
            while let Some(result) = exchanges.join_next().await { exchange_failed |= result.is_err(); }
        }).await;
        if joined.is_err() {
            exchanges.abort_all();
            tokio::time::timeout(Duration::from_secs(5), async {
                while exchanges.join_next().await.is_some() {}
            }).await.expect("retry exchange cleanup unobserved");
        }
        if root.is_err() {
            running.abort();
            let _ = tokio::time::timeout(Duration::from_secs(5), &mut running).await.expect("retry root cleanup unobserved");
        }
        let admin = Arc::try_unwrap(admin).unwrap_or_else(|_| panic!("unjoined admin owner")).into_inner();
        evidence::runtime_released(&admin).await;
        admin.close().await;
        assert!(outcome.is_ok() && matches!(root, Ok(Ok(Ok(())))) && joined.is_ok() && !exchange_failed,
            "retry fixture failed: primary={outcome:?}, root={root:?}, exchanges_joined={}, exchange_failed={exchange_failed}", joined.is_ok());
        mark(&controls, "item-retry-passed");
    });
}
