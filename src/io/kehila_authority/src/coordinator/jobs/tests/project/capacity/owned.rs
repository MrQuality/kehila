//! Shared ownership boundary for validity controls and individual capacity cells.
use super::*;
use std::future::Future;
type SharedAdmin = Arc<tokio::sync::Mutex<Admin>>;

pub(super) async fn run<T, F, Work>(
    endpoint: &crate::postgres::Endpoint,
    installation: &str,
    attempt: String,
    work: F,
) -> (T, crate::ConnectionMetrics)
where
    T: Send + 'static,
    F: FnOnce(Handle, Client, SharedAdmin) -> Work + Send + 'static,
    Work: Future<Output = T> + Send + 'static,
{
    run_with_blocker(
        endpoint,
        installation,
        attempt,
        false,
        move |handle, client, admin, _| work(handle, client, admin),
    )
    .await
}

pub(super) async fn run_with_blocker<T, F, Work>(
    endpoint: &crate::postgres::Endpoint,
    installation: &str,
    attempt: String,
    calibration: bool,
    work: F,
) -> (T, crate::ConnectionMetrics)
where
    T: Send + 'static,
    F: FnOnce(Handle, Client, SharedAdmin, Option<SharedAdmin>) -> Work + Send + 'static,
    Work: Future<Output = T> + Send + 'static,
{
    let (handle, owner) = runtime(endpoint, installation, attempt).await;
    let mut running = tokio::spawn(owner.run());
    let admin = Arc::new(tokio::sync::Mutex::new(Admin::connect(endpoint).await));
    let blocker = if calibration {
        Some(Arc::new(tokio::sync::Mutex::new(
            Admin::connect(endpoint).await,
        )))
    } else {
        None
    };
    let setup = crate::listener::tests::capacity::start(handle.clone()).await;
    let mut server = None;
    let outcome = match setup {
        Ok((client, retained)) => {
            server = Some(retained);
            let jobs = handle.clone();
            let retained_admin = admin.clone();
            let retained_blocker = blocker.clone();
            tokio::spawn(async move { work(jobs, client, retained_admin, retained_blocker).await })
                .await
                .map_err(|error| format!("{error:?}"))
        }
        Err(error) => Err(format!("transport setup: {error}")),
    };
    // Release controlled locks before draining transport or stopping authority,
    // including when the work task panicked. The child cannot own this cleanup.
    let blocker_cleanup = if let Some(blocker) = blocker {
        match Arc::try_unwrap(blocker) {
            Ok(blocker) => {
                let blocker = blocker.into_inner();
                let rollback = tokio::time::timeout(
                    crate::CONNECTION_DEADLINE,
                    blocker.client().batch_execute("ROLLBACK"),
                )
                .await;
                // Admin's close asserts driver completion. Contain that panic
                // so it cannot bypass transport/root cleanup or mask primary.
                let close = tokio::spawn(async move { blocker.close().await }).await;
                if matches!(rollback, Ok(Ok(()))) && close.is_ok() {
                    Ok(())
                } else {
                    Err(format!("rollback={rollback:?}; close={close:?}"))
                }
            }
            Err(blocker) => {
                let rollback = match blocker.try_lock() {
                    Ok(guard) => format!(
                        "{:?}",
                        tokio::time::timeout(
                            crate::CONNECTION_DEADLINE,
                            guard.client().batch_execute("ROLLBACK")
                        )
                        .await
                    ),
                    Err(_) => "unjoined owner still holds blocker mutex".into(),
                };
                drop(blocker);
                Err(format!("unjoined blocker; rollback={rollback:?}"))
            }
        }
    } else {
        Ok(())
    };
    let connections = match server {
        Some(server) => server.finish().await,
        None => Err(crate::io_error("capacity transport setup failed")),
    };
    handle.stop();
    let root = tokio::time::timeout(Duration::from_secs(10), &mut running).await;
    if root.is_err() {
        running.abort();
        let _ = tokio::time::timeout(Duration::from_secs(5), &mut running)
            .await
            .expect("capacity root cleanup unobserved");
    }
    let admin = Arc::try_unwrap(admin)
        .unwrap_or_else(|_| panic!("unjoined capacity admin"))
        .into_inner();
    evidence::runtime_released(&admin).await;
    admin.close().await;
    assert!(
        outcome.is_ok() && blocker_cleanup.is_ok() && matches!(root, Ok(Ok(Ok(())))) && connections.is_ok(),
        "capacity case failed: primary={:?}, blocker={blocker_cleanup:?}, root={root:?}, connections={connections:?}",
        outcome.as_ref().err()
    );
    (outcome.unwrap(), connections.unwrap())
}
