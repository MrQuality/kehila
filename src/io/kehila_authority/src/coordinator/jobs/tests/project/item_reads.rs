//! Real read-only product capability; fixture item writes are owner actions.
use super::*;
use crate::postgres::tests::fixture_admin::Admin;
use task_contract::{
    current_work::UserId,
    field::{FieldEntry, FieldValue},
    item_mutation::{
        decide_create, decide_edit, CreateCommand, CreateDecision, EditCommand, EditDecision,
        EstimateEdit,
    },
    project::{Estimate, Project},
    work_item::{Configuration, WorkItemId},
};
use work_item_codec::{Request, Revision, SavedResult};
mod factory_clock;
mod faults;
mod notices;

const AUTHORITY: &str = "(SELECT ROW(i.installation_id,i.owner_actor_id,i.runtime_generation, \
 c.auth_generation,c.verifier_revision)::kehila_auth.authority FROM kehila_auth.installation i \
 JOIN kehila_auth.owner_credentials c ON c.owner_actor_id=i.owner_actor_id)";

pub(super) async fn seed(
    admin: &Admin,
    current: &Project,
    configuration: &Configuration,
    index: u64,
) -> SavedResult {
    let kind = &configuration.types[usize::try_from((index - 1) % 2).unwrap()];
    let token = format!("{index:032x}");
    let command = CreateCommand {
        operation_id: token.clone(),
        expected_configuration_revision: configuration.revision,
        item_id: WorkItemId(format!("read-fixture-{index}")),
        type_id: kind.id.clone(),
        workflow_id: None,
        estimate: if index == 2 {
            Some(Estimate::try_from_thousandths(0).unwrap())
        } else {
            None
        },
        values: if index >= 2 {
            vec![FieldEntry {
                id: kind.title_field_id.clone().unwrap(),
                value: FieldValue::Text(if index == 2 {
                    "  milestone title  ".into()
                } else {
                    "\u{1}".repeat(16 * 1024)
                }),
            }]
        } else {
            vec![]
        },
    };
    let CreateDecision::Apply(result) = decide_create(current, configuration, None, true, &command)
    else {
        panic!("valid fixture rejected")
    };
    let request = Request::Create {
        project_id: current.id.clone(),
        command,
    };
    let request = work_item_codec::admit_request(&request).unwrap();
    let head=admin.client().query_one("SELECT state_version::numeric(20,0)::text,operation_row_id FROM kehila.project_operational_heads WHERE project_id=$1::text::kehila.ident",&[&current.id.0]).await.unwrap();
    let state: u64 = head.get::<_, String>(0).parse().unwrap();
    let previous: i64 = head.get(1);
    let saved = SavedResult::Create(result);
    let history = work_item_codec::admit_revision(&Revision {
        actor_id: UserId("owner".into()),
        configuration_revision: configuration.revision,
        project_state_version: state + 1,
        previous_project_operation: u64::try_from(previous).unwrap(),
        result: saved.clone(),
    })
    .unwrap();
    let history = std::str::from_utf8(history.as_bytes()).unwrap();
    let item = format!("read-fixture-{index}");
    let sql=format!("SELECT request_id FROM kehila_auth.register_item({AUTHORITY},$1::text::kehila.ident, \
      $2::text::kehila_auth.intent_key,$2::text::kehila_auth.intent_key,$3::text::kehila.ident,1,$4::bytea)");
    assert!(
        admin
            .client()
            .query_one(&sql, &[&current.id.0, &token, &item, &request.as_bytes()])
            .await
            .unwrap()
            .get::<_, i64>(0)
            > 0
    );
    let project =
        String::from_utf8(project_create_codec::encode_project_snapshot(current).unwrap()).unwrap();
    let sql=format!("SELECT (kehila_auth.save_item_create({AUTHORITY},$1::text::kehila.ident, \
      $2::text::kehila_auth.intent_key,$3::bytea, \
      ROW($4::text::jsonb,p.configuration_revision,c.operation_row_id,c.configuration_snapshot,c.project_snapshot, \
          h.state_version,h.operation_row_id,NULL,NULL,NULL)::kehila_auth.item_preparation,$5::text::jsonb)).outcome \
      FROM kehila.projects p JOIN kehila.configuration_revisions c ON c.project_id=p.project_id \
        AND c.revision=p.configuration_revision JOIN kehila.project_operational_heads h ON h.project_id=p.project_id \
      WHERE p.project_id=$1::text::kehila.ident");
    let outcome: String = admin
        .client()
        .query_one(
            &sql,
            &[
                &current.id.0,
                &token,
                &request.as_bytes(),
                &project,
                &history,
            ],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(outcome, "accepted");
    saved
}

async fn item(handle: &Handle, browser: &Browser, path: &str, body: Value) -> (u16, Value) {
    let (status, value, _) =
        call(handle, Some(browser), &format!("/items/{path}"), Some(body)).await;
    (status, value)
}

async fn clear_estimate(
    admin: &Admin,
    current: &Project,
    configuration: &Configuration,
    created: &SavedResult,
) -> SavedResult {
    let (item, data) = match created {
        SavedResult::Create(value) => (&value.item, &value.data),
        SavedResult::Edit(value) => (&value.item, &value.data),
    };
    let token = format!("{:032x}", 1000 + item.version);
    let command = EditCommand {
        operation_id: token.clone(),
        expected_item_version: item.version,
        expected_configuration_revision: configuration.revision,
        estimate: EstimateEdit::Clear,
        fields: vec![],
    };
    let EditDecision::Apply(result) =
        decide_edit(current, configuration, item, data, None, true, &command)
    else {
        panic!("valid edit fixture rejected")
    };
    let request = work_item_codec::admit_request(&Request::Edit {
        project_id: current.id.clone(),
        item_id: item.id.clone(),
        command,
    })
    .unwrap();
    let head=admin.client().query_one("SELECT state_version::numeric(20,0)::text,operation_row_id FROM kehila.project_operational_heads WHERE project_id=$1::text::kehila.ident",&[&current.id.0]).await.unwrap();
    let state: u64 = head.get::<_, String>(0).parse().unwrap();
    let previous: i64 = head.get(1);
    let saved = SavedResult::Edit(result);
    let history = work_item_codec::admit_revision(&Revision {
        actor_id: UserId("owner".into()),
        configuration_revision: configuration.revision,
        project_state_version: state + 1,
        previous_project_operation: u64::try_from(previous).unwrap(),
        result: saved.clone(),
    })
    .unwrap();
    let history = std::str::from_utf8(history.as_bytes()).unwrap();
    let project =
        String::from_utf8(project_create_codec::encode_project_snapshot(current).unwrap()).unwrap();
    let sql=format!("SELECT (kehila_auth.save_item_edit({AUTHORITY},$1::text::kehila.ident, \
      $2::text::kehila.ident,$3::text::kehila.operation_token,$4::bytea, \
      ROW($5::text::jsonb,p.configuration_revision,c.operation_row_id,c.configuration_snapshot,c.project_snapshot, \
          h.state_version,h.operation_row_id,r.item_version,r.operation_row_id,r.history)::kehila_auth.item_preparation,$6::text::jsonb)).outcome \
      FROM kehila.projects p JOIN kehila.configuration_revisions c ON c.project_id=p.project_id \
        AND c.revision=p.configuration_revision JOIN kehila.project_operational_heads h ON h.project_id=p.project_id \
      JOIN kehila.items i ON i.project_id=p.project_id AND i.item_id=$2::text::kehila.ident \
      JOIN kehila.item_revisions r ON r.project_id=i.project_id AND r.item_id=i.item_id AND r.item_version=i.current_version \
      WHERE p.project_id=$1::text::kehila.ident");
    let outcome: String = admin
        .client()
        .query_one(
            &sql,
            &[
                &current.id.0,
                &item.id.0,
                &token,
                &request.as_bytes(),
                &project,
                &history,
            ],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(outcome, "accepted");
    saved
}

async fn factory_preparations(
    handle: &Handle,
    endpoint: &crate::postgres::Endpoint,
    admin: &Admin,
    current: &Project,
    configuration: &Configuration,
    edited: &SavedResult,
) {
    use crate::postgres::transaction::{CanonicalItem, ItemDisposition, ItemPreparation};
    let expected = {
        let engine = handle.engine.upgrade().unwrap();
        let database = engine.database.lock().await;
        let a = database.authority().unwrap();
        crate::postgres::Authority {
            installation: a.installation.clone(),
            owner: a.owner.clone(),
            runtime: a.runtime,
            authentication: a.authentication,
            revision: a.revision,
        }
    };
    let deadline = || std::time::Instant::now() + std::time::Duration::from_secs(5);
    let target = "factory-target";
    let handle_key = "e".repeat(32);
    let token = "f".repeat(32);
    let kind = &configuration.types[0];
    let command = CreateCommand {
        operation_id: token.clone(),
        expected_configuration_revision: configuration.revision,
        item_id: WorkItemId(target.into()),
        type_id: kind.id.clone(),
        workflow_id: None,
        estimate: None,
        values: vec![FieldEntry {
            id: kind.title_field_id.clone().unwrap(),
            value: FieldValue::Text("  exact factory intent  ".into()),
        }],
    };
    let original = work_item_codec::admit_request(&Request::Create {
        project_id: current.id.clone(),
        command,
    })
    .unwrap();
    let register=format!("SELECT request_id FROM kehila_auth.register_item({AUTHORITY},$1::text::kehila.ident,$2::text::kehila_auth.intent_key,$3::text::kehila_auth.intent_key,$4::text::kehila.ident,1,$5::bytea)");
    admin.client().batch_execute("BEGIN").await.unwrap();
    assert!(
        admin
            .client()
            .query_one(
                &register,
                &[
                    &current.id.0,
                    &handle_key,
                    &token,
                    &target,
                    &original.as_bytes()
                ]
            )
            .await
            .unwrap()
            .get::<_, i64>(0)
            > 0
    );
    admin.client().batch_execute("COMMIT").await.unwrap();
    let before = evidence::snapshot(admin).await;
    let mut owner = Admin::connect(endpoint).await;
    let observe = owner.database_observer();
    let mut snapshot = owner.read_snapshot(&expected, deadline()).await.unwrap();
    let preparation = snapshot
        .prepare_item_create(&current.id.0, &handle_key)
        .await
        .unwrap_or_else(|_| panic!("fresh creation preparation failed"));
    snapshot.rollback().await.unwrap();
    let ItemPreparation::Ready(draft) = preparation else {
        panic!("fresh creation must be ready")
    };
    let accepted = (*draft).admit().unwrap();
    assert_eq!(accepted.request_bytes(), original.as_bytes());
    let SavedResult::Create(result) = &accepted.revision().result else {
        panic!()
    };
    assert_eq!(result.item.id.0, target);
    assert_eq!(result.item.version, 1);
    assert_eq!(result.readable_id.sequence, 21);
    assert_eq!(result.project.next_sequence, 22);
    drop(accepted);
    drop(original);
    let SavedResult::Edit(latest) = edited else {
        panic!()
    };
    assert_eq!(latest.item.version, 11);
    let edit = EditCommand {
        operation_id: "factory-edit".into(),
        expected_item_version: 11,
        expected_configuration_revision: configuration.revision,
        estimate: EstimateEdit::Keep,
        fields: vec![],
    };
    let canonical = std::sync::Arc::new(
        CanonicalItem::admit(
            &expected.installation,
            &expected.owner,
            Request::Edit {
                project_id: current.id.clone(),
                item_id: latest.item.id.clone(),
                command: edit,
            },
        )
        .unwrap(),
    );
    let mut snapshot = owner.read_snapshot(&expected, deadline()).await.unwrap();
    let start = observe();
    let preparation = snapshot
        .prepare_item_edit(canonical)
        .await
        .unwrap_or_else(|_| panic!("fresh edit preparation failed"));
    assert!(
        observe() - start > 1,
        "Ready control must observe current-state reads"
    );
    snapshot.rollback().await.unwrap();
    let ItemPreparation::Ready(draft) = preparation else {
        panic!("unseen edit must be ready")
    };
    let accepted = (*draft).admit().unwrap();
    let SavedResult::Edit(result) = &accepted.revision().result else {
        panic!()
    };
    assert_eq!(result.item.version, 12);
    assert_eq!(result.item.type_id, latest.item.type_id);
    assert_eq!(result.item.workflow_id, latest.item.workflow_id);
    assert_eq!(result.item.status_id, latest.item.status_id);
    assert_eq!(result.project.next_sequence, 21);
    assert!(result.project.ever_estimated);
    drop(accepted);
    let create_operation=admin.client().query_one("SELECT creation_operation_row_id FROM kehila.items WHERE project_id=$1::text::kehila.ident AND item_id='read-fixture-1'",&[&current.id.0]).await.unwrap().get::<_,i64>(0);
    let mut snapshot = owner.read_snapshot(&expected, deadline()).await.unwrap();
    let start = observe();
    let preparation = snapshot
        .prepare_item_create(&current.id.0, &format!("{:032x}", 1))
        .await
        .unwrap_or_else(|_| panic!("historical creation lookup failed"));
    assert_eq!(
        observe() - start,
        1,
        "historical creation must not materialize Current"
    );
    snapshot.rollback().await.unwrap();
    let ItemPreparation::Resolved(value) = preparation else {
        panic!("existing creation must not prepare again")
    };
    assert_eq!(value.outcome(), ItemDisposition::Replay);
    assert_eq!(value.operation(), Some(create_operation));
    assert_eq!(value.target(), Some("read-fixture-1"));
    // Bound bytea before the driver receives it; verify the exact stored request.
    let row=admin.client().query_one("SELECT o.operation_row_id,o.request_codec_version,o.request_sha256,CASE WHEN octet_length(p.request_bytes) BETWEEN 1 AND 1048576 THEN p.request_bytes END FROM kehila.operations o JOIN kehila.operation_payloads p USING(operation_row_id) WHERE o.actor_id='owner' AND o.command_family='item_edit' AND o.target_project_id=$1::text::kehila.ident AND o.target_item_id='read-fixture-2' AND o.token=$2::text::kehila.operation_token LIMIT 1",&[&current.id.0,&format!("{:032x}",1010)]).await.unwrap();
    assert_eq!(row.get::<_, i64>(1), 1);
    let digest: [u8; 32] = row.get::<_, Vec<u8>>(2).try_into().unwrap();
    let bytes: Vec<u8> = row
        .get::<_, Option<Vec<u8>>>(3)
        .expect("bounded request required");
    let request = work_item_codec::verify_request(1, &bytes, &digest).unwrap();
    let edit_operation = row.get::<_, i64>(0);
    drop(row);
    drop(bytes);
    let Request::Edit {
        project_id,
        item_id,
        command,
    } = &request
    else {
        panic!()
    };
    assert_eq!(project_id, &current.id);
    assert_eq!(item_id.0, "read-fixture-2");
    assert_eq!(command.operation_id, format!("{:032x}", 1010));
    assert_eq!(command.expected_item_version, 10);
    for conflict in [false, true] {
        let mut request = request.clone();
        if conflict {
            let Request::Edit { command, .. } = &mut request else {
                panic!()
            };
            command.expected_item_version = 999;
        }
        let canonical = std::sync::Arc::new(
            CanonicalItem::admit(&expected.installation, &expected.owner, request).unwrap(),
        );
        let mut snapshot = owner.read_snapshot(&expected, deadline()).await.unwrap();
        let start = observe();
        let preparation = snapshot
            .prepare_item_edit(canonical)
            .await
            .unwrap_or_else(|_| panic!("historical edit lookup failed"));
        assert_eq!(
            observe() - start,
            1,
            "resolved edit must not materialize Current"
        );
        snapshot.rollback().await.unwrap();
        let ItemPreparation::Resolved(value) = preparation else {
            panic!("existing edit identity must not prepare")
        };
        assert_eq!(
            value.outcome(),
            if conflict {
                ItemDisposition::Conflict
            } else {
                ItemDisposition::Replay
            }
        );
        assert_eq!(value.operation(), Some(edit_operation));
        assert_eq!(value.target(), Some("read-fixture-2"));
    }
    let mut snapshot = owner.read_snapshot(&expected, deadline()).await.unwrap();
    let start = observe();
    let preparation = snapshot
        .prepare_item_create(&current.id.0, &"d".repeat(32))
        .await
        .unwrap_or_else(|_| panic!("missing locator lookup failed"));
    assert_eq!(observe() - start, 1);
    snapshot.rollback().await.unwrap();
    let ItemPreparation::Resolved(value) = preparation else {
        panic!()
    };
    assert_eq!(value.outcome(), ItemDisposition::Missing);
    let mut snapshot = owner.read_snapshot(&expected, deadline()).await.unwrap();
    let result = snapshot
        .prepare_item_create("missing-scope", &handle_key)
        .await;
    assert!(
        matches!(result,Err(crate::postgres::transaction::NativeError::Database(crate::postgres::DbError::Rejected(ref code))) if code=="42501")
    );
    snapshot.rollback().await.unwrap();
    let wrong_scope = std::sync::Arc::new(
        CanonicalItem::admit(&expected.installation, "other-actor", request.clone()).unwrap(),
    );
    let mut snapshot = owner.read_snapshot(&expected, deadline()).await.unwrap();
    let start = observe();
    assert!(matches!(
        snapshot.prepare_item_edit(wrong_scope).await,
        Err(crate::postgres::transaction::NativeError::Integrity)
    ));
    assert_eq!(
        observe() - start,
        0,
        "misbound internal scope must not reach SQL"
    );
    snapshot.rollback().await.unwrap();
    factory_clock::exercise(&mut owner, admin, &expected, &current.id.0, &handle_key).await;
    assert!(
        evidence::snapshot(admin).await == before,
        "preparation or pure admission changed durable artifacts"
    );
    drop(before);
    admin.client().batch_execute("BEGIN").await.unwrap();
    let cancel=format!("SELECT state FROM kehila_auth.cancel_item({AUTHORITY},$1::text::kehila.ident,$2::text::kehila_auth.intent_key)");
    assert_eq!(
        admin
            .client()
            .query_one(&cancel, &[&current.id.0, &handle_key])
            .await
            .unwrap()
            .get::<_, String>(0),
        "cancelled"
    );
    admin.client().batch_execute("COMMIT").await.unwrap();
    let mut snapshot = owner.read_snapshot(&expected, deadline()).await.unwrap();
    let start = observe();
    let preparation = snapshot
        .prepare_item_create(&current.id.0, &handle_key)
        .await
        .unwrap_or_else(|_| panic!("cancelled locator lookup failed"));
    assert_eq!(observe() - start, 1);
    snapshot.rollback().await.unwrap();
    let ItemPreparation::Resolved(value) = preparation else {
        panic!()
    };
    assert_eq!(value.outcome(), ItemDisposition::Cancelled);
    assert_eq!(value.target(), Some(target));
    owner.close().await;
}

async fn owner_fixture_transactions(handle: &Handle, endpoint: &crate::postgres::Endpoint) {
    let expected = {
        let engine = handle.engine.upgrade().unwrap();
        let database = engine.database.lock().await;
        let authority = database.authority().unwrap();
        crate::postgres::Authority {
            installation: authority.installation.clone(),
            owner: authority.owner.clone(),
            runtime: authority.runtime,
            authentication: authority.authentication,
            revision: authority.revision,
        }
    };
    let mut owner = Admin::connect(endpoint).await;
    let stale = crate::postgres::Authority {
        installation: expected.installation.clone(),
        owner: expected.owner.clone(),
        runtime: expected.runtime,
        authentication: expected.authentication.checked_add(1).unwrap(),
        revision: expected.revision,
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    assert!(
        owner.read_snapshot(&stale, deadline).await.is_err(),
        "fixture must not replace expected authority with fresh credentials"
    );
    let snapshot = owner.read_snapshot(&expected, deadline).await.unwrap();
    assert_eq!(snapshot.authority().owner, expected.owner);
    snapshot.rollback().await.unwrap();
    let transaction = owner.write_transaction(&expected, deadline).await.unwrap();
    assert_eq!(transaction.authority().runtime, expected.runtime);
    transaction.rollback().await.unwrap();
    assert!(
        owner
            .item_write_transaction(&stale, deadline)
            .await
            .is_err(),
        "fresh item ownership must preserve expected credentials"
    );
    let transaction: crate::postgres::transaction::Fenced<
        '_,
        crate::postgres::transaction::ItemWriteProof,
    > = owner
        .item_write_transaction(&expected, deadline)
        .await
        .unwrap();
    assert_eq!(transaction.authority().revision, expected.revision);
    transaction.rollback().await.unwrap();
    let abandoned = owner.read_snapshot(&expected, deadline).await.unwrap();
    drop(abandoned);
    assert!(
        matches!(
            owner.read_snapshot(&expected, deadline).await,
            Err(crate::postgres::DbError::Unavailable)
        ),
        "abandonment must permanently close this fixture connection's typed admission"
    );
    assert!(
        matches!(
            owner.write_transaction(&expected, deadline).await,
            Err(crate::postgres::DbError::Unavailable)
        ),
        "switching proof mode must not rehabilitate the connection"
    );
    assert!(
        matches!(
            owner.item_write_transaction(&expected, deadline).await,
            Err(crate::postgres::DbError::Unavailable)
        ),
        "fresh item mode cannot rehabilitate an abandoned owner"
    );
    owner.close().await;
}

async fn assert_preparation_witness(
    handle: &Handle,
    admin: &Admin,
    current: &Project,
    configuration: &Configuration,
    configuration_project: &Project,
    state: u64,
) {
    // This fixture is quiescent; the owner query is independent expected state.
    let head=admin.client().query_one("SELECT h.state_version::numeric(20,0)::text,h.operation_row_id,c.operation_row_id FROM kehila.project_operational_heads h JOIN kehila.projects p USING(project_id) JOIN kehila.configuration_revisions c ON c.project_id=p.project_id AND c.revision=p.configuration_revision WHERE h.project_id=$1::text::kehila.ident",&[&current.id.0]).await.unwrap();
    assert_eq!(head.get::<_, String>(0).parse::<u64>().unwrap(), state);
    let engine = handle.engine.upgrade().unwrap();
    let mut database = engine.database.lock().await;
    let authority = database.authority().unwrap();
    let installation = authority.installation.clone();
    let generation = authority.runtime;
    let authentication = authority.authentication;
    let mut snapshot = database
        .read_snapshot(
            &installation,
            generation,
            authentication,
            std::time::Instant::now() + std::time::Duration::from_secs(5),
        )
        .await
        .unwrap();
    let proof = snapshot
        .read_project(&current.id.0)
        .await
        .unwrap_or_else(|_| panic!("verified preparation witness unavailable"));
    assert_eq!(&proof.project, current);
    assert_eq!(&proof.configuration, configuration);
    assert_eq!(&proof.configuration_project, configuration_project);
    assert_eq!(proof.configuration_operation, head.get::<_, i64>(2));
    assert_eq!(proof.state_version, state);
    assert_eq!(proof.state_operation, head.get::<_, i64>(1));
    if state == 0 {
        assert_eq!(proof.state_operation, proof.creation_operation);
        assert_eq!(proof.configuration_operation, proof.creation_operation);
    }
    snapshot.rollback().await.unwrap();
    drop(database);
    drop(engine);
    drop(proof);
    drop(head);
}

#[test]
#[ignore = "requires a fresh explicitly provisioned product PostgreSQL TLS fixture"]
fn native_authorized_item_reads() {
    let (endpoint, installation, attempt, controls) = fixture();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let (handle, owner) = runtime(&endpoint, &installation, attempt.clone()).await;
            let running = tokio::spawn(owner.run());
            let browser = login(&handle).await;
            let (status, pending) = project(
                &handle,
                &browser,
                "register",
                json!({"name":"Item read fixture","prefix":"IRD","estimate_unit":"hours"}),
            )
            .await;
            assert_eq!(status, 201);
            let entry = &pending["entry"];
            let (status, created) = project(&handle, &browser, "execute", execute(entry)).await;
            assert_eq!(status, 201);
            let result = project_create_codec::decode_result(
                1,
                &serde_json::to_vec(&created["result"]).unwrap(),
            )
            .unwrap();
            let id = result.project.id.0.clone();
            let (status, empty) = item(
                &handle,
                &browser,
                "list",
                json!({"project_id":id,"before":null}),
            )
            .await;
            assert_eq!(status, 200);
            assert_eq!(empty["entries"], json!([]));
            assert!(empty["before"].is_null());
            owner_fixture_transactions(&handle, &endpoint).await;
            let admin = Admin::connect(&endpoint).await;
            let mut current = result.project.clone();
            assert_preparation_witness(
                &handle,
                &admin,
                &current,
                &result.configuration,
                &result.project,
                0,
            )
            .await;
            let mut first = None;
            let mut second = None;
            for index in 1..=20 {
                let saved = seed(&admin, &current, &result.configuration, index).await;
                let SavedResult::Create(value) = &saved else {
                    unreachable!()
                };
                current = value.project.clone();
                if index == 1 {
                    first = Some(saved.clone());
                }
                if index == 2 {
                    second = Some(saved);
                }
            }
            assert!(current.ever_estimated);
            let (status, zero) = item(
                &handle,
                &browser,
                "read",
                json!({"project_id":id,"item_id":"read-fixture-2"}),
            )
            .await;
            assert_eq!(status, 200);
            assert_eq!(zero["result"]["value"]["data"]["estimate"], "0");
            let mut edited = second.unwrap();
            // Cross the decimal digit boundary: the current-version witness must
            // order numeric versions, not their projected text labels.
            for _ in 0..10 {
                edited = clear_estimate(&admin, &current, &result.configuration, &edited).await;
                let SavedResult::Edit(edited_value) = &edited else {
                    unreachable!()
                };
                current = edited_value.project.clone();
            }
            assert_preparation_witness(
                &handle,
                &admin,
                &current,
                &result.configuration,
                &result.project,
                30,
            )
            .await;
            factory_preparations(
                &handle,
                &endpoint,
                &admin,
                &current,
                &result.configuration,
                &edited,
            )
            .await;
            let (status, other) = project(
                &handle,
                &browser,
                "register",
                json!({"name":"Other read scope","prefix":"OTH","estimate_unit":"hours"}),
            )
            .await;
            assert_eq!(status, 201);
            let (status, other) =
                project(&handle, &browser, "execute", execute(&other["entry"])).await;
            assert_eq!(status, 201);
            let other_id = other["entry"]["project_id"].as_str().unwrap();
            let before = evidence::snapshot(&admin).await;
            let (status, read) = item(
                &handle,
                &browser,
                "read",
                json!({"project_id":id,"item_id":"read-fixture-1"}),
            )
            .await;
            assert_eq!(status, 200);
            assert_eq!(
                work_item_codec::decode_result(1, &serde_json::to_vec(&read["result"]).unwrap())
                    .unwrap(),
                first.unwrap()
            );
            let (status, read) = item(
                &handle,
                &browser,
                "read",
                json!({"project_id":id,"item_id":"read-fixture-2"}),
            )
            .await;
            assert_eq!(status, 200);
            assert_eq!(
                work_item_codec::decode_result(1, &serde_json::to_vec(&read["result"]).unwrap())
                    .unwrap(),
                edited
            );
            assert_eq!(read["result"]["value"]["item"]["version"], "11");
            assert_eq!(read["readable_id"]["sequence"], "2");
            let (status, page) = item(
                &handle,
                &browser,
                "list",
                json!({"project_id":id,"before":null}),
            )
            .await;
            assert_eq!(status, 200);
            assert_eq!(page["entries"].as_array().unwrap().len(), 10);
            assert_eq!(page["before"], "11");
            assert_eq!(page["entries"][0]["item_id"], "read-fixture-20");
            assert_eq!(
                page["entries"][0]["name"].as_str().unwrap(),
                "\u{1}".repeat(16 * 1024)
            );
            let (status, last) = item(
                &handle,
                &browser,
                "list",
                json!({"project_id":id,"before":page["before"]}),
            )
            .await;
            assert_eq!(status, 200);
            assert_eq!(last["entries"].as_array().unwrap().len(), 10);
            assert!(last["before"].is_null());
            for (index, value) in page["entries"]
                .as_array()
                .unwrap()
                .iter()
                .chain(last["entries"].as_array().unwrap())
                .enumerate()
            {
                let sequence = 20 - index;
                assert_eq!(value["item_id"], format!("read-fixture-{sequence}"));
                assert_eq!(value["readable_id"]["sequence"], sequence.to_string());
            }
            assert_eq!(last["entries"][8]["name"], "  milestone title  ");
            assert!(last["entries"][8]["estimate"].is_null());
            assert_eq!(last["entries"][9]["name"], "IRD-1");
            let (status, read) = project(&handle, &browser, "read", json!({"project_id":id})).await;
            assert_eq!(status, 200);
            assert_eq!(
                project_create_codec::decode_project_snapshot(
                    1,
                    &serde_json::to_vec(&read["project"]).unwrap()
                )
                .unwrap(),
                current
            );
            body::exercise(&handle, &browser, &json!(id), &read).await;
            let (status, replayed) = project(&handle, &browser, "execute", execute(entry)).await;
            assert_eq!(status, 200);
            assert_eq!(replayed["result"], created["result"]);
            assert_eq!(
                item(
                    &handle,
                    &browser,
                    "read",
                    json!({"project_id":id,"item_id":"missing"})
                )
                .await
                .0,
                404
            );
            let (status, denied) = item(
                &handle,
                &browser,
                "read",
                json!({"project_id":other_id,"item_id":"read-fixture-1"}),
            )
            .await;
            assert_eq!(status, 404);
            assert_eq!(denied, json!({"error":"not_found"}));
            let (status, denied) = item(
                &handle,
                &browser,
                "read",
                json!({"project_id":"missing-scope","item_id":"read-fixture-1"}),
            )
            .await;
            assert_eq!(status, 403);
            assert_eq!(denied, json!({"error":"forbidden"}));
            assert!(
                evidence::snapshot(&admin).await == before,
                "read changed database artifacts"
            );
            notices::exercise(&handle, &browser, &admin, &id, other_id).await;
            evidence::settled(&handle).await;
            evidence::fences_released(&admin).await;
            handle.stop();
            running.await.unwrap().unwrap();
            network::exercise(&endpoint, &installation, &attempt, &admin, &id, &read).await;
            faults::exercise(&endpoint, &installation, &attempt, &admin, &id).await;
            admin.close().await;
            mark(&controls, "item-read-passed");
        });
}
