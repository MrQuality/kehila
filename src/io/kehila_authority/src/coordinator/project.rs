use super::*;
use crate::{
    postgres::transaction::{Current, NativeError, Pending, PendingState, Replay},
    project::{self as domain, Command, Failure},
};
use serde_json::{json, Value};

pub(crate) struct ProjectDelivery {
    status: u16,
    body: Arc<Zeroizing<Vec<u8>>>,
    entry: Arc<Session>,
    snapshot: Snapshot,
    deadline: Instant,
}
pub(crate) struct ProjectMaterial {
    pub(crate) status: u16,
    pub(crate) body: Arc<Zeroizing<Vec<u8>>>,
}
impl ProjectDelivery {
    pub(crate) fn check(&self) -> Result<(), Error> {
        if Instant::now() >= self.deadline || !self.entry.eligible(&self.snapshot, Instant::now()) {
            Err(Error::Unauthorized)
        } else {
            Ok(())
        }
    }
    pub(crate) fn material(&self) -> Result<ProjectMaterial, Error> {
        self.check()?;
        Ok(ProjectMaterial {
            status: self.status,
            body: self.body.clone(),
        })
    }
}

struct Reply {
    status: u16,
    value: Value,
    business: bool,
}
fn reply(status: u16, value: Value) -> Reply {
    Reply {
        status,
        value,
        business: status < 400,
    }
}
fn pending_value(value: &Pending) -> Value {
    json!({"request_id":value.id.to_string(),"token":value.token,
        "form_version":1,"form":value.form.as_ref().map(|form|json!({"name":form.name,
            "prefix":form.prefix,"estimate_unit":domain::unit_name(form.estimate_unit)})),
        "registered_ms":value.registered_ms.to_string(),"expires_ms":value.expires_ms.to_string(),
        "state":match value.state {PendingState::Prepared=>"prepared",PendingState::Completed=>"completed",
            PendingState::Cancelled=>"cancelled",PendingState::Expired=>"expired"},"project_id":value.project})
}
fn terminal(value: &Pending) -> Reply {
    let code = match value.state {
        PendingState::Cancelled => "pending_cancelled",
        _ => "pending_expired",
    };
    reply(410, json!({"error":code,"entry":pending_value(value)}))
}
fn saved(value: Replay) -> Result<Reply, NativeError> {
    match value.saved {
        Some(saved) => Ok(reply(
            200,
            json!({"outcome":"replayed","entry":pending_value(&value.entry),
            "result":project_create_codec::parse_json(&saved.result_bytes).map_err(|_|NativeError::Integrity)?}),
        )),
        None => Ok(reply(
            410,
            json!({"error":"replay_expired","entry":pending_value(&value.entry)}),
        )),
    }
}
fn current_reply(current: Current) -> Result<Reply, NativeError> {
    let project = project_create_codec::encode_project_snapshot(&current.project)
        .map_err(|_| NativeError::Integrity)?;
    let configuration = project_create_codec::encode_snapshot(&current.configuration)
        .map_err(|_| NativeError::Integrity)?;
    Ok(reply(
        200,
        json!({"outcome":"read",
        "project":project_create_codec::parse_json(&project).map_err(|_|NativeError::Integrity)?,
        "configuration":project_create_codec::parse_json(&configuration).map_err(|_|NativeError::Integrity)?}),
    ))
}
fn retryable(error: &NativeError) -> bool {
    match error {
        NativeError::Database(DbError::Constraint { code, name }) => {
            code == "23505" && matches!(name.as_str(), "projects_pkey" | "operations_scoped_key")
        }
        NativeError::Database(DbError::Rejected(code)) => {
            matches!(code.as_str(), "40001" | "40P01")
        }
        _ => false,
    }
}
fn needs_reconciliation(error: &NativeError) -> bool {
    matches!(error, NativeError::Rejected(Failure::Expired))
        || matches!(error,NativeError::Database(DbError::Rejected(code)) if code=="P0002")
}

impl Engine {
    fn native_error(&self, error: NativeError) -> Error {
        #[cfg(test)]
        if let NativeError::Database(DbError::Rejected(code) | DbError::Constraint { code, .. }) =
            &error
        {
            self.live.probe.record_sql_state(code);
        }
        match error {
            NativeError::Rejected(reason) => Error::Project(reason),
            NativeError::Database(DbError::UnknownCommit) => {
                Error::Project(Failure::OutcomeUnknown)
            }
            NativeError::Database(DbError::Rejected(code)) if code == "55000" => {
                Error::Unauthorized
            }
            NativeError::Database(DbError::Rejected(code)) if code == "42501" => {
                Error::Project(Failure::Forbidden)
            }
            NativeError::Database(DbError::Rejected(code)) if code == "54000" => Error::Busy,
            NativeError::Integrity | NativeError::Database(DbError::Constraint { .. }) => {
                self.live.stop();
                Error::Unavailable
            }
            _ => Error::Unavailable,
        }
    }
    fn project_eligibility(
        &self,
        entry: &Session,
        credential: &Credential,
        fresh: &Snapshot,
    ) -> Result<(), Error> {
        if credential.kind != Kind::Browser {
            return Err(Error::Unauthorized);
        }
        self.csrf(entry, credential, fresh)
            .map_err(|_| Error::Project(Failure::Forbidden))?;
        if entry.eligible(fresh, Instant::now()) {
            Ok(())
        } else {
            Err(Error::Unauthorized)
        }
    }
    fn project_delivery(
        &self,
        entry: Arc<Session>,
        fresh: Snapshot,
        reply: Reply,
        deadline: Instant,
    ) -> Result<Output, Error> {
        self.check(deadline)?;
        let body = domain::wire::encode(&reply.value)?;
        if !entry.eligible(&fresh, Instant::now())
            || (reply.business && !entry.successful_business(&fresh, Instant::now()))
        {
            return Err(Error::Unauthorized);
        }
        self.check(deadline)?;
        Ok(Output::Project(ProjectDelivery {
            status: reply.status,
            body: Arc::new(body),
            entry,
            snapshot: fresh,
            deadline,
        }))
    }

    pub(super) async fn project(
        &self,
        credential: Credential,
        command: Command,
        deadline: Instant,
    ) -> Result<Output, Error> {
        let entry = self.candidate(&credential)?;
        match command {
            Command::Binding => self.project_read(entry, &credential, None, deadline).await,
            Command::Read(id) => {
                self.project_read(entry, &credential, Some(id), deadline)
                    .await
            }
            Command::Execute {
                request_id,
                candidates,
            } => {
                self.project_execute(entry, &credential, request_id, candidates, deadline)
                    .await
            }
            command => {
                self.project_pending(entry, &credential, command, deadline)
                    .await
            }
        }
    }
    async fn project_read(
        &self,
        entry: Arc<Session>,
        credential: &Credential,
        id: Option<String>,
        deadline: Instant,
    ) -> Result<Output, Error> {
        #[cfg(test)]
        self.live
            .probe
            .record(crate::lifecycle::probe::Stage::NativeMutex);
        let mut database = tokio::time::timeout_at(
            tokio::time::Instant::from_std(deadline),
            self.database.lock(),
        )
        .await
        .map_err(|_| Error::Unavailable)?;
        let binding = &entry.binding;
        let mut transaction = database
            .read_snapshot(
                &binding.installation,
                binding.runtime_generation,
                binding.auth_generation,
                deadline,
            )
            .await
            .map_err(fence_error)?;
        let fresh = snapshot(transaction.authority());
        #[cfg(test)]
        self.live
            .probe
            .transaction_point(0, crate::lifecycle::probe::Point::ReadSnapshot, deadline)
            .await;
        if let Err(error) = self.project_eligibility(&entry, credential, &fresh) {
            transaction
                .rollback()
                .await
                .map_err(|_| Error::Unavailable)?;
            return Err(error);
        }
        let outcome = if let Some(id) = id {
            match transaction.read_project(&id).await {
                Ok(current) => current_reply(current),
                Err(error) => Err(error),
            }
        } else {
            Ok(Reply {
                status: 200,
                business: false,
                value: json!({"installation_id":fresh.binding.installation,"actor_id":fresh.binding.owner}),
            })
        };
        transaction
            .rollback()
            .await
            .map_err(|_| Error::Unavailable)?;
        drop(database);
        let reply = outcome.map_err(|error| self.native_error(error))?;
        self.project_delivery(entry, fresh, reply, deadline)
    }

    async fn project_pending(
        &self,
        entry: Arc<Session>,
        credential: &Credential,
        command: Command,
        deadline: Instant,
    ) -> Result<Output, Error> {
        self.check(deadline)?;
        #[cfg(test)]
        self.live
            .probe
            .record(crate::lifecycle::probe::Stage::NativeMutex);
        let mut database = tokio::time::timeout_at(
            tokio::time::Instant::from_std(deadline),
            self.database.lock(),
        )
        .await
        .map_err(|_| Error::Unavailable)?;
        let binding = &entry.binding;
        let mut transaction = database
            .fenced_session(
                &binding.installation,
                binding.runtime_generation,
                binding.auth_generation,
                deadline,
            )
            .await
            .map_err(fence_error)?;
        let fresh = snapshot(transaction.authority());
        if let Err(error) = self.project_eligibility(&entry, credential, &fresh) {
            transaction
                .rollback()
                .await
                .map_err(|_| Error::Unavailable)?;
            return Err(error);
        }
        let outcome:Result<Reply,NativeError>=async {
            let cancel=matches!(&command,Command::Cancel(_));
            let reconcile=matches!(&command,Command::Reconcile(_));
            match command {
                Command::Register {form,handle,token}=>{
                    let pending=transaction.register_project(&form,&handle,&token).await?;
                    Ok(reply(201,json!({"outcome":"registered","entry":pending_value(&pending)})))
                },
                Command::List(before)=>{
                    let page=transaction.list_projects(before).await?;
                    Ok(reply(200,json!({"entries":page.entries.iter().map(pending_value).collect::<Vec<_>>(),
                        "before":page.before.map(|value|value.to_string())})))
                },
                Command::Open(id)|Command::Cancel(id)|Command::Reconcile(id)=>{
                    let pending=transaction.open_project(id,cancel).await?.ok_or(NativeError::Rejected(Failure::Absent))?;
                    #[cfg(test)]
                    if cancel {self.live.probe.transaction_point(id,crate::lifecycle::probe::Point::CancelCommit,deadline).await;}
                    if reconcile {
                        match pending.state {
                            PendingState::Completed=>saved(transaction.saved_project(id,true,None).await?),
                            PendingState::Cancelled|PendingState::Expired=>Ok(terminal(&pending)),
                            PendingState::Prepared=>Ok(reply(200,json!({"outcome":"not_completed_at_lookup","entry":pending_value(&pending)}))),
                        }
                    } else {Ok(reply(200,json!({"entry":pending_value(&pending)})))}
                },
                _=>Err(NativeError::Integrity),
            }
        }.await;
        let reply = match outcome {
            Ok(reply) => reply,
            Err(error) => {
                transaction
                    .rollback()
                    .await
                    .map_err(|_| Error::Unavailable)?;
                return Err(self.native_error(error));
            }
        };
        if let Err(error) = self.project_eligibility(&entry, credential, &fresh) {
            transaction
                .rollback()
                .await
                .map_err(|_| Error::Unavailable)?;
            return Err(error);
        }
        let authority = transaction
            .commit()
            .await
            .map_err(|error| self.native_error(NativeError::Database(error)))?;
        drop(database);
        self.project_delivery(entry, snapshot(&authority), reply, deadline)
    }

    async fn project_execute(
        &self,
        entry: Arc<Session>,
        credential: &Credential,
        id: i64,
        mut candidates: domain::Candidates,
        deadline: Instant,
    ) -> Result<Output, Error> {
        while let Some(prepared) = candidates.next_admitted() {
            // Every retry arrives here only after confirmed full rollback and
            // release. Consuming the sealed immutable proof is admission again;
            // no caller bytes, entropy or new envelope can enter under a DB lock.
            self.check(deadline)?;
            #[cfg(test)]
            self.live
                .probe
                .record(crate::lifecycle::probe::Stage::NativeMutex);
            let mut database = tokio::time::timeout_at(
                tokio::time::Instant::from_std(deadline),
                self.database.lock(),
            )
            .await
            .map_err(|_| Error::Unavailable)?;
            let binding = &entry.binding;
            let mut transaction = database
                .fenced_session(
                    &binding.installation,
                    binding.runtime_generation,
                    binding.auth_generation,
                    deadline,
                )
                .await
                .map_err(fence_error)?;
            let fresh = snapshot(transaction.authority());
            if let Err(error) = self.project_eligibility(&entry, credential, &fresh) {
                transaction
                    .rollback()
                    .await
                    .map_err(|_| Error::Unavailable)?;
                return Err(error);
            }
            let outcome:Result<Reply,NativeError>=async {
                let pending=transaction.open_project(id,false).await?.ok_or(NativeError::Rejected(Failure::Absent))?;
                #[cfg(test)]
                self.live.probe.transaction_point(id,crate::lifecycle::probe::Point::ExecuteLookup,deadline).await;
                match pending.state {
                    PendingState::Completed=>saved(transaction.saved_project(id,true,Some(prepared.command())).await?),
                    PendingState::Cancelled|PendingState::Expired=>Ok(terminal(&pending)),
                    PendingState::Prepared=>{
                        let operation=transaction.write_project(id,&prepared).await?;
                        #[cfg(test)]
                        self.live.probe.transaction_point(id,crate::lifecycle::probe::Point::BeforeCompletion,deadline).await;
                        transaction.complete_project(id,&prepared,operation).await?;
                        let current=transaction.reload_created(&prepared.command().project_id.0).await?;
                        let recorded=transaction.saved_project(id,false,Some(prepared.command())).await?;
                        let stored=recorded.saved.as_ref().ok_or(NativeError::Integrity)?;
                        if current.creation_operation!=operation || current.project!=prepared.result().project
                            || current.configuration!=prepared.result().configuration || stored.request!=*prepared.command()
                            || stored.result!=*prepared.result() {return Err(NativeError::Integrity);}
                        transaction.force_project_constraints().await?;
                        Ok(reply(201,json!({"outcome":"created","entry":pending_value(&recorded.entry),
                            "result":project_create_codec::parse_json(&stored.result_bytes).map_err(|_|NativeError::Integrity)?})))
                    },
                }
            }.await;
            match outcome {
                Ok(reply) => {
                    if let Err(error) = self.project_eligibility(&entry, credential, &fresh) {
                        transaction
                            .rollback()
                            .await
                            .map_err(|_| Error::Unavailable)?;
                        return Err(error);
                    }
                    let authority = transaction
                        .commit()
                        .await
                        .map_err(|error| self.native_error(NativeError::Database(error)))?;
                    drop(database);
                    return self.project_delivery(entry, snapshot(&authority), reply, deadline);
                }
                Err(error) => {
                    transaction
                        .rollback()
                        .await
                        .map_err(|_| Error::Unavailable)?;
                    drop(database);
                    if needs_reconciliation(&error) {
                        return self
                            .project_pending(entry, credential, Command::Reconcile(id), deadline)
                            .await;
                    }
                    if retryable(&error) {
                        #[cfg(test)]
                        self.live
                            .probe
                            .transaction_point(
                                id,
                                crate::lifecycle::probe::Point::Rollback,
                                deadline,
                            )
                            .await;
                        continue;
                    }
                    return Err(self.native_error(error));
                }
            }
        }
        Err(Error::Busy)
    }
}
