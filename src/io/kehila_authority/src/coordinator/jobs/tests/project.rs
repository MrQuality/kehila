//! Real PostgreSQL plus listener/HTTP/TLS journey; not a browser/startup test.
use super::*;
use http_body_util::Full;
use serde_json::{json, Value};
mod admission;
mod authorization;
mod clock;
mod competing;
mod evidence;
mod faults;
mod restored;
mod write_faults;

#[derive(Clone)]
struct Browser {
    cookie: String,
    csrf: String,
}
fn wire_request(
    browser: Option<&Browser>,
    path: &str,
    body: Option<Value>,
) -> hyper::Request<Full<bytes::Bytes>> {
    let bytes = body
        .as_ref()
        .map(|value| serde_json::to_vec(value).unwrap())
        .unwrap_or_default();
    let mut request = hyper::Request::builder()
        .method(if body.is_some() { "POST" } else { "GET" })
        .uri(path)
        .header("host", "127.0.0.1:16281")
        .header("origin", "https://127.0.0.1:16281")
        .header("x-kehila-client", "browser")
        .header("sec-fetch-site", "same-origin")
        .header("content-length", bytes.len());
    if body.is_some() {
        request = request.header("content-type", "application/json");
    }
    if let Some(browser) = browser {
        let mut cookie = hyper::header::HeaderValue::from_bytes(browser.cookie.as_bytes()).unwrap();
        cookie.set_sensitive(true);
        request = request.header("cookie", cookie);
        if body.is_some() {
            request = request.header("x-kehila-csrf", &browser.csrf);
        }
    }
    request.body(Full::new(bytes::Bytes::from(bytes))).unwrap()
}
async fn call(
    handle: &Handle,
    browser: Option<&Browser>,
    path: &str,
    body: Option<Value>,
) -> (u16, Value, Option<String>) {
    // Keep the nested HTTP/TLS exchange out of every caller's inline async
    // state. Awaiting this owned future still retains connection cleanup.
    let (status, headers, bytes) = Box::pin(crate::listener::tests::browser_exchange(
        handle.clone(),
        wire_request(browser, path, body),
    ))
    .await;
    let status = status.as_u16();
    let cookie = headers
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|value| {
            value.starts_with("__Host-kehila-session=")
                || value.starts_with("__Host-kehila-pre-session=")
        })
        .map(|value| value.split(';').next().unwrap().to_owned());
    let value = serde_json::from_slice(&bytes).unwrap();
    (status, value, cookie)
}
async fn login(handle: &Handle) -> Browser {
    let (status, value, cookie) = call(handle, None, "/auth/browser/pre-session", None).await;
    assert_eq!(status, 200);
    let pre = Browser {
        cookie: cookie.unwrap(),
        csrf: value["csrf"].as_str().unwrap().to_owned(),
    };
    let (status, value, cookie) = call(
        handle,
        Some(&pre),
        "/auth/browser/login",
        Some(json!({"password":"a fixed test password"})),
    )
    .await;
    assert_eq!(status, 200);
    Browser {
        cookie: cookie.unwrap(),
        csrf: value["csrf"].as_str().unwrap().to_owned(),
    }
}
async fn project(handle: &Handle, browser: &Browser, path: &str, body: Value) -> (u16, Value) {
    let (status, value, _) = call(
        handle,
        Some(browser),
        &format!("/projects/{path}"),
        Some(body),
    )
    .await;
    (status, value)
}
fn execute(entry: &Value) -> Value {
    json!({"request_id":entry["request_id"],"token":entry["token"],"name":entry["form"]["name"],
        "prefix":entry["form"]["prefix"],"estimate_unit":entry["form"]["estimate_unit"]})
}
async fn runtime(
    endpoint: &crate::postgres::Endpoint,
    installation: &str,
    attempt: String,
) -> (Handle, Owner) {
    let database = Database::start(
        endpoint,
        installation.to_owned(),
        attempt,
        Instant::now() + Duration::from_secs(30),
    )
    .await
    .unwrap();
    Owner::new(database, Blocklist::synthetic_for_tests())
        .await
        .unwrap()
}

#[test]
#[ignore = "requires a fresh explicitly provisioned product PostgreSQL TLS fixture"]
fn native_project_creation_and_reconciliation() {
    let (endpoint, installation, attempt, controls) = fixture();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let (handle, owner) =
                runtime(&endpoint, &installation, format!("{attempt}-project")).await;
            let engine = Arc::downgrade(owner.engine());
            let running = tokio::spawn(owner.run());
            let first = login(&handle).await;
            let second = login(&handle).await;
            assert!(
                first.cookie != second.cookie,
                "separate logins reused a browser credential"
            );
            let (denied, _, _) = call(
                &handle,
                None,
                "/projects/register",
                Some(json!({"name":"denied","prefix":"DEN","estimate_unit":"hours"})),
            )
            .await;
            assert!(denied >= 400);
            let form = json!({"name":"  Native project  ","prefix":"NAT","estimate_unit":"points"});
            let (status, registered) = project(&handle, &first, "register", form.clone()).await;
            assert_eq!(status, 201);
            let entry = &registered["entry"];
            assert_eq!(entry["form"], form);
            let id = entry["request_id"].clone();
            assert!(id.is_string());
            let (status, page) = project(&handle, &second, "list", json!({"before":null})).await;
            assert_eq!(status, 200);
            assert!(page["entries"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value["request_id"] == id));
            let (status, opened) =
                project(&handle, &second, "open", json!({"request_id":id})).await;
            assert_eq!(status, 200);
            assert_eq!(&opened["entry"], entry);
            authorization::owner_binding(&endpoint).await;
            admission::ingress(&handle, &first, entry).await;
            admission::receiver_drop_and_capacity(&handle, &first, entry).await;
            let mut changed = execute(entry);
            changed["name"] = json!("different");
            assert_eq!(project(&handle, &first, "execute", changed).await.0, 409);
            write_faults::exercise(&handle, &first, entry, &endpoint, &controls).await;
            let created = competing::execution_wins(&handle, &first, &second, entry).await;
            let result = project_create_codec::decode_result(
                1,
                &serde_json::to_vec(&created["result"]).unwrap(),
            )
            .unwrap();
            assert_eq!(result.project.name, "  Native project  ");
            let project_id = created["entry"]["project_id"].clone();
            let (status, replayed) = project(&handle, &first, "execute", execute(entry)).await;
            assert_eq!(status, 200);
            assert_eq!(replayed["result"], created["result"]);
            assert_eq!(replayed["entry"]["project_id"], project_id);
            let (status, read) =
                project(&handle, &first, "read", json!({"project_id":project_id})).await;
            assert_eq!(status, 200);
            assert_eq!(
                project_create_codec::decode_project_snapshot(
                    1,
                    &serde_json::to_vec(&read["project"]).unwrap()
                )
                .unwrap(),
                result.project
            );
            assert_eq!(
                project_create_codec::decode_snapshot(
                    1,
                    &serde_json::to_vec(&read["configuration"]).unwrap()
                )
                .unwrap(),
                result.configuration
            );
            authorization::no_credential_and_crossed_csrf(
                &handle,
                &first,
                &second,
                entry,
                &project_id,
                &endpoint,
            )
            .await;
            let (status, cancelled) =
                project(&handle, &first, "cancel", json!({"request_id":id})).await;
            assert_eq!(status, 200);
            assert_eq!(cancelled["entry"]["state"], "completed");
            competing::collision_reentry(&handle, &first, &project_id).await;
            let (status, next) = project(&handle, &first, "register", form.clone()).await;
            assert_eq!(status, 201);
            let next = &next["entry"];
            assert_ne!(next["request_id"], id);
            let cancelled = competing::cancellation_wins(&handle, &second, next).await;
            assert_eq!(cancelled["entry"]["state"], "cancelled");
            assert_eq!(
                project(&handle, &first, "execute", execute(next)).await.0,
                410
            );
            authorization::revoked(&handle, &first, &second, entry, &project_id, &endpoint).await;
            let (status, lost) = project(&handle, &first, "register", form).await;
            assert_eq!(status, 201);
            let lost = &lost["entry"];
            let lost_id = lost["request_id"].clone();
            assert_ne!(lost_id, id);
            assert_ne!(lost_id, next["request_id"]);
            engine
                .upgrade()
                .unwrap()
                .database
                .lock()
                .await
                .lose_next_confirmation_for_test();
            let (status, unknown) = project(&handle, &first, "execute", execute(lost)).await;
            assert_eq!(status, 503);
            assert_eq!(unknown["error"], "outcome_unknown");
            running.await.unwrap().unwrap();
            let (handle, owner) =
                runtime(&endpoint, &installation, format!("{attempt}-reconcile")).await;
            let engine = Arc::downgrade(owner.engine());
            let running = tokio::spawn(owner.run());
            let browser = login(&handle).await;
            let (status, reconciled) = project(
                &handle,
                &browser,
                "reconcile",
                json!({"request_id":lost_id}),
            )
            .await;
            assert_eq!(status, 200);
            assert_eq!(reconciled["outcome"], "replayed");
            let recovered = project_create_codec::decode_result(
                1,
                &serde_json::to_vec(&reconciled["result"]).unwrap(),
            )
            .unwrap();
            assert_eq!(recovered.project.name, "  Native project  ");
            assert_eq!(recovered.project.prefix, "NAT");
            assert_eq!(
                recovered.project.id.0,
                reconciled["entry"]["project_id"].as_str().unwrap()
            );
            let (status, repeated) = project(
                &handle,
                &browser,
                "reconcile",
                json!({"request_id":lost_id}),
            )
            .await;
            assert_eq!(status, 200);
            assert_eq!(repeated, reconciled);
            let (status, _) = project(
                &handle,
                &browser,
                "read",
                json!({"project_id":reconciled["entry"]["project_id"]}),
            )
            .await;
            assert_eq!(status, 200);
            // A nonsecret uncertainty reply still needs ordinary HTTP/TLS cleanup.
            // Its delivery cannot convert a lost audit confirmation into success.
            eprintln!("TLS_AUDIT_UNCERTAINTY_BEGIN");
            engine
                .upgrade()
                .unwrap()
                .database
                .lock()
                .await
                .lose_next_confirmation_for_test();
            let (status, audit, _) = call(
                &handle,
                Some(&browser),
                "/auth/browser/logout",
                Some(json!({})),
            )
            .await;
            assert_eq!(status, 503);
            assert_eq!(audit["signed_out"], true);
            assert_eq!(audit["audit_confirmed"], false);
            running.await.unwrap().unwrap();
            let (handle, owner) =
                runtime(&endpoint, &installation, format!("{attempt}-revoke")).await;
            let engine = Arc::downgrade(owner.engine());
            let running = tokio::spawn(owner.run());
            let browser = login(&handle).await;
            let _other = login(&handle).await;
            engine
                .upgrade()
                .unwrap()
                .database
                .lock()
                .await
                .lose_next_confirmation_for_test();
            let (status, audit, _) = call(
                &handle,
                Some(&browser),
                "/auth/browser/revoke-others",
                Some(json!({"password":"a fixed test password"})),
            )
            .await;
            assert_eq!(status, 503);
            assert_eq!(audit["other_sessions_revoked"], true);
            assert_eq!(audit["audit_confirmed"], false);
            running.await.unwrap().unwrap();
            admission::stop_and_abort(&endpoint, &installation, &attempt, entry).await;
            competing::lost_registration_and_cancellation(&endpoint, &installation, &attempt).await;
            faults::execution_panic(&endpoint, &installation, &attempt).await;
            faults::named_constraint(&endpoint, &installation, &attempt).await;
            mark(&controls, "passed");
        });
}
