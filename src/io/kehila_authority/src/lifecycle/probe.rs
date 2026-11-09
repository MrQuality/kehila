//! Test-only admission observations and a request-scoped trusted-seed fault.
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Mutex,
};
pub(crate) mod capacity;
mod item_attempt;
pub(crate) use item_attempt::{AttemptEntry, ItemAttempt};

#[derive(Clone, Copy)]
pub(crate) enum Stage {
    Preparation,
    Worker,
    Execution,
    NativeMutex,
    Database,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Snapshot {
    pub(crate) preparations: usize,
    pub(crate) workers: usize,
    pub(crate) executions: usize,
    pub(crate) native_mutex_attempts: usize,
    pub(crate) database_ownership_entries: usize,
}
#[derive(Default)]
pub(crate) struct Probe {
    pub(crate) capacity: capacity::Collector,
    counters: [AtomicUsize; 5],
    fault: Mutex<Option<(i64, bool)>>,
    settling: AtomicBool,
    settled_notice: tokio::sync::Notify,
    transaction_gate: Mutex<Option<TransactionHold>>,
    sql_state: Mutex<Option<String>>,
    registration_identities: Mutex<Option<crate::item_pending::Identities>>,
    item_replay: Mutex<Option<ItemReplayObservation>>,
    item_repreparations: AtomicUsize,
    item_attempt: Mutex<Option<ItemAttempt>>,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Point {
    ExecuteLookup,
    Rollback,
    CancelCommit,
    BeforeCompletion,
    ReadSnapshot,
    RegistrationRollback,
    PendingCommit,
    ItemBeforeCommit,
    ItemMutationPrepared,
    ItemAttemptPrepared,
    ItemHistoryLoaded,
    ItemReplayFenced,
    ItemReplayCleanup,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ItemReplayObservation {
    Prepared,
    Authorized,
    Expired,
    BeforeOrigin,
    RollbackFailed,
}
struct TransactionHold {
    request_id: i64,
    point: Point,
    arrived: tokio::sync::oneshot::Sender<()>,
    release: tokio::sync::oneshot::Receiver<()>,
}
pub(crate) struct TransactionGate {
    pub(crate) arrived: tokio::sync::oneshot::Receiver<()>,
    pub(crate) release: tokio::sync::oneshot::Sender<()>,
}
impl Probe {
    pub(crate) fn item_reprepared(&self) {
        self.item_repreparations.fetch_add(1, Ordering::SeqCst);
    }
    pub(crate) fn item_repreparations(&self) -> usize {
        self.item_repreparations.load(Ordering::SeqCst)
    }
    pub(crate) fn record_item_replay(&self, value: ItemReplayObservation) {
        *self.item_replay.lock().unwrap() = Some(value);
    }
    pub(crate) fn take_item_replay(&self) -> Option<ItemReplayObservation> {
        self.item_replay.lock().unwrap().take()
    }
    pub(crate) fn registration_identities(
        &self,
        identities: crate::item_pending::Identities,
    ) -> RegistrationFault<'_> {
        let mut value = self.registration_identities.lock().unwrap();
        assert!(
            value.is_none(),
            "one registration identity override per runtime"
        );
        *value = Some(identities);
        RegistrationFault { probe: self }
    }
    pub(crate) fn take_registration_identities(&self) -> Option<crate::item_pending::Identities> {
        self.registration_identities.lock().unwrap().take()
    }
    pub(crate) fn record_sql_state(&self, code: &str) {
        assert_eq!(code.len(), 5);
        *self.sql_state.lock().unwrap() = Some(code.to_owned());
    }
    pub(crate) fn take_sql_state(&self) -> Option<String> {
        self.sql_state.lock().unwrap().take()
    }
    pub(crate) fn hold_transaction(&self, request_id: i64, point: Point) -> TransactionGate {
        let (arrived, observed) = tokio::sync::oneshot::channel();
        let (release, held) = tokio::sync::oneshot::channel();
        let mut gate = self.transaction_gate.lock().unwrap();
        assert!(gate.is_none(), "one transaction gate per fixture runtime");
        *gate = Some(TransactionHold {
            request_id,
            point,
            arrived,
            release: held,
        });
        TransactionGate {
            arrived: observed,
            release,
        }
    }
    pub(crate) async fn transaction_point(
        &self,
        request_id: i64,
        point: Point,
        deadline: std::time::Instant,
    ) {
        let hold = {
            let mut gate = self.transaction_gate.lock().unwrap();
            if gate
                .as_ref()
                .is_some_and(|g| g.request_id == request_id && g.point == point)
            {
                gate.take()
            } else {
                None
            }
        };
        if let Some(hold) = hold {
            let _ = hold.arrived.send(());
            // A dropped fixture release sender also releases the transaction.
            let _ = tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), hold.release)
                .await
                .expect("bounded transaction test gate expired");
        }
    }
    pub(crate) fn begin_settling(&self) {
        self.settling.store(true, Ordering::SeqCst);
        self.settled_notice.notify_waiters();
    }
    pub(crate) async fn wait_settling(&self) {
        loop {
            let notice = self.settled_notice.notified();
            if self.settling.load(Ordering::SeqCst) {
                return;
            }
            notice.await;
        }
    }
    pub(crate) fn record(&self, stage: Stage) {
        let index = match stage {
            Stage::Preparation => 0,
            Stage::Worker => 1,
            Stage::Execution => 2,
            Stage::NativeMutex => 3,
            Stage::Database => 4,
        };
        self.counters[index].fetch_add(1, Ordering::SeqCst);
    }
    pub(crate) fn snapshot(&self) -> Snapshot {
        let value = self
            .counters
            .each_ref()
            .map(|counter| counter.load(Ordering::SeqCst));
        Snapshot {
            preparations: value[0],
            workers: value[1],
            executions: value[2],
            native_mutex_attempts: value[3],
            database_ownership_entries: value[4],
        }
    }
    pub(crate) fn seed_overflow(&self, request_id: i64) -> SeedFault<'_> {
        let mut fault = self.fault.lock().unwrap();
        if fault.is_some() {
            drop(fault);
            panic!("only one scoped seed fault is allowed");
        }
        *fault = Some((request_id, false));
        SeedFault { probe: self }
    }
    pub(crate) fn take_seed_overflow(&self, request_id: i64) -> bool {
        let mut fault = self.fault.lock().unwrap();
        match fault.as_mut() {
            Some((expected, consumed)) if *expected == request_id && !*consumed => {
                *consumed = true;
                true
            }
            _ => false,
        }
    }
}
pub(crate) struct SeedFault<'a> {
    probe: &'a Probe,
}
impl SeedFault<'_> {
    pub(crate) fn consumed(&self) -> bool {
        self.probe
            .fault
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|(_, used)| *used)
    }
}
impl Drop for SeedFault<'_> {
    fn drop(&mut self) {
        *self
            .probe
            .fault
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    }
}

pub(crate) struct RegistrationFault<'a> {
    probe: &'a Probe,
}
impl RegistrationFault<'_> {
    pub(crate) fn consumed(&self) -> bool {
        self.probe.registration_identities.lock().unwrap().is_none()
    }
}
impl Drop for RegistrationFault<'_> {
    fn drop(&mut self) {
        self.probe
            .registration_identities
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
    }
}
