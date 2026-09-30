use task_contract::relationship::{
    Direction, Endpoint, LinkAuthorization, RelationshipError, RelationshipId, RelationshipType,
    RelationshipTypeId,
};
use task_contract::relationship_command::{
    decide_create_relationship, EndpointSnapshot, RelationshipCreateCommand,
    RelationshipCreateDecision, RelationshipCreateError, SuccessfulRelationshipCreate,
};
use task_contract::work_item::{ProjectId, WorkItemId};

fn snapshot(
    project: &str,
    item: &str,
    project_revision: u64,
    item_version: u64,
) -> EndpointSnapshot {
    EndpointSnapshot {
        endpoint: Endpoint {
            project_id: ProjectId(project.into()),
            item_id: WorkItemId(item.into()),
            project_archived: false,
            item_archived: false,
        },
        configuration_revision: project_revision,
        item_version,
    }
}

fn kind() -> RelationshipType {
    RelationshipType {
        owner_project_id: ProjectId("one".into()),
        id: RelationshipTypeId("related".into()),
        direction: Direction::Directed,
        archived: false,
    }
}

fn command(from: &EndpointSnapshot, to: &EndpointSnapshot) -> RelationshipCreateCommand {
    RelationshipCreateCommand {
        operation_id: "link-op".into(),
        relationship_id: RelationshipId("link-1".into()),
        owner_project_id: ProjectId("one".into()),
        type_id: RelationshipTypeId("related".into()),
        from: from.endpoint.identity(),
        to: to.endpoint.identity(),
        expected_from_project_revision: from.configuration_revision,
        expected_to_project_revision: to.configuration_revision,
        expected_from_item_version: from.item_version,
        expected_to_item_version: to.item_version,
    }
}

fn authorized() -> LinkAuthorization {
    LinkAuthorization {
        can_use_type: true,
        can_link_from: true,
        can_link_to: true,
    }
}

#[test]
fn cross_project_creation_returns_one_stable_record() {
    let from = snapshot("one", "a", 4, 2);
    let to = snapshot("two", "b", 7, 3);
    let request = command(&from, &to);
    let RelationshipCreateDecision::Apply(record) =
        decide_create_relationship(&kind(), &from, &to, authorized(), &[], None, &request)
    else {
        panic!("valid cross-project link must apply");
    };
    assert_eq!(record.id, request.relationship_id);
    assert_eq!(record.key.owner_project_id, ProjectId("one".into()));
    assert_eq!(record.key.from, from.endpoint.identity());
    assert_eq!(record.key.to, to.endpoint.identity());
    assert_eq!(
        decide_create_relationship(&kind(), &from, &to, authorized(), &[record], None, &request),
        RelationshipCreateDecision::Reject(RelationshipCreateError::Relationship(
            RelationshipError::Duplicate
        ))
    );
}

#[test]
fn both_project_revisions_and_both_item_versions_are_checked() {
    let from = snapshot("one", "a", 4, 2);
    let to = snapshot("two", "b", 7, 3);
    let request = command(&from, &to);
    let changed_from = snapshot("one", "a", 5, 2);
    assert_eq!(
        decide_create_relationship(
            &kind(),
            &changed_from,
            &to,
            authorized(),
            &[],
            None,
            &request
        ),
        RelationshipCreateDecision::Reject(RelationshipCreateError::ConfigurationConflict {
            project_id: ProjectId("one".into()),
            current_revision: 5,
        })
    );
    let changed_to = snapshot("two", "b", 8, 3);
    assert_eq!(
        decide_create_relationship(
            &kind(),
            &from,
            &changed_to,
            authorized(),
            &[],
            None,
            &request
        ),
        RelationshipCreateDecision::Reject(RelationshipCreateError::ConfigurationConflict {
            project_id: ProjectId("two".into()),
            current_revision: 8,
        })
    );
    let changed_from = snapshot("one", "a", 4, 4);
    assert_eq!(
        decide_create_relationship(
            &kind(),
            &changed_from,
            &to,
            authorized(),
            &[],
            None,
            &request
        ),
        RelationshipCreateDecision::Reject(RelationshipCreateError::ItemVersionConflict {
            item_id: WorkItemId("a".into()),
            current_version: 4,
        })
    );
    let changed_to = snapshot("two", "b", 7, 4);
    assert_eq!(
        decide_create_relationship(
            &kind(),
            &from,
            &changed_to,
            authorized(),
            &[],
            None,
            &request
        ),
        RelationshipCreateDecision::Reject(RelationshipCreateError::ItemVersionConflict {
            item_id: WorkItemId("b".into()),
            current_version: 4,
        })
    );
}

#[test]
fn replay_precedes_stale_references_but_follows_current_authorization() {
    let from = snapshot("one", "a", 4, 2);
    let to = snapshot("two", "b", 7, 3);
    let request = command(&from, &to);
    let RelationshipCreateDecision::Apply(record) =
        decide_create_relationship(&kind(), &from, &to, authorized(), &[], None, &request)
    else {
        panic!("fixture create must apply");
    };
    let success = SuccessfulRelationshipCreate {
        request: request.clone(),
        result: record.clone(),
    };
    let mut archived_from = snapshot("one", "a", 5, 9);
    archived_from.endpoint.item_archived = true;
    assert_eq!(
        decide_create_relationship(
            &kind(),
            &archived_from,
            &to,
            authorized(),
            std::slice::from_ref(&record),
            Some(&success),
            &request,
        ),
        RelationshipCreateDecision::Replay(record)
    );
    let denied = LinkAuthorization {
        can_link_to: false,
        ..authorized()
    };
    assert_eq!(
        decide_create_relationship(&kind(), &from, &to, denied, &[], Some(&success), &request),
        RelationshipCreateDecision::Reject(RelationshipCreateError::Unauthorized)
    );
    let mut changed = request.clone();
    changed.relationship_id = RelationshipId("link-2".into());
    assert_eq!(
        decide_create_relationship(
            &kind(),
            &from,
            &to,
            authorized(),
            &[],
            Some(&success),
            &changed
        ),
        RelationshipCreateDecision::Reject(RelationshipCreateError::OperationIdReused)
    );
}

#[test]
fn same_project_endpoints_require_one_coherent_project_revision() {
    let from = snapshot("one", "a", 4, 2);
    let to = snapshot("one", "b", 5, 3);
    let request = command(&from, &to);
    assert_eq!(
        decide_create_relationship(&kind(), &from, &to, authorized(), &[], None, &request),
        RelationshipCreateDecision::Reject(RelationshipCreateError::InvalidReference)
    );
}

#[test]
fn archived_endpoint_and_changed_type_are_rejected_for_new_links() {
    let from = snapshot("one", "a", 4, 2);
    let mut to = snapshot("two", "b", 7, 3);
    let request = command(&from, &to);
    to.endpoint.item_archived = true;
    assert_eq!(
        decide_create_relationship(&kind(), &from, &to, authorized(), &[], None, &request),
        RelationshipCreateDecision::Reject(RelationshipCreateError::Relationship(
            RelationshipError::ArchivedTarget
        ))
    );
    to.endpoint.item_archived = false;
    let mut changed_type = kind();
    changed_type.id = RelationshipTypeId("other".into());
    assert_eq!(
        decide_create_relationship(&changed_type, &from, &to, authorized(), &[], None, &request),
        RelationshipCreateDecision::Reject(RelationshipCreateError::InvalidReference)
    );
}
