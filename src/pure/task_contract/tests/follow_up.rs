use task_contract::error_code::{Code, StableCode};
use task_contract::follow_up::{
    decide_follow_up, FollowUpAuthorization, FollowUpCommand, FollowUpDecision, FollowUpError,
    FollowUpId, FollowUpRecord, FollowUpSnapshot,
};
use task_contract::relationship::EndpointIdentity;
use task_contract::work_item::{ProjectId, WorkItemId};

fn endpoint(project: &str, item: &str) -> EndpointIdentity {
    EndpointIdentity {
        project_id: ProjectId(project.into()),
        item_id: WorkItemId(item.into()),
    }
}

fn snapshot(identity: EndpointIdentity) -> FollowUpSnapshot {
    FollowUpSnapshot {
        item: identity,
        project_revision: 2,
        item_version: 3,
        project_archived: false,
        item_archived: false,
    }
}

fn command() -> FollowUpCommand {
    FollowUpCommand {
        operation_id: "operation".into(),
        id: FollowUpId("origin".into()),
        source: endpoint("a", "source"),
        follow_up: endpoint("b", "child"),
        expected_source_project_revision: 2,
        expected_follow_up_project_revision: 2,
        expected_source_item_version: 3,
        expected_follow_up_item_version: 3,
    }
}

#[test]
fn cross_project_origin_requires_both_link_grants() {
    let cmd = command();
    let source = snapshot(cmd.source.clone());
    let child = snapshot(cmd.follow_up.clone());
    let allowed = FollowUpAuthorization {
        can_link_source: true,
        can_link_follow_up: true,
    };
    let result = decide_follow_up(&source, &child, allowed, None, &[], None, &cmd);
    assert_eq!(
        result,
        FollowUpDecision::Apply(FollowUpRecord {
            id: cmd.id.clone(),
            source: cmd.source.clone(),
            follow_up: cmd.follow_up.clone(),
        })
    );
    assert_eq!(
        decide_follow_up(
            &source,
            &child,
            FollowUpAuthorization {
                can_link_source: true,
                can_link_follow_up: false
            },
            None,
            &[],
            None,
            &cmd
        ),
        FollowUpDecision::Reject(FollowUpError::Unauthorized)
    );
}

#[test]
fn origin_is_single_parent_and_cannot_cycle() {
    let cmd = command();
    let source = snapshot(cmd.source.clone());
    let child = snapshot(cmd.follow_up.clone());
    let allowed = FollowUpAuthorization {
        can_link_source: true,
        can_link_follow_up: true,
    };
    let existing = FollowUpRecord {
        id: FollowUpId("other".into()),
        source: endpoint("a", "other"),
        follow_up: cmd.follow_up.clone(),
    };
    assert_eq!(
        decide_follow_up(&source, &child, allowed, Some(&existing), &[], None, &cmd),
        FollowUpDecision::Reject(FollowUpError::AlreadyHasOrigin)
    );
    assert_eq!(
        decide_follow_up(
            &source,
            &child,
            allowed,
            None,
            std::slice::from_ref(&cmd.follow_up),
            None,
            &cmd
        ),
        FollowUpDecision::Reject(FollowUpError::Cycle)
    );
    let mut self_origin = cmd.clone();
    self_origin.follow_up = self_origin.source.clone();
    assert_eq!(
        decide_follow_up(&source, &source, allowed, None, &[], None, &self_origin),
        FollowUpDecision::Reject(FollowUpError::SelfOrigin)
    );
}

#[test]
fn replay_precedes_stale_versions_and_archival_after_current_grants() {
    let cmd = command();
    let source = snapshot(cmd.source.clone());
    let child = snapshot(cmd.follow_up.clone());
    let allowed = FollowUpAuthorization {
        can_link_source: true,
        can_link_follow_up: true,
    };
    let record = match decide_follow_up(&source, &child, allowed, None, &[], None, &cmd) {
        FollowUpDecision::Apply(record) => record,
        other => panic!("unexpected {other:?}"),
    };
    let previous = (cmd.clone(), record.clone());
    let changed = FollowUpSnapshot {
        item_version: 5,
        item_archived: true,
        ..source
    };
    assert_eq!(
        decide_follow_up(
            &changed,
            &child,
            allowed,
            Some(&record),
            &[],
            Some(&previous),
            &cmd
        ),
        FollowUpDecision::Replay(record)
    );
    assert_eq!(
        decide_follow_up(
            &changed,
            &child,
            FollowUpAuthorization {
                can_link_source: false,
                can_link_follow_up: true
            },
            Some(&previous.1),
            &[],
            Some(&previous),
            &cmd
        ),
        FollowUpDecision::Reject(FollowUpError::Unauthorized)
    );
    let mut reused = cmd.clone();
    reused.follow_up = endpoint("b", "other");
    assert_eq!(
        decide_follow_up(
            &changed,
            &child,
            allowed,
            Some(&previous.1),
            &[],
            Some(&previous),
            &reused
        ),
        FollowUpDecision::Reject(FollowUpError::OperationIdReused)
    );
}

#[test]
fn new_origin_rejects_stale_versions_and_archived_endpoints() {
    let cmd = command();
    let source = snapshot(cmd.source.clone());
    let child = snapshot(cmd.follow_up.clone());
    let allowed = FollowUpAuthorization {
        can_link_source: true,
        can_link_follow_up: true,
    };
    assert_eq!(
        decide_follow_up(
            &FollowUpSnapshot {
                project_revision: 4,
                ..source.clone()
            },
            &child,
            allowed,
            None,
            &[],
            None,
            &cmd
        ),
        FollowUpDecision::Reject(FollowUpError::ConfigurationConflict)
    );
    assert_eq!(
        decide_follow_up(
            &source,
            &FollowUpSnapshot {
                item_version: 4,
                ..child.clone()
            },
            allowed,
            None,
            &[],
            None,
            &cmd
        ),
        FollowUpDecision::Reject(FollowUpError::ItemVersionConflict)
    );
    assert_eq!(
        decide_follow_up(
            &source,
            &FollowUpSnapshot {
                project_archived: true,
                ..child
            },
            allowed,
            None,
            &[],
            None,
            &cmd
        ),
        FollowUpDecision::Reject(FollowUpError::ArchivedTarget)
    );
    assert_eq!(
        FollowUpError::AlreadyHasOrigin.code(),
        Code::FollowUpOriginExists
    );
    assert_eq!(FollowUpError::Cycle.code(), Code::FollowUpCycle);
}
