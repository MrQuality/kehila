//! Serial privileged disposable-fixture controls; no host clock or runtime API.
use super::*;
use crate::{lifecycle::probe::Point, postgres::tests::fixture_admin::Admin};

const INSTALL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/integration/native_clock_install.sql"
));
const RESTORE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/integration/native_clock_restore.sql"
));
async fn set_clock(admin: &Admin, value: u64) {
    admin
        .client()
        .execute(
            "UPDATE kehila_native_fixture.clock SET value=$1::text::numeric WHERE singleton",
            &[&value.to_string()],
        )
        .await
        .unwrap();
}
async fn denied(admin: &Admin, sql: &str) {
    runtime_error(admin, sql, "42501").await;
}
async fn runtime_error(admin: &Admin, sql: &str, code: &str) {
    admin
        .client()
        .batch_execute("SET ROLE kehila_auth_runtime")
        .await
        .unwrap();
    let error = admin.client().batch_execute(sql).await.unwrap_err();
    assert_eq!(error.code().unwrap().code(), code);
    admin.client().batch_execute("RESET ROLE").await.unwrap();
}
async fn scalar(admin: &Admin, sql: &str) -> i64 {
    admin.client().query_one(sql, &[]).await.unwrap().get(0)
}
fn spawn(
    handle: &Handle,
    browser: &Browser,
    path: &'static str,
    body: Value,
) -> tokio::task::JoinHandle<(u16, Value)> {
    let handle = handle.clone();
    let browser = browser.clone();
    tokio::spawn(async move { project(&handle, &browser, path, body).await })
}
#[test]
#[ignore = "requires the native journey baseline and explicit disposable administrative fixture"]
fn native_clock_retention_and_authority() {
    let (endpoint, installation, attempt, controls) = fixture();
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let admin=Admin::connect(&endpoint).await;
        assert_eq!(scalar(&admin,"SELECT count(*) FROM kehila.projects").await,3);
        let (handle,owner)=runtime(&endpoint,&installation,format!("{attempt}-clock")).await;
        let running=tokio::spawn(owner.run());let browser=login(&handle).await;
        let (status,listed)=project(&handle,&browser,"list",json!({"before":null})).await;
        assert_eq!(status,200);
        let entry=listed["entries"].as_array().unwrap().iter().find(|e|e["state"]=="completed").unwrap().clone();
        let id:i64=entry["request_id"].as_str().unwrap().parse().unwrap();
        let project_id=entry["project_id"].clone();
        let (status,original)=project(&handle,&browser,"reconcile",json!({"request_id":entry["request_id"]})).await;
        assert_eq!(status,200);

        // An actual uncommitted actor-row update blocks execution's fence.
        admin.client().batch_execute("BEGIN; SET LOCAL ROLE kehila_owner; UPDATE kehila.actors SET can_create_project=false WHERE actor_id='owner'").await.unwrap();
        let blocked=spawn(&handle,&browser,"execute",execute(&entry));
        tokio::time::timeout(WORK_DEADLINE,async {
            loop {
                let count=scalar(&admin,"SELECT count(*) FROM pg_stat_activity WHERE usename='kehila_auth_runtime_login' AND cardinality(pg_blocking_pids(pid))>0").await;
                if count>0 {break;}tokio::task::yield_now().await;
            }
        }).await.unwrap();
        admin.client().batch_execute("COMMIT").await.unwrap();
        assert_eq!(blocked.await.unwrap().0,403);
        authorization::denied(&handle,&admin,Some(&browser),&entry,&project_id,&[403],true).await;
        assert_eq!(project(&handle,&browser,"binding",json!({})).await.0,200);
        assert_eq!(project(&handle,&browser,"read",json!({"project_id":project_id})).await.0,200);
        admin.client().batch_execute("SET ROLE kehila_owner; UPDATE kehila.actors SET can_create_project=true WHERE actor_id='owner'; RESET ROLE").await.unwrap();

        // Read uses the earlier authorized repeatable snapshot; a later read
        // must observe the independently committed actor deactivation.
        let gate=handle.live.probe.hold_transaction(0,Point::ReadSnapshot);
        let read=spawn(&handle,&browser,"read",json!({"project_id":project_id}));
        tokio::time::timeout(WORK_DEADLINE,gate.arrived).await.unwrap().unwrap();
        admin.client().batch_execute("SET ROLE kehila_owner; UPDATE kehila.actors SET active=false WHERE actor_id='owner'; RESET ROLE").await.unwrap();
        gate.release.send(()).unwrap();assert_eq!(read.await.unwrap().0,200);
        assert_eq!(project(&handle,&browser,"read",json!({"project_id":project_id})).await.0,401);
        authorization::denied(&handle,&admin,Some(&browser),&entry,&project_id,&[401],false).await;
        admin.client().batch_execute("SET ROLE kehila_owner; UPDATE kehila.actors SET active=true WHERE actor_id='owner'; RESET ROLE").await.unwrap();

        admin.client().batch_execute(INSTALL).await.unwrap();
        denied(&admin,"UPDATE kehila_native_fixture.clock SET value=0").await;
        denied(&admin,"SELECT * FROM kehila_native_fixture.clock").await;
        denied(&admin,"SELECT kehila_auth.retire_project_payload('installation',1)").await;
        denied(&admin,"ALTER FUNCTION kehila_auth.trusted_millis() SECURITY INVOKER").await;
        let base:u64=admin.client().query_one("SELECT kehila_auth.trusted_millis()::text",&[]).await.unwrap().get::<_,String>(0).parse().unwrap();
        admin.client().batch_execute("UPDATE kehila_native_fixture.clock SET value=-1").await.unwrap();
        runtime_error(&admin,"SELECT kehila_auth.trusted_millis()","22003").await;
        admin.client().batch_execute("DELETE FROM kehila_native_fixture.clock").await.unwrap();
        runtime_error(&admin,"SELECT kehila_auth.trusted_millis()","P0002").await;
        admin.client().execute("INSERT INTO kehila_native_fixture.clock VALUES(true,$1::text::numeric)",&[&base.to_string()]).await.unwrap();
        let (status,pending)=project(&handle,&browser,"register",json!({"name":"Expiry","prefix":"EX","estimate_unit":"hours"})).await;
        assert_eq!(status,201);let pending=&pending["entry"];
        let expires=pending["expires_ms"].as_str().unwrap().parse().unwrap();
        set_clock(&admin,expires).await;
        let (status,expired)=project(&handle,&browser,"execute",execute(pending)).await;
        assert_eq!(status,410);assert_eq!(expired["entry"]["state"],"expired");
        set_clock(&admin,base).await;
        assert_eq!(project(&handle,&browser,"execute",execute(pending)).await.0,410);

        let (status,pending)=project(&handle,&browser,"register",json!({"name":"Completion expiry","prefix":"CE","estimate_unit":"hours"})).await;
        assert_eq!(status,201);let pending=&pending["entry"];
        let gate=handle.live.probe.hold_transaction(pending["request_id"].as_str().unwrap().parse().unwrap(),Point::BeforeCompletion);
        let writing=spawn(&handle,&browser,"execute",execute(pending));
        tokio::time::timeout(WORK_DEADLINE,gate.arrived).await.unwrap().unwrap();
        set_clock(&admin,pending["expires_ms"].as_str().unwrap().parse().unwrap()).await;
        gate.release.send(()).unwrap();
        let (status,expired)=writing.await.unwrap();assert_eq!(status,410);assert_eq!(expired["entry"]["state"],"expired");
        for table in ["projects","operations","operation_payloads"] {
            assert_eq!(scalar(&admin,&format!("SELECT count(*) FROM kehila.{table}")).await,3);
        }

        let row=admin.client().query_one("SELECT o.operation_row_id,o.replay_origin_ms::text,o.replay_deadline_ms::text FROM kehila.operations o JOIN kehila_auth.creation_intents i USING(operation_row_id) WHERE i.request_id=$1",&[&id]).await.unwrap();
        let operation:i64=row.get(0);let origin:u64=row.get::<_,String>(1).parse().unwrap();let end:u64=row.get::<_,String>(2).parse().unwrap();
        set_clock(&admin,origin-1).await;
        let (status,unavailable)=project(&handle,&browser,"reconcile",json!({"request_id":entry["request_id"]})).await;
        assert_eq!(status,503);assert!(unavailable.get("result").is_none());assert!(handle.live.is_live());
        set_clock(&admin,origin+1).await;
        let (status,replayed)=project(&handle,&browser,"reconcile",json!({"request_id":entry["request_id"]})).await;
        assert_eq!(status,200);assert_eq!(replayed["result"],original["result"]);
        set_clock(&admin,end).await;
        let (status,expired)=project(&handle,&browser,"reconcile",json!({"request_id":entry["request_id"]})).await;
        assert_eq!(status,410);assert_eq!(expired["error"],"replay_expired");
        assert_eq!(scalar(&admin,"SELECT count(*) FROM kehila.operation_payloads").await,3);
        admin.client().batch_execute("SET ROLE kehila_owner").await.unwrap();
        admin.client().execute("SELECT kehila_auth.retire_project_payload('installation',$1)",&[&operation]).await.unwrap();
        admin.client().batch_execute("RESET ROLE").await.unwrap();
        assert_eq!(scalar(&admin,"SELECT count(*) FROM kehila.operation_payloads").await,2);
        set_clock(&admin,origin+1).await;
        let (status,expired)=project(&handle,&browser,"reconcile",json!({"request_id":entry["request_id"]})).await;
        assert_eq!(status,410);assert_eq!(expired["error"],"replay_expired");assert!(expired["entry"]["form"].is_null());
        assert_eq!(expired["entry"]["project_id"],project_id);
        admin.client().batch_execute(RESTORE).await.unwrap();
        let (status,page)=project(&handle,&browser,"list",json!({"before":null})).await;
        assert_eq!(status,200);
        let full=page["entries"].as_array().unwrap().iter().find(|e|e["state"]=="completed" && !e["form"].is_null()).unwrap();
        let (status,full_result)=project(&handle,&browser,"reconcile",json!({"request_id":full["request_id"]})).await;
        assert_eq!(status,200);
        let (status,full_read)=project(&handle,&browser,"read",json!({"project_id":full["project_id"]})).await;
        assert_eq!(status,200);
        let (status,retired_read)=project(&handle,&browser,"read",json!({"project_id":project_id})).await;
        assert_eq!(status,200);
        let generation:String=admin.client().query_one("SELECT runtime_generation::text FROM kehila_auth.installation",&[]).await.unwrap().get(0);
        let record=json!({"cookie":browser.cookie,"csrf":browser.csrf,"source_generation":generation,
            "full":full_result,"full_read":full_read,"retired_id":entry["request_id"],
            "retired_project":project_id,"retired_read":retired_read});
        {
            use std::{io::Write,os::unix::fs::OpenOptionsExt};
            let bytes=Zeroizing::new(serde_json::to_vec(&record).unwrap());
            let mut output=std::fs::OpenOptions::new().create_new(true).write(true).mode(0o600)
                .open(controls.join("restore-control.json")).unwrap();
            output.write_all(&bytes).unwrap();output.sync_all().unwrap();
        }
        handle.stop();running.await.unwrap().unwrap();admin.close().await;
        mark(&controls,"clock-passed");
    });
}
