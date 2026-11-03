use super::*;
use crate::{native_ingress::Input, project as domain};

impl Handle {
    pub(crate) fn prepare_project(
        &self,
        credential: Credential,
        input: Input,
        deadline: Instant,
    ) -> Result<oneshot::Receiver<Result<preparation::Prepared<Operation>, Error>>, Error> {
        // Caller has already counted and decoded the original bytes. Reject
        // current invalid form input before even preparatory task scheduling.
        if let Input::Register(form) | Input::Execute { form, .. } = &input {
            domain::validate_form(form)?;
        }
        let reservation = self.reserve(deadline)?;
        let active = self
            .preparations
            .begin(&self.engine, &self.live, reservation)?;
        let receiver = self.preparations.spawn(
            active,
            self.live.clone(),
            move |engine, permit, deadline| async move {
                engine
                    .prepare_project(credential, input, permit, deadline)
                    .await
            },
        )?;
        #[cfg(test)]
        self.live
            .probe
            .record(crate::lifecycle::probe::Stage::Preparation);
        Ok(receiver)
    }
    pub(crate) fn submit_project(
        &self,
        prepared: preparation::Prepared<Operation>,
    ) -> Result<oneshot::Receiver<Response>, Error> {
        self.submit_reserved(prepared.value, prepared.reservation)
    }
}

impl Engine {
    async fn prepare_project(
        &self,
        credential: Credential,
        input: Input,
        permit: Arc<OwnedSemaphorePermit>,
        deadline: Instant,
    ) -> Result<Operation, Error> {
        self.check(deadline)?;
        let entry = self.candidate(&credential)?;
        // This is only an in-memory prefilter. It neither authorizes native
        // execution nor accesses the serving mutex/database before admission.
        let preliminary = Snapshot {
            binding: entry.binding.clone(),
            actor_active: true,
        };
        if credential.kind != Kind::Browser || !entry.eligible(&preliminary, Instant::now()) {
            return Err(Error::Unauthorized);
        }
        self.csrf(&entry, &credential, &preliminary)
            .map_err(|_| Error::Project(domain::Failure::Forbidden))?;
        let command = match input {
            Input::Binding => domain::Command::Binding,
            Input::Read(id) => domain::Command::Read(id),
            Input::List(before) => domain::Command::List(before),
            Input::Open(id) => domain::Command::Open(id),
            Input::Cancel(id) => domain::Command::Cancel(id),
            Input::Reconcile(id) => domain::Command::Reconcile(id),
            Input::Register(form) => {
                domain::validate_form(&form)?;
                let (handle, token) = self
                    .hash(deadline, |worker| {
                        worker.pending_identity(deadline, permit.clone())
                    })
                    .await?;
                domain::Command::Register {
                    form,
                    handle,
                    token,
                }
            }
            Input::Execute {
                request_id,
                token,
                form,
            } => {
                domain::validate_form(&form)?;
                #[cfg(test)]
                let overflow = self.live.probe.take_seed_overflow(request_id);
                let targets = self
                    .hash(deadline, |worker| {
                        worker.project_identities(deadline, permit.clone())
                    })
                    .await?;
                #[cfg(not(test))]
                let candidates = domain::Candidates::new(form, token, targets)?;
                #[cfg(test)]
                let candidates = if overflow {
                    domain::Candidates::seed_overflow_for_test(form, token, targets)?
                } else {
                    domain::Candidates::new(form, token, targets)?
                };
                domain::Command::Execute {
                    request_id,
                    candidates,
                }
            }
        };
        self.check(deadline)?;
        Ok(Operation::Project {
            credential,
            command,
        })
    }
}
