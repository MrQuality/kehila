//! Single-task mutation rules, independent of persistence and HTTP.

pub mod archive;
pub mod configuration_change;
pub mod conversion;
pub mod current_work;
pub mod field;
pub mod item_mutation;
pub mod project;
pub mod relationship;
pub mod work_item;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mutation {
    pub operation_id: String,
    pub expected_version: i64,
    pub title: String,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuccessfulOperation {
    pub request: Mutation,
    pub result_version: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Decision {
    Replay { version: i64 },
    Apply { version: i64 },
    OperationIdReused,
    VersionConflict { current_version: i64 },
    Invalid(&'static str),
}

/// `previous` is the recorded success for this task and requested operation ID.
/// Replay precedes mutable validation and optimistic version checks.
pub fn decide(
    current_version: i64,
    previous: Option<&SuccessfulOperation>,
    mutation: &Mutation,
) -> Decision {
    if let Some(previous) = previous {
        return if previous.request == *mutation {
            Decision::Replay {
                version: previous.result_version,
            }
        } else {
            Decision::OperationIdReused
        };
    }
    if mutation.expected_version < 0
        || mutation.expected_version == i64::MAX
        || mutation.title.is_empty()
        || mutation.title.len() > 128
    {
        return Decision::Invalid("invalid_mutation");
    }
    if mutation.status != "Open" && mutation.status != "Done" {
        return Decision::Invalid("invalid_status");
    }
    if current_version != mutation.expected_version {
        return Decision::VersionConflict { current_version };
    }
    Decision::Apply {
        version: current_version + 1,
    }
}
