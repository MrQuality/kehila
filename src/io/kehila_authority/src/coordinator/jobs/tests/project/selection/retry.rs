//! Three observed ordinal invalidations exhaust one unchanged clear identity.
use super::*;
use crate::lifecycle::probe::Point;

pub(super) async fn exercise(handle: &Handle, browser: &Browser, admin: &Admin) {
    let victim = command('0', 0, Value::Null);
    let mut gate = handle
        .live
        .probe
        .hold_transaction(0, Point::SelectionPrepared);
    let mut pending = Box::pin(call(
        handle,
        Some(browser),
        "/selection/set",
        Some(victim.clone()),
    ));
    let mut early = None;
    for round in 0..3 {
        let reached=tokio::time::timeout(WORK_DEADLINE,async {
            tokio::select! {arrived=&mut gate.arrived=>arrived.is_ok(),reply=&mut pending=>{early=Some(reply);false}}
        }).await.unwrap_or(false);
        // The three gates observe initial preparation and exactly two further
        // preparations, all while this one HTTP request retains its reservation.
        let permits = handle.capacity.available_permits();
        let other = if reached {
            evidence::fences_released(admin).await;
            let token = format!("{:032x}", 16 + round);
            Some(
                call(
                    handle,
                    Some(browser),
                    "/selection/set",
                    Some(json!({
                "operation_token":token,"expected_version":"0","requested":null})),
                )
                .await,
            )
        } else {
            None
        };
        let state = if reached {
            Some(admin.client().query_one("SELECT \
            selection_version::numeric(20,0)::text,event_ordinal::numeric(20,0)::text, \
            (SELECT count(*) FROM kehila.operations WHERE command_family='selection' AND token=repeat('0',32)) \
            FROM kehila.current_work_selections WHERE user_id='owner'",&[]).await)
        } else {
            None
        };
        let next = if reached && round < 2 {
            Some(
                handle
                    .live
                    .probe
                    .hold_transaction(0, Point::SelectionPrepared),
            )
        } else {
            None
        };
        let released = gate.release.send(()).is_ok();
        if !reached || !released {
            // Join or drop the socket-owning future before reporting failure.
            if early.is_none() {
                let _ = tokio::time::timeout(WORK_DEADLINE, &mut pending).await;
            }
            panic!("selection retry gate did not activate/release at round {round}");
        }
        assert_eq!(
            permits,
            CAPACITY - 1,
            "retry must retain exactly the original serving reservation"
        );
        let (status, value, _) = other.unwrap();
        assert_eq!((status, value), (200, receipt("accepted", 0, &Value::Null)));
        let state = state.unwrap().unwrap();
        assert_eq!(state.get::<_, String>(0), "0");
        assert_eq!(state.get::<_, String>(1), (round + 1).to_string());
        assert_eq!(
            state.get::<_, i64>(2),
            0,
            "unaccepted victim identity changed during ordinal invalidation"
        );
        if let Some(next) = next {
            gate = next;
        } else {
            break;
        }
    }
    let response = match early {
        Some(value) => Ok(value),
        None => tokio::time::timeout(WORK_DEADLINE, pending).await,
    };
    let (status, value, _) = response.expect("bounded selection attempt must finish");
    assert_eq!(
        (status, value),
        (503, json!({"error":"preparation_unavailable"}))
    );
    evidence::settled(handle).await;
    assert_eq!(handle.capacity.available_permits(), CAPACITY);
    assert!(
        handle.live.is_live(),
        "ordinal exhaustion must leave the runtime usable"
    );
    current(admin, 0, 3, None).await;
    let token_count: i64 = admin
        .client()
        .query_one(
            "SELECT count(*) FROM kehila.operations \
        WHERE command_family='selection' AND token=repeat('0',32)",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(token_count, 0);
    // A separate explicit request retries the same token/version/clear without
    // rebasing; no automatic new identity is invented by the exhausted request.
    assert_eq!(
        selection(handle, Some(browser), "set", victim, 200).await,
        receipt("accepted", 0, &Value::Null)
    );
    current(admin, 0, 4, None).await;
    evidence::fences_released(admin).await;
}
