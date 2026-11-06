//! Explicit disposable-owner corruption and selected startup-profile controls.
use super::*;

pub(super) async fn exercise(
    endpoint: &crate::postgres::Endpoint,
    installation: &str,
    attempt: &str,
    admin: &Admin,
    project: &str,
) {
    let before = evidence::snapshot(admin).await;
    admin
        .client()
        .batch_execute("SET ROLE kehila_auth_runtime")
        .await
        .unwrap();
    let denied = admin
        .client()
        .query(
            "SELECT kehila_auth.save_item_create(NULL,NULL,NULL,NULL,NULL,NULL)",
            &[],
        )
        .await
        .unwrap_err();
    assert_eq!(denied.as_db_error().unwrap().code().code(), "42501");
    admin.client().batch_execute("RESET ROLE").await.unwrap();
    for (index, target) in ["read-fixture-2", "read-fixture-1"].into_iter().enumerate() {
        let original:String=admin.client().query_one("SELECT r.history::text FROM kehila.items i JOIN kehila.item_revisions r ON r.project_id=i.project_id AND r.item_id=i.item_id AND r.item_version=i.current_version WHERE i.item_id=$1::text::kehila.ident",&[&target]).await.unwrap().get(0);
        // A transaction-local superuser fixture switch bypasses normal history
        // protection solely to construct committed corrupt read input.
        admin
            .client()
            .batch_execute("BEGIN; SET LOCAL session_replication_role=replica")
            .await
            .unwrap();
        assert_eq!(admin.client().execute("UPDATE kehila.item_revisions r SET history=jsonb_set(history,'{value,result,value,project,name}','\"corrupt fixture\"'::jsonb) FROM kehila.items i WHERE r.project_id=i.project_id AND r.item_id=i.item_id AND r.item_version=i.current_version AND i.item_id=$1::text::kehila.ident",&[&target]).await.unwrap(),1);
        admin.client().batch_execute("COMMIT").await.unwrap();
        let (handle, owner) =
            runtime(endpoint, installation, format!("{attempt}-corrupt-{index}")).await;
        let running = tokio::spawn(owner.run());
        let browser = login(&handle).await;
        let response = request(
            &handle,
            Operation::Item {
                credential: admission::credential(&browser),
                input: crate::item_ingress::Input::Read {
                    project: project.into(),
                    item: target.into(),
                },
            },
        )
        .await;
        assert!(matches!(response.result(), Err(Error::Unavailable)));
        assert_eq!(
            handle.live.failure(),
            Some(crate::lifecycle::FatalCause::NativeIntegrity)
        );
        drop(response);
        assert!(running.await.unwrap().is_err());
        evidence::runtime_released(admin).await;
        admin
            .client()
            .batch_execute("BEGIN; SET LOCAL session_replication_role=replica")
            .await
            .unwrap();
        assert_eq!(admin.client().execute("UPDATE kehila.item_revisions r SET history=$1::text::jsonb FROM kehila.items i WHERE r.project_id=i.project_id AND r.item_id=i.item_id AND r.item_version=i.current_version AND i.item_id=$2::text::kehila.ident",&[&original,&target]).await.unwrap(),1);
        admin.client().batch_execute("COMMIT").await.unwrap();
        let error=admin.client().execute("UPDATE kehila.item_revisions SET history=history WHERE item_id=$1::text::kehila.ident",&[&target]).await.unwrap_err();
        let error = error.as_db_error().unwrap();
        assert_eq!(error.code().code(), "23514");
        assert_eq!(error.constraint(), Some("item_history_immutable"));
        assert!(
            evidence::snapshot(admin).await == before,
            "corruption fixture did not restore exact domain state"
        );
    }
    admin.client().batch_execute("CREATE TEMP TABLE native_migration_backup AS SELECT * FROM kehila.schema_migrations WHERE version=3").await.unwrap();
    for (index,(change,restore)) in [
        ("DELETE FROM kehila.schema_migrations WHERE version=3", "INSERT INTO kehila.schema_migrations SELECT * FROM pg_temp.native_migration_backup"),
        ("INSERT INTO kehila.schema_migrations(version,description) VALUES(4,'unsupported fixture')", "DELETE FROM kehila.schema_migrations WHERE version=4"),
        ("GRANT UPDATE ON kehila.items TO kehila_auth_runtime", "REVOKE UPDATE ON kehila.items FROM kehila_auth_runtime"),
        ("REVOKE SELECT ON kehila.items FROM kehila_auth_runtime", "GRANT SELECT ON kehila.items TO kehila_auth_runtime"),
        ("GRANT EXECUTE ON FUNCTION kehila_auth.save_item_create(kehila_auth.authority,kehila.ident,kehila_auth.intent_key,bytea,kehila_auth.item_preparation,jsonb) TO kehila_auth_runtime", "REVOKE EXECUTE ON FUNCTION kehila_auth.save_item_create(kehila_auth.authority,kehila.ident,kehila_auth.intent_key,bytea,kehila_auth.item_preparation,jsonb) FROM kehila_auth_runtime"),
        ("ALTER FUNCTION kehila_auth.item_read_authority(kehila_auth.authority,kehila.ident) OWNER TO kehila_auth_admin", "ALTER FUNCTION kehila_auth.item_read_authority(kehila_auth.authority,kehila.ident) OWNER TO kehila_owner"),
    ].into_iter().enumerate() {
        admin.client().batch_execute(change).await.unwrap();
        let result=Database::start(endpoint,installation.into(),format!("{attempt}-profile-{index}"),Instant::now()+WORK_DEADLINE).await;
        assert!(matches!(result,Err(DbError::Invalid)),"invalid startup profile was not rejected as invalid");
        evidence::runtime_released(admin).await; admin.client().batch_execute(restore).await.unwrap();
    }
    admin
        .client()
        .batch_execute("DROP TABLE pg_temp.native_migration_backup")
        .await
        .unwrap();
    // Prove restored capabilities are usable, rather than stopping at negative
    // startup observations or a possibly damaged restoration.
    let (handle, owner) = runtime(endpoint, installation, format!("{attempt}-restored")).await;
    let running = tokio::spawn(owner.run());
    let browser = login(&handle).await;
    assert_eq!(
        item(
            &handle,
            &browser,
            "read",
            json!({"project_id":project,"item_id":"read-fixture-2"})
        )
        .await
        .0,
        200
    );
    handle.stop();
    running.await.unwrap().unwrap();
    evidence::runtime_released(admin).await;
    assert!(
        evidence::snapshot(admin).await == before,
        "startup controls changed domain artifacts"
    );
}
