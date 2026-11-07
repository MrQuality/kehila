//! Controlled clock and owner-seeded containment inputs, separate from product writes.
use super::*;
use task_contract::{item_mutation::CreateCommand, work_item::WorkItemId};
const CLOCK_INSTALL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/integration/native_clock_install.sql"
));
const CLOCK_RESTORE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/integration/native_clock_restore.sql"
));

async fn clock(ctx: &Context<'_>, value: u64) {
    ctx.admin
        .client()
        .execute(
            "UPDATE kehila_native_fixture.clock SET value=$1::text::numeric WHERE singleton",
            &[&value.to_string()],
        )
        .await
        .unwrap();
}
async fn clear_terminal(ctx: &Context<'_>) {
    assert_eq!(
        ctx.admin
            .client()
            .query_one(
                "SELECT count(*) FROM kehila_auth.pending_items WHERE state='prepared'",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    ctx.admin.client().batch_execute("SET ROLE kehila_owner; DELETE FROM kehila_auth.pending_items WHERE state IN ('cancelled','expired'); RESET ROLE").await.unwrap();
}
async fn quotas(ctx: &Context<'_>, serving: &Serving) {
    let before = evidence::pending_domain_snapshot(ctx.admin).await;
    ctx.admin
        .client()
        .batch_execute("UPDATE kehila_auth.item_pending_policy SET lifetime_seconds=60")
        .await
        .unwrap();
    let other = register(serving, form(&ctx.other)).await;
    ctx.admin
        .client()
        .batch_execute("UPDATE kehila_auth.item_pending_policy SET lifetime_seconds=86400")
        .await
        .unwrap();
    let mut entries = Vec::new();
    for _ in 0..31 {
        entries.push(register(serving, form(&ctx.project)).await);
    }
    assert_eq!(
        pending(
            &serving.handle,
            &serving.browser,
            "register",
            form(&ctx.project)
        )
        .await
        .0,
        429
    );
    let first = list(serving, &ctx.project.project.id.0, Value::Null).await;
    assert_eq!(first["entries"].as_array().unwrap().len(), 16);
    assert!(first["before"].is_string());
    let second = list(serving, &ctx.project.project.id.0, first["before"].clone()).await;
    assert_eq!(second["entries"].as_array().unwrap().len(), 15);
    assert!(second["before"].is_null());
    let actual = first["entries"]
        .as_array()
        .unwrap()
        .iter()
        .chain(second["entries"].as_array().unwrap())
        .map(|entry| entry["intent_handle"].as_str().unwrap())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(actual.len(), 31);
    assert!(entries
        .iter()
        .all(|entry| actual.contains(entry["intent_handle"].as_str().unwrap())));
    ctx.admin
        .client()
        .batch_execute("UPDATE kehila_auth.item_pending_policy SET page_limit=8")
        .await
        .unwrap();
    let smaller = list(serving, &ctx.project.project.id.0, Value::Null).await;
    assert_eq!(smaller["entries"].as_array().unwrap().len(), 8);
    assert!(smaller["before"].is_string());
    ctx.admin
        .client()
        .batch_execute("UPDATE kehila_auth.item_pending_policy SET page_limit=16")
        .await
        .unwrap();
    ctx.admin
        .client()
        .batch_execute(CLOCK_INSTALL)
        .await
        .unwrap();
    clock(ctx, other["expires_ms"].as_str().unwrap().parse().unwrap()).await;
    let (status, expired) =
        pending(&serving.handle, &serving.browser, "open", locator(&other)).await;
    assert_eq!(status, 200);
    assert_eq!(expired["entry"]["state"], "expired");
    assert!(expired["request"].is_null());
    assert_eq!(ctx.admin.client().query_one("SELECT count(*) FROM kehila_auth.pending_items WHERE project_id=$1::text::kehila.ident AND state='prepared' AND request_bytes IS NOT NULL",
        &[&ctx.other.project.id.0]).await.unwrap().get::<_,i64>(0),1);
    // Scoped registration must not silently free another project's elapsed bytes.
    assert_eq!(
        pending(
            &serving.handle,
            &serving.browser,
            "register",
            form(&ctx.project)
        )
        .await
        .0,
        429
    );
    assert_eq!(cancel(serving, &other).await["state"], "expired");
    entries.push(register(serving, form(&ctx.project)).await);
    for entry in &entries {
        assert_eq!(cancel(serving, entry).await["state"], "cancelled");
    }
    ctx.admin
        .client()
        .batch_execute(CLOCK_RESTORE)
        .await
        .unwrap();
    unchanged(ctx, &before).await;
}

async fn seed_terminal_metadata(ctx: &Context<'_>, count: usize) {
    // Deliberately bypass the serving policy to construct a defensive metadata
    // boundary. Every small command is still canonically admitted and attributed.
    let registered: String = ctx
        .admin
        .client()
        .query_one("SELECT kehila_auth.trusted_millis()::text", &[])
        .await
        .unwrap()
        .get(0);
    let expires = (registered.parse::<u64>().unwrap() + 86_400_000).to_string();
    ctx.admin
        .client()
        .batch_execute("SET ROLE kehila_owner")
        .await
        .unwrap();
    for i in 0..count {
        let handle = format!("{:032x}", 100_000 + i);
        let token = format!("{:032x}", 200_000 + i);
        let target = format!("{:032x}", 300_000 + i);
        let bytes = work_item_codec::admit_request(&work_item_codec::Request::Create {
            project_id: ctx.project.project.id.clone(),
            command: CreateCommand {
                operation_id: token.clone(),
                item_id: WorkItemId(target.clone()),
                type_id: ctx.project.configuration.types[0].id.clone(),
                workflow_id: None,
                expected_configuration_revision: ctx.project.configuration.revision,
                estimate: None,
                values: vec![],
            },
        })
        .unwrap();
        ctx.admin.client().execute("INSERT INTO kehila_auth.pending_items(installation_id,actor_id,project_id,intent_handle,token,target_item_id,request_codec_version,request_sha256,request_bytes,registered_ms,expires_ms) \
          SELECT installation_id,owner_actor_id,$1::text::kehila.ident,$2::text::kehila_auth.intent_key,$3::text::kehila.operation_token,$4::text::kehila.ident,1,sha256($5::bytea),$5::bytea,$6::text::kehila.u64,$7::text::kehila.u64 FROM kehila_auth.installation",
            &[&ctx.project.project.id.0,&handle,&token,&target,&bytes.as_bytes(),&registered,&expires]).await.unwrap();
        ctx.admin.client().execute("UPDATE kehila_auth.pending_items SET state='cancelled',request_bytes=NULL WHERE intent_handle=$1::text::kehila_auth.intent_key AND project_id=$2::text::kehila.ident",
            &[&handle,&ctx.project.project.id.0]).await.unwrap();
    }
    ctx.admin
        .client()
        .batch_execute("RESET ROLE")
        .await
        .unwrap();
}
async fn metadata(ctx: &Context<'_>, serving: Serving) -> Serving {
    let before = evidence::pending_domain_snapshot(ctx.admin).await;
    clear_terminal(ctx).await;
    seed_terminal_metadata(ctx, 64).await;
    let entry = register(&serving, form(&ctx.project)).await;
    assert_eq!(
        ctx.admin
            .client()
            .query_one("SELECT count(*) FROM kehila_auth.pending_items", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    assert_eq!(cancel(&serving, &entry).await["state"], "cancelled");
    clear_terminal(ctx).await;
    seed_terminal_metadata(ctx, 65).await;
    let (status, value) = pending(
        &serving.handle,
        &serving.browser,
        "register",
        form(&ctx.project),
    )
    .await;
    assert_eq!(status, 503);
    assert_ne!(value["error"], "outcome_unknown");
    assert_eq!(
        ctx.admin
            .client()
            .query_one("SELECT count(*) FROM kehila_auth.pending_items", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        65
    );
    assert_eq!(
        serving.handle.live.failure(),
        Some(crate::lifecycle::FatalCause::NativeIntegrity)
    );
    serving.stop(ctx.admin, true).await;
    clear_terminal(ctx).await;
    unchanged(ctx, &before).await;
    ctx.start("after-synthetic-metadata").await
}

async fn completed(ctx: &Context<'_>, serving: &Serving) {
    // This is owner-created completed fixture input, NOT product item execution.
    let _ = item_reads::seed(
        ctx.admin,
        &ctx.project.project,
        &ctx.project.configuration,
        1,
    )
    .await;
    let before = evidence::pending_domain_snapshot(ctx.admin).await;
    let body = json!({"project_id":ctx.project.project.id.0,"intent_handle":format!("{:032x}",1)});
    let deadline:String=ctx.admin.client().query_one("SELECT replay_deadline_ms::text FROM kehila.operations WHERE command_family='item_create' AND target_item_id='read-fixture-1'",&[]).await.unwrap().get(0);
    ctx.admin
        .client()
        .batch_execute(CLOCK_INSTALL)
        .await
        .unwrap();
    clock(ctx, deadline.parse().unwrap()).await;
    let (status, retained) = pending(&serving.handle, &serving.browser, "open", body.clone()).await;
    assert_eq!(status, 200);
    assert_eq!(retained["entry"]["state"], "completed");
    assert!(retained["request"].is_object() && retained.get("result").is_none());
    assert_eq!(
        cancel(serving, &retained["entry"]).await["state"],
        "completed"
    );
    unchanged(ctx, &before).await;
    ctx.admin.client().batch_execute("BEGIN; SET LOCAL ROLE kehila_owner; UPDATE kehila.operations SET payload_retired=true WHERE command_family='item_create' AND target_item_id='read-fixture-1'; DELETE FROM kehila.operation_payloads WHERE operation_row_id IN (SELECT operation_row_id FROM kehila.operations WHERE command_family='item_create' AND target_item_id='read-fixture-1'); SET CONSTRAINTS ALL IMMEDIATE; COMMIT").await.unwrap();
    let retired = evidence::pending_domain_snapshot(ctx.admin).await;
    let (status, opened) = pending(&serving.handle, &serving.browser, "open", body.clone()).await;
    assert_eq!(status, 200);
    assert!(opened["request"].is_object() && opened.get("result").is_none());
    unchanged(ctx, &retired).await;
    // Prune in a separate transaction after retirement is already committed.
    ctx.admin.client().batch_execute("BEGIN; SET LOCAL ROLE kehila_owner; UPDATE kehila_auth.pending_items SET request_bytes=NULL WHERE target_item_id='read-fixture-1'; SET CONSTRAINTS ALL IMMEDIATE; COMMIT").await.unwrap();
    let (status, pruned) = pending(&serving.handle, &serving.browser, "open", body).await;
    assert_eq!(status, 200);
    assert!(pruned["request"].is_null() && pruned.get("result").is_none());
    assert_eq!(pruned["entry"], retained["entry"]);
    ctx.admin
        .client()
        .batch_execute(CLOCK_RESTORE)
        .await
        .unwrap();
    let after = evidence::pending_domain_snapshot(ctx.admin).await;
    assert_eq!(
        cancel(serving, &pruned["entry"]).await["state"],
        "completed"
    );
    unchanged(ctx, &after).await;
}

pub(super) async fn exercise(ctx: &Context<'_>, serving: Serving) -> Serving {
    quotas(ctx, &serving).await;
    let serving = metadata(ctx, serving).await;
    completed(ctx, &serving).await;
    clear_terminal(ctx).await;
    // Fixed final witness: one pruned successful owner seed plus one cancelled
    // product intent in each project, with no prepared content remaining.
    let one = register(&serving, form(&ctx.project)).await;
    assert_eq!(cancel(&serving, &one).await["state"], "cancelled");
    let two = register(&serving, form(&ctx.other)).await;
    assert_eq!(cancel(&serving, &two).await["state"], "cancelled");
    serving
}
