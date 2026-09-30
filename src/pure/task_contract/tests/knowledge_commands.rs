use task_contract::error_code::{Code, StableCode};
use task_contract::knowledge::{KnowledgeEntry, KnowledgeEntryId, KnowledgeKind, KnowledgeValue};
use task_contract::knowledge_command::{
    decide_knowledge, KnowledgeAction, KnowledgeAuthorization, KnowledgeCommand, KnowledgeDecision,
    KnowledgeError, KnowledgeSnapshot,
};
use task_contract::relationship::EndpointIdentity;
use task_contract::work_item::{ProjectId, WorkItemId};

fn item() -> EndpointIdentity {
    EndpointIdentity {
        project_id: ProjectId("project".into()),
        item_id: WorkItemId("item".into()),
    }
}

fn command(action: KnowledgeAction) -> KnowledgeCommand {
    KnowledgeCommand {
        operation_id: "operation".into(),
        entry_id: KnowledgeEntryId("entry".into()),
        item: item(),
        actor_id: "actor".into(),
        action,
    }
}

#[test]
fn create_and_edit_retain_attributed_prior_versions() {
    let create = command(KnowledgeAction::Create {
        expected_item_version: 3,
        kind: KnowledgeKind::Decision,
        value: KnowledgeValue::Text("first".into()),
    });
    let snapshot = KnowledgeSnapshot {
        item: item(),
        item_version: 3,
        project_archived: false,
        item_archived: false,
        entry: None,
        history: vec![],
    };
    let created =
        match decide_knowledge(&snapshot, KnowledgeAuthorization::allowed(), None, &create) {
            KnowledgeDecision::Apply(result) => result,
            other => panic!("unexpected {other:?}"),
        };
    assert_eq!(created.entry.version, 1);
    assert_eq!(created.history.len(), 1);
    assert_eq!(created.history[0].actor_id, "actor");
    let edit = command(KnowledgeAction::Edit {
        expected_entry_version: 1,
        value: KnowledgeValue::Text("revised".into()),
    });
    let edited = match decide_knowledge(
        &KnowledgeSnapshot {
            entry: Some(created.entry.clone()),
            history: created.history.clone(),
            ..snapshot
        },
        KnowledgeAuthorization::allowed(),
        None,
        &edit,
    ) {
        KnowledgeDecision::Apply(result) => result,
        other => panic!("unexpected {other:?}"),
    };
    assert_eq!(edited.entry.version, 2);
    assert_eq!(edited.history.len(), 2);
    assert_eq!(
        edited.history[0].value,
        KnowledgeValue::Text("first".into())
    );
    assert_eq!(
        edited.history[1].value,
        KnowledgeValue::Text("revised".into())
    );
}

#[test]
fn authorization_and_replay_precede_mutable_validation() {
    let create = command(KnowledgeAction::Create {
        expected_item_version: 3,
        kind: KnowledgeKind::Insight,
        value: KnowledgeValue::Text("note".into()),
    });
    let snapshot = KnowledgeSnapshot {
        item: item(),
        item_version: 3,
        project_archived: false,
        item_archived: false,
        entry: None,
        history: vec![],
    };
    let result = match decide_knowledge(&snapshot, KnowledgeAuthorization::allowed(), None, &create)
    {
        KnowledgeDecision::Apply(result) => result,
        other => panic!("unexpected {other:?}"),
    };
    let previous = (create.clone(), result.clone());
    let stale = KnowledgeSnapshot {
        item_archived: true,
        item_version: 9,
        ..snapshot
    };
    assert_eq!(
        decide_knowledge(
            &stale,
            KnowledgeAuthorization::allowed(),
            Some(&previous),
            &create
        ),
        KnowledgeDecision::Replay(result)
    );
    assert_eq!(
        decide_knowledge(
            &stale,
            KnowledgeAuthorization {
                can_read: true,
                can_edit: false
            },
            Some(&previous),
            &create
        ),
        KnowledgeDecision::Reject(KnowledgeError::Unauthorized)
    );
    let mut changed = create.clone();
    changed.actor_id = "other".into();
    assert_eq!(
        decide_knowledge(
            &stale,
            KnowledgeAuthorization::allowed(),
            Some(&previous),
            &changed
        ),
        KnowledgeDecision::Reject(KnowledgeError::OperationIdReused)
    );
}

#[test]
fn edit_rejects_stale_entry_and_corrupt_history() {
    let entry = KnowledgeEntry {
        id: KnowledgeEntryId("entry".into()),
        item: item(),
        kind: KnowledgeKind::Lesson,
        value: KnowledgeValue::Text("old".into()),
        version: 2,
    };
    let snapshot = KnowledgeSnapshot {
        item: item(),
        item_version: 4,
        project_archived: false,
        item_archived: false,
        entry: Some(entry),
        history: vec![],
    };
    let edit = command(KnowledgeAction::Edit {
        expected_entry_version: 1,
        value: KnowledgeValue::Text("new".into()),
    });
    assert_eq!(
        decide_knowledge(&snapshot, KnowledgeAuthorization::allowed(), None, &edit),
        KnowledgeDecision::Reject(KnowledgeError::VersionConflict { current_version: 2 })
    );
    let fresh = command(KnowledgeAction::Edit {
        expected_entry_version: 2,
        value: KnowledgeValue::Text("new".into()),
    });
    assert_eq!(
        decide_knowledge(&snapshot, KnowledgeAuthorization::allowed(), None, &fresh),
        KnowledgeDecision::Reject(KnowledgeError::InvalidReference)
    );
}

#[test]
fn commands_reject_wrong_value_kind_and_archived_targets() {
    let create = command(KnowledgeAction::Create {
        expected_item_version: 3,
        kind: KnowledgeKind::CreatedFile,
        value: KnowledgeValue::Text("path".into()),
    });
    let snapshot = KnowledgeSnapshot {
        item: item(),
        item_version: 3,
        project_archived: false,
        item_archived: false,
        entry: None,
        history: vec![],
    };
    assert_eq!(
        decide_knowledge(&snapshot, KnowledgeAuthorization::allowed(), None, &create),
        KnowledgeDecision::Reject(KnowledgeError::Value(
            task_contract::knowledge::KnowledgeError::WrongKind
        ))
    );
    assert_eq!(
        decide_knowledge(
            &KnowledgeSnapshot {
                item_archived: true,
                ..snapshot
            },
            KnowledgeAuthorization::allowed(),
            None,
            &create
        ),
        KnowledgeDecision::Reject(KnowledgeError::ArchivedTarget)
    );
    assert_eq!(
        KnowledgeError::VersionConflict { current_version: 2 }.code(),
        Code::KnowledgeVersionConflict
    );
}
