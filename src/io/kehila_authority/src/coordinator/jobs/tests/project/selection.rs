//! Prepared native selection journey. Real browser interaction is separate.
use super::*;
use crate::postgres::tests::fixture_admin::Admin;
use work_item_codec::SavedResult;
mod cleanup;
mod clock;
mod contention;
mod edge;
mod lost_reply;
mod references;
mod retry;
mod rollback;

async fn selection(
    handle: &Handle,
    browser: Option<&Browser>,
    path: &str,
    body: Value,
    expected_status: u16,
) -> Value {
    let (status, value, _) = call(handle, browser, &format!("/selection/{path}"), Some(body)).await;
    assert_eq!(status, expected_status, "selection route {path}: {value}");
    value
}

fn command(token: char, expected: u64, target: Value) -> Value {
    json!({"operation_token":token.to_string().repeat(32),
        "expected_version":expected.to_string(),"requested":target})
}

fn receipt(outcome: &str, version: u64, target: &Value) -> Value {
    json!({"codec_version":1,"outcome":outcome,
        "selection":{"user_id":"owner","version":version.to_string(),"requested":target}})
}

async fn current(admin: &Admin, version: u64, ordinal: u64, item: Option<&str>) {
    let row = admin.client().query_one(
        "SELECT selection_version::numeric(20,0)::text,event_ordinal::numeric(20,0)::text,\
         selected_item_id::text,(SELECT count(*) FROM kehila.selection_events WHERE user_id='owner'),\
         (SELECT count(*) FROM kehila.operations WHERE command_family='selection') \
         FROM kehila.current_work_selections WHERE user_id='owner'", &[]).await.unwrap();
    assert_eq!(row.get::<_, String>(0), version.to_string());
    assert_eq!(row.get::<_, String>(1), ordinal.to_string());
    assert_eq!(row.get::<_, Option<String>>(2).as_deref(), item);
    assert_eq!(row.get::<_, i64>(3), i64::try_from(ordinal).unwrap());
    assert_eq!(row.get::<_, i64>(4), i64::try_from(ordinal).unwrap());
}

async fn unrelated(admin: &Admin) -> Vec<(String, String)> {
    // Generic operation cores/payloads are expected to gain selection success.
    // Every existing domain/configuration/item/pending artifact must stay exact.
    evidence::snapshot(admin)
        .await
        .into_iter()
        .filter(|(name, _)| {
            !matches!(
                name.as_str(),
                "kehila.operations"
                    | "kehila.operation_payloads"
                    | "kehila.selection_events"
                    | "kehila.current_work_selections"
            )
        })
        .collect()
}

#[test]
#[ignore = "requires a fresh explicitly provisioned product PostgreSQL TLS fixture"]
fn native_current_work_selection_and_recovery() {
    let (endpoint, installation, attempt, controls) = fixture();
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let (handle, owner) = runtime(&endpoint, &installation, format!("{attempt}-selection-initial")).await;
        let running = tokio::spawn(owner.run());
        let browser = login(&handle).await;
        // The initial expected-red run must reach the absent authenticated
        // selection route before any item fixture setup can fail instead.
        assert_eq!(selection(&handle, Some(&browser), "read", json!({}), 200).await,
            json!({"codec_version":1,"outcome":"read","version":"0","target":{"state":"none"}}));
        let admin = Admin::connect(&endpoint).await;
        let (status, pending) = project(&handle, &browser, "register",
            json!({"name":"Selection journey","prefix":"SEL","estimate_unit":"hours"})).await;
        assert_eq!(status, 201);
        let (status, created) = project(&handle, &browser, "execute", execute(&pending["entry"])).await;
        assert_eq!(status, 201);
        let initial = project_create_codec::decode_result(1, &serde_json::to_vec(&created["result"]).unwrap()).unwrap();
        let first = item_reads::seed(&admin, &initial.project, &initial.configuration, 1).await;
        let SavedResult::Create(first) = first else { panic!("wrong seeded item family") };
        let second = item_reads::seed(&admin, &first.project, &initial.configuration, 2).await;
        let SavedResult::Create(second) = second else { panic!("wrong seeded item family") };
        let target_a = json!({"project_id":first.project.id.0,"item_id":first.item.id.0});
        let target_b = json!({"project_id":second.project.id.0,"item_id":second.item.id.0});
        let before = unrelated(&admin).await;
        edge::exercise(&handle, &browser, &admin).await;

        selection(&handle, None, "read", json!({}), 401).await;
        let mut wrong_csrf = browser.clone();
        let replacement = if wrong_csrf.csrf.starts_with('A') { "B" } else { "A" };
        wrong_csrf.csrf.replace_range(..1, replacement);
        assert_eq!(selection(&handle, Some(&wrong_csrf), "read", json!({}), 403).await,
            json!({"error":"forbidden"}));
        selection(&handle, Some(&browser), "read", json!({"user_id":"someone-else"}), 400).await;
        let absent = command('9', 0, target_a.clone());
        assert_eq!(selection(&handle, Some(&browser), "reconcile", absent, 200).await,
            json!({"codec_version":1,"outcome":"absent"}));
        assert_eq!(admin.client().query_one("SELECT count(*) FROM kehila.selection_events", &[])
            .await.unwrap().get::<_, i64>(0), 0, "lookup-only reconciliation executed an unseen command");

        let set_a = command('a', 0, target_a.clone());
        retry::exercise(&handle,&browser,&admin).await;
        references::exercise(&handle,&browser,&admin,&target_a).await;
        contention::same_identity(&handle,&browser,&admin,set_a.clone(),&target_a).await;
        current(&admin, 1, 5, Some(&first.item.id.0)).await;
        contention::noops(&handle, &browser, &admin, target_a.clone()).await;
        assert_eq!(unrelated(&admin).await, before);
        handle.stop();
        running.await.unwrap().unwrap();
        evidence::runtime_released(&admin).await;

        let (handle, owner) = runtime(&endpoint, &installation, format!("{attempt}-selection-persisted")).await;
        let engine = Arc::downgrade(owner.engine());
        let running = tokio::spawn(owner.run());
        let browser = login(&handle).await;
        let another_login = login(&handle).await;
        let expected_read = json!({"codec_version":1,"outcome":"read","version":"1",
            "target":{"state":"available","project_id":first.project.id.0,"item_id":first.item.id.0}});
        for login in [&browser, &another_login] {
            assert_eq!(selection(&handle, Some(login), "read", json!({}), 200).await, expected_read);
        }
        contention::changed_version(&handle, &browser, &admin, target_a.clone(), target_b.clone()).await;
        for route in ["set", "reconcile"] {
            assert_eq!(selection(&handle, Some(&browser), route, set_a.clone(), 200).await,
                receipt("replayed", 1, &target_a));
            current(&admin, 2, 8, Some(&second.item.id.0)).await;
            assert_eq!(selection(&handle, Some(&browser), route, command('a', 0, target_b.clone()), 409).await,
                json!({"codec_version":1,"outcome":"rejected","code":"operation_id_reused"}));
        }
        assert_eq!(selection(&handle, Some(&browser), "set", command('8', 1, target_b.clone()), 409).await,
            json!({"codec_version":1,"outcome":"rejected","code":"selection_version_conflict","current_version":"2"}));
        current(&admin, 2, 8, Some(&second.item.id.0)).await;

        // Synthetic stored-grant revocation, not a product administration route.
        admin.client().execute("UPDATE kehila.project_grants SET active=false WHERE actor_id='owner' AND project_id=$1::text::kehila.ident",
            &[&first.project.id.0]).await.unwrap();
        // This runtime already issued two anonymous sessions. Keep the real
        // two-entry burst and twelve-second token refill; never raise its limit
        // to accommodate the fixture's deliberately rapid login sequence.
        tokio::time::sleep(Duration::from_secs(12)).await;
        let revoked_login = login(&handle).await;
        assert_eq!(selection(&handle, Some(&revoked_login), "read", json!({}), 200).await,
            json!({"codec_version":1,"outcome":"read","version":"2","target":{"state":"unavailable"}}));
        for request in [set_a.clone(), command('a', 0, Value::Null)] {
            assert_eq!(selection(&handle, Some(&revoked_login), "reconcile", request, 403).await,
                json!({"error":"forbidden"}));
        }
        let clear = command('d', 2, Value::Null);
        assert_eq!(selection(&handle, Some(&revoked_login), "set", clear.clone(), 200).await,
            receipt("accepted", 3, &Value::Null));
        assert_eq!(selection(&handle, Some(&revoked_login), "reconcile", clear, 200).await,
            receipt("replayed", 3, &Value::Null));
        current(&admin, 3, 9, None).await;
        admin.client().execute("UPDATE kehila.project_grants SET active=true WHERE actor_id='owner' AND project_id=$1::text::kehila.ident",
            &[&first.project.id.0]).await.unwrap();
        assert_eq!(unrelated(&admin).await, before);
        rollback::exercise(&handle, &browser, &admin, target_a.clone()).await;
        current(&admin, 3, 9, None).await;

        // Real COMMIT, deliberately withheld confirmation at the existing DB seam.
        // Distinct from the real TCP reply-loss case below.
        let unknown = command('e', 3, target_a.clone());
        engine.upgrade().unwrap().database.lock().await.lose_next_confirmation_for_test();
        assert_eq!(selection(&handle, Some(&browser), "set", unknown.clone(), 503).await,
            json!({"error":"outcome_unknown"}));
        assert!(!handle.live.is_live(),"unknown commit must stop further admission");
        assert_eq!(handle.live.failure(),None,"uncertain commit is not a fabricated integrity fault");
        running.await.unwrap().unwrap();
        evidence::runtime_released(&admin).await;
        current(&admin, 4, 10, Some(&first.item.id.0)).await;

        let (handle, owner) = runtime(&endpoint, &installation, format!("{attempt}-selection-unknown")).await;
        let running = tokio::spawn(owner.run());
        let browser = login(&handle).await;
        assert_eq!(selection(&handle, Some(&browser), "reconcile", unknown, 200).await,
            receipt("replayed", 4, &target_a));
        current(&admin, 4, 10, Some(&first.item.id.0)).await;
        assert_eq!(selection(&handle, Some(&browser), "set", command('f', 4, Value::Null), 200).await,
            receipt("accepted", 5, &Value::Null));
        current(&admin, 5, 11, None).await;
        let lost=command('7',5,Value::Null);
        lost_reply::exercise(&handle,&browser,&admin,&lost).await;
        assert_eq!(unrelated(&admin).await, before);
        handle.stop();
        running.await.unwrap().unwrap();
        evidence::runtime_released(&admin).await;
        let (handle,owner)=runtime(&endpoint,&installation,format!("{attempt}-selection-lost-reply")).await;
        let running=tokio::spawn(owner.run());
        let browser=login(&handle).await;
        assert_eq!(selection(&handle,Some(&browser),"reconcile",lost,200).await,receipt("replayed",5,&Value::Null));
        current(&admin,5,12,None).await;
        assert_eq!(unrelated(&admin).await,before);
        let cleanup = cleanup::exercise(&handle, &browser, &admin, set_a).await;
        handle.stop();
        let joined = running.await;
        evidence::runtime_released(&admin).await;
        cleanup.verify();
        assert!(matches!(joined, Ok(Err(Error::Unavailable))), "fatal rollback must join with its retained failure");
        current(&admin,5,12,None).await;
        assert_eq!(unrelated(&admin).await,before);
        admin.close().await;
        mark(&controls, "selection-passed");
    });
}
