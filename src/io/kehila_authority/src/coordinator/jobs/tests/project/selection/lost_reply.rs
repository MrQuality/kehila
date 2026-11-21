//! Real TCP disconnect after admitted writes reach their pre-COMMIT barrier.
use super::*;
pub(super) async fn exercise(handle: &Handle, browser: &Browser, admin: &Admin, request: &Value) {
    let gate = handle
        .live
        .probe
        .hold_transaction(0, crate::lifecycle::probe::Point::SelectionBeforeCommit);
    let request = wire_request(Some(browser), "/selection/set", Some(request.clone()));
    crate::listener::tests::browser_disconnect_before_reply(handle.clone(), request, gate).await;
    evidence::settled(handle).await;
    assert!(
        handle.live.is_live(),
        "lost response must not retire confirmed selection authority"
    );
    current(admin, 5, 12, None).await;
    evidence::fences_released(admin).await;
}
