//! Native validity controls for capacity inputs; these are not timed capacity cells.
mod matrix;
mod owned;
use super::*;
use crate::lifecycle::probe::capacity::Stage;
use crate::listener::tests::capacity::Client;
use crate::native_ingress::Form;
use crate::postgres::tests::fixture_admin::Admin;
use task_contract::field::FieldValue;
use task_contract::{project::EstimateUnit, project_create::ProjectCreateResult};
use work_item_codec::SavedResult;

async fn request(
    client: &Client,
    browser: &Browser,
    path: &str,
    body: Value,
    expected: u16,
) -> Value {
    let (status, headers, bytes) = client
        .request(wire_request(Some(browser), path, Some(body)))
        .await
        .expect("capacity HTTP/TLS exchange failed");
    let (status, value, _) = decode_exchange(status, headers, bytes);
    assert_eq!(status, expected, "capacity validity request failed: {path}");
    value
}

async fn login_client(client: &Client) -> Browser {
    let (status, headers, bytes) = client
        .request(wire_request(None, "/auth/browser/pre-session", None))
        .await
        .unwrap();
    let (status, value, cookie) = decode_exchange(status, headers, bytes);
    assert_eq!(status, 200);
    let pre = Browser {
        cookie: cookie.unwrap(),
        csrf: value["csrf"].as_str().unwrap().to_owned(),
    };
    let (status, headers, bytes) = client
        .request(wire_request(
            Some(&pre),
            "/auth/browser/login",
            Some(json!({"password":"a fixed test password"})),
        ))
        .await
        .unwrap();
    let (status, value, cookie) = decode_exchange(status, headers, bytes);
    assert_eq!(status, 200);
    Browser {
        cookie: cookie.unwrap(),
        csrf: value["csrf"].as_str().unwrap().to_owned(),
    }
}

async fn login_clients(client: &Client) -> (Vec<Browser>, Duration) {
    let mut browsers = Vec::new();
    let mut pacing = Duration::ZERO;
    for _ in 0..4 {
        // Existing pre-session policy: burst two, then one issuance per 12s.
        if browsers.len() >= 2 {
            let started = Instant::now();
            tokio::time::sleep(Duration::from_secs(12)).await;
            pacing += started.elapsed();
        }
        browsers.push(login_client(client).await);
    }
    for (index, first) in browsers.iter().enumerate() {
        assert!(browsers
            .iter()
            .skip(index + 1)
            .all(|other| other.cookie != first.cookie));
    }
    (browsers, pacing)
}

async fn initialize(
    handle: &Handle,
    client: &Client,
    browser: &Browser,
    profile: usize,
) -> ProjectCreateResult {
    let name = format!("Capacity validity {profile}");
    let prefix = ["CPC", "CPS", "CPL"][profile];
    let registered = request(
        client,
        browser,
        "/projects/register",
        json!({"name":name,"prefix":prefix,"estimate_unit":"hours"}),
        201,
    )
    .await;
    if profile == 0 {
        let created = request(
            client,
            browser,
            "/projects/execute",
            execute(&registered["entry"]),
            201,
        )
        .await;
        return project_create_codec::decode_result(
            1,
            &serde_json::to_vec(&created["result"]).unwrap(),
        )
        .unwrap();
    }
    store_synthetic(
        handle,
        browser,
        &registered,
        Form {
            name,
            prefix: prefix.into(),
            estimate_unit: EstimateUnit::Hours,
        },
        format!("{profile:032x}"),
        if profile == 1 { 4 } else { 128 },
    )
    .await
}

async fn store_synthetic(
    handle: &Handle,
    browser: &Browser,
    registered: &Value,
    form: Form,
    project_id: String,
    fields: usize,
) -> ProjectCreateResult {
    // Explicitly noncanonical test input. Product creation still uses Prepared::new.
    // Preserve the fenced writer, pending completion, grants and immutable history.
    let prepared = crate::project::Prepared::synthetic_capacity_fixture(
        form,
        project_id,
        registered["entry"]["token"].as_str().unwrap().to_owned(),
        fields,
    )
    .unwrap();
    let credential = Credential {
        bearer: Digest::from_encoded(browser.cookie.split_once('=').unwrap().1.as_bytes()).unwrap(),
        kind: Kind::Browser,
        csrf: Some(Digest::from_encoded(browser.csrf.as_bytes()).unwrap()),
    };
    let engine = handle.engine.upgrade().unwrap();
    let entry = engine.candidate(&credential).unwrap();
    let binding = &entry.binding;
    let mut database = engine.database.lock().await;
    let mut transaction = database
        .fenced_session(
            &binding.installation,
            binding.runtime_generation,
            binding.auth_generation,
            Instant::now() + WORK_DEADLINE,
        )
        .await
        .unwrap();
    let id = registered["entry"]["request_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let operation = transaction.write_project(id, &prepared).await.unwrap();
    transaction
        .complete_project(id, &prepared, operation)
        .await
        .unwrap();
    let reloaded = transaction
        .reload_created(&prepared.command().project_id.0)
        .await
        .unwrap();
    assert_eq!(reloaded.project, prepared.result().project);
    assert_eq!(reloaded.configuration, prepared.result().configuration);
    transaction.force_project_constraints().await.unwrap();
    transaction.commit().await.unwrap();
    prepared.result().clone()
}

async fn cases(handle: &Handle, client: &Client, admin: &Admin) -> usize {
    assert!(
        client
            .untrusted_for_control()
            .unwrap()
            .request(wire_request(None, "/auth/browser/pre-session", None))
            .await
            .is_err(),
        "empty trust store accepted fixture certificate"
    );
    let (browsers, _) = login_clients(client).await;
    let mut snapshots = Vec::new();
    for profile in 0..3 {
        let initialized = initialize(handle, client, &browsers[0], profile).await;
        let kind = &initialized.configuration.types[0];
        let fields = initialized
            .configuration
            .fields
            .iter()
            .filter(|field| field.owner_type == kind.id)
            .collect::<Vec<_>>();
        assert_eq!(fields.len(), [2, 4, 128][profile]);
        let bytes = if profile == 2 { 1024 } else { 64 };
        let project = &initialized.project.id.0;
        let registered = request(client, &browsers[0], "/items/pending/register", json!({
            "project_id":project,"type_id":kind.id.0,"workflow_id":null,
            "expected_configuration_revision":"1","estimate":null,
            "values":fields.iter().map(|field| json!({"id":field.id.0,"value":"a".repeat(bytes)})).collect::<Vec<_>>()
        }), 201).await;
        let created = request(
            client,
            &browsers[0],
            "/items/pending/execute",
            json!({
                "project_id":project,"intent_handle":registered["entry"]["intent_handle"]
            }),
            201,
        )
        .await;
        let SavedResult::Create(created) =
            work_item_codec::decode_result(1, &serde_json::to_vec(&created["result"]).unwrap())
                .unwrap()
        else {
            panic!("create family");
        };
        assert_eq!(created.data.values.len(), fields.len());
        for field in &fields {
            assert!(created
                .data
                .values
                .iter()
                .any(|entry| entry.id == field.id
                    && entry.value == FieldValue::Text("a".repeat(bytes))));
        }
        evidence::settled(handle).await;
        handle.live.probe.capacity.start(2, 128);
        let edited = request(client, &browsers[1], "/items/edit", json!({
            "project_id":project,"item_id":created.item.id.0,"operation_id":format!("{:032x}",profile + 100),
            "expected_item_version":"1","expected_configuration_revision":"1","estimate":{"action":"keep"},
            "fields":fields.iter().map(|field| json!({"id":field.id.0,"action":"set","value":"b".repeat(bytes)})).collect::<Vec<_>>()
        }), 200).await;
        let SavedResult::Edit(edited) =
            work_item_codec::decode_result(1, &serde_json::to_vec(&edited["result"]).unwrap())
                .unwrap()
        else {
            panic!("edit family");
        };
        assert_eq!(edited.item.version, 2);
        assert_eq!(edited.data.values.len(), fields.len());
        for field in &fields {
            assert!(edited
                .data
                .values
                .iter()
                .any(|entry| entry.id == field.id
                    && entry.value == FieldValue::Text("b".repeat(bytes))));
        }
        let read = request(
            client,
            &browsers[2],
            "/items/read",
            json!({"project_id":project,"item_id":created.item.id.0}),
            200,
        )
        .await;
        let read = work_item_codec::decode_result(1, &serde_json::to_vec(&read["result"]).unwrap())
            .unwrap();
        let expected = SavedResult::Edit(edited);
        assert_eq!(read, expected);
        snapshots.push((project.clone(), created.item.id.0.clone(), expected));
        evidence::settled(handle).await;
        let rows = handle.live.probe.capacity.finish().unwrap();
        assert_eq!(rows.len(), 2);
        for row in &rows {
            assert_eq!(
                row.events
                    .iter()
                    .filter(|event| event.stage == Stage::Execution)
                    .count(),
                1
            );
            assert!(row
                .events
                .iter()
                .any(|event| event.stage == Stage::Queue && event.completed));
        }
        assert_eq!(
            rows[0]
                .events
                .iter()
                .filter(|event| event.stage == Stage::CpuWork && event.completed)
                .count(),
            3 // Canonical command, notice catalogue, state-dependent admission.
        );
        for event in rows[0]
            .events
            .iter()
            .filter(|event| event.stage == Stage::CpuWork)
        {
            assert!(
                event.cpu_time_ns.is_some(),
                "native CPU work lacked thread measurement"
            );
        }
        assert!(rows[1]
            .events
            .iter()
            .all(|event| event.cpu_time_ns.is_none()));
        for stage in [
            Stage::PrepareMutex,
            Stage::PrepareService,
            Stage::WriteMutex,
            Stage::WriteService,
        ] {
            assert!(rows[0]
                .events
                .iter()
                .any(|event| event.stage == stage && event.completed));
        }
        for stage in [Stage::ReadMutex, Stage::ReadService] {
            assert!(rows[1]
                .events
                .iter()
                .any(|event| event.stage == stage && event.completed));
        }
        evidence::fences_released(admin).await;
    }
    // Submission uses a one-entry nonblocking channel. A burst may therefore
    // reject requests even below the sixteen-permit limit. Preserve that result:
    // this checks inputs/observations, not the accepted four-inflight target.
    // Server tasks are independent of the client futures; never retry a reject.
    handle.live.probe.capacity.start(4, 128);
    let read = |slot: usize| {
        let expected = &snapshots[slot % snapshots.len()];
        let browser = &browsers[slot];
        async move {
            let exchange = client
                .request(wire_request(
                    Some(browser),
                    "/items/read",
                    Some(json!({"project_id":expected.0,"item_id":expected.1})),
                ))
                .await;
            (exchange, &expected.2)
        }
    };
    // Finish every exchange before asserting, so a failed oracle cannot cancel
    // sibling clients and manufacture transport-cleanup failures.
    let (a, b, c, d) = tokio::join!(read(0), read(1), read(2), read(3));
    evidence::settled(handle).await;
    let rows = handle.live.probe.capacity.finish().unwrap();
    assert_eq!(rows.len(), 4);
    let mut busy = 0;
    for (exchange, expected) in [a, b, c, d] {
        let (status, headers, bytes) =
            exchange.expect("concurrent capacity HTTP/TLS exchange failed");
        let (status, value, _) = decode_exchange(status, headers, bytes);
        match status {
            200 => assert_eq!(
                work_item_codec::decode_result(1, &serde_json::to_vec(&value["result"]).unwrap())
                    .unwrap(),
                *expected
            ),
            429 => busy += 1,
            _ => panic!("unexpected concurrent read status: {status}"),
        }
    }
    assert!(busy < 4, "no concurrent read executed");
    let mut submission_busy = 0;
    for row in rows {
        let count = |stage| {
            row.events
                .iter()
                .filter(|event| event.stage == stage)
                .count()
        };
        assert_eq!(count(Stage::Admission), 1);
        assert_eq!(count(Stage::AdmissionAccepted), 1);
        assert_eq!(count(Stage::Submission), 1);
        assert_eq!(count(Stage::AdmissionBusy), 0);
        assert_eq!(count(Stage::AdmissionUnavailable), 0);
        assert_eq!(count(Stage::SubmissionUnavailable), 0);
        assert!(row.events.iter().all(|event| matches!(
            event.stage,
            Stage::Admission
                | Stage::AdmissionAccepted
                | Stage::Submission
                | Stage::SubmissionBusy
                | Stage::Queue
                | Stage::Execution
                | Stage::ReadMutex
                | Stage::ReadService
        )));
        assert_eq!(count(Stage::Queue), 1);
        match count(Stage::SubmissionBusy) {
            1 => {
                submission_busy += 1;
                assert_eq!(count(Stage::Execution), 0);
                assert_eq!(count(Stage::ReadMutex), 0);
                assert_eq!(count(Stage::ReadService), 0);
                assert!(row
                    .events
                    .iter()
                    .filter(|event| event.stage == Stage::Queue)
                    .all(|event| !event.completed));
            }
            0 => {
                assert_eq!(count(Stage::Execution), 1);
                for stage in [Stage::Queue, Stage::ReadMutex, Stage::ReadService] {
                    assert_eq!(count(stage), 1);
                    assert_eq!(
                        row.events
                            .iter()
                            .filter(|event| event.stage == stage && event.completed)
                            .count(),
                        1
                    );
                }
            }
            _ => panic!("multiple submission rejections in one request"),
        }
    }
    // Aggregate agreement only: numeric collector IDs are not HTTP identities.
    assert_eq!(submission_busy, busy);
    assert!(handle.live.is_live());
    // Separate untimed recovery control; this does not replace any burst result.
    let expected = &snapshots[0];
    let value = request(
        client,
        &browsers[0],
        "/items/read",
        json!({"project_id":expected.0,"item_id":expected.1}),
        200,
    )
    .await;
    assert_eq!(
        work_item_codec::decode_result(1, &serde_json::to_vec(&value["result"]).unwrap()).unwrap(),
        expected.2
    );
    evidence::settled(handle).await;
    evidence::fences_released(admin).await;
    busy
}

#[test]
#[ignore = "requires a fresh explicitly provisioned product PostgreSQL TLS fixture"]
fn native_capacity_input_and_observer_validity() {
    let (endpoint, installation, attempt, controls) = fixture();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let (busy, connections) =
                owned::run(
                    &endpoint,
                    &installation,
                    format!("{attempt}-capacity"),
                    |jobs, client, admin| async move {
                        cases(&jobs, &client, &*admin.lock().await).await
                    },
                )
                .await;
            eprintln!("capacity input validity: four-inflight read submission rejections={busy}");
            assert_eq!(connections.accepted, 30);
            assert_eq!(connections.completed, 29);
            assert_eq!(connections.protocol_rejected, 1);
            assert_eq!(connections.tls_rejected, 1);
            assert_eq!(connections.capacity_rejected, 0);
            assert_eq!(connections.timed_out, 0);
            mark(&controls, "item-capacity-passed");
        });
}
