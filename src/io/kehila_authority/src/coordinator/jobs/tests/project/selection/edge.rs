//! Original bytes traverse the real HTTP/TLS adapter, not a Value conversion.
use super::*;

async fn raw(handle: &Handle, browser: &Browser, path: &str, bytes: &[u8], expected: u16) -> Value {
    let request = wire_request(Some(browser), path, Some(json!({})));
    let (mut parts, _) = request.into_parts();
    parts.headers.insert(
        hyper::header::CONTENT_LENGTH,
        bytes.len().to_string().parse().unwrap(),
    );
    let request =
        hyper::Request::from_parts(parts, Full::new(bytes::Bytes::copy_from_slice(bytes)));
    let (status, _, response) = Box::pin(crate::listener::tests::browser_exchange(
        handle.clone(),
        request,
    ))
    .await;
    assert_eq!(
        status.as_u16(),
        expected,
        "selection original-byte response for {path}"
    );
    serde_json::from_slice(&response).unwrap()
}

pub(super) async fn exercise(handle: &Handle, browser: &Browser, admin: &Admin) {
    evidence::settled(handle).await;
    let durable = evidence::snapshot(admin).await;
    let before_admission = handle.live.probe.snapshot();
    let valid = br#"{"operation_token":"99999999999999999999999999999999","expected_version":"0","requested":null}"#;
    // This exact-byte upper-bound control performs a real authenticated lookup,
    // then confirms absence created no operation/event/head or unrelated state.
    let mut at_limit = valid.to_vec();
    at_limit.resize(selection_codec::MAX_WIRE_BYTES, b' ');
    assert_eq!(
        raw(handle, browser, "/selection/reconcile", &at_limit, 200).await,
        json!({"codec_version":1,"outcome":"absent"})
    );
    evidence::settled(handle).await;
    assert_eq!(evidence::snapshot(admin).await, durable);
    let admissions = handle.live.probe.snapshot();
    assert!(
        admissions.executions > before_admission.executions,
        "positive selection lookup must exercise the application admission counter"
    );
    assert!(
        admissions.database_ownership_entries > before_admission.database_ownership_entries,
        "positive selection lookup must exercise the database admission counter"
    );

    for path in ["/selection/set", "/selection/reconcile"] {
        let mut malformed = vec![
            br#"{"operation_token":"99999999999999999999999999999999","expected_version":"0","expected_versio\u006e":"1","requested":null}"#.to_vec(),
            br#"{"operation_token":"99999999999999999999999999999999","expected_version":0,"requested":null}"#.to_vec(),
            br#"{"operation_token":"99999999999999999999999999999999","expected_version":"18446744073709551616","requested":null}"#.to_vec(),
            br#"{"operation_token":"99999999999999999999999999999999","expected_version":"0","requested":{"project_id":"p"}}"#.to_vec(),
            br#"{"operation_token":"99999999999999999999999999999999","expected_version":"0","requested":{"project_id":"p","item_id":"a","granted":true}}"#.to_vec(),
        ];
        for field in [
            "actor_id",
            "user_id",
            "installation_id",
            "grant",
            "authorized",
        ] {
            let mut body: Value = serde_json::from_slice(valid).unwrap();
            body[field] = json!("owner");
            malformed.push(serde_json::to_vec(&body).unwrap());
        }
        for body in malformed {
            raw(handle, browser, path, &body, 400).await;
            evidence::settled(handle).await;
            assert_eq!(
                handle.live.probe.snapshot(),
                admissions,
                "rejected original bytes admitted preparation/worker/database work: {path}"
            );
            assert_eq!(evidence::snapshot(admin).await, durable);
            assert!(handle.live.is_live());
        }
    }
    for body in [
        b"[]".as_slice(),
        br#"{"user_id":"owner"}"#,
        br#"{"requested":null,"requested":null}"#,
    ] {
        raw(handle, browser, "/selection/read", body, 400).await;
        evidence::settled(handle).await;
        assert_eq!(handle.live.probe.snapshot(), admissions);
    }
    at_limit.push(b' ');
    raw(handle, browser, "/selection/set", &at_limit, 413).await;
    raw(handle, browser, "/selection/unknown", b"{}", 404).await;
    evidence::settled(handle).await;
    assert_eq!(handle.live.probe.snapshot(), admissions);
    assert_eq!(evidence::snapshot(admin).await, durable);
    assert!(handle.live.is_live());
}
