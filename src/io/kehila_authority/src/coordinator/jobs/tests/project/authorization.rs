use super::*;
use crate::postgres::tests::fixture_admin::Admin;

fn routes(entry: &Value, project_id: &Value) -> Vec<(&'static str, Value)> {
    vec![
        ("binding", json!({})),
        (
            "register",
            json!({"name":"Denied registration","prefix":"DR","estimate_unit":"hours"}),
        ),
        ("list", json!({"before":null})),
        ("open", json!({"request_id":entry["request_id"]})),
        ("execute", execute(entry)),
        ("reconcile", json!({"request_id":entry["request_id"]})),
        ("cancel", json!({"request_id":entry["request_id"]})),
        ("read", json!({"project_id":project_id})),
    ]
}

fn no_disclosure(value: &Value) {
    let object = value.as_object().unwrap();
    assert_eq!(object.len(), 1);
    assert!(matches!(
        object.get("error").and_then(Value::as_str),
        Some("request rejected" | "forbidden")
    ));
}

pub(super) async fn denied(
    handle: &Handle,
    admin: &Admin,
    browser: Option<&Browser>,
    entry: &Value,
    project_id: &Value,
    statuses: &[u16],
    creation_only: bool,
) {
    let before = evidence::snapshot(admin).await;
    for (route, body) in routes(entry, project_id) {
        if creation_only && matches!(route, "binding" | "read") {
            continue;
        }
        let (status, value) = if let Some(browser) = browser {
            project(handle, browser, route, body).await
        } else {
            // Actor/installation headers are untrusted assertions, not identity.
            let mut request = wire_request(None, &format!("/projects/{route}"), Some(body));
            request
                .headers_mut()
                .insert("x-actor-id", "owner".parse().unwrap());
            request
                .headers_mut()
                .insert("x-installation-id", "installation".parse().unwrap());
            let (status, _, body) =
                crate::listener::tests::browser_exchange(handle.clone(), request).await;
            (status.as_u16(), serde_json::from_slice(&body).unwrap())
        };
        assert!(
            statuses.contains(&status),
            "unexpected {route} denial status {status}"
        );
        no_disclosure(&value);
    }
    evidence::settled(handle).await;
    assert_eq!(evidence::snapshot(admin).await, before);
    evidence::fences_released(admin).await;
}

pub(super) async fn no_credential_and_crossed_csrf(
    handle: &Handle,
    first: &Browser,
    second: &Browser,
    entry: &Value,
    project_id: &Value,
    endpoint: &crate::postgres::Endpoint,
) {
    let admin = Admin::connect(endpoint).await;
    denied(
        handle,
        &admin,
        None,
        entry,
        project_id,
        &[400, 401, 403],
        false,
    )
    .await;
    let crossed = Browser {
        cookie: first.cookie.clone(),
        csrf: second.csrf.clone(),
    };
    denied(
        handle,
        &admin,
        Some(&crossed),
        entry,
        project_id,
        &[403],
        false,
    )
    .await;
    assert_eq!(
        project(handle, first, "read", json!({"project_id":project_id}))
            .await
            .0,
        200
    );
    admin.close().await;
}

pub(super) async fn revoked(
    handle: &Handle,
    first: &Browser,
    second: &Browser,
    entry: &Value,
    project_id: &Value,
    endpoint: &crate::postgres::Endpoint,
) {
    let (status, result, _) = call(
        handle,
        Some(first),
        "/auth/browser/revoke-others",
        Some(json!({"password":"a fixed test password"})),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(result["other_sessions_revoked"], true);
    let admin = Admin::connect(endpoint).await;
    denied(
        handle,
        &admin,
        Some(second),
        entry,
        project_id,
        &[401],
        false,
    )
    .await;
    assert_eq!(
        project(handle, first, "read", json!({"project_id":project_id}))
            .await
            .0,
        200
    );
    admin.close().await;
}

pub(super) async fn owner_binding(endpoint: &crate::postgres::Endpoint) {
    let admin = Admin::connect(endpoint).await;
    let before = evidence::snapshot(&admin).await;
    admin.client().batch_execute("SET ROLE kehila_owner; INSERT INTO kehila.actors VALUES ('unbound-other',true,true); RESET ROLE").await.unwrap();
    let authority = admin.client().query_one("SELECT i.runtime_generation::text,c.auth_generation::text,c.verifier_revision::text FROM kehila_auth.installation i JOIN kehila_auth.owner_credentials c USING(owner_actor_id)", &[]).await.unwrap();
    let generations: Vec<u64> = (0..3)
        .map(|i| authority.get::<_, String>(i).parse().unwrap())
        .collect();
    admin
        .client()
        .batch_execute("SET ROLE kehila_auth_runtime")
        .await
        .unwrap();
    let denied = admin.client().batch_execute(&format!("SELECT * FROM kehila_auth.register_project(ROW('installation','unbound-other',{},{},{})::kehila_auth.authority,repeat('e',32)::kehila_auth.intent_key,repeat('f',32)::kehila_auth.intent_key,'Other actor','OT','hours')", generations[0],generations[1],generations[2])).await.unwrap_err();
    admin.client().batch_execute("RESET ROLE").await.unwrap();
    assert_eq!(denied.code().unwrap().code(), "42501");
    // Even an owner-authored fixture cannot create a pending row attributed
    // to an actor outside the installed owner pair.
    admin
        .client()
        .batch_execute("SET ROLE kehila_owner")
        .await
        .unwrap();
    let denied = admin.client().batch_execute("INSERT INTO kehila_auth.pending_projects(installation_id,actor_id,intent_handle,token,name,prefix,estimate_unit,registered_ms,expires_ms) SELECT 'installation','unbound-other',repeat('e',32)::kehila_auth.intent_key,repeat('f',32)::kehila_auth.intent_key,'Other actor','OT','hours',at_ms,(at_ms+86400000)::kehila.u64 FROM (SELECT kehila_auth.trusted_millis() AS at_ms) sampled").await.unwrap_err();
    admin.client().batch_execute("RESET ROLE").await.unwrap();
    assert_eq!(denied.code().unwrap().code(), "23503");
    assert_eq!(
        denied.as_db_error().unwrap().constraint(),
        Some("pending_owner")
    );
    admin.client().batch_execute("SET ROLE kehila_owner; DELETE FROM kehila.actors WHERE actor_id='unbound-other'; RESET ROLE").await.unwrap();
    assert_eq!(evidence::snapshot(&admin).await, before);
    evidence::fences_released(&admin).await;
    admin.close().await;
}
