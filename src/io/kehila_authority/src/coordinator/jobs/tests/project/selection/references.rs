//! Privileged disposable-fixture controls; no product archive/admin route.
use super::*;

pub(super) async fn exercise(handle: &Handle, browser: &Browser, admin: &Admin, target: &Value) {
    let before = evidence::snapshot(admin).await;
    assert_eq!(
        selection(
            handle,
            Some(browser),
            "set",
            command(
                '3',
                0,
                json!({"project_id":"missing-project","item_id":"missing-item"})
            ),
            403
        )
        .await,
        json!({"error":"forbidden"})
    );
    assert_eq!(
        selection(
            handle,
            Some(browser),
            "set",
            command(
                '4',
                0,
                json!({"project_id":target["project_id"],"item_id":"missing-item"})
            ),
            404
        )
        .await,
        json!({"codec_version":1,"outcome":"rejected","code":"invalid_reference"})
    );
    let project = target["project_id"].as_str().unwrap();
    admin
        .client()
        .execute(
            "UPDATE kehila.projects SET archived=true WHERE project_id=$1::text::kehila.ident",
            &[&project],
        )
        .await
        .unwrap();
    let (status, value, _) = call(
        handle,
        Some(browser),
        "/selection/set",
        Some(command('5', 0, target.clone())),
    )
    .await;
    // Restore before assertions; outer fixture owns cleanup if an earlier helper
    // panics. This does not expose archival or relax any schema/trust guard.
    admin
        .client()
        .execute(
            "UPDATE kehila.projects SET archived=false WHERE project_id=$1::text::kehila.ident",
            &[&project],
        )
        .await
        .unwrap();
    assert_eq!(
        (status, value),
        (
            409,
            json!({"codec_version":1,"outcome":"rejected","code":"archived_target"})
        )
    );
    assert_eq!(
        evidence::snapshot(admin).await,
        before,
        "rejected references must create no partial success or unrelated change"
    );
    evidence::fences_released(admin).await;
}
