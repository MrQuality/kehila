//! Tracked canonical edit preparation retains the original serving reservation.
use super::*;
use crate::item_mutation_ingress::{Input, Request};
use crate::postgres::transaction::{CanonicalItem, ItemAdmissionError};

impl Handle {
    pub(crate) fn prepare_item_mutation(
        &self,
        credential: Credential,
        input: Input,
        deadline: Instant,
    ) -> Result<oneshot::Receiver<Result<preparation::Prepared<Operation>, Error>>, Error> {
        let reservation = self.reserve(deadline)?;
        let active = self
            .preparations
            .begin(&self.engine, &self.live, reservation)?;
        let receiver = self.preparations.spawn(
            active,
            self.live.clone(),
            move |engine, permit, deadline| async move {
                engine
                    .prepare_item_mutation(credential, input, permit, deadline)
                    .await
            },
        )?;
        #[cfg(test)]
        self.live
            .probe
            .record(crate::lifecycle::probe::Stage::Preparation);
        Ok(receiver)
    }
}
impl Engine {
    async fn prepare_item_mutation(
        &self,
        credential: Credential,
        input: Input,
        permit: Arc<OwnedSemaphorePermit>,
        deadline: Instant,
    ) -> Result<Operation, Error> {
        self.check(deadline)?;
        let entry = self.candidate(&credential)?;
        let preliminary = Snapshot {
            binding: entry.binding.clone(),
            actor_active: true,
        };
        self.project_eligibility(&entry, &credential, &preliminary)?;
        let intent = match input.request {
            Request::Create { project, handle } => {
                super::super::item_write::Intent::Create { project, handle }
            }
            Request::Edit {
                project,
                item,
                command,
            } => {
                let installation = entry.binding.installation.clone();
                let actor = entry.binding.owner.clone();
                let request = work_item_codec::Request::Edit {
                    project_id: task_contract::work_item::ProjectId(project),
                    item_id: task_contract::work_item::WorkItemId(item),
                    command,
                };
                let prepared = self
                    .hash(deadline, |worker| {
                        worker.owned_preparation(deadline, permit, move || {
                            CanonicalItem::admit(&installation, &actor, request)
                        })
                    })
                    .await?;
                let canonical = prepared.map_err(|error| match error {
                    ItemAdmissionError::Representation(_) => Error::Invalid,
                    _ => {
                        self.live.fail(FatalCause::NativeIntegrity);
                        Error::Unavailable
                    }
                })?;
                super::super::item_write::Intent::Edit(Arc::new(canonical))
            }
        };
        self.check(deadline)?;
        Ok(Operation::ItemMutation {
            credential,
            command: super::super::item_write::Command::new(input.action, intent),
        })
    }
}
