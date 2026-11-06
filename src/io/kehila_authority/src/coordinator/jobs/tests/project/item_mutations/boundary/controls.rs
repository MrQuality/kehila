//! Exact backend observations and an owned, assertion-free maintenance task.
use super::*;

pub(super) struct Backend {
    pid: i32,
    started: String,
    transaction: String,
}
impl Backend {
    pub(super) fn pid(&self) -> i32 {
        self.pid
    }
}
const AUTHORIZED:&str="SELECT pid,backend_start::text,xact_start::text FROM pg_stat_activity WHERE datname=current_database() AND usename='kehila_auth_runtime_login' AND state='idle in transaction' AND query LIKE '% FROM kehila_auth.authorize_item_replay(%'";
const FENCE_WAITER:&str="SELECT pid,backend_start::text,xact_start::text FROM pg_stat_activity WHERE datname=current_database() AND usename='kehila_auth_runtime_login' AND state='active' AND query LIKE '% FROM kehila_auth.fence(%' AND $1=ANY(pg_blocking_pids(pid))";
pub(super) async fn authorized(admin: &Admin) -> Backend {
    admin
        .client()
        .batch_execute("SELECT pg_stat_clear_snapshot()")
        .await
        .unwrap();
    let rows = admin.client().query(AUTHORIZED, &[]).await.unwrap();
    assert_eq!(
        rows.len(),
        1,
        "one exact held replay-authorizer backend required"
    );
    Backend {
        pid: rows[0].get(0),
        started: rows[0].get(1),
        transaction: rows[0].get(2),
    }
}
pub(super) async fn fence_waiter(admin: &Admin, blocker: i32) -> Backend {
    tokio::time::timeout(WORK_DEADLINE, async {
        loop {
            admin
                .client()
                .batch_execute("SELECT pg_stat_clear_snapshot()")
                .await
                .unwrap();
            let rows = admin
                .client()
                .query(FENCE_WAITER, &[&blocker])
                .await
                .unwrap();
            assert!(rows.len() <= 1, "ambiguous serving fence waiter");
            if let Some(row) = rows.first() {
                return Backend {
                    pid: row.get(0),
                    started: row.get(1),
                    transaction: row.get(2),
                };
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("exact serving fence blocker was not observed")
}
pub(super) async fn blocked_by(admin: &Admin, waiter: i32, blocker: &Backend) {
    tokio::time::timeout(WORK_DEADLINE,async{
        loop {
            admin.client().batch_execute("SELECT pg_stat_clear_snapshot()").await.unwrap();
            let held=admin.client().query_one("SELECT EXISTS(SELECT FROM pg_stat_activity WHERE pid=$1 AND backend_start::text=$2 AND xact_start::text=$3 AND datname=current_database() AND usename='kehila_auth_runtime_login'), $1=ANY(pg_blocking_pids($4))",
                &[&blocker.pid,&blocker.started,&blocker.transaction,&waiter]).await.unwrap();
            assert!(held.get::<_,bool>(0),"the held serving transaction changed before blocking was observed");
            if held.get::<_,bool>(1){return}
            tokio::task::yield_now().await;
        }
    }).await.expect("exact retirement blocker was not observed")
}
pub(super) async fn terminate_held(admin: &Admin, backend: Backend) {
    // Revalidate the same connection AND transaction in the killing statement.
    let rows=admin.client().query("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE pid=$1 AND backend_start::text=$2 AND xact_start::text=$3 AND datname=current_database() AND usename='kehila_auth_runtime_login' AND state='idle in transaction' AND query LIKE '% FROM kehila_auth.authorize_item_replay(%'",
        &[&backend.pid,&backend.started,&backend.transaction]).await.unwrap();
    assert_eq!(
        rows.len(),
        1,
        "held authorizer identity changed before termination"
    );
    assert!(rows[0].get::<_, bool>(0));
    tokio::time::timeout(WORK_DEADLINE,async{
        loop {
            admin.client().batch_execute("SELECT pg_stat_clear_snapshot()").await.unwrap();
            if admin.client().query_one("SELECT NOT EXISTS(SELECT FROM pg_stat_activity WHERE pid=$1 AND backend_start::text=$2)",
                &[&backend.pid,&backend.started]).await.unwrap().get::<_,bool>(0){return}
            tokio::task::yield_now().await;
        }
    }).await.expect("terminated replay backend disappearance was not observed");
}

#[derive(Debug)]
pub(super) enum MaintenancePrimary {
    Sql(tokio_postgres::Error),
    DecisionLost,
}
#[derive(Debug)]
pub(super) struct MaintenanceFailure {
    pub(super) primary: MaintenancePrimary,
    pub(super) rollback: Option<tokio_postgres::Error>,
}
pub(super) struct Maintenance {
    pub(super) started: tokio::sync::oneshot::Receiver<i32>,
    pub(super) staged: tokio::sync::oneshot::Receiver<bool>,
    pub(super) commit: tokio::sync::oneshot::Sender<()>,
    pub(super) finished: tokio::sync::oneshot::Receiver<Result<(), MaintenanceFailure>>,
}
pub(super) fn retire(
    admin: Admin,
    installation: String,
    operation: i64,
    exchanges: &Exchanges,
) -> Maintenance {
    let (identified, started) = tokio::sync::oneshot::channel();
    let (staged, ready) = tokio::sync::oneshot::channel();
    let (commit, decision) = tokio::sync::oneshot::channel();
    let (done, finished) = tokio::sync::oneshot::channel();
    exchanges.lock().unwrap().spawn(async move {
        // No assertions in maintenance work. Preserve its primary failure and
        // always attempt transaction cleanup, then close/join the connection,
        // before publishing. A dropped parent decision requests rollback.
        let primary = async {
            // Observe the PID only after registration: cancellation or query
            // failure before BEGIN still belongs to this cleanup owner.
            let row = admin
                .client()
                .query_one("SELECT pg_backend_pid()", &[])
                .await
                .map_err(MaintenancePrimary::Sql)?;
            let _ = identified.send(row.get::<_, i32>(0));
            drop(row);
            admin
                .client()
                .batch_execute("BEGIN; SET LOCAL ROLE kehila_owner")
                .await
                .map_err(MaintenancePrimary::Sql)?;
            admin
                .client()
                .query_one(
                    "SELECT kehila_auth.retire_project_payload($1::text::kehila.ident,$2)",
                    &[&installation, &operation],
                )
                .await
                .map_err(MaintenancePrimary::Sql)?;
            let _ = staged.send(true);
            tokio::time::timeout(WORK_DEADLINE, decision)
                .await
                .map_err(|_| MaintenancePrimary::DecisionLost)?
                .map_err(|_| MaintenancePrimary::DecisionLost)?;
            admin
                .client()
                .batch_execute("COMMIT")
                .await
                .map_err(MaintenancePrimary::Sql)
        }
        .await;
        let result = match primary {
            Ok(()) => Ok(()),
            Err(primary) => {
                let rollback = admin.client().batch_execute("ROLLBACK").await.err();
                Err(MaintenanceFailure { primary, rollback })
            }
        };
        admin.close().await;
        let _ = done.send(result);
    });
    Maintenance {
        started,
        staged: ready,
        commit,
        finished,
    }
}
