//! Sampled backend waits are observations, not exact fence-duration accounting.
use super::super::*;
use super::request::Attempt;
use crate::lifecycle::probe::capacity::{Row, Stage};
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct Sampling {
    pub(super) interval_ns: u64,
    pub(super) sample_limit: usize,
    pub(super) samples: Vec<Sample>,
    pub(super) error: Option<String>,
}

#[derive(Serialize)]
pub(super) struct Sample {
    pub(super) started_ns: u64,
    pub(super) finished_ns: u64,
    pub(super) backends: Vec<Backend>,
}

#[derive(Serialize)]
pub(super) struct Backend {
    pub(super) pid: i32,
    pub(super) state: Option<String>,
    pub(super) wait_type: Option<String>,
    pub(super) wait_event: Option<String>,
    pub(super) blocking_pids: Vec<i32>,
    pub(super) query: Option<String>,
    pub(super) backend_start: Option<String>,
    pub(super) transaction_start: Option<String>,
    pub(super) ungranted_locks_json: String,
}

pub(super) fn nanos(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_nanos()).expect("capacity duration overflow")
}

pub(super) async fn sample_once(
    admin: &Admin,
    origin: Instant,
) -> Result<Sample, tokio_postgres::Error> {
    let started_ns = nanos(origin);
    let rows = admin.client().query("SELECT a.pid,a.state,a.wait_event_type,a.wait_event,pg_blocking_pids(a.pid),a.query, \
        a.backend_start::text,a.xact_start::text,coalesce((SELECT jsonb_agg(jsonb_build_object( \
        'locktype',l.locktype,'mode',l.mode,'relation',l.relation::regclass::text, \
        'transaction_id',l.transactionid::text,'wait_start',l.waitstart::text)) \
        FROM pg_locks l WHERE l.pid=a.pid AND NOT l.granted),'[]'::jsonb)::text \
        FROM pg_stat_activity a WHERE a.datname=current_database() AND a.usename='kehila_auth_runtime_login' \
        ORDER BY a.pid", &[]).await?;
    let finished_ns = nanos(origin);
    let backends = rows
        .iter()
        .map(|row| -> Result<Backend, tokio_postgres::Error> {
            Ok(Backend {
                pid: row.try_get(0)?,
                state: row.try_get(1)?,
                wait_type: row.try_get(2)?,
                wait_event: row.try_get(3)?,
                blocking_pids: row.try_get(4)?,
                query: row.try_get(5)?,
                backend_start: row.try_get(6)?,
                transaction_start: row.try_get(7)?,
                ungranted_locks_json: row.try_get(8)?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Sample {
        started_ns,
        finished_ns,
        backends,
    })
}

pub(super) async fn sample(
    admin: &Admin,
    origin: Instant,
    requests: usize,
    mut stop: tokio::sync::oneshot::Receiver<()>,
) -> Sampling {
    const INTERVAL: Duration = Duration::from_millis(5);
    // Bound evidence, not execution. This allows one entry per millisecond of
    // all original request deadlines, above the configured five-ms cadence.
    // Any overflow invalidates observation rather than truncating it silently.
    let limit = requests * usize::try_from(crate::CONNECTION_DEADLINE.as_millis()).unwrap() + 1;
    let mut result = Sampling {
        interval_ns: 5_000_000,
        sample_limit: limit,
        samples: Vec::new(),
        error: None,
    };
    let mut timer = tokio::time::interval(INTERVAL);
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            biased;
            _ = &mut stop => break,
            _ = timer.tick() => {
                if result.samples.len() >= limit || result.samples.try_reserve(1).is_err() {
                    result.error = Some("sample evidence overflow or allocation failure".into()); break;
                }
                match sample_once(admin, origin).await {
                    Ok(sample) => result.samples.push(sample),
                    Err(error) => { result.error=Some(error.to_string()); break; }
                }
            }
        }
    }
    result
}

pub(super) fn validate(rows: &[Row], attempts: &[Attempt]) -> Vec<String> {
    let mut errors = Vec::new();
    let known = attempts
        .iter()
        .filter(|attempt| matches!(attempt.status, Some(200 | 429 | 503)))
        .count();
    if rows.len() > attempts.len() || rows.len() < known {
        errors.push("request-stage population differs from admitted HTTP outcomes".into());
    }
    let mut busy = 0;
    let mut serviced = 0;
    for (index, row) in rows.iter().enumerate() {
        let count = |stage| {
            row.events
                .iter()
                .filter(|event| event.stage == stage)
                .count()
        };
        if row.id != index
            || count(Stage::Admission) != 1
            || count(Stage::AdmissionAccepted)
                + count(Stage::AdmissionBusy)
                + count(Stage::AdmissionUnavailable)
                != 1
            || count(Stage::Execution) > 1
        {
            errors.push(format!("invalid admission/execution trace {index}"));
        }
        let rejections = count(Stage::AdmissionBusy)
            + count(Stage::PreparationBusy)
            + count(Stage::SubmissionBusy)
            + count(Stage::CpuBusy);
        if rejections > 1 {
            errors.push(format!("multiple busy origins in trace {index}"));
        }
        busy += usize::from(rejections == 1);
        serviced += usize::from(row.events.iter().any(|event| {
            event.completed && matches!(event.stage, Stage::ReadService | Stage::WriteService)
        }));
        if row
            .events
            .iter()
            .any(|event| (event.stage == Stage::CpuWork) != event.cpu_time_ns.is_some())
        {
            errors.push(format!(
                "missing or misplaced thread CPU measurement in trace {index}"
            ));
        }
    }
    let http_busy = attempts
        .iter()
        .filter(|attempt| attempt.status == Some(429))
        .count();
    let success = attempts
        .iter()
        .filter(|attempt| attempt.status == Some(200))
        .count();
    if busy < http_busy || busy > attempts.len() - success || serviced < success {
        errors.push(
            "stage populations do not account for successful and rejected HTTP outcomes".into(),
        );
    }
    errors
}
