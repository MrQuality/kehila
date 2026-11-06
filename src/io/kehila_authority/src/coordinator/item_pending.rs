//! Pending creation inspection and transitions never execute a domain command.
use super::*;
use crate::{
    item_pending::{Command, Inspection, Write},
    postgres::transaction::NativeError,
};
use serde_json::json;

fn collision(error: &NativeError) -> bool {
    matches!(error,NativeError::Database(DbError::Constraint {code,name})
        if code=="23505" && matches!(name.as_str(),"pending_item_handle"|"pending_items_target_item_id_key"))
}

impl Engine {
    pub(super) async fn item_pending(
        &self,
        credential: Credential,
        command: Command,
        deadline: Instant,
    ) -> Result<Output, Error> {
        self.check(deadline)?;
        let entry = self.candidate(&credential)?;
        match command {
            Command::Inspect(command) => {
                self.inspect_pending_item(entry, &credential, command, deadline)
                    .await
            }
            Command::Write(command) => {
                self.write_pending_item(entry, &credential, command, deadline)
                    .await
            }
        }
    }

    async fn inspect_pending_item(
        &self,
        entry: Arc<Session>,
        credential: &Credential,
        command: Inspection,
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
        if let Err(error) = self.project_eligibility(&entry, credential, &fresh) {
            transaction
                .rollback()
                .await
                .map_err(|_| Error::Unavailable)?;
            return Err(error);
        }
        let value = match command {
            Inspection::List { project, before } => {
                transaction.list_pending_items(&project, before).await
            }
            Inspection::Open { project, handle } => {
                transaction.open_pending_item(&project, &handle).await
            }
        };
        transaction
            .rollback()
            .await
            .map_err(|_| Error::Unavailable)?;
        drop(database);
        let value = value.map_err(|error| super::project::native_error(&self.live, error))?;
        Ok(Output::Item(self.native_delivery(
            entry,
            fresh,
            reply(200, value),
            deadline,
        )?))
    }

    async fn write_pending_item(
        &self,
        entry: Arc<Session>,
        credential: &Credential,
        mut command: Write,
        deadline: Instant,
    ) -> Result<Output, Error> {
        loop {
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
            let outcome = match &command {
                Write::Register(registration) => transaction.register_item(registration).await,
                Write::Cancel { project, handle } => transaction.cancel_item(project, handle).await,
            };
            let pending = match outcome {
                Ok(value) => value,
                Err(error) => {
                    #[cfg(test)]
                    if let NativeError::Database(
                        DbError::Constraint { code, .. } | DbError::Rejected(code),
                    ) = &error
                    {
                        self.live.probe.record_sql_state(code);
                    }
                    let retry = matches!(&command, Write::Register(_)) && collision(&error);
                    transaction
                        .rollback()
                        .await
                        .map_err(|_| Error::Unavailable)?;
                    drop(database);
                    if retry {
                        #[cfg(test)]
                        self.live
                            .probe
                            .transaction_point(
                                0,
                                crate::lifecycle::probe::Point::RegistrationRollback,
                                deadline,
                            )
                            .await;
                        self.check(deadline)?;
                        let Write::Register(registration) = command else {
                            return Err(Error::Unavailable);
                        };
                        command = Write::Register(Box::new(
                            (*registration)
                                .next()?
                                .ok_or(Error::PendingIdentityExhausted)?,
                        ));
                        continue;
                    }
                    return Err(super::project::native_error(&self.live, error));
                }
            };
            if let Err(error) = transaction.force_project_constraints().await {
                transaction
                    .rollback()
                    .await
                    .map_err(|_| Error::Unavailable)?;
                return Err(super::project::native_error(&self.live, error));
            }
            if let Err(error) = self.project_eligibility(&entry, credential, &fresh) {
                transaction
                    .rollback()
                    .await
                    .map_err(|_| Error::Unavailable)?;
                return Err(error);
            }
            #[cfg(test)]
            self.live
                .probe
                .transaction_point(
                    if matches!(&command, Write::Register(_)) {
                        0
                    } else {
                        pending.id()
                    },
                    crate::lifecycle::probe::Point::PendingCommit,
                    deadline,
                )
                .await;
            let authority = transaction.commit().await.map_err(|error| {
                super::project::native_error(&self.live, NativeError::Database(error))
            })?;
            drop(database);
            let status = if matches!(&command, Write::Register(_)) {
                201
            } else {
                200
            };
            // Release the potentially large admitted request before response
            // materialization. No alternative identity survives an accepted COMMIT.
            drop(command);
            return Ok(Output::Item(self.native_delivery(
                entry,
                snapshot(&authority),
                reply(status, json!({"entry":pending.value()})),
                deadline,
            )?));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_expected_generated_identity_constraints_are_collision_retries() {
        for name in ["pending_item_handle", "pending_items_target_item_id_key"] {
            assert!(collision(&NativeError::Database(DbError::Constraint {
                code: "23505".into(),
                name: name.into()
            })));
            assert!(!collision(&NativeError::Database(DbError::Constraint {
                code: "23514".into(),
                name: name.into()
            })));
        }
        assert!(!collision(&NativeError::Database(DbError::UnknownCommit)));
        assert!(!collision(&NativeError::Database(DbError::Constraint {
            code: "23505".into(),
            name: "unrelated_identity".into()
        })));
    }
}
