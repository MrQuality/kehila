use task_contract::archive::{
    decide_item_archive as decide_item_archive_raw,
    decide_project_archive_command as decide_project_archive_command_raw, ArchiveCommandError,
    ItemArchiveCommand, ItemArchiveDecision, ItemArchiveEffect, ProjectArchiveCommand,
    ProjectArchiveDecision, SelectionClear, SuccessfulItemArchive, SuccessfulProjectArchive,
};
use task_contract::current_work::{CurrentWork, UserId};
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
        relationship_types: vec![],
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

fn decide_item_archive(
    configuration: &Configuration,
    item: &WorkItem,
    previous: Option<&SuccessfulItemArchive>,
    authorized: bool,
    command: &ItemArchiveCommand,
) -> ItemArchiveDecision {
    decide_item_archive_raw(configuration, item, &[], previous, authorized, command)
}

fn decide_project_archive_command(
    project: &Project,
    previous: Option<&SuccessfulProjectArchive>,
    authorized: bool,
    command: &ProjectArchiveCommand,
) -> ProjectArchiveDecision {
    let mut configuration = config();
    configuration.revision = project.configuration_revision;
    configuration.project_archived = project.archived;
    decide_project_archive_command_raw(project, &configuration, &[], previous, authorized, command)
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

#[test]
fn project_archive_advances_project_and_configuration_together() {
    let command = ProjectArchiveCommand {
        operation_id: "archive-project".into(),
        expected_configuration_revision: 4,
        action: ArchiveAction::Archive,
    };
    let ProjectArchiveDecision::Apply(result) =
        decide_project_archive_command_raw(&project(), &config(), &[], None, true, &command)
    else {
        panic!("eligible project archive must apply");
    };
    assert_eq!(result.project.configuration_revision, 5);
    assert_eq!(result.configuration.revision, 5);
    assert!(result.project.archived);
    assert!(result.configuration.project_archived);

    let mut mismatched = config();
    mismatched.project_archived = true;
    assert_eq!(
        decide_project_archive_command_raw(&project(), &mismatched, &[], None, true, &command),
        ProjectArchiveDecision::Reject(ArchiveCommandError::InvalidConfiguration)
    );
}

#[test]
fn item_archive_plans_versioned_clears_for_every_selected_user() {
    let command = ItemArchiveCommand {
        operation_id: "archive-item".into(),
        expected_item_version: 3,
        expected_configuration_revision: 4,
        action: ArchiveAction::Archive,
    };
    let selected = item();
    let selections = vec![
        CurrentWork {
            user_id: UserId("alice".into()),
            selected: Some(EndpointIdentity {
                project_id: selected.project_id.clone(),
                item_id: selected.id.clone(),
            }),
            version: 2,
        },
        CurrentWork {
            user_id: UserId("bob".into()),
            selected: Some(EndpointIdentity {
                project_id: selected.project_id.clone(),
                item_id: selected.id.clone(),
            }),
            version: 7,
        },
        CurrentWork {
            user_id: UserId("carol".into()),
            selected: None,
            version: 9,
        },
    ];
    let ItemArchiveDecision::Apply(result) =
        decide_item_archive_raw(&config(), &selected, &selections, None, true, &command)
    else {
        panic!("eligible item archive must apply");
    };
    assert_eq!(result.selection_clears.len(), 2);
    assert_eq!(
        result.selection_clears[0],
        SelectionClear {
            before: selections[0].clone(),
            after: CurrentWork {
                selected: None,
                version: 3,
                ..selections[0].clone()
            }
        }
    );
    assert_eq!(result.selection_clears[1].after.version, 8);
    assert_eq!(
        selections[0].selected,
        result.selection_clears[0].before.selected
    );
    let previous = SuccessfulItemArchive {
        item_id: selected.id.clone(),
        request: command.clone(),
        result: result.clone(),
    };
    let current_selections = result
        .selection_clears
        .iter()
        .map(|clear| clear.after.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        decide_item_archive_raw(
            &config(),
            &result.item,
            &current_selections,
            Some(&previous),
            true,
            &command,
        ),
        ItemArchiveDecision::Replay(result)
    );
}

#[test]
fn project_archive_clears_only_its_selections_and_fails_on_version_overflow() {
    let command = ProjectArchiveCommand {
        operation_id: "archive-project".into(),
        expected_configuration_revision: 4,
        action: ArchiveAction::Archive,
    };
    let selected = |user: &str, project: &str, version| CurrentWork {
        user_id: UserId(user.into()),
        selected: Some(EndpointIdentity {
            project_id: ProjectId(project.into()),
            item_id: WorkItemId("item".into()),
        }),
        version,
    };
    let selections = [selected("alice", "project", 2), selected("bob", "other", 3)];
    let ProjectArchiveDecision::Apply(result) = decide_project_archive_command_raw(
        &project(),
        &config(),
        &selections,
        None,
        true,
        &command,
    ) else {
        panic!("eligible project archive must apply");
    };
    assert_eq!(result.selection_clears.len(), 1);
    assert_eq!(result.selection_clears[0].after.version, 3);
    assert_eq!(result.selection_clears[0].after.selected, None);

    let overflow = [selected("alice", "project", u64::MAX)];
    assert_eq!(
        decide_project_archive_command_raw(&project(), &config(), &overflow, None, true, &command),
        ProjectArchiveDecision::Reject(ArchiveCommandError::VersionOverflow)
    );
}
