//! Real owner-adapter mutations plus authenticated HTTP/TLS reads.
//! This does not activate serving mutation grants or qualify reply recovery.
use super::*;
use crate::postgres::tests::fixture_admin::Admin;
use crate::postgres::{
    transaction::{
        AdmittedItem, CanonicalItem, ItemDisposition, ItemPreparation, ItemSaveOutcome,
        ItemWriteConflict, ItemWriteFailure, NativeError,
    },
    Authority, DbError,
};
use task_contract::{
    field::{FieldEdit, FieldEntry, FieldValue},
    item_mutation::{CreateCommand, EditCommand, EstimateEdit, FieldUpdate},
    project::Estimate,
    work_item::{Configuration, WorkItemId},
};
use work_item_codec::{Request, SavedResult};

const EXACT: &str = "  exact\r\n\u{e9} title  ";

async fn register(
    admin: &Admin,
    authority: &Authority,
    project: &str,
    handle: &str,
    target: &str,
    request: &[u8],
) {
    // All five authority values are captured once, not silently refreshed here.
    let sql="SELECT request_id FROM kehila_auth.register_item(ROW($1::text::kehila.ident,$2::text::kehila.ident,$3::text::kehila.u64,$4::text::kehila.u64,$5::text::kehila.u64)::kehila_auth.authority,$6::text::kehila.ident,$7::text::kehila_auth.intent_key,$7::text::kehila_auth.intent_key,$8::text::kehila.ident,1,$9::bytea)";
    assert!(
        admin
            .client()
            .query_one(
                sql,
                &[
                    &authority.installation,
                    &authority.owner,
                    &authority.runtime.to_string(),
                    &authority.authentication.to_string(),
                    &authority.revision.to_string(),
                    &project,
                    &handle,
                    &target,
                    &request
                ]
            )
            .await
            .unwrap()
            .get::<_, i64>(0)
            > 0
    );
}
struct Creation<'a> {
    target: &'a str,
    key: char,
    kind: usize,
    title: Option<&'a str>,
    estimate: Option<Estimate>,
}
async fn create(
    admin: &mut Admin,
    a: &Authority,
    deadline: Instant,
    project: &str,
    configuration: &Configuration,
    case: Creation<'_>,
) -> AdmittedItem {
    let Creation {
        target,
        key,
        kind,
        title,
        estimate,
    } = case;
    let token = key.to_string().repeat(32);
    let ty = &configuration.types[kind];
    let request = work_item_codec::admit_request(&Request::Create {
        project_id: configuration.project_id.clone(),
        command: CreateCommand {
            operation_id: token.clone(),
            expected_configuration_revision: configuration.revision,
            item_id: WorkItemId(target.into()),
            type_id: ty.id.clone(),
            workflow_id: None,
            estimate,
            values: title
                .map(|title| {
                    vec![FieldEntry {
                        id: ty.title_field_id.clone().unwrap(),
                        value: FieldValue::Text(title.into()),
                    }]
                })
                .unwrap_or_default(),
        },
    })
    .unwrap();
    register(admin, a, project, &token, target, request.as_bytes()).await;
    let mut snapshot = admin.read_snapshot(a, deadline).await.unwrap();
    let prepared = snapshot
        .prepare_item_create(project, &token)
        .await
        .unwrap_or_else(|_| panic!("native preparation failed"));
    snapshot.rollback().await.unwrap();
    let ItemPreparation::Ready(draft) = prepared else {
        panic!("fresh intent not prepared")
    };
    let admitted = draft.admit().unwrap();
    assert_eq!(admitted.request_bytes(), request.as_bytes());
    admitted
}
async fn edit(
    admin: &mut Admin,
    a: &Authority,
    deadline: Instant,
    command: Arc<CanonicalItem>,
) -> AdmittedItem {
    let mut snapshot = admin.read_snapshot(a, deadline).await.unwrap();
    let prepared = snapshot
        .prepare_item_edit(command)
        .await
        .unwrap_or_else(|_| panic!("native preparation failed"));
    snapshot.rollback().await.unwrap();
    let ItemPreparation::Ready(draft) = prepared else {
        panic!("unseen edit not prepared")
    };
    draft.admit().unwrap()
}
async fn commit(
    admin: &mut Admin,
    a: &Authority,
    deadline: Instant,
    packet: AdmittedItem,
) -> SavedResult {
    let transaction = admin.item_write_transaction(a, deadline).await.unwrap();
    let ItemSaveOutcome::Ready(write) = transaction
        .save_item(packet)
        .await
        .unwrap_or_else(|_| panic!("native save failed"))
    else {
        panic!("native save not accepted")
    };
    let committed = write.commit().await.unwrap();
    assert!(committed.operation() > 0);
    assert_eq!(committed.authority().revision, a.revision);
    work_item_codec::decode_result(1, committed.result_bytes()).unwrap()
}
fn edit_command(
    a: &Authority,
    c: &Configuration,
    token: char,
    version: u64,
    estimate: EstimateEdit,
    field: FieldEdit,
) -> Arc<CanonicalItem> {
    Arc::new(
        CanonicalItem::admit(
            &a.installation,
            &a.owner,
            Request::Edit {
                project_id: c.project_id.clone(),
                item_id: WorkItemId("write-task".into()),
                command: EditCommand {
                    operation_id: token.to_string().repeat(32),
                    expected_item_version: version,
                    expected_configuration_revision: c.revision,
                    estimate,
                    fields: vec![FieldUpdate {
                        id: c.types[0].title_field_id.clone().unwrap(),
                        edit: field,
                    }],
                },
            },
        )
        .unwrap(),
    )
}

#[test]
#[ignore = "requires a fresh explicitly provisioned product PostgreSQL TLS fixture"]
fn native_item_write_identity_and_rollback() {
    let (endpoint, installation, attempt, controls) = fixture();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let (handle, owner) = runtime(&endpoint, &installation, attempt).await;
            let running = tokio::spawn(owner.run());
            let browser = login(&handle).await;
            let (status, pending) = project(
                &handle,
                &browser,
                "register",
                json!({"name":"Item write fixture","prefix":"IWR","estimate_unit":"hours"}),
            )
            .await;
            assert_eq!(status, 201);
            let (status, created) =
                project(&handle, &browser, "execute", execute(&pending["entry"])).await;
            assert_eq!(status, 201);
            let initial = project_create_codec::decode_result(
                1,
                &serde_json::to_vec(&created["result"]).unwrap(),
            )
            .unwrap();
            let project_id = initial.project.id.0.clone();
            let configuration = &initial.configuration;
            let authority = {
                let engine = handle.engine.upgrade().unwrap();
                let database = engine.database.lock().await;
                let a = database.authority().unwrap();
                Authority {
                    installation: a.installation.clone(),
                    owner: a.owner.clone(),
                    runtime: a.runtime,
                    authentication: a.authentication,
                    revision: a.revision,
                }
            };
            // One deadline across all interleavings and snapshots; no retry renewal.
            // Fixed total adapter-fixture bound, not a product operating-deadline test.
            let deadline = Instant::now() + Duration::from_secs(30);
            let mut admin = Admin::connect(&endpoint).await;
            let observe = admin.database_observer();
            let first = create(
                &mut admin,
                &authority,
                deadline,
                &project_id,
                configuration,
                Creation {
                    target: "write-task",
                    key: 'a',
                    kind: 0,
                    title: Some("  exact task  "),
                    estimate: Some(Estimate::try_from_thousandths(0).unwrap()),
                },
            )
            .await;
            let original = first.request_bytes().to_vec();
            let second = create(
                &mut admin,
                &authority,
                deadline,
                &project_id,
                configuration,
                Creation {
                    target: "write-milestone",
                    key: 'b',
                    kind: 1,
                    title: None,
                    estimate: None,
                },
            )
            .await;
            let SavedResult::Create(second) =
                commit(&mut admin, &authority, deadline, second).await
            else {
                panic!("wrong family")
            };
            assert_eq!(second.readable_id.sequence, 1);
            let before = evidence::snapshot(&admin).await;
            let transaction = admin
                .item_write_transaction(&authority, deadline)
                .await
                .unwrap();
            let ItemSaveOutcome::Reprepare(identity) = transaction
                .save_item(first)
                .await
                .unwrap_or_else(|_| panic!("reprepare failed"))
            else {
                panic!("stale head was not reprepare")
            };
            assert_eq!(evidence::snapshot(&admin).await, before);
            let mut snapshot = admin.read_snapshot(&authority, deadline).await.unwrap();
            let prepared = snapshot
                .prepare_item_retry(identity)
                .await
                .unwrap_or_else(|_| panic!("native preparation failed"));
            snapshot.rollback().await.unwrap();
            let ItemPreparation::Ready(draft) = prepared else {
                panic!("retry not prepared")
            };
            let first = draft.admit().unwrap();
            assert_eq!(first.request_bytes(), original);
            let SavedResult::Create(first) = commit(&mut admin, &authority, deadline, first).await
            else {
                panic!("wrong family")
            };
            assert_eq!(first.readable_id.sequence, 2);
            assert_eq!(
                first.data.estimate,
                Some(Estimate::try_from_thousandths(0).unwrap())
            );
            let clear_command = edit_command(
                &authority,
                configuration,
                'c',
                1,
                EstimateEdit::Clear,
                FieldEdit::Clear,
            );
            let clear = edit(&mut admin, &authority, deadline, Arc::clone(&clear_command)).await;
            let stale = edit(
                &mut admin,
                &authority,
                deadline,
                edit_command(
                    &authority,
                    configuration,
                    'd',
                    1,
                    EstimateEdit::Keep,
                    FieldEdit::Keep,
                ),
            )
            .await;
            let SavedResult::Edit(cleared) = commit(&mut admin, &authority, deadline, clear).await
            else {
                panic!("wrong family")
            };
            assert_eq!(cleared.item.version, 2);
            assert!(cleared.project.ever_estimated);
            assert!(cleared.data.estimate.is_none() && cleared.data.values.is_empty());
            assert_eq!(cleared.item.id, first.item.id);
            let before = evidence::snapshot(&admin).await;
            let transaction = admin
                .item_write_transaction(&authority, deadline)
                .await
                .unwrap();
            assert!(matches!(
                transaction.save_item(stale).await,
                Ok(ItemSaveOutcome::Conflict(ItemWriteConflict::Item))
            ));
            assert_eq!(evidence::snapshot(&admin).await, before);
            let same = edit_command(
                &authority,
                configuration,
                'e',
                2,
                EstimateEdit::Set(Estimate::try_from_thousandths(999999999).unwrap()),
                FieldEdit::Set(FieldValue::Text(EXACT.into())),
            );
            let winner = edit(&mut admin, &authority, deadline, Arc::clone(&same)).await;
            let replay = edit(&mut admin, &authority, deadline, same).await;
            let final_task = commit(&mut admin, &authority, deadline, winner).await;
            let before = evidence::snapshot(&admin).await;
            let transaction = admin
                .item_write_transaction(&authority, deadline)
                .await
                .unwrap();
            let entered = observe();
            let ItemSaveOutcome::Resolved(resolved) = transaction
                .save_item(replay)
                .await
                .unwrap_or_else(|_| panic!("replay failed"))
            else {
                panic!("duplicate did not resolve")
            };
            assert_eq!(resolved.outcome(), ItemDisposition::Replay);
            assert!(resolved.operation().is_some());
            assert_eq!(
                observe() - entered,
                1,
                "historical resolution fetched fresh payload"
            );
            assert_eq!(evidence::snapshot(&admin).await, before);
            // Replay the original creation and version-two edit after the current item
            // has advanced to version three. No mutable acceptance/rebase is permitted.
            histories::check(
                &mut admin,
                &authority,
                deadline,
                &project_id,
                configuration,
                clear_command,
                &SavedResult::Create(first.clone()),
                &SavedResult::Edit(cleared.clone()),
            )
            .await;
            let mut rejected = create(
                &mut admin,
                &authority,
                deadline,
                &project_id,
                configuration,
                Creation {
                    target: "write-rejected",
                    key: 'f',
                    kind: 0,
                    title: Some("valid provisional title"),
                    estimate: None,
                },
            )
            .await;
            rejected.corrupt_expected_state_for_test().unwrap();
            let before = evidence::snapshot(&admin).await;
            let transaction = admin
                .item_write_transaction(&authority, deadline)
                .await
                .unwrap();
            let entered = observe();
            assert!(matches!(
                transaction.save_item(rejected).await,
                Err(ItemWriteFailure::Operation {
                    primary: NativeError::Integrity,
                    rollback: None
                })
            ));
            assert!(observe() - entered > 1, "did not reach post-save reload");
            assert_eq!(
                evidence::snapshot(&admin).await,
                before,
                "provisional artifacts survived rollback"
            );
            admin.close().await;
            // Separate raw owner for real COMMIT followed by withheld confirmation.
            let mut unknown = Admin::connect(&endpoint).await;
            let packet = create(
                &mut unknown,
                &authority,
                deadline,
                &project_id,
                configuration,
                Creation {
                    target: "write-unknown",
                    key: '1',
                    kind: 1,
                    title: None,
                    estimate: None,
                },
            )
            .await;
            let mut transaction = unknown
                .item_write_transaction(&authority, deadline)
                .await
                .unwrap();
            transaction.lose_item_commit_confirmation_for_test();
            let ItemSaveOutcome::Ready(write) = transaction
                .save_item(packet)
                .await
                .unwrap_or_else(|_| panic!("unknown-control save failed"))
            else {
                panic!("unknown-control not accepted")
            };
            assert!(matches!(write.commit().await, Err(DbError::UnknownCommit)));
            assert!(matches!(
                unknown.read_snapshot(&authority, deadline).await,
                Err(DbError::Unavailable)
            ));
            assert!(matches!(
                unknown.write_transaction(&authority, deadline).await,
                Err(DbError::Unavailable)
            ));
            assert!(matches!(
                unknown.item_write_transaction(&authority, deadline).await,
                Err(DbError::Unavailable)
            ));
            unknown.close().await;
            let (status, read, _) = call(
                &handle,
                Some(&browser),
                "/items/read",
                Some(json!({"project_id":project_id,"item_id":"write-task"})),
            )
            .await;
            assert_eq!(status, 200);
            assert_eq!(
                work_item_codec::decode_result(1, &serde_json::to_vec(&read["result"]).unwrap())
                    .unwrap(),
                final_task
            );
            assert_eq!(read["readable_id"]["sequence"], "2");
            let (status, page, _) = call(
                &handle,
                Some(&browser),
                "/items/list",
                Some(json!({"project_id":project_id,"before":null})),
            )
            .await;
            assert_eq!(status, 200);
            assert!(page["before"].is_null());
            let entries = page["entries"].as_array().unwrap();
            assert_eq!(entries.len(), 3);
            assert_eq!(entries[0]["item_id"], "write-unknown");
            assert_eq!(entries[1]["item_id"], "write-task");
            assert_eq!(entries[1]["version"], "3");
            assert_eq!(entries[1]["estimate"], "999999999");
            assert_eq!(entries[1]["name"], EXACT);
            assert_eq!(entries[2]["item_id"], "write-milestone");
            let observer = Admin::connect(&endpoint).await;
            evidence::settled(&handle).await;
            evidence::fences_released(&observer).await;
            handle.stop();
            running.await.unwrap().unwrap();
            evidence::runtime_released(&observer).await;
            observer.close().await;
            mark(&controls, "item-write-passed");
        });
}

mod histories;
