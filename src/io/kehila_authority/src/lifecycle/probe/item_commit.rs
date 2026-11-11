//! One exact canonical edit, observed immediately before its owned COMMIT.
use super::{Probe, TransactionGate};
use std::time::Instant;
use tokio::sync::oneshot;

pub(super) struct Armed {
    bytes: Vec<u8>,
    hold: Selected,
}

pub(crate) struct Selected {
    arrived: oneshot::Sender<()>,
    release: oneshot::Receiver<()>,
}

impl Probe {
    pub(crate) fn arm_item_commit(&self, bytes: Vec<u8>) -> TransactionGate {
        assert!(matches!(
            work_item_codec::decode_request(1, &bytes).unwrap(),
            work_item_codec::Request::Edit { .. }
        ));
        let (arrived, observed) = oneshot::channel();
        let (release, held) = oneshot::channel();
        let mut slot = self.item_commit.lock().unwrap();
        assert!(slot.is_none(), "one exact edit barrier per fixture");
        *slot = Some(Armed {
            bytes,
            hold: Selected {
                arrived,
                release: held,
            },
        });
        TransactionGate {
            arrived: observed,
            release,
        }
    }

    pub(crate) fn select_item_commit(&self, bytes: &[u8]) -> Option<Selected> {
        let mut slot = self.item_commit.lock().unwrap();
        if slot.as_ref().is_some_and(|armed| armed.bytes == bytes) {
            slot.take().map(|armed| armed.hold)
        } else {
            None
        }
    }
}

impl Selected {
    pub(crate) async fn before_commit(self, deadline: Instant) {
        let _ = self.arrived.send(());
        // The owner still checks eligibility after this wait. A dropped
        // release sender is a release, never evidence that SQL rolled back.
        let _ = tokio::time::timeout_at(deadline.into(), self.release)
            .await
            .expect("exact edit COMMIT barrier expired");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use task_contract::{item_mutation::*, work_item::*};

    fn edit(token: &str, version: u64) -> Vec<u8> {
        work_item_codec::encode_request(
            &work_item_codec::Request::Edit {
                project_id: ProjectId("project".into()),
                item_id: WorkItemId("item".into()),
                command: EditCommand {
                    operation_id: token.into(),
                    expected_item_version: version,
                    expected_configuration_revision: 1,
                    estimate: EstimateEdit::Keep,
                    fields: vec![],
                },
            },
            1,
        )
        .unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn only_exact_command_selects_once_and_arrival_precedes_release() {
        let probe = Probe::default();
        let bytes = edit(&"a".repeat(32), 1);
        let mut gate = probe.arm_item_commit(bytes.clone());
        assert!(probe
            .select_item_commit(&edit(&"b".repeat(32), 1))
            .is_none());
        assert!(probe
            .select_item_commit(&edit(&"a".repeat(32), 2))
            .is_none());
        let hold = probe.select_item_commit(&bytes).unwrap();
        assert!(probe.select_item_commit(&bytes).is_none());
        assert!(matches!(
            gate.arrived.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ));
        let running =
            tokio::spawn(hold.before_commit(Instant::now() + std::time::Duration::from_secs(1)));
        gate.arrived.await.unwrap();
        assert!(!running.is_finished());
        gate.release.send(()).unwrap();
        running.await.unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dropped_controller_releases_instead_of_proving_rollback() {
        let probe = Probe::default();
        let bytes = edit(&"a".repeat(32), 1);
        let gate = probe.arm_item_commit(bytes.clone());
        let hold = probe.select_item_commit(&bytes).unwrap();
        drop(gate);
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            hold.before_commit(Instant::now() + std::time::Duration::from_secs(1)),
        )
        .await
        .unwrap();
    }
}
