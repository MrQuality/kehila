//! Incomplete native socket bodies do not occupy authoritative database work.
use super::*;

pub(super) async fn exercise(
    handle: &Handle,
    browser: &Browser,
    project_id: &Value,
    expected: &Value,
) {
    let body = json!({"project_id":project_id});
    let request = || wire_request(Some(browser), "/projects/read", Some(body.clone()));
    let (status, bytes) = Box::pin(crate::listener::tests::body::exchange::<
        std::future::Ready<()>,
    >(handle.clone(), request(), None))
    .await;
    assert_eq!(status, hyper::StatusCode::OK);
    assert_eq!(&serde_json::from_slice::<Value>(&bytes).unwrap(), expected);
    evidence::settled(handle).await;
    let before = handle.live.probe.snapshot();
    let concurrent = async {
        // Called only after native admission reaches a pending socket-body poll.
        assert_eq!(handle.live.probe.snapshot(), before);
        let (status, read) = project(handle, browser, "read", body.clone()).await;
        assert_eq!(status, 200);
        assert_eq!(&read, expected);
    };
    let (status, _) = Box::pin(crate::listener::tests::body::exchange(
        handle.clone(),
        request(),
        Some(concurrent),
    ))
    .await;
    assert_eq!(status, hyper::StatusCode::REQUEST_TIMEOUT);
    evidence::settled(handle).await;
    assert!(handle.live.is_live());
    assert_eq!(
        handle.live.probe.snapshot().executions,
        before.executions + 1
    );
}
