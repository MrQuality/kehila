//! Shared irreversible local stop state; not proof of database lock ownership.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub(crate) struct RuntimeLiveness(AtomicBool);

impl RuntimeLiveness {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self(AtomicBool::new(true)))
    }

    pub(crate) fn is_live(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    pub(crate) fn stop(&self) {
        self.0.store(false, Ordering::Release);
    }
}
