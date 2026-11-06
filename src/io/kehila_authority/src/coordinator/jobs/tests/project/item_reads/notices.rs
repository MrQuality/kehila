//! Synthetic fixed conflict notices through real scope SQL and session gating.
//! No mutation route, grant writer or saved-result release is qualified here.
use super::*;
async fn receive(engine: &Engine, browser: &Browser, project: &str) -> NativeDelivery {
    let credential = admission::credential(browser);
    let notice = engine
        .notice_fixture(&credential, project, Instant::now() + WORK_DEADLINE)
        .unwrap();
    engine
        .deliver_item_notice(&credential, notice)
        .await
        .unwrap_or_else(|_| panic!("authorized notice denied"))
}
fn conflict(delivery: &NativeDelivery) {
    let material = delivery.material().unwrap();
    assert_eq!(material.status, 409);
    assert_eq!(
        serde_json::from_slice::<Value>(material.body.as_slice()).unwrap(),
        json!({"error":"item_conflict"})
    );
}
pub(super) async fn exercise(
    handle: &Handle,
    browser: &Browser,
    admin: &Admin,
    project: &str,
    other: &str,
) {
    let engine = handle.engine.upgrade().unwrap();
    let credential = admission::credential(browser);
    let before = evidence::snapshot(admin).await;
    let delivery = receive(&engine, browser, project).await;
    // Neither a returned body nor its caller retains the snapshot/DB guard.
    let database = tokio::time::timeout(WORK_DEADLINE, engine.database.lock())
        .await
        .unwrap();
    drop(database);
    evidence::fences_released(admin).await;
    conflict(&delivery);
    drop(delivery);
    let missing = engine
        .notice_fixture(
            &credential,
            "missing-notice-scope",
            Instant::now() + WORK_DEADLINE,
        )
        .unwrap();
    let denied = engine.deliver_item_notice(&credential, missing).await;
    assert!(
        matches!(denied,Err(error)if error.generic_error()==Error::Project(crate::project::Failure::Forbidden))
    );
    let notice = engine
        .notice_fixture(&credential, project, Instant::now() + WORK_DEADLINE)
        .unwrap();
    let mut wrong_csrf = admission::credential(browser);
    wrong_csrf.csrf = None;
    assert!(
        matches!(engine.deliver_item_notice(&wrong_csrf,notice).await,
  Err(error)if error.generic_error()==Error::Project(crate::project::Failure::Forbidden))
    );
    evidence::fences_released(admin).await;
    let expired = engine
        .notice_fixture(&credential, project, Instant::now())
        .unwrap();
    let entries = handle.live.probe.snapshot().database_ownership_entries;
    assert!(
        matches!(engine.deliver_item_notice(&credential,expired).await,
  Err(error)if error.generic_error()==Error::Unavailable)
    );
    assert_eq!(
        handle.live.probe.snapshot().database_ownership_entries,
        entries,
        "expired notice entered SQL"
    );
    // Another authorized browser session cannot replace the original session.
    let second = login(handle).await;
    let other_credential = admission::credential(&second);
    let notice = engine
        .notice_fixture(&credential, project, Instant::now() + WORK_DEADLINE)
        .unwrap();
    let entries = handle.live.probe.snapshot().database_ownership_entries;
    assert!(
        matches!(engine.deliver_item_notice(&other_credential,notice).await,
  Err(error)if error.generic_error()==Error::Unauthorized)
    );
    assert_eq!(
        handle.live.probe.snapshot().database_ownership_entries,
        entries
    );
    // A second project remains authorized while the notice's original project is
    // deliberately denied. This cannot substitute that second project's grant.
    let original = engine
        .notice_fixture(&credential, project, Instant::now() + WORK_DEADLINE)
        .unwrap();
    admin
        .client()
        .batch_execute("BEGIN; SET LOCAL session_replication_role=replica")
        .await
        .unwrap();
    assert_eq!(admin.client().execute("UPDATE kehila.project_grants SET active=false WHERE project_id=$1::text::kehila.ident",&[&project]).await.unwrap(),1);
    admin.client().batch_execute("COMMIT").await.unwrap();
    let other_delivery = receive(&engine, browser, other).await;
    conflict(&other_delivery);
    drop(other_delivery);
    assert!(
        matches!(engine.deliver_item_notice(&credential,original).await,
  Err(error)if error.generic_error()==Error::Project(crate::project::Failure::Forbidden))
    );
    admin
        .client()
        .batch_execute("BEGIN; SET LOCAL session_replication_role=replica")
        .await
        .unwrap();
    assert_eq!(admin.client().execute("UPDATE kehila.project_grants SET active=true WHERE project_id=$1::text::kehila.ident",&[&project]).await.unwrap(),1);
    admin.client().batch_execute("COMMIT").await.unwrap();
    // The switch above is trusted disposable-fixture denial, not qualification of
    // a grant-change API. Exact domain restoration is checked below.
    let captured = receive(&engine, &second, project).await;
    conflict(&captured);
    engine.candidate(&other_credential).unwrap().retire();
    assert!(matches!(captured.material(), Err(Error::Unauthorized)));
    drop(captured);
    let deadline = Instant::now() + WORK_DEADLINE;
    let notice = engine
        .notice_fixture(&credential, project, deadline)
        .unwrap();
    let captured = engine
        .deliver_item_notice(&credential, notice)
        .await
        .unwrap_or_else(|_| panic!("notice deadline control"));
    tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
    assert!(matches!(captured.material(), Err(Error::Unauthorized)));
    drop(captured);
    evidence::fences_released(admin).await;
    assert!(
        evidence::snapshot(admin).await == before,
        "notice controls changed domain state"
    );
}
