use task_contract::project::{
    allocate_readable_id, change_estimate_unit, change_field_kind, change_prefix,
    decide_project_archive, estimate_from_decimal, item_access, record_estimate, ArchiveAction,
    ArchiveEffect, Estimate, EstimateUnit, FieldKind, ItemAccess, Project, ProjectError,
    ReadableId,
};
use task_contract::work_item::{ProjectId, WorkItemId};

fn project() -> Project {
    Project {
        id: ProjectId("p".into()),
        name: "Project".into(),
        prefix: "YAJA".into(),
        configuration_revision: 4,
        next_sequence: 12,
        archived: false,
        estimate_unit: EstimateUnit::Hours,
        ever_estimated: false,
    }
}

#[test]
fn archive_and_restore_affect_access_and_selection_without_lifecycle_effects() {
    let initial = project();
    assert_eq!(
        item_access(&initial, false),
        ItemAccess {
            readable: true,
            writable: true,
            selectable: true
        }
    );
    let archived = decide_project_archive(&initial, ArchiveAction::Archive).unwrap();
    assert!(archived.project.archived);
    assert_eq!(archived.project.configuration_revision, 5);
    assert_eq!(
        archived.effect,
        ArchiveEffect::ClearCurrentWorkInProject(ProjectId("p".into()))
    );
    assert_eq!(
        item_access(&archived.project, false),
        ItemAccess {
            readable: true,
            writable: false,
            selectable: false
        }
    );
    let restored = decide_project_archive(&archived.project, ArchiveAction::Restore).unwrap();
    assert_eq!(restored.project.configuration_revision, 6);
    assert_eq!(restored.effect, ArchiveEffect::None);
    assert_eq!(
        item_access(&restored.project, false),
        ItemAccess {
            readable: true,
            writable: true,
            selectable: true
        }
    );
    assert_eq!(
        item_access(&restored.project, true),
        ItemAccess {
            readable: true,
            writable: false,
            selectable: false
        }
    );
}

#[test]
fn prefix_change_affects_future_ids_only_and_sequence_is_monotonic() {
    let initial = project();
    let (after_first, first) = allocate_readable_id(&initial, WorkItemId("first".into())).unwrap();
    assert_eq!(
        first,
        ReadableId {
            item_id: WorkItemId("first".into()),
            project_id: ProjectId("p".into()),
            prefix: "YAJA".into(),
            sequence: 12
        }
    );
    let renamed = change_prefix(&after_first, "WORK").unwrap();
    assert_eq!(renamed.configuration_revision, 5);
    let (after_second, second) =
        allocate_readable_id(&renamed, WorkItemId("second".into())).unwrap();
    assert_eq!(second.display(), "WORK-13");
    assert_eq!(first.display(), "YAJA-12");
    assert_eq!(after_second.next_sequence, 14);
    assert_eq!(initial.prefix, "YAJA");
    assert_eq!(after_second.configuration_revision, 5);
}

#[test]
fn readable_ids_are_project_scoped_and_prefix_bounds_are_enforced() {
    let first_project = project();
    let mut second_project = project();
    second_project.id = ProjectId("another".into());
    let (_, first) = allocate_readable_id(&first_project, WorkItemId("first".into())).unwrap();
    let (_, second) = allocate_readable_id(&second_project, WorkItemId("second".into())).unwrap();
    assert_eq!(first.display(), second.display());
    assert_ne!(first.project_id, second.project_id);
    for valid in ["AB", "ABCDEFGHIJKL", "A12345678901"] {
        assert!(change_prefix(&first_project, valid).is_ok());
    }
    for invalid in ["A", "ABCDEFGHIJKLM", "1A", "Ab", "A_"] {
        assert_eq!(
            change_prefix(&first_project, invalid),
            Err(ProjectError::InvalidPrefix)
        );
    }
}

#[test]
fn archived_projects_cannot_allocate_or_change_prefix_and_sequence_cannot_wrap() {
    let archived = Project {
        archived: true,
        ..project()
    };
    assert_eq!(
        allocate_readable_id(&archived, WorkItemId("x".into())),
        Err(ProjectError::Archived)
    );
    assert_eq!(
        change_prefix(&archived, "WORK"),
        Err(ProjectError::Archived)
    );
    let exhausted = Project {
        next_sequence: u64::MAX,
        ..project()
    };
    assert_eq!(
        allocate_readable_id(&exhausted, WorkItemId("x".into())),
        Err(ProjectError::SequenceExhausted)
    );
    assert_eq!(
        change_prefix(&project(), "bad-prefix"),
        Err(ProjectError::InvalidPrefix)
    );
}

#[test]
fn estimates_are_exact_and_zero_is_distinct_from_absence() {
    assert_eq!(
        estimate_from_decimal("0").unwrap(),
        Estimate::try_from_thousandths(0).unwrap()
    );
    assert_eq!(
        estimate_from_decimal("0.001").unwrap(),
        Estimate::try_from_thousandths(1).unwrap()
    );
    assert_eq!(
        estimate_from_decimal("12.345").unwrap(),
        Estimate::try_from_thousandths(12_345).unwrap()
    );
    for invalid in ["", "-1", "1.2345", "1e3", "1.", ".5", "18446744073709552"] {
        assert_eq!(
            estimate_from_decimal(invalid),
            Err(ProjectError::InvalidEstimate)
        );
    }
    assert_eq!(
        estimate_from_decimal("999999.999"),
        Estimate::try_from_thousandths(Estimate::MAX_THOUSANDTHS)
    );
    assert_eq!(
        estimate_from_decimal("1000000"),
        Err(ProjectError::InvalidEstimate)
    );
    let absent: Option<Estimate> = None;
    assert_ne!(absent, Some(Estimate::try_from_thousandths(0).unwrap()));
}

#[test]
fn project_unit_locks_after_first_recorded_estimate_even_if_cleared() {
    let initial = project();
    assert_eq!(
        change_estimate_unit(&initial, EstimateUnit::Points)
            .unwrap()
            .estimate_unit,
        EstimateUnit::Points
    );
    assert_eq!(
        change_estimate_unit(&initial, EstimateUnit::Points)
            .unwrap()
            .configuration_revision,
        5
    );
    let used = record_estimate(&initial, Some(Estimate::try_from_thousandths(0).unwrap())).unwrap();
    let used = record_estimate(&used, None).unwrap();
    assert_eq!(
        change_estimate_unit(&used, EstimateUnit::Points),
        Err(ProjectError::EstimateUnitLocked)
    );
    assert_eq!(change_estimate_unit(&used, EstimateUnit::Hours), Ok(used));
}

#[test]
fn populated_field_kind_changes_require_explicit_migration() {
    assert_eq!(
        change_field_kind(&project(), FieldKind::Text, FieldKind::Number, true),
        Err(ProjectError::FieldMigrationRequired)
    );
    assert_eq!(
        change_field_kind(&project(), FieldKind::Text, FieldKind::Text, true),
        Ok(())
    );
    assert_eq!(
        change_field_kind(&project(), FieldKind::Text, FieldKind::Number, false),
        Ok(())
    );
    let archived = Project {
        archived: true,
        ..project()
    };
    assert_eq!(
        change_field_kind(&archived, FieldKind::Text, FieldKind::Number, false),
        Err(ProjectError::Archived)
    );
}
