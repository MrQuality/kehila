use super::*;
use crate::lifecycle::FatalCause;
use crate::{
    postgres::transaction::{Current, NativeError, Pending, PendingState, Replay},
    project::{self as domain, Command, Failure},
};
use serde_json::{json, Value};

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

pub(super) fn native_error(live: &RuntimeLiveness, error: NativeError) -> Error {
    #[cfg(test)]
    if let NativeError::Database(DbError::Rejected(code) | DbError::Constraint { code, .. }) =
        &error
    {
        live.probe.record_sql_state(code);
    }
    match error {
        NativeError::Rejected(reason) => Error::Project(reason),
        NativeError::Database(DbError::UnknownCommit) => Error::Project(Failure::OutcomeUnknown),
        NativeError::Database(DbError::Rejected(code)) if code == "55000" => Error::Unauthorized,
        NativeError::Database(DbError::Rejected(code)) if code == "42501" => {
            Error::Project(Failure::Forbidden)
        }
        NativeError::Database(DbError::Rejected(code)) if code == "54000" => Error::Busy,
        NativeError::Integrity | NativeError::Database(DbError::Constraint { .. }) => {
            live.fail(FatalCause::NativeIntegrity);
            Error::Unavailable
        }
        _ => Error::Unavailable,
    }
}

impl Engine {
    fn project_delivery(
        &self,
        entry: Arc<Session>,
        fresh: Snapshot,
        reply: Reply,
        deadline: Instant,
    ) -> Result<Output, Error> {
        Ok(Output::Project(
            self.native_delivery(entry, fresh, reply, deadline)?,
        ))
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
        let reply = outcome.map_err(|error| native_error(&self.live, error))?;
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
                return Err(native_error(&self.live, error));
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
            .map_err(|error| native_error(&self.live, NativeError::Database(error)))?;
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
                        .map_err(|error| native_error(&self.live, NativeError::Database(error)))?;
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
                    return Err(native_error(&self.live, error));
                }
            }
        }
        Err(Error::Busy)
    }
}

#[cfg(test)]
mod failure_tests {
    use super::*;

    #[test]
    fn decoded_integrity_and_named_constraints_retain_fatal_status() {
        for error in [
            NativeError::Integrity,
            NativeError::Database(DbError::Constraint {
                code: "23514".to_owned(),
                name: "native_test_constraint".to_owned(),
            }),
        ] {
            let live = RuntimeLiveness::new();
            assert_eq!(native_error(&live, error), Error::Unavailable);
            live.stop();
            assert!(!live.is_live());
            assert_eq!(live.failure(), Some(FatalCause::NativeIntegrity));
        }
    }

    #[test]
    fn expected_rejections_and_nonconstraint_errors_do_not_become_fatal() {
        for (error, expected) in [
            (
                NativeError::Rejected(Failure::Conflict),
                Error::Project(Failure::Conflict),
            ),
            (
                NativeError::Database(DbError::Rejected("54000".to_owned())),
                Error::Busy,
            ),
            (
                NativeError::Database(DbError::Rejected("PZ001".to_owned())),
                Error::Unavailable,
            ),
        ] {
            let live = RuntimeLiveness::new();
            assert_eq!(native_error(&live, error), expected);
            assert!(live.is_live());
            assert_eq!(live.failure(), None);
        }
    }
}
