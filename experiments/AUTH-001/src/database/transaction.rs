//! Owned serving transactions. Callers receive authority and fixed operations,
//! never a PostgreSQL client or an unrestricted transaction capability.
use super::*;
use crate::session::Kind;
use tokio_postgres::{types::ToSql, Transaction};

const FENCE: &str = "SELECT installation_id::text, owner_actor_id::text, runtime_generation::text, auth_generation::text, verifier_revision::text FROM auth001.fence($1::text::kehila.ident, $2::text::kehila.u64, $3::text::kehila.u64, $4::text::kehila.u64)";
const ISSUE: &str = "SELECT auth001.audit_issue($1::text::kehila.ident, $2::text::kehila.u64, $3::text::kehila.u64, $4::text::kehila.u64, $5::text::auth001.public_id, $6::text)";
const LOGOUT: &str = "SELECT auth001.audit_logout($1::text::kehila.ident, $2::text::kehila.u64, $3::text::kehila.u64, $4::text::auth001.public_id, $5::text)";
const REVOKE: &str = "SELECT auth001.audit_revoke($1::text::kehila.ident, $2::text::kehila.u64, $3::text::kehila.u64, $4::text::kehila.u64, $5::text::auth001.public_id, $6::text, $7::text::auth001.public_id)";
const CHANGE: &str = "SELECT installation_id::text, owner_actor_id::text, runtime_generation::text, auth_generation::text, verifier_revision::text FROM auth001.change_password($1::text::kehila.ident, $2::text::kehila.u64, $3::text::kehila.u64, $4::text::kehila.u64, $5::text::auth001.verifier, $6::text::auth001.public_id, $7::text::auth001.public_id)";

enum Expected<'a> {
    Password(&'a Authority),
    Session {
        installation: &'a str,
        runtime: u64,
        authentication: u64,
    },
}

pub(crate) enum PasswordProof {}
pub(crate) enum SessionProof {}

pub(crate) struct Fenced<'a, P> {
    proof: std::marker::PhantomData<fn() -> P>,
    transaction: Option<Transaction<'a>>,
    authority: Authority,
    pending: PendingOperation,
    deadline: Instant,
    failed: bool,
}

impl Database {
    // These constructors preserve how the caller established eligibility.
    // Session-only fencing never upgrades a fresh revision into password proof.
    pub(crate) async fn fenced_password(
        &mut self,
        proof: &Authority,
        deadline: Instant,
    ) -> Result<Fenced<'_, PasswordProof>, DbError> {
        self.fenced(Expected::Password(proof), deadline).await
    }

    pub(crate) async fn fenced_session(
        &mut self,
        installation: &str,
        runtime: u64,
        authentication: u64,
        deadline: Instant,
    ) -> Result<Fenced<'_, SessionProof>, DbError> {
        self.fenced(
            Expected::Session {
                installation,
                runtime,
                authentication,
            },
            deadline,
        )
        .await
    }
    async fn fenced<P>(
        &mut self,
        expected: Expected<'_>,
        deadline: Instant,
    ) -> Result<Fenced<'_, P>, DbError> {
        if !self.live.is_live() {
            return Err(DbError::Unavailable);
        }
        let (installation, runtime, authentication, revision) = match expected {
            Expected::Password(proof) => (
                proof.installation.as_str(),
                proof.runtime,
                proof.authentication,
                Some(proof.revision.to_string()),
            ),
            Expected::Session {
                installation,
                runtime,
                authentication,
            } => (installation, runtime, authentication, None),
        };
        let runtime = runtime.to_string();
        let authentication = authentication.to_string();
        let pending = PendingOperation {
            live: self.live.clone(),
            completed: false,
        };
        let started = self.authority.as_ref().ok_or(DbError::Unavailable)?;
        let transaction = timeout_at(
            TokioInstant::from_std(deadline),
            self.serving
                .as_mut()
                .ok_or(DbError::Unavailable)?
                .build_transaction()
                .isolation_level(IsolationLevel::ReadCommitted)
                .start(),
        )
        .await
        .map_err(|_| DbError::Expired)?
        .map_err(query_error)?;
        let row = query(
            &transaction,
            &pending.live,
            deadline,
            FENCE,
            &[&installation, &runtime, &authentication, &revision],
        )
        .await;
        let checked = row.and_then(|row| authority(&row)).and_then(|fresh| {
            if fresh.installation != started.installation
                || fresh.owner != started.owner
                || fresh.runtime != started.runtime
            {
                pending.live.stop();
                Err(DbError::Unavailable)
            } else {
                Ok(fresh)
            }
        });
        match checked {
            Ok(authority) => Ok(Fenced {
                proof: std::marker::PhantomData,
                transaction: Some(transaction),
                authority,
                pending,
                deadline,
                failed: false,
            }),
            Err(error) => {
                rollback(transaction, pending).await?;
                Err(error)
            }
        }
    }
}

async fn query(
    transaction: &Transaction<'_>,
    live: &RuntimeLiveness,
    deadline: Instant,
    sql: &str,
    parameters: &[&(dyn ToSql + Sync)],
) -> Result<Row, DbError> {
    if !live.is_live() {
        return Err(DbError::Unavailable);
    }
    if Instant::now() >= deadline {
        return Err(DbError::Expired);
    }
    let row = timeout_at(
        TokioInstant::from_std(deadline),
        transaction.query_one(sql, parameters),
    )
    .await
    .map_err(|_| DbError::Expired)?
    .map_err(query_error)?;
    if !live.is_live() {
        return Err(DbError::Unavailable);
    }
    if Instant::now() >= deadline {
        return Err(DbError::Expired);
    }
    Ok(row)
}

async fn rollback(
    transaction: Transaction<'_>,
    mut pending: PendingOperation,
) -> Result<(), DbError> {
    // Cleanup retains ownership beyond the expired request budget. A dropped
    // query future does not establish completed server rollback.
    tokio::time::timeout(Duration::from_secs(5), transaction.rollback())
        .await
        .map_err(|_| DbError::Unavailable)?
        .map_err(query_error)?;
    pending.completed = true;
    Ok(())
}

impl<P> Fenced<'_, P> {
    pub(crate) fn authority(&self) -> &Authority {
        &self.authority
    }

    async fn run(&mut self, sql: &str, parameters: &[&(dyn ToSql + Sync)]) -> Result<Row, DbError> {
        if self.failed {
            return Err(DbError::Unavailable);
        }
        // Mark unavailable before yielding. Dropping just this borrowed method
        // must stop admission even when the transaction wrapper itself survives.
        self.failed = true;
        let mut operation = PendingOperation {
            live: self.pending.live.clone(),
            completed: false,
        };
        let result = query(
            self.transaction.as_ref().ok_or(DbError::Unavailable)?,
            &self.pending.live,
            self.deadline,
            sql,
            parameters,
        )
        .await;
        operation.completed = true; // The outcome was actually observed.
        self.failed = result.is_err();
        result
    }

    fn confirmed(&mut self, row: Row) -> Result<(), DbError> {
        match row.try_get::<_, bool>(0).map_err(query_error) {
            Ok(true) => Ok(()),
            _ => {
                self.failed = true;
                Err(DbError::Unavailable)
            }
        }
    }

    pub(crate) async fn rollback(mut self) -> Result<(), DbError> {
        rollback(
            self.transaction.take().ok_or(DbError::Unavailable)?,
            self.pending,
        )
        .await
    }

    pub(crate) async fn commit(mut self) -> Result<Authority, DbError> {
        if self.failed || !self.pending.live.is_live() || Instant::now() >= self.deadline {
            self.rollback().await?;
            return Err(DbError::Expired);
        }
        let transaction = self.transaction.take().ok_or(DbError::Unavailable)?;
        timeout_at(TokioInstant::from_std(self.deadline), transaction.commit())
            .await
            .map_err(|_| DbError::UnknownCommit)?
            .map_err(|_| DbError::UnknownCommit)?;
        if !self.pending.live.is_live() || Instant::now() >= self.deadline {
            return Err(DbError::UnknownCommit);
        }
        self.pending.completed = true;
        Ok(self.authority)
    }
}

impl Fenced<'_, PasswordProof> {
    pub(crate) async fn issue(&mut self, id: &str, kind: Kind) -> Result<(), DbError> {
        let a = &self.authority;
        let (installation, runtime, authentication, revision) = (
            a.installation.clone(),
            a.runtime.to_string(),
            a.authentication.to_string(),
            a.revision.to_string(),
        );
        let row = self
            .run(
                ISSUE,
                &[
                    &installation,
                    &runtime,
                    &authentication,
                    &revision,
                    &id,
                    &kind_name(kind),
                ],
            )
            .await?;
        self.confirmed(row)
    }

    pub(crate) async fn revoke(
        &mut self,
        id: &str,
        kind: Kind,
        initiating_session: &str,
    ) -> Result<(), DbError> {
        let a = &self.authority;
        let (installation, runtime, authentication, revision) = (
            a.installation.clone(),
            a.runtime.to_string(),
            a.authentication.to_string(),
            a.revision.to_string(),
        );
        let row = self
            .run(
                REVOKE,
                &[
                    &installation,
                    &runtime,
                    &authentication,
                    &revision,
                    &id,
                    &kind_name(kind),
                    &initiating_session,
                ],
            )
            .await?;
        self.confirmed(row)
    }

    pub(crate) async fn change_password(
        &mut self,
        replacement: &str,
        event_id: &str,
        initiating_session: &str,
    ) -> Result<(), DbError> {
        let a = &self.authority;
        let (installation, runtime, authentication, revision) = (
            a.installation.clone(),
            a.runtime.to_string(),
            a.authentication.to_string(),
            a.revision.to_string(),
        );
        let row = self
            .run(
                CHANGE,
                &[
                    &installation,
                    &runtime,
                    &authentication,
                    &revision,
                    &replacement,
                    &event_id,
                    &initiating_session,
                ],
            )
            .await?;
        match authority(&row) {
            Ok(fresh) => {
                self.authority = fresh;
                Ok(())
            }
            Err(error) => {
                self.failed = true;
                Err(error)
            }
        }
    }
}

impl Fenced<'_, SessionProof> {
    pub(crate) async fn logout(&mut self, id: &str, kind: Kind) -> Result<(), DbError> {
        let a = &self.authority;
        let (installation, runtime, authentication) = (
            a.installation.clone(),
            a.runtime.to_string(),
            a.authentication.to_string(),
        );
        let row = self
            .run(
                LOGOUT,
                &[
                    &installation,
                    &runtime,
                    &authentication,
                    &id,
                    &kind_name(kind),
                ],
            )
            .await?;
        self.confirmed(row)
    }
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Browser => "browser",
        Kind::Cli => "cli",
    }
}

#[cfg(test)]
mod tests;
