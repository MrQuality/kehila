use super::*;
use crate::postgres::tests::fixture_admin::Admin;

pub(super) const ARTIFACTS: &[&str] = &[
    "kehila.actors",
    "kehila.operations",
    "kehila.operation_payloads",
    "kehila.projects",
    "kehila.configuration_revisions",
    "kehila.project_grants",
    "kehila.project_grant_events",
    "kehila.statuses",
    "kehila.workflows",
    "kehila.workflow_statuses",
    "kehila.workflow_phase_changes",
    "kehila.work_item_types",
    "kehila.type_workflows",
    "kehila.fields",
    "kehila.items",
    "kehila.item_revisions",
    "kehila.item_field_uses",
    "kehila.project_operational_heads",
    "kehila_auth.pending_items",
    "kehila_auth.item_pending_policy",
    "kehila_auth.pending_projects",
    "kehila_auth.creation_intents",
];

pub(super) async fn snapshot(admin: &Admin) -> Vec<(String, String)> {
    snapshot_without(admin, &[]).await
}

pub(super) async fn pending_domain_snapshot(admin: &Admin) -> Vec<(String, String)> {
    // Exclude request bytea BEFORE SQL JSON aggregation, not after copying it.
    snapshot_without(admin, &["kehila_auth.pending_items"]).await
}

async fn snapshot_without(admin: &Admin, excluded: &[&str]) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for table in ARTIFACTS.iter().filter(|table| !excluded.contains(*table)) {
        let sql = format!("SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text),'[]'::jsonb)::text FROM {table} t");
        result.push((
            (*table).to_owned(),
            admin.client().query_one(&sql, &[]).await.unwrap().get(0),
        ));
    }
    result
}

pub(super) async fn settled(handle: &Handle) {
    let permits = tokio::time::timeout(
        WORK_DEADLINE,
        handle.capacity.clone().acquire_many_owned(CAPACITY as u32),
    )
    .await
    .unwrap()
    .unwrap();
    drop(permits);
}

pub(super) async fn fences_released(admin: &Admin) {
    assert_eq!(admin.client().query_one("SELECT count(*) FROM pg_stat_activity WHERE usename='kehila_auth_runtime_login' AND state IN ('idle in transaction','idle in transaction (aborted)')", &[])
        .await.unwrap().get::<_, i64>(0), 0);
    // The administrative connection inherits finite statement/lock timeouts.
    // Acquiring the same installation -> actor row locks independently is the
    // release control; the live authority's session advisory lock is retained.
    admin.client().batch_execute("BEGIN; SET LOCAL ROLE kehila_owner; SELECT * FROM kehila_auth.installation FOR UPDATE; SELECT * FROM kehila.actors WHERE actor_id='owner' FOR UPDATE; ROLLBACK").await.unwrap();
}

pub(super) async fn runtime_released(admin: &Admin) {
    // Joining the local drivers closes their sockets; PostgreSQL observes
    // Terminate and removes its backends independently. Observe both releases
    // within a fixed bound before restoring fixtures or starting another owner.
    let mut observed = None;
    let released = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            admin
                .client()
                .batch_execute("SELECT pg_stat_clear_snapshot()")
                .await
                .unwrap();
            let row = admin
                .client()
                .query_one(
                    "SELECT (SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() \
                 AND usename='kehila_auth_runtime_login'), (SELECT count(*) FROM pg_locks \
                 WHERE locktype='advisory' AND database=(SELECT oid FROM pg_database \
                 WHERE datname=current_database()) AND classid=1262831689::oid \
                 AND objid=1::oid AND objsubid=2)",
                    &[],
                )
                .await
                .unwrap();
            let counts = (row.get::<_, i64>(0), row.get::<_, i64>(1));
            observed = Some(counts);
            if counts == (0, 0) {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    assert!(
        released.is_ok(),
        "runtime backend/authority lock release not observed: {observed:?}"
    );
}
