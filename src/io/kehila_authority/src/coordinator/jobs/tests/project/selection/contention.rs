//! Hold an admitted packet outside the database and change only protected state.
use super::*;
use crate::lifecycle::probe::Point;

pub(super) async fn same_identity(
    handle: &Handle,
    browser: &Browser,
    admin: &Admin,
    request: Value,
    target: &Value,
) {
    let gate = handle
        .live
        .probe
        .hold_transaction(0, Point::SelectionPrepared);
    let mut pending = Box::pin(call(
        handle,
        Some(browser),
        "/selection/set",
        Some(request.clone()),
    ));
    let mut early = None;
    let reached = tokio::time::timeout(WORK_DEADLINE, async {
        tokio::select! {
            arrived=gate.arrived => arrived.is_ok(),
            reply=&mut pending => {early=Some(reply);false},
        }
    })
    .await
    .unwrap_or(false);
    let other = if reached {
        Some(call(handle, Some(browser), "/selection/set", Some(request)).await)
    } else {
        None
    };
    let released = gate.release.send(()).is_ok();
    let held = match early {
        Some(reply) => Ok(reply),
        None => tokio::time::timeout(WORK_DEADLINE, pending).await,
    };
    assert!(
        reached && released,
        "same-identity preparation gate must activate and release"
    );
    let (status, value, _) = other.expect("competing same identity executed");
    assert_eq!((status, value), (200, receipt("accepted", 1, target)));
    let (status, value, _) = held.expect("original same-identity request must finish");
    assert_eq!((status, value), (200, receipt("replayed", 1, target)));
    current(admin, 1, 5, target["item_id"].as_str()).await;
    evidence::fences_released(admin).await;
}

async fn interleave(
    handle: &Handle,
    browser: &Browser,
    admin: &Admin,
    held: Value,
    competing: Value,
    expected_during: (u64, u64),
) -> ((u16, Value), (u16, Value)) {
    let held_token = held["operation_token"].as_str().unwrap().to_owned();
    let crate::selection_ingress::Input::Command {
        token,
        expected_version,
        requested,
        ..
    } = crate::selection_ingress::decode(
        crate::selection_ingress::Route::Set,
        &serde_json::to_vec(&held).unwrap(),
    )
    .unwrap()
    else {
        panic!("expected selection command")
    };
    let owner: String = admin
        .client()
        .query_one(
            "SELECT owner_actor_id::text FROM kehila_auth.installation WHERE singleton",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    let bytes = selection_codec::encode_request(
        &task_contract::current_work::SelectionCommand {
            user_id: task_contract::current_work::UserId(owner),
            operation_id: token,
            expected_version,
            requested,
        },
        1,
    )
    .unwrap();
    let mut commit_gate = handle.live.probe.arm_selection_commit(bytes);
    let gate = handle
        .live
        .probe
        .hold_transaction(0, Point::SelectionPrepared);
    let mut pending = Box::pin(call(handle, Some(browser), "/selection/set", Some(held)));
    let mut early = None;
    let reached = tokio::time::timeout(WORK_DEADLINE, async {
        tokio::select! {
            arrived = gate.arrived => arrived.is_ok(),
            reply = &mut pending => { early = Some(reply); false },
        }
    })
    .await
    .unwrap_or(false);
    // Collect observations, release and finish the held request before asserting.
    // No detached client task or unbounded gate wait survives a failed oracle.
    let competing_reply = if reached {
        Some(call(handle, Some(browser), "/selection/set", Some(competing)).await)
    } else {
        None
    };
    let observed = if reached {
        Some(admin.client().query_one(
            "SELECT selection_version::numeric(20,0)::text,event_ordinal::numeric(20,0)::text,\
             (SELECT count(*) FROM kehila.operations WHERE command_family='selection' \
              AND actor_id='owner' AND token=$1::text::kehila.operation_token) \
             FROM kehila.current_work_selections WHERE user_id='owner'", &[&held_token]).await)
    } else {
        None
    };
    let released = gate.release.send(()).is_ok();
    let mut commit_arrived = false;
    let held_reply = match early {
        Some(reply) => Ok(reply),
        None => {
            tokio::time::timeout(WORK_DEADLINE, async {
                tokio::select! {
                    arrived = &mut commit_gate.arrived => {
                        commit_arrived = arrived.is_ok();
                        let _ = commit_gate.release.send(());
                        pending.await
                    }
                    reply = &mut pending => reply,
                }
            })
            .await
        }
    };
    assert!(reached, "selection preparation gate must actually activate");
    assert!(released, "owned preparation gate must be released");
    let observed = observed.unwrap().unwrap();
    assert_eq!(observed.get::<_, String>(0), expected_during.0.to_string());
    assert_eq!(observed.get::<_, String>(1), expected_during.1.to_string());
    assert_eq!(
        observed.get::<_, i64>(2),
        0,
        "held identity is not accepted before its fresh write"
    );
    let (status, value, _) =
        held_reply.expect("held HTTP request must finish within its cleanup bound");
    assert_eq!(
        commit_arrived,
        status == 200,
        "only the exact accepted write arrives, including after ordinal-only re-preparation"
    );
    let (other_status, other_value, _) = competing_reply.unwrap();
    ((status, value), (other_status, other_value))
}

pub(super) async fn noops(handle: &Handle, browser: &Browser, admin: &Admin, target: Value) {
    let (held, other) = interleave(
        handle,
        browser,
        admin,
        command('b', 1, target.clone()),
        command('1', 1, target.clone()),
        (1, 6),
    )
    .await;
    assert_eq!(other, (200, receipt("accepted", 1, &target)));
    assert_eq!(held, (200, receipt("accepted", 1, &target)),
        "event-only staleness must reprepare the same identity instead of reporting a version conflict");
    current(admin, 1, 7, target["item_id"].as_str()).await;
}

pub(super) async fn changed_version(
    handle: &Handle,
    browser: &Browser,
    admin: &Admin,
    original: Value,
    next: Value,
) {
    let (held, other) = interleave(
        handle,
        browser,
        admin,
        command('2', 1, original),
        command('c', 1, next.clone()),
        (2, 8),
    )
    .await;
    assert_eq!(other, (200, receipt("accepted", 2, &next)));
    assert_eq!(
        held,
        (
            409,
            json!({"codec_version":1,"outcome":"rejected",
        "code":"selection_version_conflict","current_version":"2"})
        ),
        "a changed selection version must reject the old prepared identity"
    );
    current(admin, 2, 8, next["item_id"].as_str()).await;
}
