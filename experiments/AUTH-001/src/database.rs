//! Linux-only disposable PostgreSQL transport and dedicated authority owner.
//! Serving clients stay private behind the owned application coordinator.
use crate::lifecycle::RuntimeLiveness;
use rustls::{pki_types::CertificateDer, ClientConfig, RootCertStore};
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    task::JoinSet,
    time::{timeout_at, Instant as TokioInstant},
};
use tokio_postgres::{config::SslMode, Client, Config, IsolationLevel, Row};
use tokio_postgres_rustls::MakeRustlsConnect;
use zeroize::Zeroizing;

pub(crate) mod transaction;

const LOGIN: &str = "auth001_runtime_login";
const SERVER_NAME: &str = "auth-db";
const ADVISORY_NAMESPACE: i32 = 1262831689;
const START: &str = "SELECT installation_id::text, owner_actor_id::text, runtime_generation::text, auth_generation::text, verifier_revision::text FROM auth001.start_runtime($1::text::kehila.ident, $2::text::auth001.public_id)";
const CREDENTIALS: &str = "SELECT installation_id::text, owner_actor_id::text, runtime_generation::text, auth_generation::text, verifier_revision::text, policy_version, verifier::text, owner_active FROM auth001.credentials($1::text::kehila.ident)";

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum DbError {
    Invalid,
    Busy,
    Expired,
    Unavailable,
    UnknownCommit,
    Rejected(String),
}

// No Debug/Clone/serialization: driver configuration can copy passwords, so
// owned input wiping is not a universal erasure guarantee for library buffers.
pub(crate) struct Endpoint {
    address: SocketAddr,
    database: String,
    password: Zeroizing<Vec<u8>>,
    root: CertificateDer<'static>,
}

pub(crate) struct Authority {
    pub(crate) installation: String,
    pub(crate) owner: String,
    pub(crate) runtime: u64,
    pub(crate) authentication: u64,
    pub(crate) revision: u64,
}

pub(crate) struct Credentials {
    pub(crate) authority: Authority,
    pub(crate) policy: i32,
    pub(crate) verifier: Option<Zeroizing<String>>,
    pub(crate) active: bool,
}

pub(crate) struct Database {
    dedicated: Option<Client>,
    serving: Option<Client>,
    drivers: JoinSet<Result<(), DbError>>,
    live: Arc<RuntimeLiveness>,
    installation: String,
    authority: Option<Authority>,
    #[cfg(test)]
    lose_confirmation: Arc<std::sync::atomic::AtomicBool>,
}

pub(crate) struct Shutdown {
    pub(crate) joined: usize,
    pub(crate) failed: bool,
    pub(crate) aborted: bool,
}

struct StopOnDriverExit(Arc<RuntimeLiveness>);
impl Drop for StopOnDriverExit {
    fn drop(&mut self) {
        self.0.stop();
    }
}

struct PendingOperation {
    live: Arc<RuntimeLiveness>,
    completed: bool,
}
impl Drop for PendingOperation {
    fn drop(&mut self) {
        if !self.completed {
            self.live.stop();
        }
    }
}

impl Endpoint {
    pub(crate) fn new(
        address: SocketAddr,
        database: String,
        password: Zeroizing<Vec<u8>>,
        root: Vec<u8>,
    ) -> Result<Self, DbError> {
        let private = matches!(address.ip(), IpAddr::V4(ip) if ip.is_private() || ip.is_loopback());
        if !private
            || address.port() == 0
            || database.is_empty()
            || database.len() > 63
            || !database
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || password.len() != 43
            || !password
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-')
            || root.is_empty()
            || root.len() > 64 * 1024
        {
            return Err(DbError::Invalid);
        }
        Ok(Self {
            address,
            database,
            password,
            root: CertificateDer::from(root),
        })
    }

    fn configuration(&self, remaining: Duration) -> Result<(Config, MakeRustlsConnect), DbError> {
        let mut roots = RootCertStore::empty();
        roots.add(self.root.clone()).map_err(|_| DbError::Invalid)?;
        let tls =
            ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_protocol_versions(&[&rustls::version::TLS13])
                .map_err(|_| DbError::Invalid)?
                .with_root_certificates(roots)
                .with_no_client_auth();
        let mut config = Config::new();
        config
            .user(LOGIN)
            .password(self.password.as_slice())
            .dbname(&self.database)
            .host(SERVER_NAME)
            .hostaddr(self.address.ip())
            .port(self.address.port())
            .ssl_mode(SslMode::Require)
            .connect_timeout(remaining)
            .options("-c statement_timeout=5000 -c lock_timeout=5000 -c idle_in_transaction_session_timeout=5000");
        Ok((config, MakeRustlsConnect::new(tls)))
    }
}

impl Database {
    #[cfg(test)]
    pub(crate) fn lose_next_confirmation_for_test(&mut self) {
        self.lose_confirmation
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub(crate) async fn start(
        endpoint: &Endpoint,
        installation: String,
        attempt: String,
        deadline: Instant,
    ) -> Result<Self, DbError> {
        // The owner prepares a fresh per-attempt ID before connection/lock work;
        // synchronous entropy acquisition is not an async-deadline guarantee.
        if !identifier(&installation)
            || attempt.is_empty()
            || attempt.len() > 128
            || !attempt.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(DbError::Invalid);
        }
        let mut database = Self {
            dedicated: None,
            serving: None,
            drivers: JoinSet::new(),
            live: RuntimeLiveness::new(),
            installation,
            authority: None,
            #[cfg(test)]
            lose_confirmation: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        let result = timeout_at(
            TokioInstant::from_std(deadline),
            database.initialize(endpoint, &attempt, deadline),
        )
        .await;
        match result {
            Ok(Ok(())) if database.live.is_live() => Ok(database),
            result => {
                let error = match result {
                    Ok(Err(error)) => error,
                    Err(_) => DbError::UnknownCommit,
                    _ => DbError::Unavailable,
                };
                database.shutdown().await;
                Err(error)
            }
        }
    }

    async fn connect(&mut self, endpoint: &Endpoint, deadline: Instant) -> Result<Client, DbError> {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(DbError::Expired)?;
        let (config, tls) = endpoint.configuration(remaining)?;
        let (client, connection) = config.connect(tls).await.map_err(query_error)?;
        let stop = StopOnDriverExit(self.live.clone());
        self.drivers.spawn(async move {
            let _stop = stop;
            connection.await.map_err(query_error)
        });
        Ok(client)
    }

    async fn initialize(
        &mut self,
        endpoint: &Endpoint,
        attempt: &str,
        deadline: Instant,
    ) -> Result<(), DbError> {
        self.dedicated = Some(self.connect(endpoint, deadline).await?);
        self.serving = Some(self.connect(endpoint, deadline).await?);
        self.serving
            .as_ref()
            .ok_or(DbError::Unavailable)?
            .batch_execute("SET ROLE auth001_runtime")
            .await
            .map_err(query_error)?;
        let client = self.dedicated.as_mut().ok_or(DbError::Unavailable)?;
        client
            .batch_execute("SET ROLE auth001_runtime")
            .await
            .map_err(query_error)?;
        let acquired: bool = client
            .query_one(
                "SELECT pg_catalog.pg_try_advisory_lock($1, $2)",
                &[&ADVISORY_NAMESPACE, &1i32],
            )
            .await
            .map_err(query_error)?
            .try_get(0)
            .map_err(query_error)?;
        if !acquired {
            return Err(DbError::Busy);
        }
        let transaction = client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(query_error)?;
        let row = transaction
            .query_one(START, &[&self.installation, &attempt])
            .await
            .map_err(query_error)?;
        let authority = authority(&row)?;
        if !self.live.is_live() || Instant::now() >= deadline {
            transaction.rollback().await.map_err(query_error)?;
            return Err(DbError::Expired);
        }
        transaction
            .commit()
            .await
            .map_err(|_| DbError::UnknownCommit)?;
        if !self.live.is_live() || Instant::now() >= deadline {
            return Err(DbError::UnknownCommit);
        }
        self.authority = Some(authority);
        Ok(())
    }

    pub(crate) fn is_live(&self) -> bool {
        self.live.is_live()
    }
    pub(crate) fn authority(&self) -> Result<&Authority, DbError> {
        self.authority.as_ref().ok_or(DbError::Unavailable)
    }
    pub(crate) fn liveness(&self) -> Arc<RuntimeLiveness> {
        self.live.clone()
    }

    // Exclusive access is the current native-proof boundary. No caller receives
    // Client; the bounded application job coordinator is a remaining gate.
    pub(crate) async fn credentials(&mut self, deadline: Instant) -> Result<Credentials, DbError> {
        if !self.live.is_live() {
            return Err(DbError::Unavailable);
        }
        // Abandonment cannot make a busy connection reusable. This first
        // profile stops the runtime; its owner must subsequently close/join.
        let mut pending = PendingOperation {
            live: self.live.clone(),
            completed: false,
        };
        let parameters: [&(dyn tokio_postgres::types::ToSql + Sync); 1] = [&self.installation];
        let query = self
            .serving
            .as_ref()
            .ok_or(DbError::Unavailable)?
            .query_one(CREDENTIALS, &parameters);
        let row = match timeout_at(TokioInstant::from_std(deadline), query).await {
            Ok(result) => result.map_err(query_error)?,
            Err(_) => {
                self.live.stop();
                return Err(DbError::Expired);
            }
        };
        if !self.live.is_live() {
            return Err(DbError::Unavailable);
        }
        if Instant::now() >= deadline {
            return Err(DbError::Expired);
        }
        let current = authority(&row)?;
        let started = self.authority.as_ref().ok_or(DbError::Unavailable)?;
        if current.installation != started.installation
            || current.owner != started.owner
            || current.runtime != started.runtime
        {
            self.live.stop();
            return Err(DbError::Unavailable);
        }
        let raw = row
            .try_get::<_, Option<String>>(6)
            .map_err(query_error)?
            .map(Zeroizing::new);
        if raw
            .as_ref()
            .is_some_and(|value| value.len() > 512 || !value.is_ascii())
        {
            return Err(DbError::Unavailable);
        }
        let result = Credentials {
            authority: current,
            policy: row.try_get(5).map_err(query_error)?,
            verifier: raw,
            active: row.try_get(7).map_err(query_error)?,
        };
        pending.completed = true;
        Ok(result)
    }

    pub(crate) async fn driver_exit(&mut self) -> Result<(), DbError> {
        let outcome = self.drivers.join_next().await;
        self.live.stop();
        match outcome {
            Some(Ok(result)) => result,
            _ => Err(DbError::Unavailable),
        }
    }

    pub(crate) async fn shutdown(mut self) -> Shutdown {
        self.live.stop();
        self.dedicated.take();
        self.serving.take();
        let deadline = TokioInstant::now() + Duration::from_secs(5);
        let mut result = Shutdown {
            joined: 0,
            failed: false,
            aborted: false,
        };
        loop {
            match timeout_at(deadline, self.drivers.join_next()).await {
                Ok(Some(outcome)) => {
                    result.joined += 1;
                    result.failed |= !matches!(outcome, Ok(Ok(())));
                }
                Ok(None) => break,
                Err(_) => {
                    result.aborted = true;
                    self.drivers.abort_all();
                    while self.drivers.join_next().await.is_some() {
                        result.joined += 1;
                    }
                    break;
                }
            }
        }
        result
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        self.live.stop();
        self.dedicated.take();
        self.serving.take();
        // Exceptional fallback requests abort; normal ownership calls shutdown
        // and observes joins. Transport closure never proves server rollback.
        self.drivers.abort_all();
    }
}

fn query_error(error: tokio_postgres::Error) -> DbError {
    error.code().map_or(DbError::Unavailable, |code| {
        DbError::Rejected(code.code().to_owned())
    })
}
fn identifier(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.contains('\0')
}
fn number(value: String) -> Result<u64, DbError> {
    let parsed = value.parse::<u64>().map_err(|_| DbError::Unavailable)?;
    if parsed == 0 || parsed.to_string() != value {
        return Err(DbError::Unavailable);
    }
    Ok(parsed)
}
fn authority(row: &Row) -> Result<Authority, DbError> {
    let installation: String = row.try_get(0).map_err(query_error)?;
    let owner: String = row.try_get(1).map_err(query_error)?;
    if !identifier(&installation) || !identifier(&owner) {
        return Err(DbError::Unavailable);
    }
    Ok(Authority {
        installation,
        owner,
        runtime: number(row.try_get(2).map_err(query_error)?)?,
        authentication: number(row.try_get(3).map_err(query_error)?)?,
        revision: number(row.try_get(4).map_err(query_error)?)?,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::{
        io::Read,
        path::{Path, PathBuf},
    };

    // Explicit opt-in fixture stdin: address, runtime password, CA base64,
    // installation, fresh startup-attempt ID. No password in argv/environment.
    pub(crate) fn fixture() -> (Endpoint, String, String, PathBuf) {
        let mut input = Zeroizing::new(Vec::new());
        std::io::stdin()
            .take(16385)
            .read_to_end(&mut input)
            .unwrap();
        assert!(input.len() <= 16384);
        let text = std::str::from_utf8(&input).unwrap();
        let fields: Vec<_> = text.lines().collect();
        assert_eq!(fields.len(), 5);
        let encoded = fields[2].as_bytes();
        let mut root = vec![0; data_encoding::BASE64.decode_len(encoded.len()).unwrap()];
        let length = data_encoding::BASE64
            .decode_mut(encoded, &mut root)
            .unwrap();
        root.truncate(length);
        let endpoint = Endpoint::new(
            fields[0].parse().unwrap(),
            "postgres".to_owned(),
            Zeroizing::new(fields[1].as_bytes().to_vec()),
            root,
        )
        .unwrap();
        let controls = PathBuf::from(
            std::env::var_os("AUTH001_DB_CONTROLS")
                .expect("owned external fixture controls required"),
        );
        assert!(controls.is_absolute() && controls.is_dir());
        (
            endpoint,
            fields[3].to_owned(),
            fields[4].to_owned(),
            controls,
        )
    }

    pub(crate) fn mark(directory: &Path, name: &str) {
        use std::io::Write;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join(name))
            .unwrap()
            .write_all(b"ready\n")
            .unwrap();
    }

    pub(crate) async fn wait(directory: &Path, name: &str) {
        let deadline = TokioInstant::now() + Duration::from_secs(15);
        while !directory.join(name).is_file() {
            assert!(
                TokioInstant::now() < deadline,
                "fixture coordination deadline exceeded"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    #[test]
    #[ignore = "requires the explicitly provisioned disposable PostgreSQL TLS fixture"]
    fn native_lifetime() {
        let (endpoint, installation, attempt, controls) = fixture();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let mut database = Database::start(
                    &endpoint,
                    installation,
                    attempt,
                    Instant::now() + Duration::from_secs(30),
                )
                .await
                .unwrap();
                let credentials = database
                    .credentials(Instant::now() + Duration::from_secs(5))
                    .await
                    .unwrap();
                assert!(
                    credentials.active && credentials.policy == 1 && credentials.verifier.is_some()
                );
                assert!(credentials.authority.authentication > i64::MAX as u64);
                assert_eq!(credentials.authority.revision, u64::MAX);
                let denied = database
                    .serving
                    .as_ref()
                    .unwrap()
                    .batch_execute("SET ROLE auth001_admin")
                    .await
                    .expect_err("administrator role unexpectedly accessible");
                assert!(denied.code().is_some_and(|code| code.code() == "42501"));
                assert!(database.is_live());
                mark(&controls, "ready");
                let outcome = timeout_at(
                    TokioInstant::now() + Duration::from_secs(15),
                    database.driver_exit(),
                )
                .await
                .unwrap();
                assert!(outcome.is_err());
                assert!(!database.is_live());
                assert!(matches!(
                    database
                        .credentials(Instant::now() + Duration::from_secs(1))
                        .await,
                    Err(DbError::Unavailable)
                ));
                let shutdown = database.shutdown().await;
                assert_eq!(shutdown.joined, 1); // The failed driver was already joined.
                assert!(!shutdown.aborted);
                mark(&controls, "passed");
            });
    }

    #[test]
    #[ignore = "requires the explicitly provisioned disposable PostgreSQL TLS fixture"]
    fn native_cancelled_read() {
        let (endpoint, installation, attempt, controls) = fixture();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let mut database = Database::start(
                    &endpoint,
                    installation,
                    attempt,
                    Instant::now() + Duration::from_secs(30),
                )
                .await
                .unwrap();
                mark(&controls, "ready");
                wait(&controls, "start-read").await;
                {
                    let request = database.credentials(Instant::now() + Duration::from_secs(5));
                    tokio::pin!(request);
                    tokio::select! {
                        _ = &mut request => panic!("fixture did not hold the read"),
                        () = wait(&controls, "cancel-read") => {}
                    }
                }
                assert!(!database.is_live());
                assert!(matches!(
                    database
                        .credentials(Instant::now() + Duration::from_secs(1))
                        .await,
                    Err(DbError::Unavailable)
                ));
                mark(&controls, "cancelled");
                wait(&controls, "released").await;
                let shutdown = database.shutdown().await;
                assert_eq!(shutdown.joined, 2);
                assert!(!shutdown.aborted);
                mark(&controls, "passed");
            });
    }
}
