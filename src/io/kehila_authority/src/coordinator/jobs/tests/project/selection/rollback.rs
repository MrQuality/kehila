//! Inject after all four selection writes, before SQL can return acceptance.
//! The containing fixture owns the disposable database even on failed cleanup.
use super::*;

const INSTALL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/integration/native_selection_fault_install.sql"
));
const REMOVE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/integration/native_selection_fault_remove.sql"
));

pub(super) async fn exercise(handle: &Handle, browser: &Browser, admin: &Admin, target: Value) {
    evidence::settled(handle).await;
    let before = evidence::snapshot(admin).await;
    admin.client().batch_execute(INSTALL).await.unwrap();
    for query in [
        "SELECT has_table_privilege('kehila_auth_runtime','kehila_native_fixture.selection_fault_control','SELECT,INSERT,UPDATE,DELETE')",
        "SELECT has_function_privilege('kehila_auth_runtime','kehila_native_fixture.fail_selection_head()','EXECUTE')",
    ] {
        assert!(!admin.client().query_one(query, &[]).await.unwrap().get::<_, bool>(0));
    }
    let _ = handle.live.probe.take_sql_state();
    let (status, body, _) = call(
        handle,
        Some(browser),
        "/selection/set",
        Some(command('6', 3, target)),
    )
    .await;
    evidence::settled(handle).await;
    let observed_state = handle.live.probe.take_sql_state();
    let after = evidence::snapshot(admin).await;
    let still_live = handle.live.is_live();
    // Remove the owned injection before assertions so a wrong HTTP result does
    // not leave the fixture installed in an otherwise usable test database.
    admin.client().batch_execute(REMOVE).await.unwrap();
    assert_eq!(status, 503);
    assert_eq!(
        observed_state.as_deref(),
        Some("PZ001"),
        "actual head-write failure must activate"
    );
    assert_eq!(body, json!({"error":"request rejected"}));
    assert_eq!(
        after, before,
        "native failure must roll back head, event, core and replay payload together"
    );
    assert!(
        still_live,
        "confirmed selection rollback must leave the runtime usable"
    );
    evidence::fences_released(admin).await;
}
