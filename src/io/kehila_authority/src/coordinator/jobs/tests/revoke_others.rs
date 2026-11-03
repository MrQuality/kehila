use super::*;

async fn login(handle: &Handle) -> Digest {
    let mut response = request(
        handle,
        Operation::Login {
            password: password("a fixed test password"),
            kind: LoginKind::Cli,
        },
    )
    .await;
    issued(&mut response)
}
fn revoke(bearer: Digest, value: &str) -> Operation {
    Operation::RevokeOthers {
        credential: cli(bearer),
        password: password(value),
    }
}

#[test]
#[ignore = "requires disposable PostgreSQL TLS and the controlled audit rejection/retry fixture"]
fn native_revoke_others() {
    let (endpoint, installation, attempt, controls) = fixture();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let admin = crate::postgres::tests::fixture_admin::Admin::connect(&endpoint).await;
            admin.client().batch_execute("SET ROLE kehila_owner; CREATE FUNCTION kehila_native_fixture.reject_revocation() RETURNS trigger LANGUAGE plpgsql SET search_path=pg_catalog,pg_temp AS $$ BEGIN RAISE EXCEPTION 'owned revocation audit fault' USING ERRCODE='PZ002'; END $$; REVOKE ALL ON FUNCTION kehila_native_fixture.reject_revocation() FROM PUBLIC; CREATE TRIGGER native_revoke_fault BEFORE INSERT ON kehila_auth.management_events FOR EACH ROW WHEN (NEW.event_kind='session_revoke') EXECUTE FUNCTION kehila_native_fixture.reject_revocation(); RESET ROLE").await.unwrap();
            let database = Database::start(
                &endpoint,
                installation,
                attempt,
                Instant::now() + Duration::from_secs(30),
            )
            .await
            .unwrap();
            let (handle, owner) = Owner::new(database, Blocklist::synthetic_for_tests())
                .await
                .unwrap();
            let engine = Arc::downgrade(owner.engine());
            let running = tokio::spawn(owner.run());
            let initiator = login(&handle).await;
            let other = login(&handle).await;
            // Check the rejected password before retaining a deadline-bound
            // browser reply across the separate successful revocation work.
            assert!(matches!(
                request(&handle, revoke(initiator, "a wrong owner password"))
                    .await
                    .result(),
                Err(Error::Unauthorized)
            ));
            assert!(matches!(
                request(&handle, Operation::State(cli(other)))
                    .await
                    .result(),
                Ok(Output::State { active: true, .. })
            ));
            let pre = request(&handle, Operation::PreSession(None)).await;
            let Output::PreSession {
                bearer: Some(bearer),
                csrf,
            } = pre.result().unwrap()
            else {
                panic!("missing pre-session")
            };
            let kind = LoginKind::Browser {
                bearer: bearer.digest(),
                csrf: Digest::from_encoded(csrf.as_slice()).unwrap(),
            };
            drop(pre);
            let mut retained = request(
                &handle,
                Operation::Login {
                    password: password("a fixed test password"),
                    kind,
                },
            )
            .await;


            // The fixture rejects the first SQL audit. Every selected local target
            // remains revoked, while the confirmed rollback preserves this runtime.
            assert!(matches!(
                request(&handle, revoke(initiator, "a fixed test password"))
                    .await
                    .result(),
                Err(Error::OtherAuditUnconfirmed)
            ));
            assert!(handle.live.is_live());
            assert!(matches!(
                request(&handle, Operation::State(cli(initiator)))
                    .await
                    .result(),
                Ok(Output::State { active: true, .. })
            ));
            assert!(matches!(
                request(&handle, Operation::State(cli(other)))
                    .await
                    .result(),
                Err(Error::UnknownCredential)
            ));
            let Output::Issued(delivery) = retained.result().unwrap() else {
                panic!("missing retained issue")
            };
            assert!(delivery.material().is_err());
            assert!(retained.confirm_exchange().is_err());
            drop(retained);
            let later = login(&handle).await;
            mark(&controls, "retry-ready");
            assert_eq!(admin.client().query_one("SELECT count(*) FROM kehila_auth.management_events WHERE event_kind='session_revoke'", &[]).await.unwrap().get::<_,i64>(0),0);
            admin.client().batch_execute("SET ROLE kehila_owner; DROP TRIGGER native_revoke_fault ON kehila_auth.management_events; DROP FUNCTION kehila_native_fixture.reject_revocation(); RESET ROLE").await.unwrap();

            // Retrying the pending batch cannot select a later login.
            assert!(matches!(
                request(&handle, revoke(initiator, "a fixed test password"))
                    .await
                    .result(),
                Ok(Output::OthersRevoked { count: 2 })
            ));
            assert!(matches!(
                request(&handle, Operation::State(cli(later)))
                    .await
                    .result(),
                Ok(Output::State { active: true, .. })
            ));
            assert!(matches!(
                request(&handle, Operation::Logout(cli(other)))
                    .await
                    .result(),
                Err(Error::UnknownCredential)
            ));
            assert!(matches!(
                request(&handle, revoke(initiator, "a fixed test password"))
                    .await
                    .result(),
                Ok(Output::OthersRevoked { count: 1 })
            ));

            // A new batch may commit without its confirmation reaching the caller.
            login(&handle).await;
            {
                let engine = engine.upgrade().unwrap();
                engine
                    .database
                    .lock()
                    .await
                    .lose_next_confirmation_for_test();
            }
            assert!(matches!(
                request(&handle, revoke(initiator, "a fixed test password"))
                    .await
                    .result(),
                Err(Error::OtherAuditUnconfirmed)
            ));
            assert!(!handle.live.is_live());
            running.await.unwrap().unwrap();
            admin.close().await;
            mark(&controls, "passed");
        });
}
