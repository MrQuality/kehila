//! Kill only the observed aborted serving transaction, then fail real rollback.
use super::*;
use crate::lifecycle::{probe::Point, FatalCause};

pub(super) struct Observed {
    reached: bool,
    terminated: Result<(), &'static str>,
    released: bool,
    unavailable: bool,
    stopped: bool,
    cause: Option<FatalCause>,
    restored: bool,
    unchanged: bool,
}
impl Observed {
    pub(super) fn verify(self) {
        assert!(
            self.reached && self.released,
            "actual failed-preparation cleanup gate required"
        );
        assert_eq!(self.terminated, Ok(()));
        assert!(self.unavailable && self.stopped && self.restored && self.unchanged);
        let Some(FatalCause::NativeRollback {
            primary: Some(primary),
            cleanup,
        }) = self.cause
        else {
            panic!("both primary and cleanup diagnostics must survive fatal shutdown")
        };
        assert_eq!(primary.kind, "database rejection");
        assert_eq!(primary.sql_state, Some(*b"42501"));
        assert!(matches!(
            cleanup.kind,
            "database unavailable" | "database rejection"
        ));
        assert!(cleanup.sql_state.is_none() || cleanup.sql_state == Some(*b"57P01"));
        assert_eq!((primary.constraint_bytes, cleanup.constraint_bytes), (0, 0));
    }
}

async fn terminate_aborted(admin: &Admin) -> Result<(), &'static str> {
    let client = admin.client();
    client
        .batch_execute("SELECT pg_stat_clear_snapshot()")
        .await
        .map_err(|_| "backend observation failed")?;
    let rows = client.query(
        "SELECT pid,backend_start::text,xact_start::text,query FROM pg_stat_activity \
         WHERE datname=current_database() AND usename='kehila_auth_runtime_login' \
         AND state='idle in transaction (aborted)' AND query LIKE 'SELECT (kehila_auth.item_read_authority(%'",
        &[],
    ).await.map_err(|_| "aborted backend observation failed")?;
    if rows.len() != 1 {
        return Err("one exact aborted serving backend required");
    }
    let pid: i32 = rows[0].get(0);
    let birth: String = rows[0].get(1);
    let transaction: Option<String> = rows[0].get(2);
    let query: String = rows[0].get(3);
    let killed = client.query(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
         WHERE pid=$1 AND backend_start::text=$2 AND xact_start::text IS NOT DISTINCT FROM $3::text \
         AND query=$4 AND datname=current_database() AND usename='kehila_auth_runtime_login' \
         AND state='idle in transaction (aborted)'",
        &[&pid, &birth, &transaction, &query],
    ).await.map_err(|_| "exact backend termination failed")?;
    if killed.len() != 1 || !killed[0].get::<_, bool>(0) {
        return Err("held backend identity changed before termination");
    }
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            client.batch_execute("SELECT pg_stat_clear_snapshot()").await.map_err(|_| "backend disappearance observation failed")?;
            if client.query_one("SELECT NOT EXISTS(SELECT FROM pg_stat_activity WHERE pid=$1 AND backend_start::text=$2)",
                &[&pid, &birth]).await.map_err(|_| "backend disappearance query failed")?.get::<_, bool>(0) { return Ok(()); }
            tokio::task::yield_now().await;
        }
    }).await.map_err(|_| "terminated backend disappearance timed out")?
}

pub(super) async fn exercise(
    handle: &Handle,
    browser: &Browser,
    admin: &Admin,
    original: Value,
) -> Observed {
    let before = evidence::snapshot(admin).await;
    let project = original["requested"]["project_id"].as_str().unwrap();
    let changed = admin.client().execute(
        "UPDATE kehila.project_grants SET active=false WHERE actor_id='owner' AND project_id=$1::text::kehila.ident AND active",
        &[&project],
    ).await.unwrap();
    assert_eq!(changed, 1);
    let mut gate = handle
        .live
        .probe
        .hold_transaction(0, Point::SelectionPreparationCleanup);
    let deadline = Instant::now() + WORK_DEADLINE;
    let input = crate::selection_ingress::decode(
        crate::selection_ingress::Route::Reconcile,
        &serde_json::to_vec(&original).unwrap(),
    )
    .unwrap();
    let reply = handle
        .submit(
            Operation::SelectionCommand {
                credential: admission::credential(browser),
                input,
            },
            deadline,
        )
        .unwrap();
    let reached = tokio::time::timeout_at(deadline.into(), &mut gate.arrived)
        .await
        .is_ok_and(|value| value.is_ok());
    let terminated = if reached {
        terminate_aborted(admin).await
    } else {
        Err("failed-preparation gate did not activate")
    };
    // Always release and finish owned request work before evaluating the fault.
    let released = gate.release.send(()).is_ok();
    let response = tokio::time::timeout(WORK_DEADLINE, reply).await;
    let unavailable = match response {
        Ok(Ok(mut response)) => {
            matches!(&response.result, Err(Error::Unavailable))
                && matches!(response.result(), Err(Error::Unavailable))
                && response.confirm_exchange().is_err()
        }
        _ => false,
    };
    let cause = handle.live.failure();
    let stopped = !handle.live.is_live();
    let restored = admin.client().execute(
        "UPDATE kehila.project_grants SET active=true WHERE actor_id='owner' AND project_id=$1::text::kehila.ident AND NOT active",
        &[&project],
    ).await.is_ok_and(|count| count == 1);
    let unchanged = evidence::snapshot(admin).await == before;
    Observed {
        reached,
        terminated,
        released,
        unavailable,
        stopped,
        cause,
        restored,
        unchanged,
    }
}
