use task_contract::current_work::{
    decide_selection, CurrentWork, SelectionError, SelectionResult, UserId,
};
use task_contract::relationship::{
    canonical_key, decide_relationship, Direction, Endpoint, RelationshipError, RelationshipKey,
    RelationshipType, RelationshipTypeId,
};
use task_contract::work_item::{ProjectId, WorkItemId};

fn endpoint(project: &str, item: &str) -> Endpoint {
    Endpoint {
        project_id: ProjectId(project.into()),
        item_id: WorkItemId(item.into()),
        project_archived: false,
        item_archived: false,
    }
}

fn relation_type(direction: Direction) -> RelationshipType {
    RelationshipType {
        id: RelationshipTypeId("related".into()),
        direction,
        archived: false,
    }
}

#[test]
fn cross_project_links_require_access_to_both_endpoints() {
    let left = endpoint("one", "a");
    let right = endpoint("two", "b");
    let kind = relation_type(Direction::Directed);
    assert_eq!(
        decide_relationship(&kind, &left, &right, true, false, &[]),
        Err(RelationshipError::Unauthorized)
    );
    assert_eq!(
        decide_relationship(&kind, &left, &right, true, true, &[]),
        Ok(RelationshipKey {
            type_id: kind.id.clone(),
            from: left.identity(),
            to: right.identity(),
        })
    );
}

#[test]
fn self_links_and_duplicate_canonical_links_are_rejected() {
    let left = endpoint("one", "a");
    let right = endpoint("one", "b");
    let kind = relation_type(Direction::Directed);
    assert_eq!(
        decide_relationship(&kind, &left, &left, true, true, &[]),
        Err(RelationshipError::SelfLink)
    );
    let first = decide_relationship(&kind, &left, &right, true, true, &[]).unwrap();
    assert_eq!(
        decide_relationship(&kind, &left, &right, true, true, &[first]),
        Err(RelationshipError::Duplicate)
    );
}

#[test]
fn symmetric_links_use_one_order_independent_identity() {
    let left = endpoint("one", "a");
    let right = endpoint("two", "b");
    let kind = relation_type(Direction::Symmetric);
    let first = canonical_key(&kind, &left, &right).unwrap();
    let reversed = canonical_key(&kind, &right, &left).unwrap();
    assert_eq!(first, reversed);
    assert_eq!(
        decide_relationship(&kind, &right, &left, true, true, &[first]),
        Err(RelationshipError::Duplicate)
    );
}

#[test]
fn directed_links_keep_their_orientation() {
    let left = endpoint("one", "a");
    let right = endpoint("two", "b");
    let kind = relation_type(Direction::Directed);
    assert_ne!(
        canonical_key(&kind, &left, &right).unwrap(),
        canonical_key(&kind, &right, &left).unwrap()
    );
}

#[test]
fn archived_targets_and_types_cannot_receive_new_links() {
    let left = endpoint("one", "a");
    let mut right = endpoint("two", "b");
    let mut kind = relation_type(Direction::Directed);
    right.item_archived = true;
    assert_eq!(
        decide_relationship(&kind, &left, &right, true, true, &[]),
        Err(RelationshipError::ArchivedTarget)
    );
    right.item_archived = false;
    kind.archived = true;
    assert_eq!(
        decide_relationship(&kind, &left, &right, true, true, &[]),
        Err(RelationshipError::ArchivedType)
    );
}

#[test]
fn current_work_is_user_scoped_and_selection_has_no_lifecycle_effect() {
    let current = CurrentWork {
        user_id: UserId("owner".into()),
        selected: None,
        version: 2,
    };
    let chosen = endpoint("one", "done-item");
    let result = decide_selection(&current, 2, Some(&chosen), true).unwrap();
    assert_eq!(
        result,
        SelectionResult {
            next: CurrentWork {
                user_id: current.user_id.clone(),
                selected: Some(chosen.identity()),
                version: 3
            },
        }
    );
    assert_eq!(current.selected, None);
    assert_eq!(
        decide_selection(&result.next, 3, None, true)
            .unwrap()
            .next
            .selected,
        None
    );
}

#[test]
fn selection_rejects_stale_versions_unauthorized_and_archived_items() {
    let current = CurrentWork {
        user_id: UserId("owner".into()),
        selected: None,
        version: 2,
    };
    let mut chosen = endpoint("one", "a");
    assert_eq!(
        decide_selection(&current, 2, Some(&chosen), false),
        Err(SelectionError::Unauthorized)
    );
    assert_eq!(
        decide_selection(&current, 1, Some(&chosen), true),
        Err(SelectionError::VersionConflict { current_version: 2 })
    );
    chosen.project_archived = true;
    assert_eq!(
        decide_selection(&current, 2, Some(&chosen), true),
        Err(SelectionError::ArchivedTarget)
    );
}

#[test]
fn selecting_the_same_item_is_an_idempotent_no_op() {
    let selected = endpoint("one", "a");
    let current = CurrentWork {
        user_id: UserId("owner".into()),
        selected: Some(selected.identity()),
        version: 7,
    };
    assert_eq!(
        decide_selection(&current, 7, Some(&selected), true)
            .unwrap()
            .next,
        current
    );
}
