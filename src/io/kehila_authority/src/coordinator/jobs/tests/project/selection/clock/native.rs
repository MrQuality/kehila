//! Observe the actual runtime's payload wait after historical reply admission.
use super::*;
use crate::lifecycle::probe::Point;

async fn delayed_native(
    handle: &Handle,
    browser: &Browser,
    admin: &Admin,
    blocker: &Admin,
    request: &Value,
    end: u64,
    advance: bool,
) -> Check<(u16, Value)> {
    clock(admin, end - 1).await?;
    let blocker_pid: i32 = blocker
        .client()
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .map_err(|value| error("native blocker identity", value))?
        .get(0);
    let relation: String = admin
        .client()
        .query_one(
            "SELECT 'kehila.operation_payloads'::regclass::oid::text",
            &[],
        )
        .await
        .map_err(|value| error("native payload identity", value))?
        .get(0);
    let gate = handle
        .live
        .probe
        .hold_transaction(0, Point::SelectionHistoryPrepared);
    let mut pending = Box::pin(call(
        handle,
        Some(browser),
        "/selection/reconcile",
        Some(request.clone()),
    ));
    let mut early = None;
    let prepared = tokio::time::timeout(WORK_DEADLINE, async {
        tokio::select! {
            arrived=gate.arrived=>arrived.is_ok(),
            reply=&mut pending=>{early=Some(reply);false},
        }
    })
    .await
    .unwrap_or(false);
    let locked = if prepared {
        blocker
            .client()
            .batch_execute(
                "BEGIN; SET LOCAL ROLE kehila_owner; \
        LOCK TABLE kehila.operation_payloads IN ACCESS EXCLUSIVE MODE",
            )
            .await
            .map_err(|value| error("native payload lock", value))
    } else {
        Err("historical prepared gate not reached".into())
    };
    let released = gate.release.send(()).is_ok();
    let waited = if locked.is_ok() && released {
        tokio::time::timeout(WORK_DEADLINE,async {
            loop {
                tokio::select! {
                    reply=&mut pending=>{early=Some(reply);return Ok(false);},
                    _=tokio::time::sleep(Duration::from_millis(1))=>{
                        let count:i64=admin.client().query_one("SELECT count(*) FROM pg_locks l \
                            JOIN pg_stat_activity a ON a.pid=l.pid WHERE a.datname=current_database() \
                            AND a.usename='kehila_auth_runtime_login' AND NOT l.granted \
                            AND l.relation::text=$2::text AND l.mode='AccessShareLock' \
                            AND $1=ANY(pg_blocking_pids(l.pid))",&[&blocker_pid,&relation]).await
                            .map_err(|value|error("native payload wait observation",value))?.get(0);
                        if count==1 {return Ok(true);}
                        if count>1 {return Err("multiple native payload waiters".into());}
                    }
                }
            }
        }).await.map_err(|_|"native payload wait deadline".to_owned()).and_then(|value:Check<bool>|value)
    } else {
        Err("native payload lock/gate unavailable".into())
    };
    let advanced = if matches!(waited, Ok(true)) && advance {
        clock(admin, end).await
    } else {
        Ok(())
    };
    let cleanup = blocker
        .client()
        .batch_execute("ROLLBACK")
        .await
        .map_err(|value| error("native payload rollback", value));
    let response = match early {
        Some(value) => Ok(value),
        None => tokio::time::timeout(WORK_DEADLINE, pending)
            .await
            .map_err(|_| "native replay completion deadline".to_owned()),
    };
    if !prepared
        || !released
        || locked.is_err()
        || !matches!(waited, Ok(true))
        || advanced.is_err()
        || cleanup.is_err()
    {
        return Err(format!("native prepared={prepared}; release={released}; lock={locked:?}; wait={waited:?}; clock={advanced:?}; rollback={cleanup:?}; response_completed={}",response.is_ok()));
    }
    let (status, value, _) = response?;
    Ok((status, value))
}

async fn retire_after_preparation(
    handle: &Handle,
    browser: &Browser,
    admin: &Admin,
    request: &Value,
) -> Check<(u16, Value)> {
    let gate = handle
        .live
        .probe
        .hold_transaction(0, Point::SelectionHistoryPrepared);
    let mut pending = Box::pin(call(
        handle,
        Some(browser),
        "/selection/reconcile",
        Some(request.clone()),
    ));
    let mut early = None;
    let reached=tokio::time::timeout(WORK_DEADLINE,async {
        tokio::select! {arrived=gate.arrived=>arrived.is_ok(),reply=&mut pending=>{early=Some(reply);false}}
    }).await.unwrap_or(false);
    let retired = if reached {
        admin.client().batch_execute("BEGIN; \
        UPDATE kehila.operations SET payload_retired=true WHERE command_family='selection' AND token=repeat('0',32); \
        DELETE FROM kehila.operation_payloads WHERE operation_row_id IN \
          (SELECT operation_row_id FROM kehila.operations WHERE command_family='selection' AND token=repeat('0',32)); \
        SET CONSTRAINTS ALL IMMEDIATE; COMMIT").await.map_err(|value|error("native fixture retirement",value))
    } else {
        Err("retirement preparation gate not reached".into())
    };
    let observed = if retired.is_ok() {
        admin.client().query_one("SELECT count(*)=1 AND bool_and(o.payload_retired \
        AND NOT EXISTS(SELECT FROM kehila.operation_payloads p WHERE p.operation_row_id=o.operation_row_id)) \
        FROM kehila.operations o WHERE o.command_family='selection' AND o.token=repeat('0',32)",&[]).await
        .map_err(|value|error("retirement observation",value)).map(|row|row.get::<_,bool>(0))
    } else {
        Ok(false)
    };
    let rollback = admin
        .client()
        .batch_execute("ROLLBACK")
        .await
        .map_err(|value| error("retirement transaction cleanup", value));
    let released = gate.release.send(()).is_ok();
    let response = match early {
        Some(value) => Ok(value),
        None => tokio::time::timeout(WORK_DEADLINE, pending)
            .await
            .map_err(|_| "retirement replay deadline".to_owned()),
    };
    if !reached
        || !released
        || retired.is_err()
        || !matches!(observed, Ok(true))
        || rollback.is_err()
    {
        return Err(format!("retirement reached={reached}; released={released}; change={retired:?}; observed={observed:?}; rollback={rollback:?}; response_completed={}",response.is_ok()));
    }
    let (status, value, _) = response?;
    Ok((status, value))
}

pub(super) async fn exercise(
    handle: &Handle,
    browser: &Browser,
    admin: &Admin,
    blocker: &Admin,
    target: &Value,
    origin: u64,
    end: u64,
) -> Check<()> {
    let request = command('0', 0, target.clone());
    let live = delayed_native(handle, browser, admin, blocker, &request, end, false).await?;
    if live != (200, receipt("replayed", 1, target)) {
        return Err(format!("native live wait positive control: {live:?}"));
    }
    let expired = delayed_native(handle, browser, admin, blocker, &request, end, true).await?;
    let expiry = (410, json!({"codec_version":1,"outcome":"replay_expired"}));
    if expired != expiry {
        return Err(format!("native crossed-expiry wait: {expired:?}"));
    }
    clock(admin, origin + 1).await?;
    let retired = retire_after_preparation(handle, browser, admin, &request).await?;
    if retired != expiry {
        return Err(format!("native retirement after admission: {retired:?}"));
    }
    let changed = command('0', 0, Value::Null);
    let (status, value, _) = call(
        handle,
        Some(browser),
        "/selection/reconcile",
        Some(changed.clone()),
    )
    .await;
    if (status, value)
        != (
            409,
            json!({"codec_version":1,"outcome":"rejected","code":"operation_id_reused"}),
        )
    {
        return Err("changed retired identity must reject reuse".into());
    }
    let project = target["project_id"]
        .as_str()
        .ok_or("invalid fixture project")?;
    admin.client().execute("UPDATE kehila.project_grants SET active=false WHERE actor_id='owner' AND project_id=$1::text::kehila.ident",&[&project]).await
        .map_err(|value|error("original grant revocation",value))?;
    let denied = call(handle, Some(browser), "/selection/reconcile", Some(request)).await;
    let changed_denied = call(handle, Some(browser), "/selection/reconcile", Some(changed)).await;
    admin.client().execute("UPDATE kehila.project_grants SET active=true WHERE actor_id='owner' AND project_id=$1::text::kehila.ident",&[&project]).await
        .map_err(|value|error("original grant restoration",value))?;
    if denied.0 != 403
        || denied.1 != json!({"error":"forbidden"})
        || changed_denied.0 != 403
        || changed_denied.1 != json!({"error":"forbidden"})
    {
        return Err("original revoked authority must precede retirement/reuse disclosure".into());
    }
    let stable:bool=admin.client().query_one("SELECT \
        (SELECT count(*)=1 FROM kehila.selection_events) AND \
        (SELECT count(*)=1 FROM kehila.operations WHERE command_family='selection') AND \
        EXISTS(SELECT FROM kehila.current_work_selections WHERE user_id='owner' AND selection_version=1 AND event_ordinal=1)",&[]).await
        .map_err(|value|error("native release state observation",value))?.get(0);
    if !stable {
        return Err("native historical release changed permanent selection state".into());
    }
    println!("selection_native_release_controls live_wait=passed expired_wait=passed retirement=passed authority_first=passed");
    Ok(())
}
