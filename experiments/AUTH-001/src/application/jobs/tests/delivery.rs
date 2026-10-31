use super::*;
use crate::owner_server::tests::{cli_login_without_peer_close, cli_state};

#[test]
#[ignore = "requires the explicitly provisioned disposable PostgreSQL TLS fixture"]
fn native_cli_delivery_without_peer_close() {
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
            let running = tokio::spawn(owner.run());
            let (status, body) = cli_login_without_peer_close(handle.clone()).await;
            assert_eq!(status, hyper::StatusCode::OK);
            let reply: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(reply.as_object().unwrap().len(), 1);
            let bearer = Zeroizing::new(
                reply["bearer"]
                    .as_str()
                    .unwrap()
                    .as_bytes()
                    .try_into()
                    .unwrap(),
            );
            let (status, body) = cli_state(handle.clone(), &bearer).await;
            assert_eq!(
                status,
                hyper::StatusCode::OK,
                "a fully flushed login was retired by later TLS cleanup"
            );
            assert_eq!(body.as_ref(), b"{\"active\":true}");
            handle.stop();
            assert!(running.await.unwrap().is_ok());
        });
}
