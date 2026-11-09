//! Finite storage observations. Historical retirement is a separate owner fixture.
use super::*;
use serde::Serialize;
use task_contract::field::{FieldEdit, FieldEntry, FieldId};
use task_contract::item_mutation::{
    decide_create, decide_edit, CreateCommand, CreateDecision, EditCommand, EditDecision,
    EstimateEdit, FieldUpdate,
};
use task_contract::work_item::{ProjectId, WorkItemId};

const CENSUS: &str =
    include_str!("../../../../../../../../../tests/integration/storage_growth_observer.sql");
const HISTORICAL: &str =
    include_str!("../../../../../../../../../tests/integration/storage_growth_historical.sql");

#[derive(Clone, Copy)]
struct Sample {
    index: usize,
    fields: usize,
    bytes: usize,
    repeated: bool,
    all_fields: bool,
    data_limit_mib: usize,
}

impl Sample {
    fn new(index: usize) -> Self {
        assert!(index < 24, "storage sample outside finite plan");
        let input =
            std::env::var("KEHILA_STORAGE_DATA_MIB").expect("explicit storage data bound required");
        let data_limit_mib = input.parse::<usize>().unwrap();
        assert_eq!(data_limit_mib.to_string(), input);
        assert!(
            data_limit_mib == 256 || (data_limit_mib == 512 && index >= 20),
            "unknown storage resource profile"
        );
        Self {
            index,
            fields: if index < 8 { 4 } else { 128 },
            // Pure candidate, checked against ingress/native codecs before use
            // and the actual rendered PostgreSQL history after every insertion.
            bytes: [64, 1024, 7600][index / 8],
            repeated: index / 4 % 2 == 0,
            all_fields: index / 2 % 2 == 1,
            data_limit_mib,
        }
    }

    fn text(self, version: usize, field: usize) -> String {
        if self.repeated {
            return char::from(b'a' + (version % 26) as u8)
                .to_string()
                .repeat(self.bytes);
        }
        let mut text = String::with_capacity(self.bytes + 64);
        use std::fmt::Write;
        for block in 0..self.bytes.div_ceil(64) {
            let input = format!("storage:{}:{version}:{field}:{block}", self.index);
            for byte in ring::digest::digest(&ring::digest::SHA256, input.as_bytes()).as_ref() {
                write!(text, "{byte:02x}").unwrap();
            }
        }
        text.truncate(self.bytes);
        text
    }
}

#[derive(Serialize)]
struct Expected {
    version: i32,
    token: String,
    request_hex: String,
    result: Value,
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(encoded, "{byte:02x}").unwrap();
    }
    encoded
}

fn unhex(encoded: &str) -> Vec<u8> {
    let mut decoded = vec![0; data_encoding::HEXLOWER.decode_len(encoded.len()).unwrap()];
    let length = data_encoding::HEXLOWER
        .decode_mut(encoded.as_bytes(), &mut decoded)
        .unwrap();
    decoded.truncate(length);
    decoded
}

fn write_once(folder: &std::path::Path, name: &str, value: &impl Serialize) {
    use std::io::Write;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(folder.join(name))
        .unwrap();
    let mut writer = std::io::BufWriter::new(file);
    serde_json::to_writer(&mut writer, value).unwrap();
    writer.flush().unwrap();
}

fn expected(
    version: i32,
    token: String,
    request: work_item_codec::Request,
    result: SavedResult,
) -> Expected {
    let request = work_item_codec::admit_request(&request).unwrap();
    let result = work_item_codec::admit_result(&result).unwrap();
    Expected {
        version,
        token,
        request_hex: hex(request.as_bytes()),
        result: serde_json::from_slice(result.as_bytes()).unwrap(),
    }
}

async fn admit(admin: &Admin) {
    let row = admin
        .client()
        .query_one(
            "SELECT (SELECT rolsuper FROM pg_roles WHERE rolname=current_user), \
         (SELECT count(*) FROM kehila.projects), \
         (SELECT count(*) FROM kehila.item_revisions)",
            &[],
        )
        .await
        .unwrap();
    assert!(
        row.get::<_, bool>(0),
        "storage census requires isolated fixture administrator"
    );
    assert_eq!(row.get::<_, i64>(1), 0);
    assert_eq!(row.get::<_, i64>(2), 0);
    admin
        .client()
        .batch_execute("SET statement_timeout='15s'; SET lock_timeout='3s'")
        .await
        .unwrap();
}

async fn headroom(admin: &Admin, sample: Sample) {
    // Whole cluster allocation, including the pristine template and both roles.
    // Leave 64 MiB inside the explicitly admitted data/memory bound before a write.
    let row = admin
        .client()
        .query_one(
            "SELECT (SELECT sum(pg_database_size(oid))::bigint FROM pg_database) \
          + (SELECT coalesce(sum(size),0)::bigint FROM pg_ls_waldir())",
            &[],
        )
        .await
        .unwrap();
    assert!(
        row.get::<_, i64>(0) < ((sample.data_limit_mib - 64) * 1024 * 1024) as i64,
        "storage sample stop margin reached"
    );
}

async fn checkpoint(
    admin: &Admin,
    folder: &std::path::Path,
    role: &str,
    stage: &str,
    records: &[Expected],
    sample: Sample,
    retired: bool,
    pruned: bool,
) {
    let count = admin
        .client()
        .query_one(
            "SELECT (SELECT count(*) FROM kehila.item_revisions), \
          (SELECT count(*) FROM kehila.items), (SELECT count(*) FROM kehila.operations), \
          (SELECT count(*) FROM kehila.item_field_uses)",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(count.get::<_, i64>(0), records.len() as i64);
    assert_eq!(count.get::<_, i64>(1), i64::from(!records.is_empty()));
    assert_eq!(count.get::<_, i64>(2), records.len() as i64 + 1);
    assert_eq!(
        count.get::<_, i64>(3),
        if records.is_empty() {
            0
        } else {
            sample.fields as i64
        }
    );
    let project = admin
        .client()
        .query_one("SELECT creation_operation_row_id FROM kehila.projects", &[])
        .await
        .unwrap();
    let mut predecessor: i64 = project.get(0);
    for record in records {
        let row = admin
            .client()
            .query_one(
                "SELECT r.history::text,octet_length(r.history::text),r.operation_row_id, \
              o.payload_retired,o.request_sha256,p.request_bytes,p.result::text, \
              o.actor_id::text,o.target_project_id::text,o.target_item_id::text, \
              o.command_family,o.replay_origin_ms::text, \
              r.previous_item_version::text,r.project_state_version::text,r.token::text \
             FROM kehila.item_revisions r JOIN kehila.operations o USING(operation_row_id) \
             LEFT JOIN kehila.operation_payloads p USING(operation_row_id) \
             WHERE r.item_version=$1::int",
                &[&record.version],
            )
            .await
            .unwrap();
        let expected_history = json!({"codec_version":1,"kind":"item_revision","value":{
            "actor_id":"owner","configuration_revision":"1",
            "project_state_version":record.version.to_string(),
            "previous_project_operation":predecessor.to_string(),"result":record.result}});
        assert_eq!(
            serde_json::from_str::<Value>(&row.get::<_, String>(0)).unwrap(),
            expected_history
        );
        let bytes: i32 = row.get(1);
        assert!(bytes <= 1_048_576 && (sample.index < 16 || bytes >= 943_719));
        predecessor = row.get(2);
        assert_eq!(row.get::<_, bool>(3), retired);
        let raw = unhex(&record.request_hex);
        assert_eq!(
            row.get::<_, Vec<u8>>(4),
            ring::digest::digest(&ring::digest::SHA256, &raw).as_ref()
        );
        assert_eq!(row.get::<_, Option<Vec<u8>>>(5), (!retired).then_some(raw));
        assert_eq!(
            row.get::<_, Option<String>>(6)
                .map(|text| serde_json::from_str::<Value>(&text).unwrap()),
            (!retired).then(|| record.result.clone())
        );
        assert_eq!(row.get::<_, String>(7), "owner");
        assert_eq!(
            row.get::<_, String>(8),
            record.result["value"]["project"]["id"].as_str().unwrap()
        );
        assert_eq!(
            row.get::<_, String>(9),
            record.result["value"]["item"]["id"].as_str().unwrap()
        );
        assert_eq!(
            row.get::<_, String>(10),
            if record.version == 1 {
                "item_create"
            } else {
                "item_edit"
            }
        );
        if role == "historical" {
            assert_eq!(row.get::<_, String>(11), "0");
        }
        assert_eq!(
            row.get::<_, Option<String>>(12),
            (record.version > 1).then(|| (record.version - 1).to_string())
        );
        assert_eq!(row.get::<_, String>(13), record.version.to_string());
        assert_eq!(row.get::<_, String>(14), record.token);
        work_item_codec::decode_revision(1, &serde_json::to_vec(&expected_history).unwrap())
            .unwrap();
    }
    if !records.is_empty() {
        let row = admin
            .client()
            .query_one(
                "SELECT i.current_version::text,h.state_version::text,h.operation_row_id, \
              i.issued_sequence::text,i.creation_operation_row_id=p.operation_row_id, \
              p.state,p.request_bytes,i.item_id=p.target_item_id \
             FROM kehila.items i JOIN kehila.project_operational_heads h USING(project_id) \
             JOIN kehila_auth.pending_items p ON p.request_id=i.creation_request_id",
                &[],
            )
            .await
            .unwrap();
        assert_eq!(row.get::<_, String>(0), records.len().to_string());
        assert_eq!(row.get::<_, String>(1), records.len().to_string());
        assert_eq!(row.get::<_, i64>(2), predecessor);
        assert_eq!(row.get::<_, String>(3), "1");
        assert!(row.get::<_, bool>(4) && row.get::<_, bool>(7));
        assert_eq!(row.get::<_, String>(5), "completed");
        let request = unhex(&records[0].request_hex);
        assert_eq!(
            row.get::<_, Option<Vec<u8>>>(6),
            (!pruned).then_some(request)
        );
    }
    let query = format!(
        "SELECT census::text FROM ({}) AS census_record(census)",
        CENSUS.trim().trim_end_matches(';')
    );
    let census = admin
        .client()
        .query_one(&query, &[])
        .await
        .unwrap()
        .get::<_, String>(0);
    let census: Value = serde_json::from_str(&census).unwrap();
    write_once(folder, &format!("{role}-{stage}.json"), &census);
}

async fn native(
    handle: &Handle,
    client: &Client,
    admin: &Admin,
    folder: &std::path::Path,
    sample: Sample,
) -> Vec<Expected> {
    admit(admin).await;
    let browser = login_client(client).await;
    let form = Form {
        name: "Storage project".into(),
        prefix: "ST".into(),
        estimate_unit: EstimateUnit::Hours,
    };
    let registered = request(
        client,
        &browser,
        "/projects/register",
        json!({"name":form.name,"prefix":form.prefix,"estimate_unit":"hours"}),
        201,
    )
    .await;
    let project = store_synthetic(
        handle,
        &browser,
        &registered,
        form,
        "1".repeat(32),
        sample.fields,
    )
    .await;
    let kind = &project.configuration.types[0];
    let mut values = project
        .configuration
        .fields
        .iter()
        .map(|field| FieldEntry {
            id: field.id.clone(),
            value: FieldValue::Text(sample.text(0, values_index(&project, &field.id))),
        })
        .collect::<Vec<_>>();
    let body = json!({"project_id":project.project.id.0,"type_id":kind.id.0,"workflow_id":null,
        "expected_configuration_revision":"1","estimate":null,
        "values":values.iter().map(|field| {
            let FieldValue::Text(value)=&field.value else { panic!("text profile"); };
            json!({"id":field.id.0,"value":value})
        }).collect::<Vec<_>>()});
    assert!(serde_json::to_vec(&body).unwrap().len() <= 1_048_576);
    checkpoint(
        admin,
        folder,
        "native",
        "baseline",
        &[],
        sample,
        false,
        false,
    )
    .await;
    headroom(admin, sample).await;
    let registered = request(client, &browser, "/items/pending/register", body, 201).await;
    let token = registered["entry"]["token"].as_str().unwrap().to_owned();
    let item = WorkItemId(
        registered["entry"]["target_item_id"]
            .as_str()
            .unwrap()
            .to_owned(),
    );
    let command = CreateCommand {
        operation_id: token.clone(),
        expected_configuration_revision: 1,
        item_id: item.clone(),
        type_id: kind.id.clone(),
        workflow_id: None,
        estimate: None,
        values: values.clone(),
    };
    let CreateDecision::Apply(created) = decide_create(
        &project.project,
        &project.configuration,
        None,
        true,
        &command,
    ) else {
        panic!("synthetic create must be valid");
    };
    let first = expected(
        1,
        token,
        work_item_codec::Request::Create {
            project_id: ProjectId(project.project.id.0.clone()),
            command,
        },
        SavedResult::Create(created.clone()),
    );
    let response = request(client,&browser,"/items/pending/execute",
        json!({"project_id":project.project.id.0,"intent_handle":registered["entry"]["intent_handle"]}),201).await;
    assert_eq!(response["result"], first.result);
    let mut records = vec![first];
    checkpoint(
        admin, folder, "native", "create", &records, sample, false, false,
    )
    .await;
    let mut current_item = created.item;
    let mut current_data = created.data;
    for edit in 1..=20 {
        headroom(admin, sample).await;
        let fields = (0..sample.fields)
            .filter(|index| sample.all_fields || *index == 0)
            .map(|index| FieldUpdate {
                id: values[index].id.clone(),
                edit: FieldEdit::Set(FieldValue::Text(sample.text(edit, index))),
            })
            .collect::<Vec<_>>();
        let token = format!("{:032x}", 1000 + edit);
        let body = json!({"project_id":project.project.id.0,"item_id":item.0,"operation_id":token,
            "expected_item_version":edit.to_string(),"expected_configuration_revision":"1",
            "estimate":{"action":"keep"},"fields":fields.iter().map(|field| {
                let FieldEdit::Set(FieldValue::Text(value))=&field.edit else { panic!("text edit"); };
                json!({"id":field.id.0,"action":"set","value":value})
            }).collect::<Vec<_>>()});
        crate::item_mutation_ingress::decode(
            crate::item_mutation_ingress::Route::Edit,
            &serde_json::to_vec(&body).unwrap(),
        )
        .unwrap();
        let command = EditCommand {
            operation_id: token.clone(),
            expected_item_version: edit as u64,
            expected_configuration_revision: 1,
            estimate: EstimateEdit::Keep,
            fields,
        };
        let EditDecision::Apply(edited) = decide_edit(
            &created.project,
            &project.configuration,
            &current_item,
            &current_data,
            None,
            true,
            &command,
        ) else {
            panic!("synthetic edit must be valid");
        };
        let record = expected(
            (edit + 1) as i32,
            token,
            work_item_codec::Request::Edit {
                project_id: ProjectId(project.project.id.0.clone()),
                item_id: item.clone(),
                command,
            },
            SavedResult::Edit(edited.clone()),
        );
        let response = request(client, &browser, "/items/edit", body, 200).await;
        assert_eq!(response["result"], record.result);
        current_item = edited.item;
        current_data = edited.data;
        values = current_data.values.clone();
        records.push(record);
        if [1, 5, 10, 20].contains(&edit) {
            checkpoint(
                admin,
                folder,
                "native",
                &format!("edit-{edit}"),
                &records,
                sample,
                false,
                false,
            )
            .await;
        }
    }
    assert!(handle.live.is_live());
    write_once(folder, "expected.json", &records);
    records
}

fn values_index(project: &ProjectCreateResult, id: &FieldId) -> usize {
    project
        .configuration
        .fields
        .iter()
        .position(|field| &field.id == id)
        .unwrap()
}

async fn historical(admin: &Admin, folder: &std::path::Path, sample: Sample, records: &[Expected]) {
    admit(admin).await;
    admin.client().batch_execute(HISTORICAL).await.unwrap();
    admin
        .client()
        .execute(
            "SELECT pg_temp.storage_project($1::text::jsonb)",
            &[&records[0].result.to_string()],
        )
        .await
        .unwrap();
    checkpoint(
        admin,
        folder,
        "historical",
        "baseline",
        &[],
        sample,
        false,
        false,
    )
    .await;
    for record in records {
        headroom(admin, sample).await;
        let request = unhex(&record.request_hex);
        admin
            .client()
            .execute(
                "SELECT pg_temp.storage_revision($1::bytea,$2::text::jsonb,$3::text,$4::int)",
                &[
                    &request,
                    &record.result.to_string(),
                    &record.token,
                    &record.version,
                ],
            )
            .await
            .unwrap();
        if [1, 2, 6, 11, 21].contains(&record.version) {
            let stage = if record.version == 1 {
                "create".to_owned()
            } else {
                format!("edit-{}", record.version - 1)
            };
            checkpoint(
                admin,
                folder,
                "historical",
                &stage,
                &records[..record.version as usize],
                sample,
                false,
                false,
            )
            .await;
        }
    }
    admin.client().batch_execute("BEGIN; UPDATE kehila.operations SET payload_retired=true WHERE target_kind='work_item'; \
        DELETE FROM kehila.operation_payloads WHERE operation_row_id IN (SELECT operation_row_id FROM kehila.operations WHERE target_kind='work_item'); \
        SET CONSTRAINTS ALL IMMEDIATE; COMMIT").await.unwrap();
    checkpoint(
        admin,
        folder,
        "historical",
        "retired",
        records,
        sample,
        true,
        false,
    )
    .await;
    admin
        .client()
        .batch_execute(
            "BEGIN; UPDATE kehila_auth.pending_items SET request_bytes=NULL; \
        SET CONSTRAINTS ALL IMMEDIATE; COMMIT",
        )
        .await
        .unwrap();
    checkpoint(
        admin,
        folder,
        "historical",
        "pruned",
        records,
        sample,
        true,
        true,
    )
    .await;
}

#[test]
#[ignore = "requires a fresh explicitly provisioned paired storage PostgreSQL TLS fixture"]
fn native_storage_growth_sample() {
    let index = std::env::var("KEHILA_STORAGE_SAMPLE_INDEX")
        .expect("explicit finite storage sample required");
    let parsed = index.parse::<usize>().unwrap();
    assert_eq!(parsed.to_string(), index);
    let sample = Sample::new(parsed);
    let (endpoint, installation, attempt, controls) = fixture();
    let historical_endpoint = endpoint.copied_fixture_database("native_storage_historical");
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let output=controls.clone();
        let (records,connections)=owned::run(&endpoint,&installation,format!("{attempt}-storage"),
            move |handle,client,admin| async move {
                native(&handle,&client,&*admin.lock().await,&output,sample).await
            }).await;
        assert_eq!(connections.accepted,25);
        assert_eq!(connections.completed,25);
        assert_eq!(connections.capacity_rejected+connections.protocol_rejected+connections.timed_out,0);
        let admin=Arc::new(tokio::sync::Mutex::new(Admin::connect(&historical_endpoint).await));
        let retained_admin=admin.clone();
        let output=controls.clone();
        let result=tokio::spawn(async move {
            historical(&*retained_admin.lock().await,&output,sample,&records).await;
        }).await;
        let admin=Arc::try_unwrap(admin).unwrap_or_else(|_| panic!("unjoined storage admin")).into_inner();
        admin.close().await;
        assert!(result.is_ok(),"historical storage work failed: {result:?}");
        write_once(&controls,"storage-summary.json",&json!({"schema":1,"sample":sample.index,
            "native_revisions":21,"historical_revisions":21,"runtime_joined":true,
            "native_http_completed":connections.completed,"historical_retirement":"owner-only constructed past origin"}));
        mark(&controls,"storage-growth-passed");
    });
}
