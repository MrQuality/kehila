//! Controlled collision rollback, lost replies, and startup capability probes.
use super::*;

fn identities(start: usize) -> crate::item_pending::Identities {
    std::array::from_fn(|attempt| {
        std::array::from_fn(|part| format!("{:032x}", start + attempt * 3 + part))
    })
}
async fn collisions(ctx: &Context<'_>, serving: &Serving) {
    let one = register(serving, form(&ctx.project)).await;
    let two = register(serving, form(&ctx.project)).await;
    let three = register(serving, form(&ctx.project)).await;
    let removed = register(serving, form(&ctx.project)).await;
    assert_eq!(cancel(serving, &removed).await["state"], "cancelled");
    let mut keys = identities(10_000);
    keys[0][0] = one["intent_handle"].as_str().unwrap().into();
    let fault = serving.handle.live.probe.registration_identities(keys);
    let gate = serving
        .handle
        .live
        .probe
        .hold_transaction(0, Point::RegistrationRollback);
    let before = serving.handle.live.probe.snapshot();
    let body = form(&ctx.project);
    let mut work = Box::pin(pending(&serving.handle, &serving.browser, "register", body));
    tokio::select! {
        arrived=gate.arrived=>arrived.unwrap(),
        _=&mut work=>panic!("registration did not reach its post-rollback boundary"),
    }
    assert_eq!(ctx.admin.client().query_one("SELECT count(*) FROM kehila_auth.pending_items WHERE intent_handle=$1::text::kehila_auth.intent_key AND state='cancelled'",
        &[&removed["intent_handle"].as_str().unwrap()]).await.unwrap().get::<_,i64>(0),1,
        "failed registration cleanup was not rolled back");
    let engine = serving.handle.engine.upgrade().unwrap();
    assert!(
        engine.database.try_lock().is_ok(),
        "collision retained database guard during readmission"
    );
    drop(engine);
    gate.release.send(()).unwrap();
    let (status, value) = work.await;
    assert_eq!(status, 201);
    assert_eq!(value["entry"]["intent_handle"], format!("{:032x}", 10_003));
    assert!(fault.consumed());
    drop(fault);
    assert_eq!(
        serving
            .handle
            .live
            .probe
            .snapshot()
            .native_mutex_attempts
            - before.native_mutex_attempts,
        2
    );
    assert_eq!(
        serving.handle.live.probe.take_sql_state().as_deref(),
        Some("23505")
    );

    let mut keys = identities(15_000);
    keys[0][2] = one["target_item_id"].as_str().unwrap().into();
    let fault = serving.handle.live.probe.registration_identities(keys);
    let before = serving.handle.live.probe.snapshot();
    let (status, value) = pending(
        &serving.handle,
        &serving.browser,
        "register",
        form(&ctx.project),
    )
    .await;
    assert_eq!(status, 201);
    assert_eq!(value["entry"]["target_item_id"], format!("{:032x}", 15_005));
    assert!(fault.consumed());
    drop(fault);
    assert_eq!(
        serving
            .handle
            .live
            .probe
            .snapshot()
            .native_mutex_attempts
            - before.native_mutex_attempts,
        2
    );
    assert_eq!(
        serving.handle.live.probe.take_sql_state().as_deref(),
        Some("23505")
    );

    let mut keys = identities(20_000);
    for (attempt, entry) in [&one, &two, &three].into_iter().enumerate() {
        keys[attempt][0] = entry["intent_handle"].as_str().unwrap().into();
    }
    let fault = serving.handle.live.probe.registration_identities(keys);
    let count = ctx
        .admin
        .client()
        .query_one("SELECT count(*) FROM kehila_auth.pending_items", &[])
        .await
        .unwrap()
        .get::<_, i64>(0);
    let before = serving.handle.live.probe.snapshot();
    assert_eq!(
        pending(
            &serving.handle,
            &serving.browser,
            "register",
            form(&ctx.project)
        )
        .await,
        (503, json!({"error":"registration_identity_unavailable"}))
    );
    assert!(fault.consumed());
    drop(fault);
    assert_eq!(
        serving
            .handle
            .live
            .probe
            .snapshot()
            .native_mutex_attempts
            - before.native_mutex_attempts,
        3
    );
    assert_eq!(
        ctx.admin
            .client()
            .query_one("SELECT count(*) FROM kehila_auth.pending_items", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        count
    );
    assert!(serving.handle.live.is_live());
}

async fn unrelated(ctx: &Context<'_>, serving: Serving) -> Serving {
    for (code, name, stop) in [
        ("PZ001", "controlled_pending_failure", false),
        ("23505", "unrelated_pending_identity", true),
    ] {
        let constraint = if stop {
            format!(",CONSTRAINT='{name}'")
        } else {
            String::new()
        };
        let sql=format!("SET ROLE kehila_owner; CREATE FUNCTION kehila_native_fixture.reject_pending() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'controlled pending failure' USING ERRCODE='{code}'{constraint}; END $$; CREATE TRIGGER pending_fault BEFORE INSERT ON kehila_auth.pending_items FOR EACH ROW EXECUTE FUNCTION kehila_native_fixture.reject_pending(); RESET ROLE");
        ctx.admin.client().batch_execute(&sql).await.unwrap();
        let _ = serving.handle.live.probe.take_sql_state();
        let before = serving.handle.live.probe.snapshot();
        let (status, value) = pending(
            &serving.handle,
            &serving.browser,
            "register",
            form(&ctx.project),
        )
        .await;
        assert_eq!(status, 503);
        assert_ne!(value["error"], "outcome_unknown");
        assert_eq!(
            serving.handle.live.probe.take_sql_state().as_deref(),
            Some(code)
        );
        assert_eq!(
            serving
                .handle
                .live
                .probe
                .snapshot()
                .native_mutex_attempts
                - before.native_mutex_attempts,
            1
        );
        ctx.admin.client().batch_execute("SET ROLE kehila_owner; DROP TRIGGER pending_fault ON kehila_auth.pending_items; DROP FUNCTION kehila_native_fixture.reject_pending(); RESET ROLE").await.unwrap();
        if stop {
            assert_eq!(
                serving.handle.live.failure(),
                Some(crate::lifecycle::FatalCause::NativeIntegrity)
            );
        } else {
            assert!(serving.handle.live.is_live());
        }
    }
    serving.stop(ctx.admin, true).await;
    ctx.start("after-unrelated-failure").await
}

async fn tcp_loss(ctx: &Context<'_>, serving: &Serving) {
    let old = list(serving, &ctx.project.project.id.0, Value::Null).await;
    let old = old["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["intent_handle"].clone())
        .collect::<Vec<_>>();
    let gate = serving
        .handle
        .live
        .probe
        .hold_transaction(0, Point::PendingCommit);
    let request = wire_request(
        Some(&serving.browser),
        "/items/pending/register",
        Some(form(&ctx.project)),
    );
    let _ = Box::pin(crate::listener::tests::browser_disconnect_before_reply(
        serving.handle.clone(),
        request,
        gate,
    ))
    .await;
    evidence::settled(&serving.handle).await;
    let page = list(serving, &ctx.project.project.id.0, Value::Null).await;
    let recovered = page["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| !old.contains(&entry["intent_handle"]))
        .collect::<Vec<_>>();
    assert_eq!(
        recovered.len(),
        1,
        "lost TCP reply created an unexpected number of intents"
    );
    let other = login(&serving.handle).await;
    let (status, opened) = pending(&serving.handle, &other, "open", locator(recovered[0])).await;
    assert_eq!(status, 200);
    assert!(opened["request"].is_object() && opened.get("result").is_none());
}

async fn unknown_registration(ctx: &Context<'_>, serving: Serving) -> (Serving, Value) {
    let old = list(&serving, &ctx.project.project.id.0, Value::Null).await;
    let old = old["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["intent_handle"].clone())
        .collect::<Vec<_>>();
    let engine = serving.handle.engine.upgrade().unwrap();
    engine
        .database
        .lock()
        .await
        .lose_next_confirmation_for_test();
    drop(engine);
    let before = serving.handle.live.probe.snapshot();
    assert_eq!(
        pending(
            &serving.handle,
            &serving.browser,
            "register",
            form(&ctx.project)
        )
        .await,
        (503, json!({"error":"outcome_unknown"}))
    );
    assert_eq!(
        serving
            .handle
            .live
            .probe
            .snapshot()
            .native_mutex_attempts
            - before.native_mutex_attempts,
        1
    );
    assert!(!serving.handle.live.is_live());
    assert_eq!(serving.handle.live.failure(), None);
    serving.stop(ctx.admin, false).await;
    let serving = ctx.start("registration-recovery").await;
    let page = list(&serving, &ctx.project.project.id.0, Value::Null).await;
    assert!(
        old.iter().all(|handle| page["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["intent_handle"] == *handle)),
        "restart lost an earlier prepared intent, including the TCP response-loss case"
    );
    let entries = page["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| !old.contains(&entry["intent_handle"]))
        .collect::<Vec<_>>();
    assert_eq!(
        entries.len(),
        1,
        "unknown COMMIT retried or lost registration identity"
    );
    let entry = entries[0].clone();
    assert_eq!(
        pending(&serving.handle, &serving.browser, "open", locator(&entry))
            .await
            .0,
        200
    );
    (serving, entry)
}
async fn unknown_cancellation(ctx: &Context<'_>, serving: Serving, entry: &Value) -> Serving {
    let engine = serving.handle.engine.upgrade().unwrap();
    engine
        .database
        .lock()
        .await
        .lose_next_confirmation_for_test();
    drop(engine);
    let before = serving.handle.live.probe.snapshot();
    assert_eq!(
        pending(&serving.handle, &serving.browser, "cancel", locator(entry)).await,
        (503, json!({"error":"outcome_unknown"}))
    );
    assert_eq!(
        serving
            .handle
            .live
            .probe
            .snapshot()
            .native_mutex_attempts
            - before.native_mutex_attempts,
        1
    );
    assert!(!serving.handle.live.is_live());
    assert_eq!(serving.handle.live.failure(), None);
    serving.stop(ctx.admin, false).await;
    let serving = ctx.start("cancellation-recovery").await;
    let (status, opened) = pending(&serving.handle, &serving.browser, "open", locator(entry)).await;
    assert_eq!(status, 200);
    assert_eq!(opened["entry"]["state"], "cancelled");
    assert_eq!(opened["entry"]["target_item_id"], entry["target_item_id"]);
    assert!(opened["request"].is_null());
    serving
}
pub(super) async fn exercise(ctx: &Context<'_>, serving: Serving) -> Serving {
    let before = evidence::pending_domain_snapshot(ctx.admin).await;
    collisions(ctx, &serving).await;
    let serving = unrelated(ctx, serving).await;
    tcp_loss(ctx, &serving).await;
    let (serving, entry) = unknown_registration(ctx, serving).await;
    let serving = unknown_cancellation(ctx, serving, &entry).await;
    let page = list(&serving, &ctx.project.project.id.0, Value::Null).await;
    assert!(page["before"].is_null());
    for entry in page["entries"].as_array().unwrap() {
        if entry["state"] == "prepared" {
            assert_eq!(cancel(&serving, entry).await["state"], "cancelled");
        }
    }
    unchanged(ctx, &before).await;
    serving
}

pub(super) async fn profiles(ctx: &Context<'_>) {
    let functions=[
        "kehila_auth.register_item(kehila_auth.authority,kehila.ident,kehila_auth.intent_key,kehila_auth.intent_key,kehila.ident,bigint,bytea)",
        "kehila_auth.list_pending_items(kehila_auth.authority,kehila.ident,bigint)",
        "kehila_auth.open_pending_item(kehila_auth.authority,kehila.ident,kehila_auth.intent_key)",
        "kehila_auth.cancel_item(kehila_auth.authority,kehila.ident,kehila_auth.intent_key)",
    ];
    for (i, function) in functions.iter().enumerate() {
        ctx.admin
            .client()
            .batch_execute(&format!(
                "REVOKE EXECUTE ON FUNCTION {function} FROM kehila_auth_runtime"
            ))
            .await
            .unwrap();
        assert!(Database::start(
            ctx.endpoint,
            ctx.installation.into(),
            format!("{}-missing-pending-{i}", ctx.attempt),
            Instant::now() + Duration::from_secs(10)
        )
        .await
        .is_err());
        evidence::runtime_released(ctx.admin).await;
        ctx.admin
            .client()
            .batch_execute(&format!(
                "GRANT EXECUTE ON FUNCTION {function} TO kehila_auth_runtime"
            ))
            .await
            .unwrap();
    }
    for (i,(change,restore)) in [
        ("REVOKE SELECT ON kehila_auth.item_pending_policy FROM kehila_auth_runtime","GRANT SELECT ON kehila_auth.item_pending_policy TO kehila_auth_runtime"),
        ("GRANT SELECT ON kehila_auth.pending_items TO kehila_auth_runtime","REVOKE SELECT ON kehila_auth.pending_items FROM kehila_auth_runtime"),
        ("ALTER FUNCTION kehila_auth.open_pending_item(kehila_auth.authority,kehila.ident,kehila_auth.intent_key) OWNER TO kehila_auth_admin","ALTER FUNCTION kehila_auth.open_pending_item(kehila_auth.authority,kehila.ident,kehila_auth.intent_key) OWNER TO kehila_owner"),
        ("ALTER FUNCTION kehila_auth.open_pending_item(kehila_auth.authority,kehila.ident,kehila_auth.intent_key) SECURITY INVOKER","ALTER FUNCTION kehila_auth.open_pending_item(kehila_auth.authority,kehila.ident,kehila_auth.intent_key) SECURITY DEFINER"),
        ("GRANT EXECUTE ON FUNCTION kehila_auth.save_item_create(kehila_auth.authority,kehila.ident,kehila_auth.intent_key,bytea,kehila_auth.item_preparation,jsonb) TO kehila_auth_runtime","REVOKE EXECUTE ON FUNCTION kehila_auth.save_item_create(kehila_auth.authority,kehila.ident,kehila_auth.intent_key,bytea,kehila_auth.item_preparation,jsonb) FROM kehila_auth_runtime"),
    ].into_iter().enumerate() {
        ctx.admin.client().batch_execute(change).await.unwrap();
        if i>=2 {assert!(ctx.admin.client().query_one("SELECT has_function_privilege('kehila_auth_runtime','kehila_auth.open_pending_item(kehila_auth.authority,kehila.ident,kehila_auth.intent_key)','EXECUTE')",&[]).await.unwrap().get::<_,bool>(0));}
        assert!(Database::start(ctx.endpoint,ctx.installation.into(),format!("{}-pending-profile-{i}",ctx.attempt),Instant::now()+Duration::from_secs(10)).await.is_err());
        evidence::runtime_released(ctx.admin).await;
        ctx.admin.client().batch_execute(restore).await.unwrap();
    }
}
