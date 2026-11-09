//! Bounded test observations. Context owns numeric evidence, never runtime resources.
#[cfg(target_os = "linux")]
mod cpu;
use std::{
    future::Future,
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Stage {
    Admission,
    AdmissionAccepted,
    AdmissionBusy,
    AdmissionUnavailable,
    Preparation,
    PreparationBusy,
    Submission,
    SubmissionBusy,
    SubmissionUnavailable,
    Queue,
    Execution,
    CpuBusy,
    CpuSkipped,
    CpuLane,
    CpuWork,
    CpuJoined,
    ReadMutex,
    ReadService,
    PrepareMutex,
    ItemPreparationAttempt,
    PrepareService,
    WriteMutex,
    ItemWriteAttempt,
    WriteService,
    Reprepare,
}

#[derive(Clone, Debug)]
pub(crate) struct Event {
    pub(crate) stage: Stage,
    pub(crate) offset_ns: u64,
    pub(crate) duration_ns: Option<u64>,
    // Actual OS-thread CPU time, separate from elapsed wall time.
    pub(crate) cpu_time_ns: Option<u64>,
    pub(crate) completed: bool,
}

#[derive(Debug)]
pub(crate) struct Row {
    pub(crate) id: usize,
    // Events are appended on completion. Offsets are interval starts, not sort keys.
    pub(crate) events: Vec<Event>,
    invalid: Option<&'static str>,
}

#[derive(Clone)]
pub(crate) struct Request {
    row: Arc<Mutex<Row>>,
    origin: Instant,
    event_limit: usize,
}

#[derive(Default)]
struct Cell {
    enabled: bool,
    request_limit: usize,
    event_limit: usize,
    requests: Vec<Request>,
    invalid: Option<&'static str>,
}

#[derive(Default)]
pub(crate) struct Collector(Mutex<Cell>);

impl Collector {
    pub(crate) fn start(&self, request_limit: usize, event_limit: usize) {
        assert!(request_limit > 0 && event_limit > 0);
        let mut cell = self.0.lock().unwrap();
        assert!(
            !cell.enabled && cell.requests.is_empty(),
            "previous capacity cell remains"
        );
        *cell = Cell {
            enabled: true,
            request_limit,
            event_limit,
            requests: Vec::new(),
            invalid: None,
        };
    }

    pub(crate) fn begin(&self) -> Option<Request> {
        let mut cell = self.0.lock().unwrap();
        if !cell.enabled {
            return None;
        }
        if cell.requests.len() >= cell.request_limit {
            cell.invalid = Some("capacity request evidence overflow");
            return None;
        }
        let request = Request {
            row: Arc::new(Mutex::new(Row {
                id: cell.requests.len(),
                events: Vec::new(),
                invalid: None,
            })),
            origin: Instant::now(),
            event_limit: cell.event_limit,
        };
        cell.requests.push(request.clone());
        Some(request)
    }

    pub(crate) fn finish(&self) -> Result<Vec<Row>, &'static str> {
        let mut cell = self
            .0
            .lock()
            .map_err(|_| "capacity collector mutex poisoned")?;
        if !cell.enabled {
            return Err("capacity cell is not active");
        }
        if let Some(error) = cell.invalid {
            return Err(error);
        }
        if cell
            .requests
            .iter()
            .any(|request| Arc::strong_count(&request.row) != 1)
        {
            return Err("capacity evidence still owned by work");
        }
        for request in &cell.requests {
            let row = request
                .row
                .lock()
                .map_err(|_| "capacity evidence mutex poisoned")?;
            if let Some(error) = row.invalid {
                return Err(error);
            }
        }
        cell.enabled = false;
        Ok(std::mem::take(&mut cell.requests)
            .into_iter()
            .map(|request| {
                Arc::try_unwrap(request.row)
                    .expect("capacity owner changed")
                    .into_inner()
                    .unwrap()
            })
            .collect())
    }
}

impl Request {
    pub(crate) fn event(&self, stage: Stage) {
        self.append(stage, Instant::now(), None, None, true);
    }

    fn append(
        &self,
        stage: Stage,
        at: Instant,
        duration_ns: Option<u64>,
        cpu_time_ns: Option<u64>,
        completed: bool,
    ) {
        // Span destruction also runs during unwinding. It must never panic,
        // including when a prior observer failure poisoned this numeric state.
        let mut row = match self.row.lock() {
            Ok(row) => row,
            Err(poisoned) => {
                poisoned.into_inner().invalid = Some("capacity evidence mutex poisoned");
                return;
            }
        };
        let Some(offset_ns) = at
            .checked_duration_since(self.origin)
            .and_then(|elapsed| u64::try_from(elapsed.as_nanos()).ok())
        else {
            row.invalid = Some("capacity timestamp overflow");
            return;
        };
        if row.events.len() >= self.event_limit {
            row.invalid = Some("capacity event evidence overflow");
            return;
        }
        if row.events.try_reserve(1).is_err() {
            row.invalid = Some("capacity event allocation failed");
            return;
        }
        row.events.push(Event {
            stage,
            offset_ns,
            duration_ns,
            cpu_time_ns,
            completed,
        });
    }

    pub(crate) fn span(&self, stage: Stage) -> Span {
        Span {
            request: self.clone(),
            stage,
            started: Instant::now(),
            cpu_time_ns: None,
            completed: false,
        }
    }

    fn invalidate(&self, error: &'static str) {
        match self.row.lock() {
            Ok(mut row) => row.invalid = Some(error),
            Err(poisoned) => poisoned.into_inner().invalid = Some(error),
        }
    }
}

pub(crate) struct Span {
    request: Request,
    stage: Stage,
    started: Instant,
    cpu_time_ns: Option<u64>,
    completed: bool,
}

impl Span {
    pub(crate) fn complete(mut self) {
        self.completed = true;
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        let Ok(duration_ns) = u64::try_from(self.started.elapsed().as_nanos()) else {
            self.request.invalidate("capacity duration overflow");
            return;
        };
        self.request.append(
            self.stage,
            self.started,
            Some(duration_ns),
            self.cpu_time_ns,
            self.completed,
        );
    }
}

tokio::task_local! { static CURRENT: Option<Request>; }

pub(crate) fn capture() -> Option<Request> {
    CURRENT.try_with(Clone::clone).ok().flatten()
}

pub(crate) fn event(stage: Stage) {
    if let Some(request) = capture() {
        request.event(stage);
    }
}

pub(crate) fn span(stage: Stage) -> Option<Span> {
    capture().map(|request| request.span(stage))
}

pub(crate) async fn scope<T>(request: Option<Request>, future: impl Future<Output = T>) -> T {
    CURRENT.scope(request, future).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_collector_and_unsettled_cell_cannot_create_results() {
        let collector = Collector::default();
        assert!(collector.begin().is_none());
        assert!(collector.finish().is_err());
        collector.start(2, 8);
        let request = collector.begin().unwrap();
        let span = request.span(Stage::ReadMutex);
        drop(request);
        assert!(
            collector.finish().is_err(),
            "a live observation was silently frozen"
        );
        span.complete();
        let rows = collector.finish().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].events.len(), 1);
        assert_eq!(rows[0].events[0].stage, Stage::ReadMutex);
        assert!(rows[0].events[0].completed);
        assert!(rows[0].events[0].duration_ns.is_some());
        assert!(rows[0].events[0].cpu_time_ns.is_none());
    }

    #[test]
    fn spawned_tasks_and_worker_capture_keep_one_original_numeric_identity() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let collector = Collector::default();
                collector.start(2, 16);
                let request = collector.begin().unwrap();
                scope(Some(request), async {
                    capture().unwrap().event(Stage::Admission);
                    let captured = capture();
                    tokio::spawn(scope(captured, async {
                        capture().unwrap().event(Stage::Preparation);
                        let captured = capture().unwrap();
                        std::thread::spawn(move || {
                            captured.span(Stage::CpuWork).complete();
                        })
                        .join()
                        .unwrap();
                        // Tokio does not inherit task-local context into an unscoped child.
                        assert!(tokio::spawn(async { capture().is_none() }).await.unwrap());
                    }))
                    .await
                    .unwrap();
                })
                .await;
                assert!(capture().is_none());
                let rows = collector.finish().unwrap();
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].id, 0);
                assert_eq!(
                    rows[0]
                        .events
                        .iter()
                        .map(|event| event.stage)
                        .collect::<Vec<_>>(),
                    [Stage::Admission, Stage::Preparation, Stage::CpuWork]
                );
                assert!(rows[0]
                    .events
                    .windows(2)
                    .all(|pair| pair[0].offset_ns <= pair[1].offset_ns));
            });
    }

    #[test]
    fn abandoned_span_is_distinct_from_completed_work_and_cells_do_not_leak() {
        let collector = Collector::default();
        collector.start(1, 4);
        let request = collector.begin().unwrap();
        drop(request.span(Stage::WriteService));
        drop(request);
        let rows = collector.finish().unwrap();
        assert_eq!(rows[0].events.len(), 1);
        assert!(!rows[0].events[0].completed);
        collector.start(1, 4);
        assert!(collector.finish().unwrap().is_empty());
    }

    #[test]
    fn request_overflow_cannot_silently_drop_rejections() {
        let collector = Collector::default();
        collector.start(1, 2);
        drop(collector.begin());
        assert!(collector.begin().is_none());
        assert_eq!(
            collector.finish().unwrap_err(),
            "capacity request evidence overflow"
        );
    }

    #[test]
    fn event_overflow_cannot_make_an_incomplete_trace_look_complete() {
        let collector = Collector::default();
        collector.start(1, 1);
        let request = collector.begin().unwrap();
        let outer = request.span(Stage::CpuLane);
        let inner = request.span(Stage::CpuWork);
        request.event(Stage::Admission);
        inner.complete();
        // Overflow is sticky; destruction of more spans cannot double-panic.
        drop(outer);
        drop(request);
        assert_eq!(
            collector.finish().unwrap_err(),
            "capacity event evidence overflow"
        );
    }

    #[test]
    fn poisoned_observer_does_not_panic_during_owned_span_cleanup() {
        let collector = Collector::default();
        collector.start(1, 4);
        let request = collector.begin().unwrap();
        let span = request.span(Stage::CpuWork);
        let row = request.row.clone();
        assert!(std::thread::spawn(move || {
            let _guard = row.lock().unwrap();
            panic!("synthetic observer failure");
        })
        .join()
        .is_err());
        drop(span);
        drop(request);
        assert_eq!(
            collector.finish().unwrap_err(),
            "capacity evidence mutex poisoned"
        );
    }

    #[test]
    fn nested_spans_keep_interval_containment_without_claiming_append_time_order() {
        let collector = Collector::default();
        collector.start(1, 4);
        let request = collector.begin().unwrap();
        let parent = request.span(Stage::CpuLane);
        request.span(Stage::CpuWork).complete();
        parent.complete();
        drop(request);
        let rows = collector.finish().unwrap();
        let [child, parent] = rows[0].events.as_slice() else {
            panic!("missing interval");
        };
        assert!(parent.offset_ns <= child.offset_ns);
        assert!(
            parent.offset_ns + parent.duration_ns.unwrap()
                >= child.offset_ns + child.duration_ns.unwrap()
        );
    }

    #[test]
    fn overflow_during_unwinding_preserves_original_panic_and_owned_cleanup() {
        use std::sync::atomic::{AtomicBool, Ordering};
        struct Cleanup<'a>(&'a AtomicBool);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let collector = Collector::default();
        collector.start(1, 1);
        let request = collector.begin().unwrap();
        let cleaned = AtomicBool::new(false);
        let outcome = std::panic::catch_unwind(|| {
            let _cleanup = Cleanup(&cleaned);
            let _outer = request.span(Stage::CpuLane);
            let _inner = request.span(Stage::CpuWork);
            request.event(Stage::Admission);
            panic!("original fixture failure");
        });
        assert_eq!(
            outcome.unwrap_err().downcast_ref::<&str>(),
            Some(&"original fixture failure")
        );
        assert!(
            cleaned.load(Ordering::SeqCst),
            "observer prevented owned cleanup"
        );
        drop(request);
        assert_eq!(
            collector.finish().unwrap_err(),
            "capacity event evidence overflow"
        );
    }
}
