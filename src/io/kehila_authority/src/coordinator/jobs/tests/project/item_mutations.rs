//! Serving-principal HTTP/TLS mutation and historical identity journey.
//! Browser interaction is separate; owned native replay controls run last.
use super::*;
use crate::postgres::tests::fixture_admin::Admin;
use work_item_codec::SavedResult;

const EXACT: &str = "  exact\r\n\u{e9} title  ";
async fn request(
    handle: &Handle,
    browser: &Browser,
    path: &str,
    body: Value,
    status: u16,
) -> Value {
    let (actual, value, _) = call(handle, Some(browser), path, Some(body)).await;
    assert_eq!(actual, status, "unexpected mutation HTTP status");
    value
}
async fn lose_network_reply(handle: &Handle, browser: &Browser, path: &str, body: Value) {
    let gate = handle
        .live
        .probe
        .hold_transaction(0, crate::lifecycle::probe::Point::ItemBeforeCommit);
    // Existing listener helper closes the real TLS client's TCP socket before
    // releasing the owned SQL COMMIT. The manager still owns and joins work.
    crate::listener::tests::browser_disconnect_before_reply(
        handle.clone(),
        wire_request(Some(browser), path, Some(body)),
        gate,
    )
    .await;
}
async fn lose_reply(handle: &Handle, browser: &Browser, path: &str, body: Value) {
    lose_network_reply(handle, browser, path, body).await;
    evidence::settled(handle).await;
    assert!(handle.live.is_live(), "reply loss poisoned the runtime");
}
async fn reject_wrong_csrf(handle: &Handle, browser: &Browser, admin: &Admin, edit: &Value) {
    evidence::settled(handle).await;
    let before = evidence::snapshot(admin).await;
    let admitted = handle.live.probe.snapshot();
    let mut incorrect = browser.clone();
    // Change a non-padding base64url character, preserving a well-formed digest.
    // This must reach credential validation rather than header syntax rejection.
    let replacement = if incorrect.csrf.starts_with('A') {
        "B"
    } else {
        "A"
    };
    incorrect.csrf.replace_range(..1, replacement);
    let (status, value, _) =
        call(handle, Some(&incorrect), "/items/edit", Some(edit.clone())).await;
    assert_eq!(status, 403);
    assert_eq!(value, json!({"error":"forbidden"}));
    evidence::settled(handle).await;
    let rejected = handle.live.probe.snapshot();
    assert_eq!(
        rejected.preparations,
        admitted.preparations + 1,
        "well-formed incorrect CSRF did not reach tracked credential validation"
    );
    assert_eq!(rejected.workers, admitted.workers);
    assert_eq!(
        rejected.native_mutex_attempts,
        admitted.native_mutex_attempts
    );
    assert_eq!(
        rejected.database_ownership_entries,
        admitted.database_ownership_entries
    );
    assert_eq!(
        evidence::snapshot(admin).await,
        before,
        "rejected credential changed durable work"
    );
    assert!(handle.live.is_live());
}
fn decoded(value: &Value) -> SavedResult {
    work_item_codec::decode_result(1, &serde_json::to_vec(&value["result"]).unwrap()).unwrap()
}
#[test]
#[ignore = "requires a fresh explicitly provisioned product PostgreSQL TLS fixture"]
fn native_item_mutation_execution_and_replay() {
    let (endpoint, installation, attempt, controls) = fixture();
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async{
    let(handle,owner)=runtime(&endpoint,&installation,format!("{attempt}-item-initial")).await;
    let mut running=tokio::spawn(owner.run());let browser=login(&handle).await;
    let(status,pending)=project(&handle,&browser,"register",
        json!({"name":"Item mutation journey","prefix":"IMJ","estimate_unit":"hours"})).await;
    assert_eq!(status,201);
    let(status,created)=project(&handle,&browser,"execute",execute(&pending["entry"])).await;
    assert_eq!(status,201);
    let initial=project_create_codec::decode_result(1,&serde_json::to_vec(&created["result"]).unwrap()).unwrap();
    let project_id=&initial.project.id.0;
    let task=&initial.configuration.types[0];let milestone=&initial.configuration.types[1];
    let registered=request(&handle,&browser,"/items/pending/register",json!({
        "project_id":project_id,"type_id":task.id.0,"workflow_id":null,"expected_configuration_revision":"1",
        "estimate":"0","values":[{"id":task.title_field_id.as_ref().unwrap().0,"value":EXACT}]}),201).await;
    let locator=json!({"project_id":project_id,"intent_handle":registered["entry"]["intent_handle"]});
    let admin=Admin::connect(&endpoint).await;
    assert!(!admin.client().query_one("SELECT has_table_privilege('kehila_auth_runtime','kehila_auth.pending_items','SELECT')",&[])
        .await.unwrap().get::<_,bool>(0));
    let before=evidence::snapshot(&admin).await;
    let absent=request(&handle,&browser,"/items/pending/reconcile",locator.clone(),404).await;
    assert_eq!(absent,json!({"error":"operation_not_found"}));
    assert_eq!(evidence::snapshot(&admin).await,before,"reconciliation executed a prepared creation");
    let inspected=request(&handle,&browser,"/items/pending/open",locator.clone(),200).await;
    assert!(inspected.get("result").is_none());
    let recovery_creation=recovery::Creation{entry:registered["entry"].clone(),request:inspected["request"].clone()};
    lose_reply(&handle,&browser,"/items/pending/execute",locator.clone()).await;
    // No creation result has acknowledged the lost write before this restart.
    let recovery::Serving{handle,browser,admin,running:next}=recovery::restart(handle,browser,admin,&endpoint,
        &installation,format!("{attempt}-item-lost"),recovery_creation.clone(),None,&mut running).await;
    running=next;
    let first=request(&handle,&browser,"/items/pending/execute",locator.clone(),200).await;
    assert_eq!(first["outcome"],"replayed");
    let SavedResult::Create(first_result)=decoded(&first) else{panic!("wrong result family")};
    assert_eq!(first_result.item.version,1);assert_eq!(first_result.readable_id.sequence,1);
    assert_eq!(first_result.data.values[0].value,task_contract::field::FieldValue::Text(EXACT.into()));
    let registered=request(&handle,&browser,"/items/pending/register",json!({
        "project_id":project_id,"type_id":milestone.id.0,"workflow_id":null,"expected_configuration_revision":"1",
        "estimate":null,"values":[]}),201).await;
    let milestone_locator=json!({"project_id":project_id,"intent_handle":registered["entry"]["intent_handle"]});
    let edit=json!({"project_id":project_id,"item_id":first_result.item.id.0,"operation_id":"c".repeat(32),
        "expected_item_version":"1","expected_configuration_revision":"1",
        "estimate":{"action":"set","value":"999999999"},
        "fields":[{"id":task.title_field_id.as_ref().unwrap().0,"action":"set","value":EXACT}]});

    reject_wrong_csrf(&handle,&browser,&admin,&edit).await;
    let clear=json!({
        "project_id":project_id,"item_id":first_result.item.id.0,"operation_id":"d".repeat(32),
        "expected_item_version":"2","expected_configuration_revision":"1",
        "estimate":{"action":"clear"},"fields":[{"id":task.title_field_id.as_ref().unwrap().0,"action":"clear"}]});
    let cancel_form=json!({"project_id":project_id,"type_id":task.id.0,"workflow_id":null,
        "expected_configuration_revision":"1","estimate":null,"values":[]});
    let(second,changed,cleared,admin)=contention::exercise(&handle,&browser,admin,milestone_locator,
        edit.clone(),clear.clone(),cancel_form,&mut running).await;
    let SavedResult::Create(second)=decoded(&second) else{panic!("wrong result family")};
    assert_eq!(second.item.type_id,milestone.id);assert_eq!(second.readable_id.sequence,2);
    assert_eq!(changed["outcome"],"replayed");
    let SavedResult::Edit(changed_result)=decoded(&changed) else{panic!("wrong result family")};
    assert_eq!(changed_result.item.version,2);
    assert_eq!(changed_result.data.estimate.unwrap().thousandths(),999999999);
    let SavedResult::Edit(cleared)=decoded(&cleared) else{panic!("wrong result family")};
    assert_eq!(cleared.item.version,3);assert!(cleared.data.estimate.is_none());
    assert!(cleared.data.values.is_empty());assert!(cleared.project.ever_estimated);
    let before=evidence::snapshot(&admin).await;
    // Repeat identities recovered after actual earlier TCP response loss.
    // Both execution and reconciliation must return the historical result even
    // though current versions/operational state have advanced.
    for path in ["/items/edit","/items/reconcile"] {
        let replay=request(&handle,&browser,path,edit.clone(),200).await;
        assert_eq!(replay["outcome"],"replayed");assert_eq!(decoded(&replay),SavedResult::Edit(changed_result.clone()));
    }
    for path in ["/items/pending/execute","/items/pending/reconcile"] {
        let replay=request(&handle,&browser,path,locator.clone(),200).await;
        assert_eq!(replay["outcome"],"replayed");assert_eq!(decoded(&replay),SavedResult::Create(first_result.clone()));
    }
    let mut conflict=edit.clone();conflict["estimate"]=json!({"action":"keep"});
    assert_eq!(request(&handle,&browser,"/items/edit",conflict,409).await,json!({"error":"operation_conflict"}));
    let mut stale=edit.clone();stale["operation_id"]=json!("e".repeat(32));
    assert_eq!(request(&handle,&browser,"/items/edit",stale,409).await,json!({"error":"item_conflict"}));
    let mut unseen=edit.clone();unseen["operation_id"]=json!("f".repeat(32));unseen["expected_item_version"]=json!(u64::MAX.to_string());
    assert_eq!(request(&handle,&browser,"/items/reconcile",unseen,404).await,json!({"error":"operation_not_found"}));
    assert_eq!(evidence::snapshot(&admin).await,before,"retry/reconciliation/conflict changed durable artifacts");
    let read=request(&handle,&browser,"/items/read",json!({"project_id":project_id,"item_id":first_result.item.id.0}),200).await;
    let SavedResult::Edit(current)=decoded(&read) else{panic!("current read lost edited result family")};
    assert_eq!(current.item.version,3);assert!(current.data.estimate.is_none());
    assert!(current.data.values.is_empty());assert_eq!(read["readable_id"]["sequence"],"1");
    let page=request(&handle,&browser,"/items/list",json!({"project_id":project_id,"before":null}),200).await;
    assert_eq!(page["entries"].as_array().unwrap().len(),2);assert!(page["before"].is_null());
    let history=recovery::History{creation_result:first["result"].clone(),edit:edit.clone(),edit_result:changed["result"].clone(),
        project_entry:pending["entry"].clone(),project_result:created["result"].clone(),current:read.clone()};
    let recovery::Serving{handle,browser,admin,running:next}=recovery::restart(handle,browser,admin,&endpoint,
        &installation,format!("{attempt}-item-edited"),recovery_creation,Some(history),&mut running).await;
    running=next;
    boundary::exercise(&handle,&browser,&admin,&endpoint,&edit,&clear,&locator,&changed["result"],&mut running).await;
    evidence::runtime_released(&admin).await;
    admin.close().await;mark(&controls,"item-mutation-passed");
 });
}

mod boundary;
mod contention;
mod recovery;
