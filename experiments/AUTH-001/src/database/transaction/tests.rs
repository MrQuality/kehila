use super::*;
use crate::database::tests::{fixture, mark, wait};

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(5)
}

#[test]
#[ignore = "requires the explicitly provisioned disposable PostgreSQL TLS fixture"]
fn native_fenced_roundtrip() {
    let (endpoint, installation, attempt, controls) = fixture();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut database = Database::start(
                &endpoint,
                installation,
                attempt,
                Instant::now() + Duration::from_secs(30),
            )
            .await
            .unwrap();
            let proof = database.credentials(deadline()).await.unwrap().authority;
            assert_eq!((proof.authentication, proof.revision), (1, 1));

            let wrong_revision = Authority {
                installation: proof.installation.clone(),
                owner: proof.owner.clone(),
                runtime: proof.runtime,
                authentication: proof.authentication,
                revision: 2,
            };
            let error = match database.fenced_password(&wrong_revision, deadline()).await {
                Ok(_) => panic!("wrong verifier revision passed the password fence"),
                Err(error) => error,
            };
            assert_eq!(error, DbError::Rejected("55000".to_owned()));
            assert!(database.is_live());

            let mut transaction = database.fenced_password(&proof, deadline()).await.unwrap();
            transaction
                .issue("rolled-back-session", Kind::Browser)
                .await
                .unwrap();
            transaction.rollback().await.unwrap();
            mark(&controls, "rolled-back");
            // The independent fixture verifies that the actual audit write rolled
            // back before releasing this barrier; a duplicate audit API cannot
            // substitute for inspecting durable absence.
            wait(&controls, "rollback-observed").await;

            let mut transaction = database.fenced_password(&proof, deadline()).await.unwrap();
            transaction
                .issue("driver-session", Kind::Browser)
                .await
                .unwrap();
            let issued = transaction.commit().await.unwrap();
            assert_eq!(
                (issued.runtime, issued.authentication, issued.revision),
                (proof.runtime, 1, 1)
            );

            let mut transaction = database
                .fenced_session(
                    &issued.installation,
                    issued.runtime,
                    issued.authentication,
                    deadline(),
                )
                .await
                .unwrap();
            transaction
                .logout("driver-session", Kind::Browser)
                .await
                .unwrap();
            transaction.commit().await.unwrap();

            let mut transaction = database.fenced_password(&proof, deadline()).await.unwrap();
            transaction
                .change_password(
                    "synthetic-updated-driver-only-verifier",
                    "driver-change",
                    "driver-session",
                )
                .await
                .unwrap();
            let changed = transaction.commit().await.unwrap();
            assert_eq!((changed.authentication, changed.revision), (2, 2));
            assert_eq!(changed.owner, issued.owner);

            let error = match database.fenced_password(&proof, deadline()).await {
                Ok(_) => panic!("stale password proof passed the current fence"),
                Err(error) => error,
            };
            assert_eq!(error, DbError::Rejected("55000".to_owned()));
            assert!(database.is_live()); // Confirmed rollback permits another read.
            let current = database.credentials(deadline()).await.unwrap();
            assert_eq!(
                (current.authority.authentication, current.authority.revision),
                (2, 2)
            );

            let live = database.liveness();
            mark(&controls, "operation-ready");
            wait(&controls, "start-operation").await;
            let mut transaction = database
                .fenced_session(
                    &changed.installation,
                    changed.runtime,
                    changed.authentication,
                    deadline(),
                )
                .await
                .unwrap();
            {
                let operation = transaction.logout("cancelled-logout", Kind::Browser);
                tokio::pin!(operation);
                tokio::select! {
                    _ = &mut operation => panic!("fixture did not hold the audit operation"),
                    () = wait(&controls, "cancel-operation") => {}
                }
            }
            // The wrapper itself is still alive: its borrowed operation's
            // cancellation must independently close liveness and forbid reuse.
            assert!(!live.is_live());
            assert_eq!(
                transaction.logout("must-not-write", Kind::Browser).await,
                Err(DbError::Unavailable)
            );
            mark(&controls, "operation-cancelled");
            wait(&controls, "operation-released").await;
            assert!(transaction.commit().await.is_err());
            let shutdown = database.shutdown().await;
            assert_eq!(shutdown.joined, 2);
            assert!(!shutdown.aborted);
            mark(&controls, "passed");
        });
}
