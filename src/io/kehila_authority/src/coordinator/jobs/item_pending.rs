//! Tracked pending-command admission shares the original serving reservation.
use super::*;
use crate::item_pending::{self as domain, ingress::Input};

impl Handle {
    pub(crate) fn prepare_item_pending(
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
                    .prepare_item_pending(credential, input, permit, deadline)
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
    async fn prepare_item_pending(
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
        let command = match input {
            Input::Register(form) => {
                let identities = self
                    .hash(deadline, |worker| {
                        worker.item_registration_identities(deadline, permit.clone())
                    })
                    .await?;
                #[cfg(test)]
                let identities = self
                    .live
                    .probe
                    .take_registration_identities()
                    .unwrap_or(identities);
                domain::Command::Write(domain::Write::Register(Box::new(form.prepare(identities)?)))
            }
            Input::List { project, before } => {
                domain::Command::Inspect(domain::Inspection::List { project, before })
            }
            Input::Open { project, handle } => {
                domain::Command::Inspect(domain::Inspection::Open { project, handle })
            }
            Input::Cancel { project, handle } => {
                domain::Command::Write(domain::Write::Cancel { project, handle })
            }
        };
        self.check(deadline)?;
        Ok(Operation::ItemPending {
            credential,
            command,
        })
    }
}
