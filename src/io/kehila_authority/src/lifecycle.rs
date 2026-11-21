//! Shared irreversible local stop state; not proof of database lock ownership.
#[cfg(any(target_os = "linux", test))]
use std::sync::Arc;
use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};
use tokio::sync::watch;
#[cfg(test)]
pub(crate) mod probe;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FatalCause {
    OwnedExecution,
    NativeIntegrity,
    NativeRollback {
        primary: Option<FailureDetail>,
        cleanup: FailureDetail,
    },
}

// Bounded internal diagnostics only: no SQL statements, messages or values.
// Retain both error classifications before reducing the public response.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FailureDetail {
    pub(crate) kind: &'static str,
    pub(crate) sql_state: Option<[u8; 5]>,
    pub(crate) constraint: [u8; 32],
    pub(crate) constraint_bytes: usize,
    pub(crate) constraint_truncated: bool,
}
impl FailureDetail {
    pub(crate) fn new(
        kind: &'static str,
        sql_state: Option<&str>,
        constraint: Option<&str>,
    ) -> Self {
        let bytes = constraint.unwrap_or("").as_bytes();
        let retained = bytes.len().min(32);
        let mut prefix = [0; 32];
        prefix[..retained].copy_from_slice(&bytes[..retained]);
        Self {
            kind,
            sql_state: sql_state.and_then(|value| value.as_bytes().try_into().ok()),
            constraint: prefix,
            constraint_bytes: bytes.len(),
            constraint_truncated: bytes.len() > retained,
        }
    }
}

pub(crate) struct RuntimeLiveness {
    state: watch::Sender<bool>,
    stop_deadline: OnceLock<Instant>,
    failure: OnceLock<FatalCause>,
    #[cfg(test)]
    pub(crate) probe: probe::Probe,
}

impl RuntimeLiveness {
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            state: watch::channel(true).0,
            stop_deadline: OnceLock::new(),
            failure: OnceLock::new(),
            #[cfg(test)]
            probe: probe::Probe::default(),
        })
    }

    pub(crate) fn is_live(&self) -> bool {
        *self.state.borrow()
    }

    pub(crate) fn stop(&self) {
        // Retain the stop even when nobody is currently observing it.
        self.stop_deadline
            .get_or_init(|| Instant::now() + Duration::from_secs(5));
        self.state.send_replace(false);
    }

    pub(crate) fn fail(&self, cause: FatalCause) {
        // Publish the first fault before waking shutdown. Intentional cleanup
        // may follow, but cannot erase a fault or renew the first stop budget.
        let _ = self.failure.set(cause);
        self.stop();
    }

    pub(crate) fn failure(&self) -> Option<FatalCause> {
        self.failure.get().copied()
    }

    /// Stop and obtain the first stop's budget. Repeated cleanup cannot renew it.
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn shutdown_deadline(&self) -> Instant {
        self.stop();
        *self
            .stop_deadline
            .get()
            .expect("stop establishes its deadline")
    }

    pub(crate) async fn stopped(&self) {
        let mut receiver = self.state.subscribe();
        // Channel failure also ends the wait: notification failure cannot
        // reopen admission. There is no caller-accessible reset operation.
        let _ = receiver.wait_for(|live| !*live).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{future::Future, task::Poll, time::Duration};

    #[test]
    fn a_later_fault_promotes_stop_without_clearing_its_first_cause_or_budget() {
        let live = RuntimeLiveness::new();
        live.stop();
        let deadline = live.shutdown_deadline();
        assert_eq!(live.failure(), None);
        live.fail(FatalCause::OwnedExecution);
        live.stop();
        live.fail(FatalCause::NativeIntegrity);
        assert_eq!(live.failure(), Some(FatalCause::OwnedExecution));
        assert_eq!(live.shutdown_deadline(), deadline);
        assert!(!live.is_live());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn active_and_late_observers_both_see_irreversible_stop() {
        let live = RuntimeLiveness::new();
        let mut active = std::pin::pin!(live.stopped());
        std::future::poll_fn(|context| {
            assert!(active.as_mut().poll(context).is_pending());
            Poll::Ready(())
        })
        .await;
        live.stop();
        let deadline = live.shutdown_deadline();
        tokio::time::timeout(Duration::from_secs(1), active)
            .await
            .unwrap();
        assert!(!live.is_live());
        live.stop();
        assert_eq!(live.shutdown_deadline(), deadline);
        tokio::time::timeout(Duration::from_secs(1), live.stopped())
            .await
            .unwrap();
        assert!(!live.is_live());
    }
}
