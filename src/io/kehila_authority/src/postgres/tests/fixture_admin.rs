//! Privileged test connection, never part of the serving authority.
use super::super::*;
use std::{io::Read, path::PathBuf};

pub(crate) struct Admin {
    client: Option<Client>,
    drivers: JoinSet<Result<(), tokio_postgres::Error>>,
    live: Arc<RuntimeLiveness>,
}
impl Admin {
    pub(crate) async fn connect(endpoint: &Endpoint) -> Self {
        let path = PathBuf::from(
            std::env::var_os("KEHILA_NATIVE_FIXTURE_ADMIN_PASSWORD_FILE")
                .expect("explicit disposable fixture credential required"),
        );
        assert!(path.is_absolute());
        let mut password = Zeroizing::new(Vec::new());
        std::fs::File::open(path)
            .unwrap()
            .take(44)
            .read_to_end(&mut password)
            .unwrap();
        assert_eq!(password.len(), 43);
        let (mut config, tls) = endpoint.configuration(Duration::from_secs(5)).unwrap();
        config.user("postgres").password(password.as_slice());
        let (client, connection) = timeout_at(
            TokioInstant::now() + Duration::from_secs(5),
            config.connect(tls),
        )
        .await
        .unwrap()
        .unwrap();
        let mut drivers = JoinSet::new();
        drivers.spawn(connection);
        let admin = Self {
            client: Some(client),
            drivers,
            live: RuntimeLiveness::new(),
        };
        let marker = admin
            .client()
            .query_one("SELECT value FROM kehila_native_fixture.marker", &[])
            .await
            .unwrap();
        assert_eq!(marker.get::<_, String>(0), "disposable native fixture");
        admin
    }
    pub(crate) fn client(&self) -> &Client {
        self.client.as_ref().unwrap()
    }
    // PendingOperation counts transaction entry and individual adapter
    // statements. Capture after construction to observe factory statements.
    pub(crate) fn database_observer(&self) -> impl Fn() -> usize + 'static {
        let live = self.live.clone();
        move || live.probe.snapshot().database_ownership_entries
    }
    pub(crate) async fn read_snapshot(
        &mut self,
        expected: &Authority,
        deadline: Instant,
    ) -> Result<transaction::Fenced<'_, transaction::ReadProof>, DbError> {
        let live = self.live.clone();
        let client = self.client.as_mut().ok_or(DbError::Unavailable)?;
        transaction::fixture::read_snapshot(client, expected, live, deadline).await
    }
    pub(crate) async fn write_transaction(
        &mut self,
        expected: &Authority,
        deadline: Instant,
    ) -> Result<transaction::Fenced<'_, transaction::SessionProof>, DbError> {
        let live = self.live.clone();
        let client = self.client.as_mut().ok_or(DbError::Unavailable)?;
        transaction::fixture::write_transaction(client, expected, live, deadline).await
    }
    pub(crate) async fn item_write_transaction(
        &mut self,
        expected: &Authority,
        deadline: Instant,
    ) -> Result<transaction::Fenced<'_, transaction::ItemWriteProof>, DbError> {
        let live = self.live.clone();
        let client = self.client.as_mut().ok_or(DbError::Unavailable)?;
        transaction::fixture::item_write_transaction(client, expected, live, deadline).await
    }
    pub(crate) async fn item_replay_transaction(
        &mut self,
        expected: &Authority,
        deadline: Instant,
    ) -> Result<transaction::Fenced<'_, transaction::ItemReplayProof>, DbError> {
        let live = Arc::clone(&self.live);
        let client = self.client.as_mut().ok_or(DbError::Unavailable)?;
        transaction::fixture::item_replay_transaction(client, expected, live, deadline).await
    }

    pub(crate) async fn terminate_authority(&self) {
        let rows = self.client().query("SELECT DISTINCT a.pid FROM pg_stat_activity a JOIN pg_locks l ON l.pid=a.pid WHERE a.datname=current_database() AND a.usename='kehila_auth_runtime_login' AND l.locktype='advisory' AND l.classid=1262831689::oid AND l.objid=1::oid AND l.objsubid=2 AND l.granted AND l.mode='ExclusiveLock'", &[]).await.unwrap();
        assert_eq!(
            rows.len(),
            1,
            "one exact current authority backend required"
        );
        let pid: i32 = rows[0].get(0);
        assert!(self
            .client()
            .query_one("SELECT pg_terminate_backend($1)", &[&pid])
            .await
            .unwrap()
            .get::<_, bool>(0));
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let absent = self
                    .client()
                    .query_one(
                        "SELECT NOT EXISTS (SELECT FROM pg_stat_activity WHERE pid=$1)",
                        &[&pid],
                    )
                    .await
                    .unwrap()
                    .get::<_, bool>(0);
                if absent {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    pub(crate) async fn serving_blocked_by_me(&self) {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                self.client().batch_execute("SELECT pg_stat_clear_snapshot()").await.unwrap();
                let count = self.client().query_one("SELECT count(*) FROM pg_stat_activity a WHERE a.datname=current_database() AND a.usename='kehila_auth_runtime_login' AND pg_backend_pid()=ANY(pg_blocking_pids(a.pid))", &[]).await.unwrap().get::<_,i64>(0);
                assert!(count <= 1, "ambiguous serving blocker");
                if count == 1 { break; }
                tokio::task::yield_now().await;
            }
        }).await.unwrap();
    }
    pub(crate) async fn close(mut self) {
        drop(self.client.take());
        while let Some(joined) = timeout_at(
            TokioInstant::now() + Duration::from_secs(5),
            self.drivers.join_next(),
        )
        .await
        .unwrap()
        {
            joined.unwrap().unwrap();
        }
    }
}
impl Drop for Admin {
    fn drop(&mut self) {
        self.live.stop();
        drop(self.client.take());
        // Exceptional test failure aborts owned drivers; normal close joins.
        self.drivers.abort_all();
    }
}
