//! One pristine, finite closed-loop cell. A valid observation may miss its target.
mod calibration;
mod observe;
mod plan;
mod request;
use super::*;
use crate::lifecycle::probe::capacity::Row;
use observe::{nanos, Sampling};
use plan::Cell;
use request::{Attempt, Phase, Slot};
use serde::Serialize;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy, Eq, PartialEq)]
enum Observation {
    Off,
    Stages,
    Sampled,
}

impl Observation {
    fn from_environment() -> Self {
        match std::env::var("KEHILA_NATIVE_CAPACITY_OBSERVER").as_deref() {
            Ok("off") => Self::Off,
            Ok("stages") => Self::Stages,
            Ok("sampled") | Err(std::env::VarError::NotPresent) => Self::Sampled,
            _ => panic!("unknown capacity observation mode"),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Stages => "stages",
            Self::Sampled => "sampled",
        }
    }
}

#[derive(Serialize)]
struct PhaseReport {
    phase: Phase,
    configured_concurrency: usize,
    planned_requests: usize,
    request_window_ns: u64,
    checkpoint_ns: u64,
    attempts: Vec<Attempt>,
    recoveries: Vec<Attempt>,
    rows: Option<Vec<Row>>,
    sampling: Option<Sampling>,
    errors: Vec<String>,
    controls: Vec<Value>,
}

struct Setup {
    slots: Vec<Slot>,
    browsers: Vec<Browser>,
    texts: Arc<Vec<Vec<String>>>,
    metadata: Value,
}

fn write_once(path: &std::path::Path, value: &impl Serialize) {
    use std::io::Write;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    let mut writer = std::io::BufWriter::new(file);
    serde_json::to_writer(&mut writer, value).unwrap();
    writer.flush().unwrap();
}

async fn setup(handle: &Handle, client: &Client, cell: Cell) -> Setup {
    let started = Instant::now();
    let authentication = Instant::now();
    let (browsers, pacing) = login_clients(client).await;
    let authentication_ns = nanos(authentication);
    let mut projects = Vec::new();
    let mut profiles = Vec::new();
    for index in 0..cell.projects {
        let form = Form {
            name: format!("Capacity project {index}"),
            prefix: format!("CP{index}"),
            estimate_unit: EstimateUnit::Hours,
        };
        let registered = super::request(
            client,
            &browsers[0],
            "/projects/register",
            json!({"name":form.name,"prefix":form.prefix,"estimate_unit":"hours"}),
            201,
        )
        .await;
        let project = store_synthetic(
            handle,
            &browsers[0],
            &registered,
            form,
            format!("{:032x}", index + 1),
            cell.fields,
        )
        .await;
        profiles.push(json!({"project_id":project.project.id.0,
            "configuration_json_bytes":project_create_codec::encode_snapshot(&project.configuration).unwrap().len()}));
        projects.push(project);
    }
    let mut slots = Vec::new();
    for index in 0..16 {
        let project = &projects[index % cell.projects];
        let kind = &project.configuration.types[0];
        let fields = project
            .configuration
            .fields
            .iter()
            .filter(|field| field.owner_type == kind.id)
            .map(|field| field.id.0.clone())
            .collect::<Vec<_>>();
        assert_eq!(fields.len(), cell.fields);
        let values = fields
            .iter()
            .enumerate()
            .map(|(field, id)| json!({"id":id,"value":request::text(cell,1_000+index,field)}))
            .collect::<Vec<_>>();
        let body = json!({"project_id":project.project.id.0,"type_id":kind.id.0,"workflow_id":null,
            "expected_configuration_revision":"1","estimate":null,"values":values});
        let registered = super::request(
            client,
            &browsers[0],
            "/items/pending/register",
            body.clone(),
            201,
        )
        .await;
        let created = super::request(client,&browsers[0],"/items/pending/execute",
            json!({"project_id":project.project.id.0,"intent_handle":registered["entry"]["intent_handle"]}),201).await;
        let saved =
            work_item_codec::decode_result(1, &serde_json::to_vec(&created["result"]).unwrap())
                .unwrap();
        let SavedResult::Create(value) = &saved else {
            panic!("seed creation family");
        };
        assert_eq!(value.item.version, 1);
        assert_eq!(value.data.values.len(), fields.len());
        assert_eq!(registered["entry"]["target_item_id"], value.item.id.0);
        for field in &values {
            assert!(value.data.values.iter().any(|entry|entry.id.0==field["id"].as_str().unwrap()
                && matches!(&entry.value,FieldValue::Text(text) if text==field["value"].as_str().unwrap())));
        }
        let readable = &value.readable_id;
        let seed = json!({"registration":registered["entry"],"request":body,"result":created["result"],
            "readable_id":{"project_id":readable.project_id.0,"item_id":readable.item_id.0,
                "prefix":readable.prefix,"sequence":readable.sequence.to_string()}});
        slots.push(Slot {
            index,
            project: project.project.id.0.clone(),
            item: value.item.id.0.clone(),
            fields,
            saved,
            seed,
            pending: None,
        });
    }
    // Generate deterministic ASCII text outside the request windows. It avoids
    // making every long value a compressible repetition of one character.
    let texts = (0..111)
        .map(|ordinal| {
            if ordinal == 110 || cell.edit(if ordinal < 10 { ordinal } else { ordinal - 10 }) {
                (0..cell.fields)
                    .map(|field| request::text(cell, ordinal, field))
                    .collect()
            } else {
                Vec::new()
            }
        })
        .collect();
    evidence::settled(handle).await;
    Setup {
        slots,
        browsers,
        texts: Arc::new(texts),
        metadata: json!({"elapsed_ns":nanos(started),
        "authentication_ns":authentication_ns,"authentication_pacing_ns":u64::try_from(pacing.as_nanos()).unwrap(),
        "projects":profiles,"text_generation":"deterministic SHA256 hex, exact ASCII byte lengths"}),
    }
}

async fn phase(
    handle: &Handle,
    client: &Client,
    admin: &Admin,
    setup: Setup,
    cell: Cell,
    mode: Observation,
    kind: Phase,
) -> (Setup, PhaseReport) {
    let count = match kind {
        Phase::Warmup => 10,
        Phase::Measured => 100,
        Phase::Control => 1,
        Phase::Recovery | Phase::Calibration => unreachable!(),
    };
    let concurrency = if kind == Phase::Control {
        1
    } else {
        cell.concurrency
    };
    evidence::settled(handle).await;
    if mode != Observation::Off {
        handle.live.probe.capacity.start(count, 128);
    }
    let origin = if mode == Observation::Off {
        Instant::now()
    } else {
        handle
            .live
            .probe
            .capacity
            .phase_origin()
            .expect("capacity phase origin unavailable")
    };
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let Setup {
        mut slots,
        browsers,
        texts,
        metadata,
    } = setup;
    let inactive = slots.split_off(concurrency);
    let next = Arc::new(AtomicUsize::new(0));
    let work = async {
        let mut running = tokio::task::JoinSet::new();
        for mut slot in slots {
            let client = client.clone();
            let browsers = browsers.clone();
            let next = next.clone();
            let texts = texts.clone();
            running.spawn(async move {
                let mut attempts = Vec::new();
                loop {
                    let ordinal = next.fetch_add(1, Ordering::Relaxed);
                    if ordinal >= count {
                        break;
                    }
                    let text_index = match kind {
                        Phase::Warmup => ordinal,
                        Phase::Measured => 10 + ordinal,
                        Phase::Control => 110,
                        Phase::Recovery | Phase::Calibration => unreachable!(),
                    };
                    let command = slot.command(cell, kind, ordinal, &texts[text_index]);
                    let attempt = slot
                        .send(cell, command, &client, &browsers[ordinal % 4])
                        .await;
                    let stop = attempt.integrity_error.is_some() || attempt.uncertain;
                    attempts.push(attempt);
                    if stop {
                        break;
                    }
                }
                (slot, attempts)
            });
        }
        let mut states = inactive;
        let mut attempts = Vec::new();
        let mut errors = Vec::new();
        // Always join every client before interpreting an error or assertion.
        while let Some(result) = running.join_next().await {
            match result {
                Ok((slot, mut recorded)) => {
                    states.push(slot);
                    attempts.append(&mut recorded);
                }
                Err(error) => errors.push(format!("client task failed: {error:?}")),
            }
        }
        let elapsed = nanos(origin);
        let _ = stop.send(());
        (states, attempts, errors, elapsed)
    };
    let sampling = async {
        if mode == Observation::Sampled {
            Some(observe::sample(admin, origin, count, stopped).await)
        } else {
            None
        }
    };
    let ((mut slots, mut attempts, mut errors, request_window_ns), sampling) =
        tokio::join!(work, sampling);
    attempts.sort_by_key(|attempt| attempt.ordinal);
    slots.sort_by_key(|slot| slot.index);
    evidence::settled(handle).await;
    let rows = if mode == Observation::Off {
        None
    } else {
        match handle.live.probe.capacity.finish() {
            Ok(rows) => {
                errors.extend(observe::validate(&rows, &attempts));
                Some(rows)
            }
            Err(error) => {
                errors.push(error.into());
                None
            }
        }
    };
    if attempts.len() != count
        || attempts
            .iter()
            .enumerate()
            .any(|(index, attempt)| attempt.ordinal != index)
    {
        errors.push("original request population differs from the finite plan".into());
    }
    if slots.len() != 16 {
        errors.push("client item ownership did not return intact".into());
    }
    for attempt in &attempts {
        if let Some(error) = &attempt.integrity_error {
            errors.push(error.clone());
        }
    }
    if let Some(observed) = &sampling {
        if let Some(error) = &observed.error {
            errors.push(error.clone());
        }
        if observed.samples.is_empty() {
            errors.push("lock sampler observed no points".into());
        }
    }
    let mut recoveries = Vec::new();
    if mode == Observation::Off || rows.is_some() {
        for slot in &mut slots {
            if let Some(recovery) = slot.recover(cell, client, &browsers).await {
                if recovery.uncertain || recovery.integrity_error.is_some() {
                    errors.push("original edit outcome remains unresolved".into());
                }
                recoveries.push(recovery);
            }
        }
        evidence::settled(handle).await;
        evidence::fences_released(admin).await;
    }
    if kind == Phase::Control
        && attempts
            .first()
            .is_none_or(|attempt| attempt.outcome != "success")
    {
        errors.push("serial post-load edit did not complete successfully".into());
    }
    let checkpoint_ns = nanos(origin).checked_sub(request_window_ns).unwrap();
    (
        Setup {
            slots,
            browsers,
            texts,
            metadata,
        },
        PhaseReport {
            phase: kind,
            configured_concurrency: concurrency,
            planned_requests: count,
            request_window_ns,
            checkpoint_ns,
            attempts,
            recoveries,
            rows,
            sampling,
            errors,
            controls: Vec::new(),
        },
    )
}

async fn cases(
    handle: &Handle,
    client: &Client,
    admin: &Admin,
    controls: &std::path::Path,
    cell: Cell,
    mode: Observation,
    blocker: Option<&Admin>,
) -> (bool, usize) {
    let mut setup = setup(handle, client, cell).await;
    let mut phases = Vec::new();
    if let Some(blocker) = blocker {
        phases.push(calibration::phase(handle, client, admin, blocker, &mut setup, cell).await);
    }
    for kind in [Phase::Warmup, Phase::Measured, Phase::Control] {
        if phases.iter().any(|phase| !phase.errors.is_empty()) {
            break;
        }
        let (next, report) = phase(handle, client, admin, setup, cell, mode, kind).await;
        setup = next;
        let valid = report.errors.is_empty();
        phases.push(report);
        if !valid {
            break;
        }
    }
    let expected_phases = if blocker.is_some() { 4 } else { 3 };
    let mut valid = phases.len() == expected_phases
        && phases.iter().all(|phase| phase.errors.is_empty())
        && handle.live.is_live();
    let requests = 40
        + cell.projects
        + phases
            .iter()
            .map(|phase| phase.attempts.len() + phase.recoveries.len())
            .sum::<usize>();
    let settings = admin.client().query_one("SELECT current_setting('server_version'),current_setting('max_connections'), \
        current_setting('ssl'),pg_database_size(current_database()),(SELECT coalesce(sum(size),0)::bigint FROM pg_ls_waldir())",&[]).await;
    let postgres = match settings {
        Ok(settings) => {
            valid &= settings.get::<_, String>(1) == "8" && settings.get::<_, String>(2) == "on";
            json!({"server_version":settings.get::<_,String>(0),"max_connections":settings.get::<_,String>(1),
                "ssl":settings.get::<_,String>(2),"database_bytes_after":settings.get::<_,i64>(3),
                "wal_bytes_after":settings.get::<_,i64>(4)})
        }
        Err(error) => {
            valid = false;
            json!({"observation_error":error.to_string()})
        }
    };
    write_once(
        &controls.join("matrix-work.json"),
        &json!({"schema":1,"plan":cell.document(),"observer":mode.name(),
        "valid":valid,"calibration":blocker.is_some(),"setup":setup.metadata,"phases":phases,"slots":setup.slots.iter().map(Slot::document).collect::<Vec<_>>(),
        "postgres":postgres,
        "latency_boundary":"prepared HTTP request through verified TLS/driver cleanup and result validation",
        "throughput_boundary":"phase clock through joined clients, including request preparation and evidence encoding",
        "tls_policy":"new verified TLS connection per request; shared client configuration and default resumption policy",
        "stage_population":"aggregate server observations; numeric row IDs do not identify client attempts"}),
    );
    (valid, requests)
}

#[test]
#[ignore = "requires a fresh explicitly provisioned product PostgreSQL TLS fixture"]
fn native_capacity_cell() {
    let index =
        std::env::var("KEHILA_NATIVE_CAPACITY_CELL_INDEX").expect("explicit finite cell required");
    let parsed = index.parse::<usize>().unwrap();
    assert_eq!(parsed.to_string(), index);
    let cell = Cell::new(parsed).expect("capacity cell index outside finite plan");
    let mode = Observation::from_environment();
    let calibration = match std::env::var("KEHILA_NATIVE_CAPACITY_CALIBRATION").as_deref() {
        Ok("0") | Err(std::env::VarError::NotPresent) => false,
        Ok("1") => true,
        _ => panic!("invalid capacity calibration selector"),
    };
    assert!(
        !calibration || (parsed == 15 && mode == Observation::Sampled),
        "calibration requires separate sampled serial-edit cell 15"
    );
    let (endpoint, installation, attempt, controls) = fixture();
    let total = Instant::now();
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let output = controls.clone();
        let ((valid,expected_connections),connections) = owned::run_with_blocker(&endpoint,&installation,
            format!("{attempt}-cell-{parsed}"),calibration,move |handle,client,admin,blocker| async move {
                let blocker = match blocker.as_ref() { Some(value) => Some(value.lock().await), None => None };
                cases(&handle,&client,&*admin.lock().await,&output,cell,mode,blocker.as_deref()).await
            }).await;
        let accounted = connections.accepted+connections.capacity_rejected==expected_connections
            && connections.completed+connections.protocol_rejected+connections.timed_out==connections.accepted;
        write_once(&controls.join("matrix-owned.json"),&json!({"schema":1,"runtime_joined":true,
            "connection_accounting_valid":accounted,"expected_connections":expected_connections,"total_ns":nanos(total),
            "connections":{"accepted":connections.accepted,"completed":connections.completed,
                "capacity_rejected":connections.capacity_rejected,"protocol_rejected":connections.protocol_rejected,
                "timed_out":connections.timed_out,"tls_rejected":connections.tls_rejected,
                "http_rejected":connections.http_rejected,"tls_shutdown_rejected":connections.tls_shutdown_rejected,
                "peer_close_rejected":connections.peer_close_rejected}}));
        assert!(valid && accounted,"capacity cell evidence invalid; external work and ownership records retained");
        mark(&controls,"capacity-cell-passed");
    });
}
