//! Real pending-only product routes; completed item input is separately seeded.
use super::*;
use crate::{lifecycle::probe::Point, postgres::tests::fixture_admin::Admin};
use task_contract::project_create::ProjectCreateResult;
mod faults;
mod limits;

struct Context<'a> {
    endpoint: &'a crate::postgres::Endpoint,
    installation: &'a str,
    attempt: &'a str,
    admin: &'a Admin,
    project: ProjectCreateResult,
    other: ProjectCreateResult,
}
struct Serving {
    handle: Handle,
    running: tokio::task::JoinHandle<Result<(), Error>>,
    browser: Browser,
}
impl Context<'_> {
    async fn start(&self, suffix: &str) -> Serving {
        let (handle, owner) = runtime(
            self.endpoint,
            self.installation,
            format!("{}-{suffix}", self.attempt),
        )
        .await;
        let running = tokio::spawn(owner.run());
        let browser = login(&handle).await;
        Serving {
            handle,
            running,
            browser,
        }
    }
}
impl Serving {
    async fn stop(self, admin: &Admin, failed: bool) {
        self.handle.stop();
        assert_eq!(self.running.await.unwrap().is_err(), failed);
        evidence::runtime_released(admin).await;
    }
}
fn form(project: &ProjectCreateResult) -> Value {
    let kind = &project.configuration.types[0];
    json!({"project_id":project.project.id.0,"type_id":kind.id.0,"workflow_id":null,
        "expected_configuration_revision":project.configuration.revision.to_string(),"estimate":"0",
        "values":[{"id":kind.title_field_id.as_ref().unwrap().0,"value":"  exact pending title  "}]})
}
fn locator(entry: &Value) -> Value {
    json!({"project_id":entry["project_id"],"intent_handle":entry["intent_handle"]})
}
async fn pending(handle: &Handle, browser: &Browser, action: &str, body: Value) -> (u16, Value) {
    let (status, value, _) = call(
        handle,
        Some(browser),
        &format!("/items/pending/{action}"),
        Some(body),
    )
    .await;
    (status, value)
}
async fn register(serving: &Serving, body: Value) -> Value {
    let (status, mut value) = pending(&serving.handle, &serving.browser, "register", body).await;
    assert_eq!(status, 201);
    assert_eq!(value["entry"]["state"], "prepared");
    assert_eq!(value["entry"].as_object().unwrap().len(), 8);
    value.get_mut("entry").unwrap().take()
}
async fn list(serving: &Serving, project: &str, before: Value) -> Value {
    let (status, value) = pending(
        &serving.handle,
        &serving.browser,
        "list",
        json!({"project_id":project,"before":before}),
    )
    .await;
    assert_eq!(status, 200);
    value
}
async fn cancel(serving: &Serving, entry: &Value) -> Value {
    let (status, value) =
        pending(&serving.handle, &serving.browser, "cancel", locator(entry)).await;
    assert_eq!(status, 200);
    value["entry"].clone()
}
async fn create_project(serving: &Serving, name: &str, prefix: &str) -> ProjectCreateResult {
    let (status, registered) = project(
        &serving.handle,
        &serving.browser,
        "register",
        json!({"name":name,"prefix":prefix,"estimate_unit":"hours"}),
    )
    .await;
    assert_eq!(status, 201);
    let (status, created) = project(
        &serving.handle,
        &serving.browser,
        "execute",
        execute(&registered["entry"]),
    )
    .await;
    assert_eq!(status, 201);
    project_create_codec::decode_result(1, &serde_json::to_vec(&created["result"]).unwrap())
        .unwrap()
}
async fn unchanged(ctx: &Context<'_>, before: &[(String, String)]) {
    assert!(
        evidence::pending_domain_snapshot(ctx.admin)
            .await
            .as_slice()
            == before,
        "pending-only route changed domain operations, items, allocation or history"
    );
}

async fn basic(ctx: &Context<'_>, serving: &Serving) {
    let before = evidence::pending_domain_snapshot(ctx.admin).await;
    let first = register(serving, form(&ctx.project)).await;
    let second = register(serving, form(&ctx.project)).await;
    for field in ["intent_handle", "token", "target_item_id"] {
        assert_ne!(first[field], second[field]);
    }
    let other_session = login(&serving.handle).await;
    let (status, opened) = pending(&serving.handle, &other_session, "open", locator(&first)).await;
    assert_eq!(status, 200);
    assert!(opened.get("result").is_none());
    let stored=ctx.admin.client().query_one("SELECT request_bytes,request_sha256 FROM kehila_auth.pending_items WHERE intent_handle=$1::text::kehila_auth.intent_key AND project_id=$2::text::kehila.ident",
        &[&first["intent_handle"].as_str().unwrap(),&ctx.project.project.id.0]).await.unwrap();
    let bytes: Vec<u8> = stored.get(0);
    let digest: [u8; 32] = stored.get::<_, Vec<u8>>(1).try_into().unwrap();
    drop(stored);
    let request = work_item_codec::verify_request(1, &bytes, &digest).unwrap();
    assert!(
        work_item_codec::decode_request(1, &serde_json::to_vec(&opened["request"]).unwrap())
            .unwrap()
            == request,
        "cross-session Open changed the frozen command"
    );
    let work_item_codec::Request::Create {
        project_id,
        command,
    } = request
    else {
        panic!()
    };
    assert_eq!(project_id.0, ctx.project.project.id.0);
    assert_eq!(command.item_id.0, first["target_item_id"].as_str().unwrap());
    assert_eq!(command.operation_id, first["token"].as_str().unwrap());
    assert_eq!(command.estimate.unwrap().thousandths(), 0);
    assert_eq!(
        command.values[0].value,
        task_contract::field::FieldValue::Text("  exact pending title  ".into())
    );
    let page = list(serving, &ctx.project.project.id.0, Value::Null).await;
    assert_eq!(page["entries"].as_array().unwrap().len(), 2);
    assert!(page["entries"]
        .as_array()
        .unwrap()
        .iter()
        .all(|entry| entry.get("request").is_none()));
    assert!(page["before"].is_null());
    let mut crossed = locator(&first);
    crossed["project_id"] = json!(ctx.other.project.id.0);
    assert_eq!(
        pending(&serving.handle, &other_session, "open", crossed).await,
        (404, json!({"error":"not_found"}))
    );
    assert_eq!(
        pending(
            &serving.handle,
            &other_session,
            "list",
            json!({"project_id":"missing-scope","before":null})
        )
        .await
        .0,
        403
    );
    let anonymous = call(
        &serving.handle,
        None,
        "/items/pending/list",
        Some(json!({"project_id":ctx.project.project.id.0,"before":null})),
    )
    .await;
    assert_eq!(anonymous.0, 401);
    ctx.admin
        .client()
        .batch_execute("UPDATE kehila.actors SET can_create_project=false WHERE actor_id='owner'")
        .await
        .unwrap();
    let item_permission = register(serving, form(&ctx.project)).await;
    assert_eq!(
        cancel(serving, &item_permission).await["state"],
        "cancelled"
    );
    ctx.admin
        .client()
        .batch_execute("UPDATE kehila.actors SET can_create_project=true WHERE actor_id='owner'")
        .await
        .unwrap();
    ctx.admin
        .client()
        .execute(
            "UPDATE kehila.project_grants SET active=false WHERE project_id=$1::text::kehila.ident",
            &[&ctx.project.project.id.0],
        )
        .await
        .unwrap();
    for (action, body) in [
        ("register", form(&ctx.project)),
        (
            "list",
            json!({"project_id":ctx.project.project.id.0,"before":null}),
        ),
        ("open", locator(&first)),
        ("cancel", locator(&first)),
    ] {
        assert_eq!(
            pending(&serving.handle, &serving.browser, action, body)
                .await
                .0,
            403
        );
    }
    ctx.admin
        .client()
        .execute(
            "UPDATE kehila.project_grants SET active=true WHERE project_id=$1::text::kehila.ident",
            &[&ctx.project.project.id.0],
        )
        .await
        .unwrap();
    let (status, _, _) = call(
        &serving.handle,
        Some(&serving.browser),
        "/auth/browser/revoke-others",
        Some(json!({"password":"a fixed test password"})),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(
        pending(&serving.handle, &other_session, "open", locator(&first))
            .await
            .0,
        401
    );
    assert_eq!(cancel(serving, &first).await["state"], "cancelled");
    assert_eq!(cancel(serving, &second).await["state"], "cancelled");
    unchanged(ctx, &before).await;
}

async fn ingress(ctx: &Context<'_>, serving: &Serving) {
    let domain = evidence::pending_domain_snapshot(ctx.admin).await;
    let before = serving.handle.live.probe.snapshot();
    let mut malformed = form(&ctx.project);
    malformed["actor_id"] = json!("spoof");
    assert_eq!(
        pending(&serving.handle, &serving.browser, "register", malformed)
            .await
            .0,
        400
    );
    assert_eq!(serving.handle.live.probe.snapshot(), before);
    let mut deep = Value::Null;
    for _ in 0..project_create_codec::MAX_DEPTH + 2 {
        deep = Value::Array(vec![deep]);
    }
    let mut malformed = form(&ctx.project);
    malformed["values"][0]["value"] = deep;
    assert_eq!(
        pending(&serving.handle, &serving.browser, "register", malformed)
            .await
            .0,
        400
    );
    assert_eq!(serving.handle.live.probe.snapshot(), before);
    let mut malformed = form(&ctx.project);
    malformed["values"][0]["value"] = json!("decoded\0NUL");
    assert_eq!(
        pending(&serving.handle, &serving.browser, "register", malformed)
            .await
            .0,
        400
    );
    assert_eq!(serving.handle.live.probe.snapshot(), before);
    let body = format!(
        "{{\"project_id\":\"{}\",\"project_id\":\"other\",\"before\":null}}",
        ctx.project.project.id.0
    );
    let mut request = wire_request(
        Some(&serving.browser),
        "/items/pending/list",
        Some(json!({})),
    );
    request
        .headers_mut()
        .insert("content-length", body.len().to_string().parse().unwrap());
    *request.body_mut() = Full::new(bytes::Bytes::from(body));
    let (status, _, _) = Box::pin(crate::listener::tests::browser_exchange(
        serving.handle.clone(),
        request,
    ))
    .await;
    assert_eq!(status.as_u16(), 400);
    assert_eq!(serving.handle.live.probe.snapshot(), before);
    let mut large = form(&ctx.project);
    large["values"]=Value::Array((0..16).map(|i|json!({"id":format!("field-{i}"),"value":"x".repeat(project_create_codec::MAX_STRING_BYTES)})).collect());
    let excess =
        serde_json::to_vec(&large).unwrap().len() - (project_create_codec::MAX_RECORD_BYTES - 100);
    large["values"][15]["value"] =
        json!("x".repeat(project_create_codec::MAX_STRING_BYTES - excess));
    assert!(crate::item_pending::ingress::decode(
        crate::item_pending::ingress::Route::Register,
        &serde_json::to_vec(&large).unwrap()
    )
    .is_ok());
    assert_eq!(
        pending(&serving.handle, &serving.browser, "register", large)
            .await
            .0,
        400
    );
    let after = serving.handle.live.probe.snapshot();
    assert_eq!(after.preparations, before.preparations + 1);
    assert_eq!(after.native_mutex_attempts, before.native_mutex_attempts);
    assert_eq!(
        after.database_ownership_entries,
        before.database_ownership_entries
    );
    let mut large = form(&ctx.project);
    large["values"]=Value::Array((0..15).map(|i|json!({"id":format!("field-{i}"),"value":"x".repeat(project_create_codec::MAX_STRING_BYTES-1024)})).collect());
    let entry = register(serving, large).await;
    let (status, opened) =
        pending(&serving.handle, &serving.browser, "open", locator(&entry)).await;
    assert_eq!(status, 200);
    assert!(serde_json::to_vec(&opened).unwrap().len() > 900_000);
    assert!(opened.get("result").is_none());
    assert_eq!(cancel(serving, &entry).await["state"], "cancelled");
    unchanged(ctx, &domain).await;
}

#[test]
#[ignore = "requires a fresh explicitly provisioned product PostgreSQL TLS fixture"]
fn native_pending_creation_lifecycle() {
    let (endpoint, installation, attempt, controls) = fixture();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let admin = Admin::connect(&endpoint).await;
            let (handle, owner) =
                runtime(&endpoint, &installation, format!("{attempt}-initial")).await;
            let running = tokio::spawn(owner.run());
            let browser = login(&handle).await;
            let serving = Serving {
                handle,
                running,
                browser,
            };
            let project = create_project(&serving, "Pending lifecycle", "PC").await;
            let other = create_project(&serving, "Other pending scope", "PO").await;
            let ctx = Context {
                endpoint: &endpoint,
                installation: &installation,
                attempt: &attempt,
                admin: &admin,
                project,
                other,
            };
            basic(&ctx, &serving).await;
            ingress(&ctx, &serving).await;
            let serving = faults::exercise(&ctx, serving).await;
            let serving = limits::exercise(&ctx, serving).await;
            serving.stop(&admin, false).await;
            faults::profiles(&ctx).await;
            let serving = ctx.start("final-positive").await;
            let (status, opened) = pending(
                &serving.handle,
                &serving.browser,
                "open",
                json!({"project_id":ctx.project.project.id.0,"intent_handle":format!("{:032x}",1)}),
            )
            .await;
            assert_eq!(status, 200);
            assert_eq!(opened["entry"]["state"], "completed");
            assert!(opened["request"].is_null() && opened.get("result").is_none());
            let (status, _, _) = call(
                &serving.handle,
                Some(&serving.browser),
                "/items/read",
                Some(json!({"project_id":ctx.project.project.id.0,"item_id":"read-fixture-1"})),
            )
            .await;
            assert_eq!(status, 200);
            serving.stop(&admin, false).await;
            admin.close().await;
            mark(&controls, "item-pending-passed");
        });
}
