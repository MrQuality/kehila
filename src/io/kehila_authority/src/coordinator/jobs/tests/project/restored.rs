//! Quiescent synthetic copy in one disposable cluster, not supported recovery.
use super::*;
use crate::postgres::tests::fixture_admin::Admin;

#[test]
#[ignore = "requires the explicit quiescent native fixture copy and retained source controls"]
fn native_restored_coherence() {
    let (source, installation, attempt, controls) = fixture();
    let endpoint = source.copied_fixture_database("kehila_native_restored");
    let mut bytes = Zeroizing::new(Vec::new());
    use std::io::Read;
    std::fs::File::open(controls.join("restore-control.json"))
        .unwrap()
        .take(1048577)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(bytes.len() <= 1048576);
    let expected: Value = serde_json::from_slice(&bytes).unwrap();
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let admin=Admin::connect(&endpoint).await;
        let old:u64=expected["source_generation"].as_str().unwrap().parse().unwrap();
        let copied:String=admin.client().query_one("SELECT runtime_generation::text FROM kehila_auth.installation",&[]).await.unwrap().get(0);
        assert_eq!(copied.parse::<u64>().unwrap(),old);
        let (handle,owner)=runtime(&endpoint,&installation,format!("{attempt}-restored")).await;
        let running=tokio::spawn(owner.run());
        let fresh:String=admin.client().query_one("SELECT runtime_generation::text FROM kehila_auth.installation",&[]).await.unwrap().get(0);
        assert_eq!(fresh.parse::<u64>().unwrap(),old+1);
        let previous=Browser {cookie:expected["cookie"].as_str().unwrap().to_owned(),csrf:expected["csrf"].as_str().unwrap().to_owned()};
        assert_eq!(project(&handle,&previous,"list",json!({"before":null})).await.0,401);
        let browser=login(&handle).await;
        let full=&expected["full"];
        let (status,replayed)=project(&handle,&browser,"reconcile",json!({"request_id":full["entry"]["request_id"]})).await;
        assert_eq!(status,200);assert_eq!(&replayed,full);
        let (status,read)=project(&handle,&browser,"read",json!({"project_id":full["entry"]["project_id"]})).await;
        assert_eq!(status,200);assert_eq!(read,expected["full_read"]);
        let (status,expired)=project(&handle,&browser,"reconcile",json!({"request_id":expected["retired_id"]})).await;
        assert_eq!(status,410);assert_eq!(expired["error"],"replay_expired");
        assert_eq!(expired["entry"]["project_id"],expected["retired_project"]);assert!(expired["entry"]["form"].is_null());
        let (status,read)=project(&handle,&browser,"read",json!({"project_id":expected["retired_project"]})).await;
        assert_eq!(status,200);assert_eq!(read,expected["retired_read"]);
        // A copied noncycling sequence must not issue a previously used ID,
        // including the number of a terminal entry pruned before the dump.
        let sequence=admin.client().query_one("SELECT last_value,is_called FROM kehila_auth.pending_projects_request_id_seq",&[]).await.unwrap();
        let last:i64=sequence.get(0);assert!(sequence.get::<_,bool>(1));
        assert!(!admin.client().query_one("SELECT seqcycle FROM pg_sequence WHERE seqrelid='kehila_auth.pending_projects_request_id_seq'::regclass",&[]).await.unwrap().get::<_,bool>(0));
        let (status,registered)=project(&handle,&browser,"register",json!({"name":"Restored sequence","prefix":"RS","estimate_unit":"hours"})).await;
        assert_eq!(status,201);assert!(registered["entry"]["request_id"].as_str().unwrap().parse::<i64>().unwrap()>last);
        assert_eq!(project(&handle,&browser,"cancel",json!({"request_id":registered["entry"]["request_id"]})).await.0,200);
        handle.stop();running.await.unwrap().unwrap();admin.close().await;
        mark(&controls,"restore-passed");
    });
}
