//! One selected pending creation; observation never retains its admission permit.
use super::Probe;
use std::{
    sync::{Arc, Weak},
    time::Instant,
};
use tokio::sync::OwnedSemaphorePermit;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AttemptEntry {
    Preparation,
    Write,
    Reprepare,
}
pub(crate) struct ItemAttempt {
    project: String,
    handle: String,
    request_bytes: Vec<u8>,
    pub(crate) deadline: Option<Instant>,
    pub(crate) admission: Weak<OwnedSemaphorePermit>,
    pub(crate) entries: Vec<AttemptEntry>,
}
impl Probe {
    pub(crate) fn watch_item_attempt(&self, project: &str, handle: &str, request_bytes: Vec<u8>) {
        let mut slot = self.item_attempt.lock().unwrap();
        assert!(slot.is_none(), "one selected item attempt per runtime");
        *slot = Some(ItemAttempt {
            project: project.into(),
            handle: handle.into(),
            request_bytes,
            deadline: None,
            admission: Weak::new(),
            entries: Vec::new(),
        });
    }
    pub(crate) fn begin_item_attempt(
        &self,
        project: &str,
        handle: &str,
        deadline: Instant,
        admission: &Arc<OwnedSemaphorePermit>,
    ) -> bool {
        let mut slot = self.item_attempt.lock().unwrap();
        let Some(watch) = slot
            .as_mut()
            .filter(|w| w.project == project && w.handle == handle)
        else {
            return false;
        };
        assert!(
            watch.deadline.replace(deadline).is_none(),
            "selected attempt restarted"
        );
        watch.admission = Arc::downgrade(admission);
        true
    }
    pub(crate) fn item_attempt_entry(
        &self,
        entry: AttemptEntry,
        deadline: Instant,
        admission: &Arc<OwnedSemaphorePermit>,
    ) {
        let mut slot = self.item_attempt.lock().unwrap();
        let watch = slot.as_mut().expect("selected attempt lost its observer");
        assert_eq!(watch.deadline, Some(deadline), "retry renewed its deadline");
        assert!(
            Weak::ptr_eq(&watch.admission, &Arc::downgrade(admission)),
            "retry replaced admission"
        );
        assert!(Instant::now() < deadline, "observed attempt expired");
        watch.entries.push(entry);
    }
    pub(crate) fn take_item_attempt(&self) -> ItemAttempt {
        self.item_attempt
            .lock()
            .unwrap()
            .take()
            .expect("missing selected attempt")
    }
    pub(crate) fn item_attempt_bytes(&self, bytes: &[u8]) {
        let slot = self.item_attempt.lock().unwrap();
        assert_eq!(
            slot.as_ref()
                .expect("missing selected attempt")
                .request_bytes,
            bytes,
            "retry changed original canonical request bytes"
        );
    }
}
