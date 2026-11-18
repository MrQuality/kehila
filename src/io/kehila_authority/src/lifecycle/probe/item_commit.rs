//! Exact canonical commands, observed immediately before their owned COMMIT.
use super::{Probe, TransactionGate};
use std::{sync::Mutex, time::Instant};
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
        arm(&self.item_commit, bytes)
    }

    pub(crate) fn arm_selection_commit(&self, bytes: Vec<u8>) -> TransactionGate {
        selection_codec::decode_request(1, &bytes).unwrap();
        arm(&self.selection_commit, bytes)
    }

    pub(crate) fn select_item_commit(&self, bytes: &[u8]) -> Option<Selected> {
        select(&self.item_commit, bytes)
    }

    pub(crate) fn select_selection_commit(&self, bytes: &[u8]) -> Option<Selected> {
        select(&self.selection_commit, bytes)
    }
}

fn arm(slot: &Mutex<Option<Armed>>, bytes: Vec<u8>) -> TransactionGate {
    let (arrived, observed) = oneshot::channel();
    let (release, held) = oneshot::channel();
    let mut slot = slot.lock().unwrap();
    assert!(slot.is_none(), "one exact barrier per command family");
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

fn select(slot: &Mutex<Option<Armed>>, bytes: &[u8]) -> Option<Selected> {
    let mut slot = slot.lock().unwrap();
    if slot.as_ref().is_some_and(|armed| armed.bytes == bytes) {
        slot.take().map(|armed| armed.hold)
    } else {
        None
    }
}

impl Selected {
    pub(crate) async fn before_commit(self, deadline: Instant) {
        let _ = self.arrived.send(());
        // The owner still checks eligibility after this wait. A dropped
        // release sender is a release, never evidence that SQL rolled back.
        let _ = tokio::time::timeout_at(deadline.into(), self.release)
            .await
            .expect("exact command COMMIT barrier expired");
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

    fn selection(owner: &str, token: &str, version: u64, item: &str) -> Vec<u8> {
        selection_codec::encode_request(
            &task_contract::current_work::SelectionCommand {
                user_id: task_contract::current_work::UserId(owner.into()),
                operation_id: token.into(),
                expected_version: version,
                requested: Some(task_contract::relationship::EndpointIdentity {
                    project_id: ProjectId("project".into()),
                    item_id: WorkItemId(item.into()),
                }),
            },
            1,
        )
        .unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn exact_selection_and_edit_slots_are_independent() {
        let probe = Probe::default();
        let bytes = selection("owner", &"a".repeat(32), 1, "item");
        let gate = probe.arm_selection_commit(bytes.clone());
        let edit_bytes = edit(&"a".repeat(32), 1);
        let edit_gate = probe.arm_item_commit(edit_bytes.clone());
        for other in [
            selection("other-owner", &"a".repeat(32), 1, "item"),
            selection("owner", &"b".repeat(32), 1, "item"),
            selection("owner", &"a".repeat(32), 2, "item"),
            selection("owner", &"a".repeat(32), 1, "other-item"),
            selection_codec::encode_request(
                &task_contract::current_work::SelectionCommand {
                    requested: Some(task_contract::relationship::EndpointIdentity {
                        project_id: ProjectId("other-project".into()),
                        item_id: WorkItemId("item".into()),
                    }),
                    ..selection_codec::decode_request(1, &bytes).unwrap()
                },
                1,
            )
            .unwrap(),
            edit_bytes.clone(),
        ] {
            assert!(probe.select_selection_commit(&other).is_none());
        }
        assert!(probe.select_item_commit(&bytes).is_none());
        let hold = probe.select_selection_commit(&bytes).unwrap();
        assert!(probe.select_selection_commit(&bytes).is_none());
        let running =
            tokio::spawn(hold.before_commit(Instant::now() + std::time::Duration::from_secs(1)));
        gate.arrived.await.unwrap();
        assert!(!running.is_finished());
        gate.release.send(()).unwrap();
        running.await.unwrap();
        let edit_hold = probe.select_item_commit(&edit_bytes).unwrap();
        drop(edit_gate);
        edit_hold
            .before_commit(Instant::now() + std::time::Duration::from_secs(1))
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn selected_hold_owns_arrival_and_dropped_controller_releases() {
        let probe = Probe::default();
        let bytes = selection("owner", &"a".repeat(32), 1, "item");
        let mut gate = probe.arm_selection_commit(bytes.clone());
        let hold = probe.select_selection_commit(&bytes).unwrap();
        // Another lookup does not arrive or acquire a replacement hold.
        assert!(probe.select_selection_commit(&bytes).is_none());
        assert!(matches!(
            gate.arrived.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ));
        drop(gate);
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            hold.before_commit(Instant::now() + std::time::Duration::from_secs(1)),
        )
        .await
        .unwrap();
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
