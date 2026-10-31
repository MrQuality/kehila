use super::*;
use crate::{owner_server::tests::cli_state, session::ExpiryCase};
use hyper::StatusCode;

#[test]
#[ignore = "requires the explicitly provisioned disposable PostgreSQL TLS fixture"]
fn native_cli_expiry_over_tls() {
    let (endpoint, installation, attempt, _) = fixture();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
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
            for case in [ExpiryCase::Idle, ExpiryCase::Absolute] {
                let mut response = request(
                    &handle,
                    Operation::Login {
                        password: password("a fixed test password"),
                        kind: LoginKind::Cli,
                    },
                )
                .await;
                let Output::Issued(delivery) = response.result().unwrap() else {
                    panic!("login did not issue a credential");
                };
                let encoded = Zeroizing::new(*delivery.material().unwrap().0);
                let digest = issued(&mut response);
                drop(response);
                let (status, body) = cli_state(handle.clone(), &encoded).await;
                assert_eq!(status, StatusCode::OK);
                assert_eq!(body.as_ref(), b"{\"active\":true}");
                engine
                    .upgrade()
                    .unwrap()
                    .sessions
                    .lock()
                    .unwrap()
                    .expire_for_test(&digest, case)
                    .unwrap();
                let (status, body) = cli_state(handle.clone(), &encoded).await;
                assert_eq!(
                    status,
                    StatusCode::UNAUTHORIZED,
                    "{case:?} expired session reached HTTP success"
                );
                assert_eq!(body.as_ref(), b"{\"error\":\"request rejected\"}");
            }
            handle.stop();
            assert!(running.await.unwrap().is_ok());
        });
}
