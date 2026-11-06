//! Pending TTL moves between calls while relational rows remain in one RR snapshot.
use super::*;
use crate::postgres::{
    transaction::{ItemDisposition, ItemPreparation, NativeError},
    Authority, DbError,
};
#[derive(PartialEq, Eq)]
struct ClockDefinition {
    oid: i64,
    owner: i64,
    acl: Option<String>,
    definer: bool,
    volatility: String,
    settings: Option<Vec<String>>,
    definition: String,
}
impl ClockDefinition {
    fn same_authority(&self, other: &Self) -> bool {
        self.oid == other.oid
            && self.owner == other.owner
            && self.acl == other.acl
            && self.definer == other.definer
            && self.volatility == other.volatility
            && self.settings == other.settings
    }
}
async fn definition(admin: &Admin) -> ClockDefinition {
    let row=admin.client().query_one("SELECT oid::bigint,proowner::bigint,proacl::text,prosecdef,provolatile::text,proconfig,CASE WHEN octet_length(pg_get_functiondef(oid))<=65536 THEN pg_get_functiondef(oid) END FROM pg_proc WHERE oid='kehila_auth.trusted_millis()'::regprocedure",&[]).await.unwrap();
    ClockDefinition {
        oid: row.get(0),
        owner: row.get(1),
        acl: row.get(2),
        definer: row.get(3),
        volatility: row.get(4),
        settings: row.get(5),
        definition: row
            .get::<_, Option<String>>(6)
            .expect("bounded clock definition"),
    }
}
const CLEAR:&str="SELECT set_config('kehila_native_fixture.first_ms','',false),set_config('kehila_native_fixture.second_ms','',false),set_config('kehila_native_fixture.clock_step','',false)";
pub(super) async fn exercise(
    owner: &mut Admin,
    admin: &Admin,
    expected: &Authority,
    project: &str,
    handle: &str,
) {
    let physical="SELECT state,registered_ms::text,expires_ms::text,CASE WHEN octet_length(request_bytes) BETWEEN 1 AND 1048576 THEN request_bytes END,request_sha256 FROM kehila_auth.pending_items WHERE project_id=$1::text::kehila.ident AND intent_handle=$2::text::kehila_auth.intent_key LIMIT 1";
    let row = admin
        .client()
        .query_one(physical, &[&project, &handle])
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "prepared");
    let first = row.get::<_, String>(1).parse::<u64>().unwrap();
    let second = row.get::<_, String>(2).parse::<u64>().unwrap();
    assert!(first < second);
    let bytes = row
        .get::<_, Option<Vec<u8>>>(3)
        .expect("physical request required");
    let digest = row.get::<_, Vec<u8>>(4);
    drop(row);
    let original = definition(admin).await;
    admin
        .client()
        .batch_execute(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../tests/integration/native_item_factory_clock.sql"
        )))
        .await
        .unwrap();
    assert!(
        definition(admin).await.same_authority(&original),
        "fixture clock replacement changed identity or authority"
    );
    let deadline = || std::time::Instant::now() + std::time::Duration::from_secs(5);
    let observe = owner.database_observer();
    // An unconfigured owner still observes the ordinary production clock.
    owner.client().batch_execute(CLEAR).await.unwrap();
    let mut snapshot = owner.read_snapshot(expected, deadline()).await.unwrap();
    let result = snapshot.prepare_item_create(project, handle).await;
    snapshot.rollback().await.unwrap();
    assert!(
        matches!(&result, Ok(ItemPreparation::Ready(_))),
        "unconfigured clock must leave this live intent ready"
    );
    drop(result);
    // Partial configuration must fail explicitly, rather than fall back silently.
    owner.client().batch_execute(&format!("SELECT set_config('kehila_native_fixture.first_ms','{first}',false),set_config('kehila_native_fixture.clock_step','0',false)")).await.unwrap();
    let mut snapshot = owner.read_snapshot(expected, deadline()).await.unwrap();
    let result = snapshot.prepare_item_create(project, handle).await;
    snapshot.rollback().await.unwrap();
    assert!(
        matches!(result,Err(NativeError::Database(DbError::Rejected(ref code))) if code=="22023")
    );
    owner.client().batch_execute(&format!("SELECT set_config('kehila_native_fixture.first_ms','{first}',false),set_config('kehila_native_fixture.second_ms','{second}',false),set_config('kehila_native_fixture.clock_step','0',false)")).await.unwrap();
    let mut snapshot = owner.read_snapshot(expected, deadline()).await.unwrap();
    let start = observe();
    let result = snapshot.prepare_item_create(project, handle).await;
    let statements = observe() - start;
    snapshot.rollback().await.unwrap();
    assert_eq!(
        statements, 2,
        "expected reconciliation and Open only; no Current materialization"
    );
    let ItemPreparation::Resolved(result) =
        result.unwrap_or_else(|_| panic!("pending interlookup expiry failed"))
    else {
        panic!("later expiry cannot become a fresh draft")
    };
    assert_eq!(result.outcome(), ItemDisposition::Expired);
    assert_eq!(result.operation(), None);
    assert_eq!(result.target(), Some("factory-target"));
    // The changed samples were session-local clock observations, not committed
    // relational changes visible across snapshots.
    let row = admin
        .client()
        .query_one(physical, &[&project, &handle])
        .await
        .unwrap();
    assert!(
        row.get::<_, String>(0) == "prepared"
            && row.get::<_, String>(1) == first.to_string()
            && row.get::<_, String>(2) == second.to_string()
            && row.get::<_, Option<Vec<u8>>>(3).as_deref() == Some(bytes.as_slice())
            && row.get::<_, Vec<u8>>(4) == digest,
        "pending row changed during readonly clock control"
    );
    drop(row);
    admin
        .client()
        .batch_execute(&original.definition)
        .await
        .unwrap();
    assert!(
        definition(admin).await == original,
        "exact clock baseline was not restored"
    );
    owner.client().batch_execute(CLEAR).await.unwrap();
}
