//! Immutable task operations on the FerretDB mapping.

use mongodb::{
    bson::doc,
    error::{ErrorKind, WriteFailure},
    options::{FindOneOptions, IndexOptions},
    sync::{Client, Collection},
    IndexModel,
};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use task_contract::{decide, Decision, Mutation, SuccessfulOperation, Task};
use uuid::{Uuid, Version};

const REPLAY_DAYS: u128 = 90;
const FUTURE_SKEW_MS: u128 = 5 * 60 * 1000;

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

pub struct Store {
    records: Collection<OperationRecord>,
}

fn now_ms() -> Option<u128> {
    Some(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_millis(),
    )
}

pub fn validate(request: &MutationRequest, now: u128) -> Result<(), &'static str> {
    if request.expected_version < 0
        || request.expected_version == i64::MAX
        || request.title.is_empty()
        || request.title.len() > 128
    {
        return Err("invalid_mutation");
    }
    if request.status != "Open" && request.status != "Done" {
        return Err("invalid_status");
    }
    let id = Uuid::parse_str(&request.operation_id).map_err(|_| "invalid_operation_id")?;
    if id.get_version() != Some(Version::SortRand) || id.to_string() != request.operation_id {
        return Err("invalid_operation_id");
    }
    let (seconds, nanos) = id.get_timestamp().ok_or("invalid_operation_id")?.to_unix();
    let issued_ms = u128::from(seconds) * 1000 + u128::from(nanos / 1_000_000);
    if issued_ms > now + FUTURE_SKEW_MS {
        return Err("operation_id_from_future");
    }
    if now.saturating_sub(issued_ms) > REPLAY_DAYS * 24 * 60 * 60 * 1000 {
        return Err("operation_id_expired");
    }
    Ok(())
}

fn duplicate_key(error: &mongodb::error::Error) -> bool {
    matches!(error.kind.as_ref(), ErrorKind::Write(WriteFailure::WriteError(write)) if write.code == 11000)
}

impl Store {
    pub fn connect(
        uri: &str,
        database: &str,
        collection: &str,
    ) -> Result<Self, mongodb::error::Error> {
        let client = Client::with_uri_str(uri)?;
        Ok(Self {
            records: client.database(database).collection(collection),
        })
    }

    /// Installation uses separate credentials from the serving process.
    pub fn install_indexes(&self) -> Result<(), mongodb::error::Error> {
        for (keys, unique) in [
            (doc! { "task_id": 1, "operation_id": 1 }, true),
            (doc! { "task_id": 1, "version": -1 }, false),
        ] {
            let model = IndexModel::builder()
                .keys(keys)
                .options(IndexOptions::builder().unique(unique).build())
                .build();
            self.records.create_index(model, None)?;
        }
        Ok(())
    }

    pub fn check_indexes(&self) -> Result<bool, mongodb::error::Error> {
        let mut operation = false;
        let mut version = false;
        for index in self.records.list_indexes(None)? {
            let index = index?;
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

    pub fn read(&self, task_id: &str) -> Result<Option<OperationRecord>, mongodb::error::Error> {
        self.records.find_one(
            doc! { "task_id": task_id },
            FindOneOptions::builder()
                .sort(doc! { "version": -1 })
                .build(),
        )
    }

    pub fn mutate(&self, task_id: &str, request: &MutationRequest) -> MutationOutcome {
        let Some(clock) = now_ms() else {
            return MutationOutcome::Unavailable;
        };
        if let Err(reason) = validate(request, clock) {
            return MutationOutcome::Invalid(reason);
        }
        let mutation = Mutation {
            operation_id: request.operation_id.clone(),
            expected_version: request.expected_version as u64,
            title: request.title.clone(),
            status: request.status.clone(),
        };
        for _ in 0..8 {
            let previous = match self.records.find_one(
                doc! { "task_id": task_id, "operation_id": &request.operation_id },
                None,
            ) {
                Ok(value) => value,
                Err(_) => return MutationOutcome::Unavailable,
            };
            if let Some(previous) = previous {
                let original = Mutation {
                    operation_id: previous.operation_id.clone(),
                    expected_version: previous.expected_version as u64,
                    title: previous.title.clone(),
                    status: previous.status.clone(),
                };
                let current = Task {
                    version: previous.version as u64,
                    title: previous.title.clone(),
                    status: previous.status.clone(),
                    successful_operations: vec![SuccessfulOperation {
                        operation_id: previous.operation_id,
                        request: original,
                        result_version: previous.version as u64,
                    }],
                };
                return match decide(Some(&current), &mutation) {
                    Decision::Replay { version } => MutationOutcome::Replay(version as i64),
                    _ => MutationOutcome::OperationIdReused,
                };
            }
            let current = match self.read(task_id) {
                Ok(value) => value,
                Err(_) => return MutationOutcome::Unavailable,
            };
            let version = current.as_ref().map_or(0, |record| record.version);
            if version != request.expected_version {
                return MutationOutcome::VersionConflict(version);
            }
            let next = version + 1;
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
            match self.records.insert_one(record, None) {
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
    use super::{validate, MutationRequest};
    use uuid::Uuid;

    #[test]
    fn expired_id_is_rejected_before_it_could_be_reexecuted() {
        let id = Uuid::now_v7();
        let (seconds, nanos) = id.get_timestamp().unwrap().to_unix();
        let issued = u128::from(seconds) * 1000 + u128::from(nanos / 1_000_000);
        let request = MutationRequest {
            operation_id: id.to_string(),
            expected_version: 0,
            title: "Task".into(),
            status: "Open".into(),
        };
        assert!(validate(&request, issued).is_ok());
        assert_eq!(
            validate(&request, issued + 91 * 24 * 60 * 60 * 1000),
            Err("operation_id_expired")
        );
    }
}
