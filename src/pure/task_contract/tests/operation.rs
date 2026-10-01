use task_contract::current_work::UserId;
use task_contract::operation::{
    decide_replay, CommandFamily, OperationError, OperationId, OperationKey, OperationSuccess,
    OperationTarget, ReplayDecision,
};
use task_contract::work_item::{ProjectId, WorkItemId};

fn key() -> OperationKey {
    OperationKey {
        actor_id: UserId("actor".into()),
        family: CommandFamily::ItemEdit,
        target: OperationTarget::WorkItem {
            project_id: ProjectId("project".into()),
            item_id: WorkItemId("item".into()),
        },
        operation_id: OperationId::parse("operation").unwrap(),
    }
}

#[test]
fn tokens_are_bounded_case_sensitive_and_not_normalized() {
    for bad in [
        "".to_string(),
        "x".repeat(129),
        "a b".into(),
        "a\n".into(),
        "é".into(),
    ] {
        assert_eq!(
            OperationId::parse(&bad),
            Err(OperationError::InvalidOperation)
        );
    }
    assert!(OperationId::parse(&"x".repeat(128)).is_ok());
    assert_ne!(OperationId::parse("Case"), OperationId::parse("case"));
}

#[test]
fn lookup_is_bound_to_actor_family_target_and_exact_ordered_content() {
    let request = vec!["first", "second"];
    let record = OperationSuccess {
        key: key(),
        request: request.clone(),
        result: 7,
        required_grants: vec!["edit"],
    };
    assert_eq!(
        decide_replay(&key(), &request, Some(&record), &vec![], |_| true),
        ReplayDecision::Replay(7)
    );
    assert_eq!(
        decide_replay(
            &key(),
            &vec!["second", "first"],
            Some(&record),
            &vec![],
            |_| true
        ),
        ReplayDecision::Reject(OperationError::OperationIdReused)
    );
    for field in 0..3 {
        let mut changed = key();
        match field {
            0 => changed.actor_id = UserId("other".into()),
            1 => changed.family = CommandFamily::ItemCreate,
            _ => {
                changed.target = OperationTarget::WorkItem {
                    project_id: ProjectId("other".into()),
                    item_id: WorkItemId("item".into()),
                }
            }
        }
        assert_eq!(
            decide_replay(&changed, &request, Some(&record), &vec![], |_| true),
            ReplayDecision::Reject(OperationError::InvalidReference)
        );
    }
}

#[test]
fn replay_authorizes_the_retained_scope_and_unseen_authorizes_the_new_scope() {
    let record = OperationSuccess {
        key: key(),
        request: 1,
        result: 7,
        required_grants: vec!["edit", "migrate"],
    };
    assert_eq!(
        decide_replay(&key(), &1, Some(&record), &vec!["edit"], |scope| !scope
            .contains(&"migrate")),
        ReplayDecision::Reject(OperationError::Unauthorized)
    );
    assert_eq!(
        decide_replay::<_, i32, _>(&key(), &1, None, &vec!["edit"], |scope| scope
            == &vec!["edit"]),
        ReplayDecision::Unseen
    );
}
