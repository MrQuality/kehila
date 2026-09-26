//! Pure single-task mutation decisions; persistence is owned by the worker.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mutation {
    pub operation_id: String,
    pub expected_version: u64,
    pub title: String,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuccessfulOperation {
    pub operation_id: String,
    pub request: Mutation,
    pub result_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Task {
    pub version: u64,
    pub title: String,
    pub status: String,
    pub successful_operations: Vec<SuccessfulOperation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Decision {
    Replay { version: u64 },
    Apply { version: u64 },
    OperationIdReused,
    VersionConflict { current_version: u64 },
}

/// A recorded identical intention wins over a now-stale expected version.
pub fn decide(current: Option<&Task>, mutation: &Mutation) -> Decision {
    let Some(task) = current else {
        return if mutation.expected_version == 0 {
            Decision::Apply { version: 1 }
        } else {
            Decision::VersionConflict { current_version: 0 }
        };
    };
    if let Some(previous) = task
        .successful_operations
        .iter()
        .find(|entry| entry.operation_id == mutation.operation_id)
    {
        return if previous.request == *mutation {
            Decision::Replay {
                version: previous.result_version,
            }
        } else {
            Decision::OperationIdReused
        };
    }
    if task.version != mutation.expected_version {
        return Decision::VersionConflict {
            current_version: task.version,
        };
    }
    Decision::Apply {
        version: task.version + 1,
    }
}

#[cfg(test)]
mod tests {
    use super::{decide, Decision, Mutation, SuccessfulOperation, Task};

    fn first() -> Mutation {
        Mutation {
            operation_id: "intent-1".into(),
            expected_version: 0,
            title: "Created".into(),
            status: "Open".into(),
        }
    }

    #[test]
    fn identical_replay_returns_original_version_after_later_update() {
        let original = first();
        let task = Task {
            version: 3,
            title: "Later".into(),
            status: "Done".into(),
            successful_operations: vec![SuccessfulOperation {
                operation_id: original.operation_id.clone(),
                request: original.clone(),
                result_version: 1,
            }],
        };
        assert_eq!(
            decide(Some(&task), &original),
            Decision::Replay { version: 1 }
        );
    }

    #[test]
    fn changed_intention_and_stale_version_have_distinct_conflicts() {
        let original = first();
        let task = Task {
            version: 1,
            title: "Created".into(),
            status: "Open".into(),
            successful_operations: vec![SuccessfulOperation {
                operation_id: original.operation_id.clone(),
                request: original.clone(),
                result_version: 1,
            }],
        };
        let mut changed = original.clone();
        changed.title = "Different".into();
        assert_eq!(decide(Some(&task), &changed), Decision::OperationIdReused);
        changed.operation_id = "intent-2".into();
        assert_eq!(
            decide(Some(&task), &changed),
            Decision::VersionConflict { current_version: 1 }
        );
    }

    #[test]
    fn new_intention_requires_matching_version() {
        let mutation = first();
        assert_eq!(decide(None, &mutation), Decision::Apply { version: 1 });
        let mut stale = mutation;
        stale.expected_version = 4;
        assert_eq!(
            decide(None, &stale),
            Decision::VersionConflict { current_version: 0 }
        );
    }
}
