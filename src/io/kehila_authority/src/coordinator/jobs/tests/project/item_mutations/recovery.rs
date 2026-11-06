//! Successor-runtime recovery of retained creation and exact edit identities.
use super::*;
#[derive(Clone)]
pub(super) struct Creation {
    pub(super) entry: Value,
    pub(super) request: Value,
}
pub(super) struct History {
    pub(super) creation_result: Value,
    pub(super) edit: Value,
    pub(super) edit_result: Value,
    pub(super) project_entry: Value,
    pub(super) project_result: Value,
    pub(super) current: Value,
}
pub(super) struct Serving {
    pub(super) handle: Handle,
    pub(super) browser: Browser,
    pub(super) admin: Admin,
    pub(super) running: tokio::task::JoinHandle<Result<(), Error>>,
}
fn same_entry(actual: &Value, original: &Value) {
    for field in [
        "project_id",
        "request_id",
        "intent_handle",
        "token",
        "target_item_id",
        "registered_ms",
        "expires_ms",
    ] {
        assert_eq!(
            actual[field], original[field],
            "restart changed immutable creation identity"
        );
    }
    assert_eq!(actual["state"], "completed");
}
async fn checks(
    handle: &Handle,
    previous: Browser,
    admin: &Admin,
    generation: u64,
    before: &[(String, String)],
    creation: Creation,
    history: Option<History>,
) -> Browser {
    let fresh = admin
        .client()
        .query_one(
            "SELECT runtime_generation::text FROM kehila_auth.installation WHERE singleton",
            &[],
        )
        .await
        .unwrap()
        .get::<_, String>(0)
        .parse::<u64>()
        .unwrap();
    assert_eq!(fresh, generation.checked_add(1).unwrap());
    let project_id = &creation.entry["project_id"];
    let locator = json!({"project_id":project_id,"intent_handle":creation.entry["intent_handle"]});
    let (status, _, _) = call(
        handle,
        Some(&previous),
        "/items/pending/list",
        Some(json!({"project_id":project_id,"before":null})),
    )
    .await;
    assert_eq!(status, 401);
    let browser = login(handle).await;
    assert!(
        browser.cookie != previous.cookie,
        "successor login reused previous credential"
    );
    drop(previous);
    // This existing artifact inventory excludes runtime/session/audit metadata;
    // generation and session replacement are checked explicitly above.
    assert_eq!(
        evidence::snapshot(admin).await.as_slice(),
        before,
        "restart/login changed domain history"
    );
    let page = request(
        handle,
        &browser,
        "/items/pending/list",
        json!({"project_id":project_id,"before":null}),
        200,
    )
    .await;
    assert!(page["before"].is_null());
    let found = page["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["intent_handle"] == creation.entry["intent_handle"])
        .unwrap();
    same_entry(found, &creation.entry);
    assert!(page.get("result").is_none());
    let opened = request(
        handle,
        &browser,
        "/items/pending/open",
        locator.clone(),
        200,
    )
    .await;
    same_entry(&opened["entry"], &creation.entry);
    assert_eq!(opened["request"], creation.request);
    assert!(!opened["request"].is_null());
    assert!(opened.get("result").is_none());
    // Discovery/Open expose only the original retained command and metadata.
    // On the first transition the lost creation has not yet been acknowledged.
    assert_eq!(
        evidence::snapshot(admin).await.as_slice(),
        before,
        "discovery executed or changed accepted work"
    );
    if let Some(history) = history {
        for path in ["/items/pending/reconcile", "/items/pending/execute"] {
            let saved = request(handle, &browser, path, locator.clone(), 200).await;
            assert_eq!(
                saved,
                json!({"outcome":"replayed","result":history.creation_result})
            );
        }
        for path in ["/items/reconcile", "/items/edit"] {
            let saved = request(handle, &browser, path, history.edit.clone(), 200).await;
            assert_eq!(
                saved,
                json!({"outcome":"replayed","result":history.edit_result})
            );
        }
        let (status, saved) =
            project(handle, &browser, "execute", execute(&history.project_entry)).await;
        assert_eq!(status, 200);
        assert_eq!(saved["result"], history.project_result);
        let current = request(
            handle,
            &browser,
            "/items/read",
            json!({"project_id":project_id,"item_id":history.edit["item_id"]}),
            200,
        )
        .await;
        assert_eq!(
            current, history.current,
            "historical replay changed the separately read current item"
        );
        let SavedResult::Edit(current) = decoded(&current) else {
            panic!("current result family lost")
        };
        assert_eq!(current.item.version, 3);
        assert_eq!(
            evidence::snapshot(admin).await.as_slice(),
            before,
            "historical recovery changed durable artifacts"
        );
    }
    browser
}

pub(super) struct RestartTarget<'a> {
    pub(super) endpoint: &'a crate::postgres::Endpoint,
    pub(super) installation: &'a str,
    pub(super) attempt: String,
}
pub(super) async fn restart(
    handle: Handle,
    browser: Browser,
    admin: Admin,
    target: RestartTarget<'_>,
    creation: Creation,
    history: Option<History>,
    running: &mut tokio::task::JoinHandle<Result<(), Error>>,
) -> Serving {
    let RestartTarget {
        endpoint,
        installation,
        attempt,
    } = target;
    evidence::settled(&handle).await;
    let before = evidence::snapshot(&admin).await;
    let generation = admin
        .client()
        .query_one(
            "SELECT runtime_generation::text FROM kehila_auth.installation WHERE singleton",
            &[],
        )
        .await
        .unwrap()
        .get::<_, String>(0)
        .parse::<u64>()
        .unwrap();
    handle.stop();
    let old = tokio::time::timeout(Duration::from_secs(10), &mut *running)
        .await
        .expect("previous runtime join unobserved");
    assert!(
        matches!(&old, Ok(Ok(()))),
        "previous runtime did not stop cleanly: {old:?}"
    );
    evidence::runtime_released(&admin).await;
    drop(handle);
    let (handle, owner) = runtime(endpoint, installation, attempt).await;
    // Parent owns the successor before generation/login/discovery assertions.
    let mut successor = tokio::spawn(owner.run());
    let admin = Arc::new(tokio::sync::Mutex::new(admin));
    let owned_admin = Arc::clone(&admin);
    let jobs = handle.clone();
    let outcome = tokio::spawn(async move {
        let admin = owned_admin.lock().await;
        checks(
            &jobs, browser, &admin, generation, &before, creation, history,
        )
        .await
    })
    .await;
    let permits = tokio::time::timeout(
        WORK_DEADLINE,
        handle.capacity.clone().acquire_many_owned(CAPACITY as u32),
    )
    .await;
    let settled = matches!(&permits, Ok(Ok(_)));
    drop(permits);
    let admin = Arc::try_unwrap(admin)
        .unwrap_or_else(|_| panic!("unobserved recovery administrative owner"))
        .into_inner();
    if outcome.is_err() || !settled {
        handle.stop();
        let root = tokio::time::timeout(Duration::from_secs(10), &mut successor)
            .await
            .expect("successor runtime cleanup unobserved");
        evidence::runtime_released(&admin).await;
        admin.close().await;
        panic!(
            "restart recovery failed: cases_ok={}, settled={settled}, root={root:?}",
            outcome.is_ok()
        );
    }
    Serving {
        handle,
        browser: outcome.unwrap(),
        admin,
        running: successor,
    }
}
