//! Shared irreversible local stop state; not proof of database lock ownership.
#[cfg(any(target_os = "linux", test))]
use std::sync::Arc;
use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};
use tokio::sync::watch;

pub(crate) struct RuntimeLiveness {
    state: watch::Sender<bool>,
    stop_deadline: OnceLock<Instant>,
}

impl RuntimeLiveness {
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            state: watch::channel(true).0,
            stop_deadline: OnceLock::new(),
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

    /// Stop and obtain the first stop's budget. Repeated cleanup cannot renew it.
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn shutdown_deadline(&self) -> Instant {
        self.stop();
        *self
            .stop_deadline
            .get()
            .expect("stop establishes its deadline")
    }

    #[cfg_attr(
        all(not(test), not(target_os = "linux")),
        expect(dead_code, reason = "Application-owner shutdown wiring is in progress")
    )]
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
