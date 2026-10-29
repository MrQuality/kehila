//! Owned disposable authentication jobs. Transport adapters receive replies,
//! never ownership of database/hash futures or an authority-selection API.
use crate::{
    database::{Authority, Credentials, Database, DbError},
    lifecycle::RuntimeLiveness,
    password::{
        admission::Bucket,
        blocklist::Blocklist,
        hash::Verifier,
        worker::{Change, HashPending, Worker, WorkerError},
        NormalizedPassword,
    },
    pre_session::{Pool, PreSession},
    secret::{Digest, Secret},
    session::{Binding, Kind, NewSession, Registry, Session, Snapshot},
};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::Mutex as AsyncMutex;
use zeroize::Zeroizing;

pub(crate) mod jobs;

const WORK_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    Invalid,
    Unauthorized,
    UnknownCredential,
    Busy,
    Unavailable,
    AuditUnconfirmed,
}

pub(crate) enum LoginKind {
    Cli,
    Browser { bearer: Digest, csrf: Digest },
}

pub(crate) struct Credential {
    pub(crate) bearer: Digest,
    pub(crate) kind: Kind,
    pub(crate) csrf: Option<Digest>,
}

pub(crate) enum Operation {
    PreSession(Option<Digest>),
    PreCsrf(Digest),
    Login {
        password: NormalizedPassword,
        kind: LoginKind,
    },
    State(Credential),
    Logout(Credential),
    Password {
        credential: Credential,
        current: NormalizedPassword,
        replacement: NormalizedPassword,
    },
}

pub(crate) enum Output {
    PreSession {
        bearer: Option<Secret>,
        csrf: Zeroizing<[u8; 43]>,
    },
    Issued(Delivery),
    State {
        csrf: Option<Zeroizing<[u8; 43]>>,
        active: bool,
    },
    LoggedOut,
    PasswordChanged,
}

// The adapter retains this through its observed exchange outcome. Channel
// transfer alone is not HTTP delivery. Abandonment irreversibly retires lookup.
pub(crate) struct Delivery {
    bearer: Secret,
    csrf: Option<Zeroizing<[u8; 43]>>,
    entry: Arc<Session>,
    snapshot: Snapshot,
    deadline: Instant,
    confirmed: bool,
}

impl Delivery {
    pub(crate) fn material(&self) -> Result<(&[u8; 43], Option<&[u8; 43]>), Error> {
        let now = Instant::now();
        if now >= self.deadline || !self.entry.eligible(&self.snapshot, now) {
            return Err(Error::Unavailable);
        }
        Ok((self.bearer.encoded(), self.csrf.as_deref()))
    }
    pub(crate) fn confirm_exchange(&mut self) -> Result<(), Error> {
        self.material()?;
        self.confirmed = true;
        Ok(())
    }
}
impl Drop for Delivery {
    fn drop(&mut self) {
        if !self.confirmed {
            self.entry.retire();
        }
    }
}

struct Engine {
    live: Arc<RuntimeLiveness>,
    database: AsyncMutex<Database>,
    sessions: Mutex<Registry>,
    pre_sessions: Pool,
    hash: Arc<AsyncMutex<Worker>>,
    login_allowance: Mutex<Bucket>,
    management_allowance: Mutex<Bucket>,
    blocklist: Blocklist,
}

fn snapshot(authority: &Authority) -> Snapshot {
    Snapshot {
        binding: Binding {
            installation: authority.installation.clone(),
            owner: authority.owner.clone(),
            runtime_generation: authority.runtime,
            auth_generation: authority.authentication,
        },
        actor_active: true,
    }
}

fn verifier(credentials: Credentials) -> Result<(Authority, Verifier), Error> {
    if !credentials.active {
        return Err(Error::Unauthorized);
    }
    let policy = u32::try_from(credentials.policy).map_err(|_| Error::Unavailable)?;
    let verifier = Verifier::from_stored(credentials.verifier.ok_or(Error::Unavailable)?, policy)
        .map_err(|_| Error::Unavailable)?;
    Ok((credentials.authority, verifier))
}

fn charge(bucket: &Mutex<Bucket>) -> Result<(), Error> {
    if bucket
        .lock()
        .map_err(|_| Error::Unavailable)?
        .charge(Instant::now())
    {
        Ok(())
    } else {
        Err(Error::Busy)
    }
}

fn fence_error(error: DbError) -> Error {
    match error {
        DbError::Rejected(code) if code == "55000" => Error::Unauthorized,
        _ => Error::Unavailable,
    }
}

fn csrf_matches(encoded: Option<Zeroizing<[u8; 43]>>, supplied: &Digest) -> bool {
    encoded.is_some_and(|value| {
        Digest::from_encoded(value.as_slice()).is_ok_and(|actual| actual.matches(supplied))
    })
}

impl Engine {
    fn check(&self, deadline: Instant) -> Result<(), Error> {
        if self.live.is_live() && Instant::now() < deadline {
            Ok(())
        } else {
            Err(Error::Unavailable)
        }
    }

    fn candidate(&self, credential: &Credential) -> Result<Arc<Session>, Error> {
        self.sessions
            .lock()
            .map_err(|_| Error::Unavailable)?
            .lookup(&credential.bearer, credential.kind, Instant::now())
            .ok_or(Error::UnknownCredential)
    }

    fn csrf(
        &self,
        entry: &Session,
        credential: &Credential,
        fresh: &Snapshot,
    ) -> Result<(), Error> {
        match credential.kind {
            Kind::Cli if credential.csrf.is_none() => Ok(()),
            Kind::Browser
                if credential.csrf.as_ref().is_some_and(|csrf| {
                    csrf_matches(entry.csrf_for_bootstrap(fresh, Instant::now()), csrf)
                }) =>
            {
                Ok(())
            }
            _ => Err(Error::Unauthorized),
        }
    }

    async fn hash<T: Send + 'static>(
        &self,
        deadline: Instant,
        start: impl FnOnce(&mut Worker) -> Result<HashPending<T>, WorkerError>,
    ) -> Result<T, Error> {
        self.check(deadline)?;
        let worker = self
            .hash
            .clone()
            .try_lock_owned()
            .map_err(|_| Error::Busy)?;
        let mut lease = HashLease {
            worker,
            live: self.live.clone(),
            dirty: false,
        };
        let pending = start(&mut lease.worker).map_err(|_| Error::Unavailable)?;
        lease.dirty = true;
        let result =
            tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), pending.result())
                .await;
        // An elapsed/abandoned receiver cannot release this lane. Wait through
        // real completion, then synchronously join only an already-finished thread.
        while !lease.worker.finished() {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        lease
            .worker
            .join_completed()
            .map_err(|_| Error::Unavailable)?;
        lease.dirty = false;
        self.check(deadline)?;
        result
            .map_err(|_| Error::Unavailable)?
            .map_err(|_| Error::Unavailable)?
            .map_err(|_| Error::Unavailable)
    }

    async fn pre_session(
        &self,
        bearer: Option<Digest>,
        deadline: Instant,
    ) -> Result<Output, Error> {
        if let Some(bearer) = bearer {
            let entry = self
                .pre_sessions
                .reuse(&bearer, Instant::now())
                .ok_or(Error::UnknownCredential)?;
            return Ok(Output::PreSession {
                bearer: None,
                csrf: entry.csrf(Instant::now()).ok_or(Error::Unauthorized)?,
            });
        }
        let reservation = self
            .pre_sessions
            .reserve(Instant::now())
            .map_err(|_| Error::Busy)?;
        let (bearer, csrf) = self
            .hash(deadline, |worker| worker.anonymous(deadline))
            .await?;
        let entry = reservation
            .finish(bearer.digest(), csrf, Instant::now())
            .map_err(|_| Error::Unavailable)?;
        Ok(Output::PreSession {
            bearer: Some(bearer),
            csrf: entry.csrf(Instant::now()).ok_or(Error::Unavailable)?,
        })
    }

    async fn login(
        &self,
        password: NormalizedPassword,
        kind: LoginKind,
        deadline: Instant,
    ) -> Result<Output, Error> {
        let (kind, pre): (Kind, Option<Arc<PreSession>>) = match kind {
            LoginKind::Cli => (Kind::Cli, None),
            LoginKind::Browser { bearer, csrf } => {
                let entry = self
                    .pre_sessions
                    .reuse(&bearer, Instant::now())
                    .ok_or(Error::UnknownCredential)?;
                if !csrf_matches(entry.csrf(Instant::now()), &csrf) {
                    return Err(Error::Unauthorized);
                }
                (Kind::Browser, Some(entry))
            }
        };
        charge(&self.login_allowance)?;
        let credentials = tokio::time::timeout_at(
            tokio::time::Instant::from_std(deadline),
            self.database.lock(),
        )
        .await
        .map_err(|_| Error::Unavailable)?
        .credentials(deadline)
        .await
        .map_err(|_| Error::Unavailable)?;
        let (proof, verifier) = verifier(credentials)?;
        let secrets = self
            .hash(deadline, |worker| {
                worker.login(deadline, password, verifier, kind)
            })
            .await?
            .ok_or(Error::Unauthorized)?;
        self.check(deadline)?;
        let mut database = tokio::time::timeout_at(
            tokio::time::Instant::from_std(deadline),
            self.database.lock(),
        )
        .await
        .map_err(|_| Error::Unavailable)?;
        let mut transaction = database
            .fenced_password(&proof, deadline)
            .await
            .map_err(fence_error)?;
        let fresh = snapshot(transaction.authority());
        let observed = self
            .sessions
            .lock()
            .map_err(|_| Error::Unavailable)?
            .observe_generation(&fresh);
        if observed.is_err() {
            transaction
                .rollback()
                .await
                .map_err(|_| Error::Unavailable)?;
            return Err(Error::Unavailable);
        }
        let mut claim = match pre {
            Some(entry) => match self.pre_sessions.claim(&entry, Instant::now()) {
                Ok(claim) => Some(claim),
                Err(_) => {
                    transaction
                        .rollback()
                        .await
                        .map_err(|_| Error::Unavailable)?;
                    return Err(Error::Unauthorized);
                }
            },
            None => None,
        };
        let csrf = secrets
            .csrf
            .as_ref()
            .map(|value| Zeroizing::new(*value.encoded()));
        let entry = self
            .sessions
            .lock()
            .map_err(|_| Error::Unavailable)?
            .reserve(
                NewSession {
                    public_id: secrets.public_id,
                    kind,
                    bearer: secrets.bearer.digest(),
                    csrf: secrets.csrf,
                },
                &fresh,
                Instant::now(),
            )
            .map_err(|_| Error::Busy);
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                transaction
                    .rollback()
                    .await
                    .map_err(|_| Error::Unavailable)?;
                if let Some(claim) = claim.take() {
                    let _ = claim.rollback_confirmed(Instant::now());
                }
                return Err(error);
            }
        };
        let mut delivery = Delivery {
            bearer: secrets.bearer,
            csrf,
            entry,
            snapshot: fresh,
            deadline,
            confirmed: false,
        };
        if transaction
            .issue(&delivery.entry.public_id, kind)
            .await
            .is_err()
        {
            transaction
                .rollback()
                .await
                .map_err(|_| Error::Unavailable)?;
            if let Some(claim) = claim.take() {
                let _ = claim.rollback_confirmed(Instant::now());
            }
            return Err(Error::Unavailable);
        }
        let committed = transaction.commit().await.map_err(|_| Error::Unavailable)?;
        if let Some(claim) = claim.take() {
            claim
                .commit_confirmed(Instant::now())
                .map_err(|_| Error::Unavailable)?;
        }
        delivery.snapshot = snapshot(&committed);
        self.check(deadline)?;
        self.sessions
            .lock()
            .map_err(|_| Error::Unavailable)?
            .publish(&delivery.entry, &delivery.snapshot, Instant::now())
            .map_err(|_| Error::Unavailable)?;
        Ok(Output::Issued(delivery))
    }

    async fn session(
        &self,
        credential: Credential,
        logout: bool,
        deadline: Instant,
    ) -> Result<Output, Error> {
        let entry = self.candidate(&credential)?; // Unknown credentials do not reach SQL.
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
        let validation = (if logout {
            self.csrf(&entry, &credential, &fresh)
        } else {
            Ok(())
        })
        .and_then(|()| {
            let now = Instant::now();
            if entry.eligible(&fresh, now) || entry.logout_retry(&fresh, now) {
                Ok(())
            } else {
                Err(Error::Unauthorized)
            }
        });
        if let Err(error) = validation {
            transaction
                .rollback()
                .await
                .map_err(|_| Error::Unavailable)?;
            return Err(error);
        }
        if !logout {
            let csrf = entry.csrf_for_bootstrap(&fresh, Instant::now());
            transaction
                .rollback()
                .await
                .map_err(|_| Error::Unavailable)?;
            return Ok(Output::State {
                csrf,
                active: entry.eligible(&fresh, Instant::now()),
            });
        }
        entry
            .revoke_for_logout(&fresh, Instant::now())
            .map_err(|_| Error::Unavailable)?;
        if transaction
            .logout(&entry.public_id, entry.kind)
            .await
            .is_err()
        {
            transaction
                .rollback()
                .await
                .map_err(|_| Error::AuditUnconfirmed)?;
            return Err(Error::AuditUnconfirmed);
        }
        transaction
            .commit()
            .await
            .map_err(|_| Error::AuditUnconfirmed)?;
        entry
            .confirm_logout(&fresh, Instant::now())
            .map_err(|_| Error::Unavailable)?;
        Ok(Output::LoggedOut)
    }

    async fn password(
        &self,
        credential: Credential,
        current: NormalizedPassword,
        replacement: NormalizedPassword,
        deadline: Instant,
    ) -> Result<Output, Error> {
        let entry = self.candidate(&credential)?;
        let credentials = tokio::time::timeout_at(
            tokio::time::Instant::from_std(deadline),
            self.database.lock(),
        )
        .await
        .map_err(|_| Error::Unavailable)?
        .credentials(deadline)
        .await
        .map_err(|_| Error::Unavailable)?;
        let (proof, verifier) = verifier(credentials)?;
        let fresh = snapshot(&proof);
        self.csrf(&entry, &credential, &fresh)?;
        if !entry.eligible(&fresh, Instant::now()) {
            return Err(Error::Unauthorized);
        }
        charge(&self.management_allowance)?;
        let replacement = self
            .blocklist
            .admit(replacement)
            .map_err(|_| Error::Invalid)?;
        let replacement = match self
            .hash(deadline, |worker| {
                worker.replacement(deadline, current, replacement, verifier)
            })
            .await?
        {
            Change::Mismatch => return Err(Error::Unauthorized),
            Change::Replacement(value) => value,
        };
        // Public event identity uses owned OS randomness on the same bounded lane.
        let event = self
            .hash(deadline, |worker| worker.public_id(deadline))
            .await?;
        let mut database = tokio::time::timeout_at(
            tokio::time::Instant::from_std(deadline),
            self.database.lock(),
        )
        .await
        .map_err(|_| Error::Unavailable)?;
        let mut transaction = database
            .fenced_password(&proof, deadline)
            .await
            .map_err(fence_error)?;
        let fresh = snapshot(transaction.authority());
        if !entry.eligible(&fresh, Instant::now()) {
            transaction
                .rollback()
                .await
                .map_err(|_| Error::Unavailable)?;
            return Err(Error::Unauthorized);
        }
        if transaction
            .change_password(replacement.encoded(), &event, &entry.public_id)
            .await
            .is_err()
        {
            transaction
                .rollback()
                .await
                .map_err(|_| Error::Unavailable)?;
            return Err(Error::Unavailable);
        }
        let committed = transaction.commit().await.map_err(|_| Error::Unavailable)?;
        let invalidation = self
            .sessions
            .lock()
            .map_err(|_| ())
            .and_then(|mut registry| {
                registry
                    .observe_generation(&snapshot(&committed))
                    .map_err(|_| ())
            });
        if invalidation.is_err() {
            self.live.stop();
            return Err(Error::Unavailable);
        }
        Ok(Output::PasswordChanged)
    }

    async fn execute(&self, operation: Operation, deadline: Instant) -> Result<Output, Error> {
        self.check(deadline)?;
        match operation {
            Operation::PreSession(bearer) => self.pre_session(bearer, deadline).await,
            Operation::PreCsrf(bearer) => {
                let entry = self
                    .pre_sessions
                    .reuse(&bearer, Instant::now())
                    .ok_or(Error::UnknownCredential)?;
                Ok(Output::PreSession {
                    bearer: None,
                    csrf: entry.csrf(Instant::now()).ok_or(Error::Unauthorized)?,
                })
            }
            Operation::Login { password, kind } => self.login(password, kind, deadline).await,
            Operation::State(credential) => self.session(credential, false, deadline).await,
            Operation::Logout(credential) => self.session(credential, true, deadline).await,
            Operation::Password {
                credential,
                current,
                replacement,
            } => {
                self.password(credential, current, replacement, deadline)
                    .await
            }
        }
    }
}

struct HashLease {
    worker: tokio::sync::OwnedMutexGuard<Worker>,
    live: Arc<RuntimeLiveness>,
    dirty: bool,
}
impl Drop for HashLease {
    fn drop(&mut self) {
        if self.dirty {
            self.live.stop();
        }
    }
}
