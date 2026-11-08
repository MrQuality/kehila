//! Observed serving-network deadline and one owned successor, not same-owner retry.
use super::*;
use crate::postgres::tests::{fixture_admin::Admin, relay::Relay};

async fn joined<T>(mut task: tokio::task::JoinHandle<T>) -> Result<T, String> {
    match tokio::time::timeout(Duration::from_secs(10), &mut task).await {
        Ok(result) => result.map_err(|error| error.to_string()),
        Err(_) => {
            task.abort();
            let joined = tokio::time::timeout(Duration::from_secs(5), &mut task).await;
            Err(format!(
                "owned task deadline expired; abort join observed={}",
                joined.is_ok()
            ))
        }
    }
}

async fn generation(admin: &Admin) -> u64 {
    admin
        .client()
        .query_one(
            "SELECT runtime_generation::text FROM kehila_auth.installation WHERE singleton",
            &[],
        )
        .await
        .unwrap()
        .get::<_, String>(0)
        .parse()
        .unwrap()
}

pub(super) async fn exercise(
    endpoint: &crate::postgres::Endpoint,
    installation: &str,
    attempt: &str,
    admin: &Admin,
    project_id: &str,
    expected: &Value,
) {
    evidence::runtime_released(admin).await;
    let before = evidence::snapshot(admin).await;
    let initial_generation = generation(admin).await;
    let (relayed_endpoint, mut relay, control) = Relay::start(endpoint).await;
    let (handle, owner) = runtime(
        &relayed_endpoint,
        installation,
        format!("{attempt}-network-stall"),
    )
    .await;
    let running = tokio::spawn(owner.run());
    let jobs = handle.clone();
    let id = project_id.to_owned();
    let expected_read = expected.clone();
    let work = tokio::spawn(async move {
        let browser = login(&jobs).await;
        let (status, read) = project(&jobs, &browser, "read", json!({"project_id":id})).await;
        assert_eq!(status, 200);
        assert_eq!(read, expected_read);
        evidence::settled(&jobs).await;
        assert!(jobs.live.is_live());
        let admissions = jobs.live.probe.snapshot();
        let held = control.arm();
        // A containment budget for this fault control, not a product latency SLO.
        let expires = Instant::now() + Duration::from_secs(1);
        let mut reply = jobs
            .submit(
                Operation::Project {
                    credential: admission::credential(&browser),
                    command: crate::project::Command::Read(id.clone()),
                },
                expires,
            )
            .unwrap();
        tokio::select! {
            observed = held => observed.unwrap(),
            _ = &mut reply => panic!("native read completed before serving-response stall"),
        }
        assert!(
            jobs.live.is_live(),
            "unrelated liveness failure masked the stall"
        );
        assert!(
            jobs.live.probe.snapshot().database_ownership_entries
                > admissions.database_ownership_entries
        );
        let response = tokio::time::timeout(WORK_DEADLINE, reply)
            .await
            .unwrap()
            .unwrap();
        assert!(Instant::now() >= expires);
        // Do not let result() replace an expired stored success with Unavailable.
        assert!(matches!(&response.result, Err(Error::Unavailable)));
        assert!(!jobs.live.is_live());
        assert!(matches!(
            jobs.submit(
                Operation::Project {
                    credential: admission::credential(&browser),
                    command: crate::project::Command::Read(id),
                },
                deadline()
            ),
            Err(Error::Unavailable)
        ));
        drop(response);
    });
    let tested = joined(work).await;
    // Parent owns cleanup even when any fault assertion panics or times out.
    // Explicit cleanup releases the gate after the original deadline. A closed
    // serving peer may terminate the downstream relay write during shutdown.
    relay.begin_cleanup();
    handle.stop();
    let stopped = joined(running)
        .await
        .and_then(|result| result.map_err(|error| format!("{error:?}")));
    let relay_joined = relay.join().await;
    eprintln!("network-stall assertions={tested:?}; owner={stopped:?}; relay={relay_joined:?}");
    evidence::runtime_released(admin).await;
    assert!(
        tested.is_ok(),
        "network-stall assertions failed: {tested:?}"
    );
    assert!(
        stopped.is_ok(),
        "network-stall owner join failed: {stopped:?}"
    );
    assert!(
        relay_joined.is_ok(),
        "network-stall relay failed: {relay_joined:?}"
    );
    assert_eq!(generation(admin).await, initial_generation + 1);
    assert_eq!(evidence::snapshot(admin).await, before);

    let (handle, owner) = runtime(
        endpoint,
        installation,
        format!("{attempt}-network-successor"),
    )
    .await;
    let running = tokio::spawn(owner.run());
    let jobs = handle.clone();
    let id = project_id.to_owned();
    let expected = expected.clone();
    let tested = joined(tokio::spawn(async move {
        let browser = login(&jobs).await;
        let (status, read) = project(&jobs, &browser, "read", json!({"project_id":id})).await;
        assert_eq!(status, 200);
        assert_eq!(read, expected);
        assert!(jobs.live.is_live());
        evidence::settled(&jobs).await;
    }))
    .await;
    handle.stop();
    let stopped = joined(running)
        .await
        .and_then(|result| result.map_err(|error| format!("{error:?}")));
    eprintln!("network-successor assertions={tested:?}; owner={stopped:?}");
    evidence::runtime_released(admin).await;
    assert!(tested.is_ok(), "successor read failed: {tested:?}");
    assert!(stopped.is_ok(), "successor join failed: {stopped:?}");
    assert_eq!(generation(admin).await, initial_generation + 2);
    assert_eq!(evidence::snapshot(admin).await, before);
}
