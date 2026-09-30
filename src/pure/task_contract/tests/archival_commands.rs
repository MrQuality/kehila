use task_contract::archive::{
    decide_item_archive, decide_project_archive_command, ArchiveCommandError, ItemArchiveCommand,
    ItemArchiveDecision, ItemArchiveEffect, ProjectArchiveCommand, ProjectArchiveDecision,
    SuccessfulItemArchive, SuccessfulProjectArchive,
};
use task_contract::project::{ArchiveAction, ArchiveEffect, EstimateUnit, Project};
use task_contract::relationship::EndpointIdentity;
use task_contract::work_item::{
    Configuration, Phase, ProjectId, Status, StatusId, WorkItem, WorkItemId, WorkItemType,
    WorkItemTypeId, Workflow, WorkflowId,
};

fn project() -> Project {
    Project {
        id: ProjectId("project".into()),
        name: "Project".into(),
        prefix: "PRJ".into(),
        configuration_revision: 4,
        next_sequence: 9,
        archived: false,
        estimate_unit: EstimateUnit::Hours,
        ever_estimated: false,
    }
}

fn config() -> Configuration {
    Configuration {
        project_id: ProjectId("project".into()),
        revision: 4,
        project_archived: false,
        statuses: vec![
            Status {
                id: StatusId("active".into()),
                phase: Phase::Active,
                archived: false,
            },
            Status {
                id: StatusId("new".into()),
                phase: Phase::New,
                archived: false,
            },
        ],
        workflows: vec![Workflow {
            id: WorkflowId("flow".into()),
            status_ids: vec![StatusId("active".into()), StatusId("new".into())],
            initial_status_id: StatusId("new".into()),
            permitted_phase_changes: vec![],
            archived: false,
        }],
        types: vec![WorkItemType {
            id: WorkItemTypeId("task".into()),
            permitted_workflows: vec![WorkflowId("flow".into())],
            default_workflow_id: WorkflowId("flow".into()),
            title_field_id: None,
            archived: false,
        }],
        fields: vec![],
    }
}

fn item() -> WorkItem {
    WorkItem {
        id: WorkItemId("item".into()),
        project_id: ProjectId("project".into()),
        type_id: WorkItemTypeId("task".into()),
        workflow_id: WorkflowId("flow".into()),
        status_id: StatusId("active".into()),
        version: 3,
        archived: false,
    }
}

#[test]
fn archiving_an_item_clears_its_selection_without_completing_it() {
    let command = ItemArchiveCommand {
        operation_id: "archive-item".into(),
        expected_item_version: 3,
        expected_configuration_revision: 4,
        action: ArchiveAction::Archive,
    };
    let ItemArchiveDecision::Apply(result) =
        decide_item_archive(&config(), &item(), None, true, &command)
    else {
        panic!("eligible archive must apply");
    };
    assert!(result.item.archived);
    assert_eq!(result.item.version, 4);
    assert_eq!(result.item.status_id, StatusId("active".into()));
    assert_eq!(
        result.effect,
        ItemArchiveEffect::ClearCurrentWorkForItem(EndpointIdentity {
            project_id: ProjectId("project".into()),
            item_id: WorkItemId("item".into()),
        })
    );
    let restore = ItemArchiveCommand {
        operation_id: "restore-item".into(),
        expected_item_version: 4,
        action: ArchiveAction::Restore,
        ..command
    };
    let ItemArchiveDecision::Apply(restored) =
        decide_item_archive(&config(), &result.item, None, true, &restore)
    else {
        panic!("eligible restore must apply");
    };
    assert!(!restored.item.archived);
    assert_eq!(restored.effect, ItemArchiveEffect::None);
}

#[test]
fn project_archival_advances_revision_and_clears_project_selections() {
    let command = ProjectArchiveCommand {
        operation_id: "archive-project".into(),
        expected_configuration_revision: 4,
        action: ArchiveAction::Archive,
    };
    let ProjectArchiveDecision::Apply(result) =
        decide_project_archive_command(&project(), None, true, &command)
    else {
        panic!("eligible project archive must apply");
    };
    assert!(result.project.archived);
    assert_eq!(result.project.configuration_revision, 5);
    assert_eq!(result.project.next_sequence, 9);
    assert_eq!(
        result.effect,
        ArchiveEffect::ClearCurrentWorkInProject(ProjectId("project".into()))
    );
}

#[test]
fn archive_commands_check_versions_and_replay_recorded_success() {
    let command = ItemArchiveCommand {
        operation_id: "archive-item".into(),
        expected_item_version: 3,
        expected_configuration_revision: 4,
        action: ArchiveAction::Archive,
    };
    let ItemArchiveDecision::Apply(result) =
        decide_item_archive(&config(), &item(), None, true, &command)
    else {
        panic!("eligible archive must apply")
    };
    assert_eq!(
        decide_item_archive(&config(), &result.item, None, true, &command),
        ItemArchiveDecision::Reject(ArchiveCommandError::ItemVersionConflict {
            current_version: 4
        })
    );
    let previous = SuccessfulItemArchive {
        item_id: WorkItemId("item".into()),
        request: command.clone(),
        result: result.clone(),
    };
    let mut changed = config();
    changed.revision = 5;
    changed.project_archived = true;
    assert_eq!(
        decide_item_archive(&changed, &result.item, Some(&previous), true, &command),
        ItemArchiveDecision::Replay(result)
    );
    let mut reused = command;
    reused.action = ArchiveAction::Restore;
    assert_eq!(
        decide_item_archive(&changed, &item(), Some(&previous), true, &reused),
        ItemArchiveDecision::Reject(ArchiveCommandError::OperationIdReused)
    );
}

#[test]
fn project_archive_replay_precedes_revision_checks_and_restoration_requires_archival() {
    let command = ProjectArchiveCommand {
        operation_id: "archive-project".into(),
        expected_configuration_revision: 4,
        action: ArchiveAction::Archive,
    };
    let ProjectArchiveDecision::Apply(result) =
        decide_project_archive_command(&project(), None, true, &command)
    else {
        panic!("eligible archive must apply")
    };
    let previous = SuccessfulProjectArchive {
        project_id: ProjectId("project".into()),
        request: command.clone(),
        result: result.clone(),
    };
    assert_eq!(
        decide_project_archive_command(&result.project, Some(&previous), true, &command),
        ProjectArchiveDecision::Replay(result.clone())
    );
    let restore = ProjectArchiveCommand {
        operation_id: "restore-project".into(),
        expected_configuration_revision: 4,
        action: ArchiveAction::Restore,
    };
    assert_eq!(
        decide_project_archive_command(&result.project, None, true, &restore),
        ProjectArchiveDecision::Reject(ArchiveCommandError::ConfigurationConflict {
            current_revision: 5
        })
    );
    assert_eq!(
        decide_project_archive_command(&project(), None, true, &restore),
        ProjectArchiveDecision::Reject(ArchiveCommandError::InvalidOperation)
    );
    let restore = ProjectArchiveCommand {
        expected_configuration_revision: 5,
        ..restore
    };
    let ProjectArchiveDecision::Apply(restored) =
        decide_project_archive_command(&result.project, None, true, &restore)
    else {
        panic!("eligible project restore must apply");
    };
    assert!(!restored.project.archived);
    assert_eq!(restored.project.configuration_revision, 6);
    assert_eq!(restored.effect, ArchiveEffect::None);
}

#[test]
fn an_archived_project_blocks_item_restoration_until_project_restore() {
    let mut archived_item = item();
    archived_item.archived = true;
    let mut archived_config = config();
    archived_config.project_archived = true;
    let command = ItemArchiveCommand {
        operation_id: "restore-item".into(),
        expected_item_version: 3,
        expected_configuration_revision: 4,
        action: ArchiveAction::Restore,
    };
    assert_eq!(
        decide_item_archive(&archived_config, &archived_item, None, true, &command),
        ItemArchiveDecision::Reject(ArchiveCommandError::ArchivedProject)
    );
    assert!(archived_item.archived);
}

#[test]
fn archival_rejects_an_orphaned_status_reference() {
    let mut orphaned = item();
    orphaned.status_id = StatusId("missing".into());
    let command = ItemArchiveCommand {
        operation_id: "archive-item".into(),
        expected_item_version: 3,
        expected_configuration_revision: 4,
        action: ArchiveAction::Archive,
    };
    assert_eq!(
        decide_item_archive(&config(), &orphaned, None, true, &command),
        ItemArchiveDecision::Reject(ArchiveCommandError::InvalidReference)
    );
}
