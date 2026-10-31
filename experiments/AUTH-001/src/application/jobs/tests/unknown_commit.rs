use super::*;
use crate::database::Endpoint;

async fn runtime(endpoint: &Endpoint, installation: &str, attempt: &str) -> (Handle, Owner) {
    let database = Database::start(
        endpoint,
        installation.to_owned(),
        attempt.to_owned(),
        Instant::now() + Duration::from_secs(30),
    )
    .await
    .unwrap();
    Owner::new(database, Blocklist::synthetic_for_tests())
        .await
        .unwrap()
}

async fn lose_next_confirmation(engine: &std::sync::Weak<Engine>) {
    let engine = engine.upgrade().unwrap();
    engine
        .database
        .lock()
        .await
        .lose_next_confirmation_for_test();
}

async fn login(handle: &Handle, value: &str) -> Digest {
    let mut response = request(
        handle,
        Operation::Login {
            password: password(value),
            kind: LoginKind::Cli,
        },
    )
    .await;
    issued(&mut response)
}

#[test]
#[ignore = "requires the explicitly provisioned disposable PostgreSQL TLS fixture"]
fn native_unknown_commit_consumers() {
    let (endpoint, installation, attempt, controls) = fixture();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            // Issuance history may commit without any credential being delivered.
            let (handle, owner) =
                runtime(&endpoint, &installation, &format!("{attempt}-issue")).await;
            let engine = Arc::downgrade(owner.engine());
            let running = tokio::spawn(owner.run());
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
            lose_next_confirmation(&engine).await;
            let response = request(
                &handle,
                Operation::Login {
                    password: password("a fixed test password"),
                    kind,
                },
            )
            .await;
            assert!(matches!(response.result(), Err(Error::Unavailable)));
            assert!(!handle.live.is_live());
            assert!(matches!(
                handle.submit(Operation::PreSession(None), deadline()),
                Err(Error::Unavailable)
            ));
            drop(response);
            running.await.unwrap().unwrap();

            // Locally revoked means signed out even when logout's durable audit
            // confirmation is unknown. The typed outcome must survive runtime stop.
            let (handle, owner) =
                runtime(&endpoint, &installation, &format!("{attempt}-logout")).await;
            let engine = Arc::downgrade(owner.engine());
            let running = tokio::spawn(owner.run());
            let bearer = login(&handle, "a fixed test password").await;
            lose_next_confirmation(&engine).await;
            let response = request(&handle, Operation::Logout(cli(bearer))).await;
            assert!(matches!(response.result(), Err(Error::AuditUnconfirmed)));
            assert!(!handle.live.is_live());
            drop(response);
            running.await.unwrap().unwrap();

            // Do not blindly retry a replacement after losing its confirmation.
            // A fresh runtime/login establishes which password actually committed.
            let (handle, owner) =
                runtime(&endpoint, &installation, &format!("{attempt}-password")).await;
            let engine = Arc::downgrade(owner.engine());
            let running = tokio::spawn(owner.run());
            let bearer = login(&handle, "a fixed test password").await;
            lose_next_confirmation(&engine).await;
            let response = request(
                &handle,
                Operation::Password {
                    credential: cli(bearer),
                    current: password("a fixed test password"),
                    replacement: password("a newly chosen owner password"),
                },
            )
            .await;
            assert!(matches!(response.result(), Err(Error::Unavailable)));
            assert!(!handle.live.is_live());
            drop(response);
            running.await.unwrap().unwrap();

            let (handle, owner) =
                runtime(&endpoint, &installation, &format!("{attempt}-reconcile")).await;
            let running = tokio::spawn(owner.run());
            let wrong = request(
                &handle,
                Operation::Login {
                    password: password("a fixed test password"),
                    kind: LoginKind::Cli,
                },
            )
            .await;
            assert!(matches!(wrong.result(), Err(Error::Unauthorized)));
            drop(wrong);
            let current = login(&handle, "a newly chosen owner password").await;
            assert!(matches!(
                request(&handle, Operation::State(cli(current)))
                    .await
                    .result(),
                Ok(Output::State { active: true, .. })
            ));
            handle.stop();
            running.await.unwrap().unwrap();
            mark(&controls, "passed");
        });
}
