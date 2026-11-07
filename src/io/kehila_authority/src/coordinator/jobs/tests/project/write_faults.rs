//! Every admitted M1 row-write ordinal, through actual PostgreSQL errors.
//! Commands are sealed directly; HTTP/RNG preparation is qualified separately.
use super::*;
use crate::{native_ingress::Input, postgres::tests::fixture_admin::Admin};

const INSTALL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/integration/native_write_fault_install.sql"
));
const REMOVE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/integration/native_write_fault_remove.sql"
));
const TARGET: &str = "native-write-fault-target";

fn command(browser: &Browser, entry: &Value) -> (Operation, Vec<String>) {
    let input = crate::native_ingress::decode(
        crate::native_ingress::Route::Execute,
        &serde_json::to_vec(&execute(entry)).unwrap(),
    )
    .unwrap();
    let Input::Execute {
        request_id,
        token,
        form,
    } = input
    else {
        unreachable!()
    };
    let prepared =
        crate::project::Prepared::new(form.clone(), TARGET.to_owned(), token.clone()).unwrap();
    let configuration = &prepared.result().configuration;
    let mut trace = vec![
        "kehila.operations INSERT",
        "kehila.operation_payloads INSERT",
        "kehila.projects INSERT",
        "kehila.project_operational_heads INSERT",
        "kehila.configuration_revisions INSERT",
        "kehila.project_grants INSERT",
        "kehila.project_grant_events INSERT",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    for _ in &configuration.statuses {
        trace.push("kehila.statuses INSERT".into());
    }
    for workflow in &configuration.workflows {
        trace.push("kehila.workflows INSERT".into());
        for _ in &workflow.status_ids {
            trace.push("kehila.workflow_statuses INSERT".into());
        }
        for _ in &workflow.permitted_phase_changes {
            trace.push("kehila.workflow_phase_changes INSERT".into());
        }
    }
    for kind in &configuration.types {
        trace.push("kehila.work_item_types INSERT".into());
        for _ in &kind.permitted_workflows {
            trace.push("kehila.type_workflows INSERT".into());
        }
    }
    for _ in &configuration.fields {
        trace.push("kehila.fields INSERT".into());
    }
    trace.extend([
        "kehila_auth.pending_projects UPDATE".into(),
        "kehila_auth.creation_intents INSERT".into(),
    ]);
    let candidates = crate::project::Candidates::new(
        form,
        token,
        [
            TARGET.into(),
            "native-write-unused-two".into(),
            "native-write-unused-three".into(),
        ],
    )
    .unwrap();
    (
        Operation::Project {
            credential: admission::credential(browser),
            command: crate::project::Command::Execute {
                request_id,
                candidates,
            },
        },
        trace,
    )
}

async fn failed(handle: &Handle, browser: &Browser, entry: &Value) {
    let _ = handle.live.probe.take_sql_state();
    let response = request(handle, command(browser, entry).0).await;
    assert!(matches!(response.result(), Err(Error::Unavailable)));
    // native_error is reached after this execution's confirmed rollback and
    // release. PZ001 must not become collision retry, expiry or unknown commit.
    assert_eq!(handle.live.probe.take_sql_state().as_deref(), Some("PZ001"));
    assert!(handle.live.is_live());
    drop(response);
    evidence::settled(handle).await;
}

pub(super) async fn exercise(
    handle: &Handle,
    browser: &Browser,
    entry: &Value,
    endpoint: &crate::postgres::Endpoint,
    controls: &std::path::Path,
) {
    let admin = Admin::connect(endpoint).await;
    assert_eq!(admin.client().query_one("SELECT count(*) FROM kehila.projects WHERE project_id IN ('native-write-fault-target','native-write-unused-two','native-write-unused-three')",&[]).await.unwrap().get::<_,i64>(0),0);
    let baseline = evidence::snapshot(&admin).await;
    let (_, expected) = command(browser, entry);
    let count = i64::try_from(expected.len()).unwrap();
    assert!(count > 0);
    let id: i64 = entry["request_id"].as_str().unwrap().parse().unwrap();
    admin.client().batch_execute(INSTALL).await.unwrap();
    admin
        .client()
        .execute(
            "INSERT INTO kehila_native_fixture.write_control VALUES(true,$1,$2,NULL,true,true)",
            &[&id, &TARGET],
        )
        .await
        .unwrap();
    for privilege in [
        "SELECT has_table_privilege('kehila_auth_runtime','kehila_native_fixture.write_control','INSERT,UPDATE,DELETE')",
        "SELECT has_sequence_privilege('kehila_auth_runtime','kehila_native_fixture.write_ordinal','USAGE,UPDATE')",
        "SELECT has_function_privilege('kehila_auth_runtime','kehila_native_fixture.write_count()','EXECUTE')",
        "SELECT has_function_privilege('kehila_auth_runtime','kehila_native_fixture.write_finish()','EXECUTE')",
    ] {assert!(!admin.client().query_one(privilege,&[]).await.unwrap().get::<_,bool>(0));}
    // Full dry trace rolls back at deferred-check entry after reconstruction.
    failed(handle, browser, entry).await;
    assert_eq!(
        admin
            .client()
            .query_one(
                "SELECT last_value FROM kehila_native_fixture.write_ordinal",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        count
    );
    assert_eq!(evidence::snapshot(&admin).await, baseline);
    evidence::fences_released(&admin).await;
    for ordinal in 1..=count {
        admin.client().execute("UPDATE kehila_native_fixture.write_control SET fail_at=$1,finish_failure=false,trace=false WHERE singleton",&[&ordinal]).await.unwrap();
        admin
            .client()
            .batch_execute("SELECT setval('kehila_native_fixture.write_ordinal',1,false)")
            .await
            .unwrap();
        failed(handle, browser, entry).await;
        assert_eq!(
            admin
                .client()
                .query_one(
                    "SELECT last_value FROM kehila_native_fixture.write_ordinal",
                    &[]
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            ordinal
        );
        assert_eq!(
            evidence::snapshot(&admin).await,
            baseline,
            "write ordinal {ordinal} left artifacts"
        );
        evidence::fences_released(&admin).await;
    }
    admin.client().batch_execute(REMOVE).await.unwrap();
    assert_eq!(evidence::snapshot(&admin).await, baseline);
    {
        use std::{io::Write, os::unix::fs::OpenOptionsExt};
        let mut output = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(controls.join("write-trace.json"))
            .unwrap();
        output
            .write_all(&serde_json::to_vec(&expected).unwrap())
            .unwrap();
        output.sync_all().unwrap();
    }
    admin.close().await;
    // The caller's actual HTTP creation follows with all injection removed.
}
