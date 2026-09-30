//! Pure commands for independently versioned WorkItem knowledge.

use crate::knowledge::{KnowledgeEntry, KnowledgeEntryId, KnowledgeKind, KnowledgeValue};
use crate::relationship::EndpointIdentity;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeRevision {
    pub version: i64,
    pub actor_id: String,
    pub value: KnowledgeValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeSnapshot {
    pub item: EndpointIdentity,
    pub item_version: u64,
    pub project_archived: bool,
    pub item_archived: bool,
    pub entry: Option<KnowledgeEntry>,
    /// Complete, ordered revision history, including the current revision.
    pub history: Vec<KnowledgeRevision>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KnowledgeAuthorization {
    pub can_read: bool,
    pub can_edit: bool,
}

impl KnowledgeAuthorization {
    pub const fn allowed() -> Self {
        Self {
            can_read: true,
            can_edit: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KnowledgeAction {
    Create {
        expected_item_version: u64,
        kind: KnowledgeKind,
        value: KnowledgeValue,
    },
    Edit {
        expected_entry_version: i64,
        value: KnowledgeValue,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeCommand {
    pub operation_id: String,
    /// Assigned by the trusted command boundary; storage must ensure uniqueness.
    pub entry_id: KnowledgeEntryId,
    pub item: EndpointIdentity,
    pub actor_id: String,
    pub action: KnowledgeAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeResult {
    pub entry: KnowledgeEntry,
    pub history: Vec<KnowledgeRevision>,
}

pub type SuccessfulKnowledge = (KnowledgeCommand, KnowledgeResult);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KnowledgeDecision {
    Replay(KnowledgeResult),
    Apply(KnowledgeResult),
    Reject(KnowledgeError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KnowledgeError {
    Unauthorized,
    OperationIdReused,
    VersionConflict { current_version: i64 },
    ItemVersionConflict { current_version: u64 },
    InvalidReference,
    InvalidOperation,
    ArchivedTarget,
    AlreadyExists,
    Value(crate::knowledge::KnowledgeError),
    VersionExhausted,
}

/// The snapshot, prior success, and grants must be authoritative. B-005 must
/// atomically protect item archival, entry identity/version/history, and the
/// success record. A successful replay is still subject to current grants.
pub fn decide_knowledge(
    snapshot: &KnowledgeSnapshot,
    authorization: KnowledgeAuthorization,
    previous: Option<&SuccessfulKnowledge>,
    command: &KnowledgeCommand,
) -> KnowledgeDecision {
    use KnowledgeDecision::{Apply, Reject, Replay};
    use KnowledgeError as E;

    if !authorization.can_read || !authorization.can_edit {
        return Reject(E::Unauthorized);
    }
    if let Some((request, result)) = previous {
        if request.operation_id != command.operation_id || request.item != command.item {
            return Reject(E::InvalidReference);
        }
        return if request == command {
            Replay(result.clone())
        } else {
            Reject(E::OperationIdReused)
        };
    }
    if command.operation_id.is_empty() || command.actor_id.trim().is_empty() {
        return Reject(E::InvalidOperation);
    }
    if command.entry_id.0.is_empty() || command.item != snapshot.item || snapshot.item_version == 0
    {
        return Reject(E::InvalidReference);
    }
    if snapshot.project_archived || snapshot.item_archived {
        return Reject(E::ArchivedTarget);
    }
    match &command.action {
        KnowledgeAction::Create {
            expected_item_version,
            kind,
            value,
        } => {
            if *expected_item_version != snapshot.item_version {
                return Reject(E::ItemVersionConflict {
                    current_version: snapshot.item_version,
                });
            }
            if snapshot.entry.is_some() || !snapshot.history.is_empty() {
                return Reject(E::AlreadyExists);
            }
            let entry = KnowledgeEntry {
                id: command.entry_id.clone(),
                item: command.item.clone(),
                kind: *kind,
                value: value.clone(),
                version: 1,
                archived: false,
            };
            if let Err(error) = entry.validate() {
                return Reject(E::Value(error));
            }
            Apply(KnowledgeResult {
                history: vec![KnowledgeRevision {
                    version: 1,
                    actor_id: command.actor_id.clone(),
                    value: value.clone(),
                }],
                entry,
            })
        }
        KnowledgeAction::Edit {
            expected_entry_version,
            value,
        } => {
            let Some(current) = &snapshot.entry else {
                return Reject(E::InvalidReference);
            };
            if current.id != command.entry_id || current.item != command.item {
                return Reject(E::InvalidReference);
            }
            if *expected_entry_version != current.version {
                return Reject(E::VersionConflict {
                    current_version: current.version,
                });
            }
            if current.archived {
                return Reject(E::ArchivedTarget);
            }
            if current.version == i64::MAX {
                return Reject(E::VersionExhausted);
            }
            if !valid_history(current, &snapshot.history) {
                return Reject(E::InvalidReference);
            }
            if current.value == *value {
                return Reject(E::InvalidOperation);
            }
            let mut entry = current.clone();
            entry.value = value.clone();
            entry.version += 1;
            if let Err(error) = entry.validate() {
                return Reject(E::Value(error));
            }
            let mut history = snapshot.history.clone();
            history.push(KnowledgeRevision {
                version: entry.version,
                actor_id: command.actor_id.clone(),
                value: value.clone(),
            });
            Apply(KnowledgeResult { entry, history })
        }
    }
}

fn valid_history(entry: &KnowledgeEntry, history: &[KnowledgeRevision]) -> bool {
    usize::try_from(entry.version).ok() == Some(history.len())
        && history.iter().enumerate().all(|(index, revision)| {
            revision.version == (index + 1) as i64 && !revision.actor_id.trim().is_empty()
        })
        && history.last().is_some_and(|last| last.value == entry.value)
}
