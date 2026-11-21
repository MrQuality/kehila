//! Bounded real payload-lock/fixture-clock control; no host clock changes.
use super::*;
use task_contract::current_work::{
    decide_selection_command, CurrentWork, SelectionCommand, SelectionDecision, UserId,
};
use task_contract::relationship::Endpoint;
mod native;

const INSTALL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/integration/native_clock_install.sql"
));
const RESTORE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/integration/native_clock_restore.sql"
));
const REPLAY: &str = "SELECT outcome FROM kehila_auth.reconcile_selection(\
 ROW($1::text::kehila.ident,'owner'::kehila.ident,$2::text::kehila.u64,\
 $3::text::kehila.u64,$4::text::kehila.u64)::kehila_auth.authority,\
 $5::text::kehila.operation_token,1::bigint,$6::bytea)";
type Check<T> = std::result::Result<T, String>;
fn error(label: &str, value: tokio_postgres::Error) -> String {
    format!(
        "{label}: {}",
        value.code().map(|code| code.code()).unwrap_or("transport")
    )
}
async fn clock(admin: &Admin, value: u64) -> Check<()> {
    admin
        .client()
        .execute(
            "UPDATE kehila_native_fixture.clock SET value=$1::text::numeric WHERE singleton",
            &[&value.to_string()],
        )
        .await
        .map_err(|value| error("fixture clock", value))?;
    Ok(())
}
struct RecordedReplay<'a> {
    proof: &'a [String; 4],
    token: &'a String,
    wire: &'a Vec<u8>,
}
async fn replay(admin: &Admin, request: &RecordedReplay<'_>) -> Check<String> {
    let proof = request.proof;
    admin
        .client()
        .query_one(
            REPLAY,
            &[
                &proof[0],
                &proof[1],
                &proof[2],
                &proof[3],
                request.token,
                request.wire,
            ],
        )
        .await
        .map_err(|value| error("reconcile", value))
        .map(|row| row.get(0))
}
async fn delayed(
    admin: &Admin,
    blocker: &Admin,
    reader: &Admin,
    request: &RecordedReplay<'_>,
    end: u64,
    advance: bool,
) -> Check<String> {
    clock(admin, end - 1).await?;
    let reader_pid: i32 = reader
        .client()
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .map_err(|value| error("reader identity", value))?
        .get(0);
    let blocker_pid: i32 = blocker
        .client()
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .map_err(|value| error("blocker identity", value))?
        .get(0);
    let payload_oid: String = admin
        .client()
        .query_one(
            "SELECT 'kehila.operation_payloads'::regclass::oid::text",
            &[],
        )
        .await
        .map_err(|value| error("payload relation identity", value))?
        .get(0);
    let lock = blocker
        .client()
        .batch_execute(
            "BEGIN; SET LOCAL ROLE kehila_owner; \
        LOCK TABLE kehila.operation_payloads IN ACCESS EXCLUSIVE MODE",
        )
        .await;
    if let Err(primary) = lock {
        let cleanup = blocker.client().batch_execute("ROLLBACK").await;
        return Err(format!(
            "{}; rollback={}",
            error("payload lock", primary),
            cleanup.is_ok()
        ));
    }
    let mut pending = Box::pin(replay(reader, request));
    let mut early = None;
    let gate = tokio::time::timeout(WORK_DEADLINE,async {
        loop {
            tokio::select! {
                outcome = &mut pending => { early = Some(outcome); return Ok(false); },
                _ = tokio::time::sleep(Duration::from_millis(1)) => {
                    let row = admin.client().query_one("SELECT EXISTS(SELECT FROM pg_locks l \
                        WHERE l.pid=$1 AND NOT l.granted AND l.relation::text=$3::text \
                        AND l.mode='AccessShareLock' AND $2=ANY(pg_blocking_pids(l.pid)))",&[&reader_pid,&blocker_pid,&payload_oid])
                        .await.map_err(|value|error("payload wait observation",value))?;
                    if row.get::<_,bool>(0) { return Ok(true); }
                }
            }
        }
    }).await.map_err(|_|"payload wait observation deadline".to_owned()).and_then(|value:Check<bool>|value);
    let advanced = if matches!(gate, Ok(true)) && advance {
        clock(admin, end).await
    } else {
        Ok(())
    };
    // Release and join before reporting a failed gate or clock assertion.
    let released = blocker
        .client()
        .batch_execute("ROLLBACK")
        .await
        .map_err(|value| error("payload lock rollback", value));
    let outcome = match early {
        Some(value) => value,
        None => tokio::time::timeout(WORK_DEADLINE, &mut pending)
            .await
            .map_err(|_| "replay completion deadline".to_owned())
            .and_then(|value| value),
    };
    if !matches!(gate, Ok(true)) || advanced.is_err() || released.is_err() {
        return Err(format!(
            "gate={gate:?}; advance={advanced:?}; rollback={released:?}; replay={outcome:?}"
        ));
    }
    outcome
}

#[test]
#[ignore = "requires an explicitly provisioned disposable TLS selection clock fixture"]
fn native_selection_replay_release_clock() {
    let (endpoint, installation, attempt, controls) = fixture();
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let admin = Admin::connect(&endpoint).await;
        let (handle,owner) = runtime(&endpoint,&installation,format!("{attempt}-selection-clock")).await;
        let running = tokio::spawn(owner.run());
        let browser = login(&handle).await;
        let (status,pending) = project(&handle,&browser,"register",json!({"name":"Selection clock","prefix":"CLK","estimate_unit":"hours"})).await;
        assert_eq!(status,201);
        let (status,created) = project(&handle,&browser,"execute",execute(&pending["entry"])).await;
        assert_eq!(status,201);
        let project = project_create_codec::decode_result(1,&serde_json::to_vec(&created["result"]).unwrap()).unwrap();
        let SavedResult::Create(item) = item_reads::seed(&admin,&project.project,&project.configuration,1).await else { panic!("wrong seeded item family") };
        let requested = Endpoint {project_id:item.project.id.clone(),item_id:item.item.id.clone(),project_archived:false,item_archived:false};
        let target = json!({"project_id":item.project.id.0,"item_id":item.item.id.0});
        let initial = CurrentWork {user_id:UserId("owner".into()),version:0,selected:None};
        let token = "0".repeat(32);
        let command = SelectionCommand {user_id:initial.user_id.clone(),operation_id:token.clone(),expected_version:0,requested:Some(requested.identity())};
        let SelectionDecision::Apply(result) = decide_selection_command(&initial,Some(&requested),None,true,&command) else { panic!("invalid fixture selection") };
        let event = selection_codec::Event {ordinal:1,operation_id:token.clone(),before:initial,result};
        let wire = selection_codec::encode_request(&command,1).unwrap();
        let history = String::from_utf8(selection_codec::encode_event(&event,1).unwrap()).unwrap();
        let facts = admin.client().query_one("SELECT i.runtime_generation::text,c.auth_generation::text,c.verifier_revision::text \
            FROM kehila_auth.installation i JOIN kehila_auth.owner_credentials c USING(owner_actor_id)",&[]).await.unwrap();
        let proof = [installation,facts.get(0),facts.get(1),facts.get(2)];
        admin.client().batch_execute(INSTALL).await.unwrap();
        let blocker = Admin::connect(&endpoint).await;
        let reader = Admin::connect(&endpoint).await;
        let observed: Check<(String,String,String,String)> = async {
            admin.client().batch_execute("SET statement_timeout='3s'").await.map_err(|value|error("admin deadline",value))?;
            blocker.client().batch_execute("SET statement_timeout='3s'; SET lock_timeout='2s'").await.map_err(|value|error("blocker deadline",value))?;
            reader.client().batch_execute("SET statement_timeout='3s'; SET ROLE kehila_auth_runtime").await.map_err(|value|error("runtime role",value))?;
            let saved = reader.client().query_one("SELECT outcome FROM kehila_auth.save_selection(\
                ROW($1::text::kehila.ident,'owner'::kehila.ident,$2::text::kehila.u64,$3::text::kehila.u64,$4::text::kehila.u64)::kehila_auth.authority,\
                $5::text::kehila.operation_token,0::kehila.u64,$6::bytea,$7::text::jsonb)",
                &[&proof[0],&proof[1],&proof[2],&proof[3],&token,&wire,&history]).await.map_err(|value|error("fixture save",value))?;
            if saved.get::<_,String>(0) != "accepted" { return Err("fixture save not accepted".into()); }
            let row = admin.client().query_one("SELECT replay_origin_ms::numeric(20,0)::text,replay_deadline_ms::numeric(20,0)::text \
                FROM kehila.operations WHERE command_family='selection' AND token=$1::text::kehila.operation_token",&[&token]).await.map_err(|value|error("retention metadata",value))?;
            let origin: u64 = row.get::<_,String>(0).parse().map_err(|_|"invalid origin")?;
            let end: u64 = row.get::<_,String>(1).parse().map_err(|_|"invalid deadline")?;
            if end.checked_sub(origin) != Some(7_776_000_000) { return Err("invalid fixture replay window".into()); }
            let request=RecordedReplay {proof:&proof,token:&token,wire:&wire};
            clock(&admin,origin+1).await?; let live = replay(&reader,&request).await?;
            if live != "replay" { return Err(format!("unexpired positive control: {live}")); }
            clock(&admin,end).await?; let expired = replay(&reader,&request).await?;
            if expired != "replay_expired" { return Err(format!("expiry negative control: {expired}")); }
            let contended = delayed(&admin,&blocker,&reader,&request,end,false).await?;
            if contended != "replay" { return Err(format!("unchanged-clock wait control: {contended}")); }
            let crossed = delayed(&admin,&blocker,&reader,&request,end,true).await?;
            native::exercise(&handle,&browser,&admin,&blocker,&target,origin,end).await?;
            Ok((live,expired,contended,crossed))
        }.await;
        let lock_cleanup = blocker.client().batch_execute("ROLLBACK").await;
        reader.close().await; blocker.close().await;
        let clock_cleanup = admin.client().batch_execute(RESTORE).await;
        let available = if clock_cleanup.is_ok() {
            admin.client().batch_execute("UPDATE kehila.actors SET can_create_project=false WHERE actor_id='owner'").await.unwrap();
            Some(call(&handle,Some(&browser),"/selection/read",Some(json!({}))).await)
        } else { None };
        let redacted = if available.as_ref().is_some_and(|reply|reply.0 == 200) {
            admin.client().execute("UPDATE kehila.project_grants SET active=false WHERE actor_id='owner' AND project_id=$1::text::kehila.ident",&[&item.project.id.0]).await.unwrap();
            let reply = call(&handle,Some(&browser),"/selection/read",Some(json!({}))).await;
            admin.client().execute("UPDATE kehila.project_grants SET active=true WHERE actor_id='owner' AND project_id=$1::text::kehila.ident",&[&item.project.id.0]).await.unwrap();
            Some(reply)
        } else { None };
        admin.client().batch_execute("UPDATE kehila.actors SET can_create_project=true WHERE actor_id='owner'").await.unwrap();
        handle.stop(); let runtime_cleanup = running.await;
        evidence::runtime_released(&admin).await;
        admin.close().await;
        assert!(lock_cleanup.is_ok(),"payload lock cleanup failed; observations={observed:?}");
        assert!(clock_cleanup.is_ok(),"clock restoration failed; observations={observed:?}");
        assert!(matches!(runtime_cleanup,Ok(Ok(()))),"runtime cleanup failed; observations={observed:?}");
        let (status,value,_) = available.expect("clock restored before current read");
        assert_eq!(status,200);
        assert_eq!(value,json!({"codec_version":1,"outcome":"read","version":"1","target":{"state":"available","project_id":target["project_id"],"item_id":target["item_id"]}}));
        let (status,value,_) = redacted.expect("populated read succeeded before revocation control");
        assert_eq!(status,200);
        assert_eq!(value,json!({"codec_version":1,"outcome":"read","version":"1","target":{"state":"unavailable"}}));
        println!("selection_read_controls available=passed redacted=passed");
        let (live,expired,contended,crossed) = observed.expect("bounded payload wait experiment must complete");
        assert_eq!(live,"replay","unexpired positive control");
        assert_eq!(expired,"replay_expired","expiry negative control");
        assert_eq!(contended,"replay","a payload wait without advancing the clock remains replayable");
        println!("selection_clock_controls live=passed expired=passed unchanged_wait=passed");
        assert_eq!(crossed,"replay_expired","selection replay must sample expiry after blocked payload read");
        mark(&controls,"selection-clock-passed");
    });
}
