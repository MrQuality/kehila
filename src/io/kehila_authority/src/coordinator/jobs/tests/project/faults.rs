//! Fault disposition after successful owned cleanup, distinct from cleanup failure.
use super::*;
use crate::postgres::tests::fixture_admin::Admin;

async fn released(admin: &Admin) {
    evidence::fences_released(admin).await;
    let row = admin
        .client()
        .query_one(
            "SELECT NOT EXISTS (SELECT FROM pg_stat_activity WHERE datname=current_database() \
         AND usename='kehila_auth_runtime_login'), NOT EXISTS (SELECT FROM pg_locks \
         WHERE database=(SELECT oid FROM pg_database WHERE datname=current_database()) \
         AND locktype='advisory' AND classid=1262831689::oid)",
            &[],
        )
        .await
        .unwrap();
    assert!(row.get::<_, bool>(0));
    assert!(row.get::<_, bool>(1));
}

pub(super) async fn execution_panic(
    endpoint: &crate::postgres::Endpoint,
    installation: &str,
    attempt: &str,
) {
    let admin = Admin::connect(endpoint).await;
    let before = evidence::snapshot(&admin).await;
    let (handle, mut owner) =
        runtime(endpoint, installation, format!("{attempt}-execution-panic")).await;
    let admissions = handle.live.probe.snapshot();
    let permit = handle.reserve(deadline()).unwrap().permit;
    let live = handle.live.clone();
    // Put a real failing task in the owner's JoinSet before any SQL admission.
    let task = owner.jobs.spawn(async move {
        let _abandoned = StopOnAbandonment {
            live,
            completed: false,
        };
        panic!("controlled owned execution failure");
    });
    owner.slots.push((task.id(), permit));
    let result = owner.run().await;
    assert!(!handle.live.is_live());
    assert_eq!(
        handle.live.failure(),
        Some(crate::lifecycle::FatalCause::OwnedExecution)
    );
    assert_eq!(handle.capacity.available_permits(), CAPACITY);
    assert_eq!(
        handle.live.probe.snapshot().database_ownership_entries,
        admissions.database_ownership_entries,
    );
    assert_eq!(evidence::snapshot(&admin).await, before);
    released(&admin).await;
    admin.close().await;
    assert!(
        result.is_err(),
        "execution failure was reported as a clean stop"
    );
}

pub(super) async fn named_constraint(
    endpoint: &crate::postgres::Endpoint,
    installation: &str,
    attempt: &str,
) {
    let (handle, owner) = runtime(
        endpoint,
        installation,
        format!("{attempt}-named-constraint"),
    )
    .await;
    let running = tokio::spawn(owner.run());
    let browser = login(&handle).await;
    let admin = Admin::connect(endpoint).await;
    let before = evidence::snapshot(&admin).await;
    admin
        .client()
        .batch_execute(
            "CREATE FUNCTION kehila_native_fixture.reject_named_pending() RETURNS trigger \
         LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'controlled named constraint' \
         USING ERRCODE='23514', CONSTRAINT='native_named_pending'; END $$; \
         CREATE TRIGGER native_named_pending BEFORE INSERT ON kehila_auth.pending_projects \
         FOR EACH ROW EXECUTE FUNCTION kehila_native_fixture.reject_named_pending()",
        )
        .await
        .unwrap();
    let crate::native_ingress::Input::Register(form) = crate::native_ingress::decode(
        crate::native_ingress::Route::Register,
        br#"{"name":"Rejected registration","prefix":"RJ","estimate_unit":"hours"}"#,
    )
    .unwrap() else {
        panic!("register form expected")
    };
    let _ = handle.live.probe.take_sql_state();
    let response = request(
        &handle,
        Operation::Project {
            credential: admission::credential(&browser),
            command: crate::project::Command::Register {
                form,
                handle: "1".repeat(32),
                token: "2".repeat(32),
            },
        },
    )
    .await;
    assert!(matches!(&response.result, Err(Error::Unavailable)));
    assert_eq!(handle.live.probe.take_sql_state().as_deref(), Some("23514"));
    assert!(!handle.live.is_live());
    assert_eq!(
        handle.live.failure(),
        Some(crate::lifecycle::FatalCause::NativeIntegrity)
    );
    drop(response);
    let result = running.await.unwrap();
    admin
        .client()
        .batch_execute(
            "DROP TRIGGER native_named_pending ON kehila_auth.pending_projects; \
         DROP FUNCTION kehila_native_fixture.reject_named_pending()",
        )
        .await
        .unwrap();
    assert_eq!(handle.capacity.available_permits(), CAPACITY);
    assert_eq!(evidence::snapshot(&admin).await, before);
    released(&admin).await;
    admin.close().await;
    assert!(
        result.is_err(),
        "named-constraint failure was reported as a clean stop"
    );
}
