//! Shared irreversible local stop state; not proof of database lock ownership.
#[cfg(any(target_os = "linux", test))]
use std::sync::Arc;
use tokio::sync::watch;

pub(crate) struct RuntimeLiveness(watch::Sender<bool>);

impl RuntimeLiveness {
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self(watch::channel(true).0))
    }

    pub(crate) fn is_live(&self) -> bool {
        *self.0.borrow()
    }

    pub(crate) fn stop(&self) {
        // Retain the stop even when nobody is currently observing it.
        self.0.send_replace(false);
    }

    #[cfg_attr(
        all(not(test), not(target_os = "linux")),
        expect(dead_code, reason = "Application-owner shutdown wiring is in progress")
    )]
    pub(crate) async fn stopped(&self) {
        let mut receiver = self.0.subscribe();
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
        tokio::time::timeout(Duration::from_secs(1), active)
            .await
            .unwrap();
        assert!(!live.is_live());
        live.stop();
        tokio::time::timeout(Duration::from_secs(1), live.stopped())
            .await
            .unwrap();
        assert!(!live.is_live());
    }
}
