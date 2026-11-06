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
mod faults;

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
            let admin = Admin::connect(&endpoint).await;
            let mut current = result.project.clone();
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
            evidence::settled(&handle).await;
            evidence::fences_released(&admin).await;
            handle.stop();
            running.await.unwrap().unwrap();
            faults::exercise(&endpoint, &installation, &attempt, &admin, &id).await;
            admin.close().await;
            mark(&controls, "item-read-passed");
        });
}
