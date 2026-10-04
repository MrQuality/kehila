//! Immutable task operations on the FerretDB mapping.

use futures_util::TryStreamExt;
use mongodb::{
    bson::doc,
    error::{ErrorKind, WriteFailure},
    options::{ClientOptions, FindOneOptions, IndexOptions},
    Client, Collection, IndexModel,
};
use serde::{Deserialize, Serialize};
use std::{
    future::Future,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use task_contract::{decide, Decision, Mutation, SuccessfulOperation};
use tokio::{
    sync::Semaphore,
    time::{timeout, Instant},
};
use uuid::{Uuid, Version};

const DAY_MS: u128 = 24 * 60 * 60 * 1000;
const FUTURE_SKEW_MS: u128 = 5 * 60 * 1000;

#[derive(Clone, Copy, Debug)]
pub struct OperationPolicy {
    pub admission_days: u64,
    pub replay_days: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OperationRecord {
    #[serde(rename = "_id")]
    pub id: String,
    pub task_id: String,
    pub version: i64,
    pub operation_id: String,
    pub expected_version: i64,
    pub title: String,
    pub status: String,
    pub committed_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MutationRequest {
    pub operation_id: String,
    pub expected_version: i64,
    pub title: String,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MutationOutcome {
    Saved(i64),
    Replay(i64),
    OperationIdReused,
    VersionConflict(i64),
    Invalid(&'static str),
    Unavailable,
}

#[derive(Clone)]
pub struct Store {
    records: Collection<OperationRecord>,
    policy: OperationPolicy,
    operations: Arc<Semaphore>,
}

fn now_ms() -> Option<u128> {
    Some(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_millis(),
    )
}

pub fn validate_operation_id(request: &MutationRequest, now: u128) -> Result<u128, &'static str> {
    let id = Uuid::parse_str(&request.operation_id).map_err(|_| "invalid_operation_id")?;
    if id.get_version() != Some(Version::SortRand) || id.to_string() != request.operation_id {
        return Err("invalid_operation_id");
    }
    let (seconds, nanos) = id.get_timestamp().ok_or("invalid_operation_id")?.to_unix();
    let issued_ms = u128::from(seconds) * 1000 + u128::from(nanos / 1_000_000);
    if issued_ms > now + FUTURE_SKEW_MS {
        return Err("operation_id_from_future");
    }
    Ok(issued_ms)
}

fn admission_valid(issued_ms: u128, now: u128, policy: OperationPolicy) -> bool {
    now.saturating_sub(issued_ms) <= u128::from(policy.admission_days) * DAY_MS
}

fn replay_valid(committed_ms: i64, now: u128, policy: OperationPolicy) -> bool {
    committed_ms >= 0
        && now.saturating_sub(committed_ms as u128) <= u128::from(policy.replay_days) * DAY_MS
}

fn duplicate_key(error: &mongodb::error::Error) -> bool {
    matches!(error.kind.as_ref(), ErrorKind::Write(WriteFailure::WriteError(write)) if write.code == 11000)
}

impl Store {
    pub async fn connect(
        uri: &str,
        database: &str,
        collection: &str,
        policy: OperationPolicy,
    ) -> Result<Self, mongodb::error::Error> {
        let mut options = ClientOptions::parse(uri).await?;
        options.connect_timeout = Some(Duration::from_secs(2));
        options.server_selection_timeout = Some(Duration::from_secs(2));
        options.max_pool_size = Some(8);
        options.retry_reads = Some(false);
        options.retry_writes = Some(false);
        let client = Client::with_options(options)?;
        Ok(Self {
            records: client.database(database).collection(collection),
            policy,
            operations: Arc::new(Semaphore::new(8)),
        })
    }

    // MongoDB 2.x requires polling driver futures to completion. A timed-out
    // caller drops only the JoinHandle; the task retains its permit until the
    // driver completes. This bounds abandoned work as well as active callers.
    async fn execute<T, F, Fut>(&self, operation: F) -> Result<T, mongodb::error::Error>
    where
        T: Send + 'static,
        F: FnOnce(Store, Instant) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, mongodb::error::Error>> + Send + 'static,
    {
        let permit = self
            .operations
            .clone()
            .try_acquire_owned()
            .map_err(|_| mongodb::error::Error::custom("worker_busy"))?;
        let store = self.clone();
        let budget = Duration::from_secs(4);
        let deadline = Instant::now() + budget;
        let task = tokio::spawn(async move {
            let _permit = permit;
            operation(store, deadline).await
        });
        timeout(budget, task)
            .await
            .map_err(|_| mongodb::error::Error::custom("dependency_timeout"))?
            .map_err(|_| mongodb::error::Error::custom("operation_failed"))?
    }

    pub async fn install_indexes(&self) -> Result<(), mongodb::error::Error> {
        self.execute(|store, _| async move { store.install_indexes_inner().await })
            .await
    }

    pub async fn check_indexes(&self) -> Result<bool, mongodb::error::Error> {
        self.execute(|store, _| async move { store.check_indexes_inner().await })
            .await
    }

    pub async fn read(
        &self,
        task_id: &str,
    ) -> Result<Option<OperationRecord>, mongodb::error::Error> {
        let task_id = task_id.to_owned();
        self.execute(move |store, _| async move { store.read_inner(&task_id).await })
            .await
    }

    /// Installation uses separate credentials from the serving process.
    async fn install_indexes_inner(&self) -> Result<(), mongodb::error::Error> {
        for (keys, unique) in [
            (doc! { "task_id": 1, "operation_id": 1 }, true),
            (doc! { "task_id": 1, "version": -1 }, false),
        ] {
            let model = IndexModel::builder()
                .keys(keys)
                .options(IndexOptions::builder().unique(unique).build())
                .build();
            self.records.create_index(model, None).await?;
        }
        Ok(())
    }

    async fn check_indexes_inner(&self) -> Result<bool, mongodb::error::Error> {
        let mut operation = false;
        let mut version = false;
        let mut indexes = self.records.list_indexes(None).await?;
        while let Some(index) = indexes.try_next().await? {
            if index.keys == doc! { "task_id": 1, "operation_id": 1 } {
                operation = index
                    .options
                    .as_ref()
                    .and_then(|options| options.unique)
                    .unwrap_or(false);
            }
            if index.keys == doc! { "task_id": 1, "version": -1 } {
                version = true;
            }
        }
        Ok(operation && version)
    }

    async fn read_inner(
        &self,
        task_id: &str,
    ) -> Result<Option<OperationRecord>, mongodb::error::Error> {
        self.records
            .find_one(
                doc! { "task_id": task_id },
                FindOneOptions::builder()
                    .sort(doc! { "version": -1 })
                    .build(),
            )
            .await
    }

    /// Cleanup is restricted to uniquely named regression collections.
    pub async fn drop_test_collection(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.records.name().starts_with("kehila_test_") {
            return Err("cleanup requires a kehila_test_ collection".into());
        }
        self.execute(|store, _| async move { store.records.drop(None).await })
            .await?;
        Ok(())
    }

    pub async fn mutate(&self, task_id: &str, request: &MutationRequest) -> MutationOutcome {
        let task_id = task_id.to_owned();
        let request = request.clone();
        self.execute(move |store, deadline| async move {
            Ok(store.mutate_inner(&task_id, &request, deadline).await)
        })
        .await
        .unwrap_or(MutationOutcome::Unavailable)
    }

    async fn mutate_inner(
        &self,
        task_id: &str,
        request: &MutationRequest,
        deadline: Instant,
    ) -> MutationOutcome {
        let mutation = Mutation {
            operation_id: request.operation_id.clone(),
            expected_version: request.expected_version,
            title: request.title.clone(),
            status: request.status.clone(),
        };
        for _ in 0..8 {
            if Instant::now() >= deadline {
                return MutationOutcome::Unavailable;
            }
            // Observe the version BEFORE checking the operation record. If a
            // competing identical request committed before this version read,
            // the following lookup sees it. If it commits later, either lookup
            // sees it or the unique insert fails and we reload both observations.
            let current = match self.read_inner(task_id).await {
                Ok(value) => value,
                Err(_) => return MutationOutcome::Unavailable,
            };
            if Instant::now() >= deadline {
                return MutationOutcome::Unavailable;
            }
            let previous = match self
                .records
                .find_one(
                    doc! { "task_id": task_id, "operation_id": &request.operation_id },
                    None,
                )
                .await
            {
                Ok(value) => value,
                Err(_) => return MutationOutcome::Unavailable,
            };
            if Instant::now() >= deadline {
                return MutationOutcome::Unavailable;
            }
            let Some(clock) = now_ms() else {
                return MutationOutcome::Unavailable;
            };
            let issued_ms = match validate_operation_id(request, clock) {
                Ok(value) => value,
                Err(reason) => return MutationOutcome::Invalid(reason),
            };
            if let Some(previous) = &previous {
                if !replay_valid(previous.committed_ms, clock, self.policy) {
                    return MutationOutcome::Invalid("operation_id_expired");
                }
            } else if !admission_valid(issued_ms, clock, self.policy) {
                return MutationOutcome::Invalid("operation_id_expired");
            }
            let recorded = previous.map(|record| SuccessfulOperation {
                request: Mutation {
                    operation_id: record.operation_id,
                    expected_version: record.expected_version,
                    title: record.title,
                    status: record.status,
                },
                result_version: record.version,
            });
            let version = current.as_ref().map_or(0, |record| record.version);
            let next = match decide(version, recorded.as_ref(), &mutation) {
                Decision::Replay { version } => return MutationOutcome::Replay(version),
                Decision::OperationIdReused => return MutationOutcome::OperationIdReused,
                Decision::VersionConflict { current_version } => {
                    return MutationOutcome::VersionConflict(current_version)
                }
                Decision::Invalid(reason) => return MutationOutcome::Invalid(reason),
                Decision::Apply { version } => version,
            };
            let record = OperationRecord {
                id: format!("{task_id}:{next}"),
                task_id: task_id.into(),
                version: next,
                operation_id: request.operation_id.clone(),
                expected_version: request.expected_version,
                title: request.title.clone(),
                status: request.status.clone(),
                committed_ms: clock as i64,
            };
            if Instant::now() >= deadline {
                return MutationOutcome::Unavailable;
            }
            match self.records.insert_one(record, None).await {
                Ok(_) => return MutationOutcome::Saved(next),
                Err(error) if duplicate_key(&error) => continue,
                Err(_) => return MutationOutcome::Unavailable,
            }
        }
        MutationOutcome::Unavailable
    }
}

#[cfg(test)]
mod tests {
    use super::{
        admission_valid, replay_valid, validate_operation_id, MutationRequest, OperationPolicy,
        DAY_MS,
    };
    use uuid::Uuid;

    #[tokio::test]
    async fn cancelled_callers_do_not_cancel_or_unbound_storage_work() {
        use std::sync::Arc;
        use tokio::sync::Barrier;
        let store = super::Store::connect(
            "mongodb://127.0.0.1:1",
            "test",
            "unused",
            OperationPolicy {
                admission_days: 90,
                replay_days: 90,
            },
        )
        .await
        .unwrap();
        // This tests task ownership, not database behavior; no command is sent.
        let started = Arc::new(Barrier::new(9));
        let release = Arc::new(Barrier::new(9));
        let mut callers = Vec::new();
        for _ in 0..8 {
            let store = store.clone();
            let started = started.clone();
            let release = release.clone();
            callers.push(tokio::spawn(async move {
                store
                    .execute(move |_, _| async move {
                        started.wait().await;
                        release.wait().await;
                        Ok(())
                    })
                    .await
            }));
        }
        started.wait().await;
        for caller in callers {
            caller.abort();
            let _ = caller.await;
        }
        assert!(store.execute(|_, _| async { Ok(()) }).await.is_err());
        release.wait().await;
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while store.operations.available_permits() != 8 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(store.execute(|_, _| async { Ok(()) }).await.is_ok());
    }

    #[test]
    fn admission_and_replay_have_independent_clocks() {
        let id = Uuid::now_v7();
        let (seconds, nanos) = id.get_timestamp().unwrap().to_unix();
        let issued = u128::from(seconds) * 1000 + u128::from(nanos / 1_000_000);
        let request = MutationRequest {
            operation_id: id.to_string(),
            expected_version: 0,
            title: "Task".into(),
            status: "Open".into(),
        };
        let policy = OperationPolicy {
            admission_days: 90,
            replay_days: 90,
        };
        assert_eq!(
            validate_operation_id(&request, issued + 91 * DAY_MS),
            Ok(issued)
        );
        assert!(!admission_valid(issued, issued + 91 * DAY_MS, policy));
        let committed = issued + 89 * DAY_MS;
        assert!(replay_valid(
            committed as i64,
            committed + 89 * DAY_MS,
            policy
        ));
        assert!(!replay_valid(
            committed as i64,
            committed + 91 * DAY_MS,
            policy
        ));
    }
}
