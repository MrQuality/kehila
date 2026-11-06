//! Last control: fail the actual confirmed rollback after known authorization.
use super::*;
pub(super) async fn case(handle: &Handle, browser: &Browser, admin: &Admin, creation: &Value) {
    let origin=admin.client().query_one("SELECT o.replay_origin_ms::text FROM kehila.operations o JOIN kehila_auth.pending_items p USING(operation_row_id) WHERE o.command_family='item_create' AND p.project_id=$1::text::kehila.ident AND p.intent_handle=$2::text::kehila_auth.intent_key",
        &[&creation["project_id"].as_str().unwrap(),&creation["intent_handle"].as_str().unwrap()]).await.unwrap().get::<_,String>(0).parse::<u64>().unwrap();
    at(admin, origin + 1).await;
    handle.live.probe.take_item_replay();
    assert!(handle.live.is_live());
    assert_eq!(handle.live.failure(), None);
    let gate = handle
        .live
        .probe
        .hold_transaction(0, Point::ItemReplayCleanup);
    let deadline = Instant::now() + WORK_DEADLINE;
    let reply = submitted(
        handle,
        browser,
        crate::item_mutation_ingress::Route::ReconcileCreate,
        creation,
        deadline,
    )
    .await;
    tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), gate.arrived)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        handle.live.probe.take_item_replay(),
        Some(Observation::Authorized)
    );
    let backend = controls::authorized(admin).await;
    controls::terminate_held(admin, backend).await;
    gate.release.send(()).unwrap();
    let mut response = tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), reply)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        handle.live.probe.take_item_replay(),
        Some(Observation::RollbackFailed),
        "backend loss must reach the actual rollback error, not merely a generic unavailable reply"
    );
    // Inspect the underlying owned result as well as its delivery guard. A live
    // state failure alone could suppress a mistakenly retained successful body.
    assert!(matches!(&response.result, Err(Error::Unavailable)));
    assert!(matches!(response.result(), Err(Error::Unavailable)));
    assert!(response.confirm_exchange().is_err());
    drop(response);
    assert!(!handle.live.is_live());
    assert_eq!(
        handle.live.failure(),
        Some(crate::lifecycle::FatalCause::OwnedExecution)
    );
}
