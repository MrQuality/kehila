use task_contract::work_item::{
    decide, Action, Command, Configuration, Decision, Error, LifecycleEffect, Phase, PhaseChange,
    ProjectId, Status, StatusId, SuccessfulCommand, WorkItem, WorkItemChange, WorkItemId,
    WorkItemType, WorkItemTypeId, Workflow, WorkflowId,
};

fn configuration() -> Configuration {
    Configuration {
        project_id: ProjectId("project".into()),
        revision: 4,
        project_archived: false,
        statuses: vec![
            Status {
                id: StatusId("backlog".into()),
                phase: Phase::New,
                archived: false,
            },
            Status {
                id: StatusId("ready".into()),
                phase: Phase::New,
                archived: false,
            },
            Status {
                id: StatusId("active".into()),
                phase: Phase::Active,
                archived: false,
            },
            Status {
                id: StatusId("done".into()),
                phase: Phase::Done,
                archived: false,
            },
            Status {
                id: StatusId("old".into()),
                phase: Phase::Active,
                archived: true,
            },
        ],
        workflows: vec![
            Workflow {
                id: WorkflowId("development".into()),
                status_ids: ["backlog", "ready", "active", "done", "old"]
                    .into_iter()
                    .map(|id| StatusId(id.into()))
                    .collect(),
                initial_status_id: StatusId("backlog".into()),
                permitted_phase_changes: vec![
                    PhaseChange::new(Phase::New, Phase::Active),
                    PhaseChange::new(Phase::New, Phase::Done),
                    PhaseChange::new(Phase::Active, Phase::New),
                    PhaseChange::new(Phase::Active, Phase::Done),
                    PhaseChange::new(Phase::Done, Phase::Active),
                ],
                archived: false,
            },
            Workflow {
                id: WorkflowId("alternate".into()),
                status_ids: vec![
                    StatusId("backlog".into()),
                    StatusId("active".into()),
                    StatusId("done".into()),
                ],
                initial_status_id: StatusId("backlog".into()),
                permitted_phase_changes: vec![PhaseChange::new(Phase::Done, Phase::Active)],
                archived: false,
            },
        ],
        types: vec![WorkItemType {
            id: WorkItemTypeId("task".into()),
            permitted_workflows: vec![
                WorkflowId("development".into()),
                WorkflowId("alternate".into()),
            ],
            archived: false,
        }],
    }
}

fn item(status: &str, version: u64) -> WorkItem {
    WorkItem {
        id: WorkItemId("item".into()),
        project_id: ProjectId("project".into()),
        type_id: WorkItemTypeId("task".into()),
        workflow_id: WorkflowId("development".into()),
        status_id: StatusId(status.into()),
        version,
        archived: false,
    }
}

fn command(action: Action) -> Command {
    Command {
        operation_id: "op".into(),
        expected_item_version: 2,
        expected_configuration_revision: 4,
        action,
    }
}

#[test]
fn same_phase_changes_derive_phase_without_lifecycle_effects() {
    let result = decide(
        &configuration(),
        &item("backlog", 2),
        None,
        true,
        &command(Action::ChangeStatus {
            target: StatusId("ready".into()),
        }),
    );
    assert_eq!(
        result,
        Decision::Apply(WorkItemChange {
            workflow_id: WorkflowId("development".into()),
            status_id: StatusId("ready".into()),
            phase: Phase::New,
            version: 3,
            effect: LifecycleEffect::None,
        })
    );
}

#[test]
fn phase_rules_apply_to_status_changes_and_migrations() {
    let config = configuration();
    let done = item("done", 2);
    assert_eq!(
        decide(
            &config,
            &done,
            None,
            true,
            &command(Action::ChangeStatus {
                target: StatusId("backlog".into())
            })
        ),
        Decision::Reject(Error::ProhibitedPhaseChange)
    );
    let reopened = decide(
        &config,
        &done,
        None,
        true,
        &command(Action::MigrateWorkflow {
            destination_workflow: WorkflowId("alternate".into()),
            destination_status: StatusId("active".into()),
        }),
    );
    assert_eq!(
        reopened,
        Decision::Apply(WorkItemChange {
            workflow_id: WorkflowId("alternate".into()),
            status_id: StatusId("active".into()),
            phase: Phase::Active,
            version: 3,
            effect: LifecycleEffect::Reopening,
        })
    );
}

#[test]
fn migration_requires_both_workflows_and_authorization() {
    let mut config = configuration();
    let done = item("done", 2);
    let migrate = command(Action::MigrateWorkflow {
        destination_workflow: WorkflowId("alternate".into()),
        destination_status: StatusId("active".into()),
    });
    assert_eq!(
        decide(&config, &done, None, false, &migrate),
        Decision::Reject(Error::Unauthorized)
    );
    config.workflows[0].permitted_phase_changes.clear();
    assert_eq!(
        decide(&config, &done, None, true, &migrate),
        Decision::Reject(Error::ProhibitedPhaseChange)
    );
    config.workflows[0]
        .permitted_phase_changes
        .push(PhaseChange::new(Phase::Done, Phase::Active));
    config.workflows[1].permitted_phase_changes.clear();
    assert_eq!(
        decide(&config, &done, None, true, &migrate),
        Decision::Reject(Error::ProhibitedPhaseChange)
    );
}

#[test]
fn successful_replay_precedes_mutable_configuration_and_version_checks() {
    let original = command(Action::ChangeStatus {
        target: StatusId("ready".into()),
    });
    let success = SuccessfulCommand {
        item_id: WorkItemId("item".into()),
        request: original.clone(),
        result: WorkItemChange {
            workflow_id: WorkflowId("development".into()),
            status_id: StatusId("ready".into()),
            phase: Phase::New,
            version: 3,
            effect: LifecycleEffect::None,
        },
    };
    let mut changed_config = configuration();
    changed_config.revision = 5;
    changed_config
        .statuses
        .retain(|status| status.id != StatusId("ready".into()));
    assert_eq!(
        decide(
            &changed_config,
            &item("done", 9),
            Some(&success),
            true,
            &original
        ),
        Decision::Replay(success.result.clone())
    );
    assert_eq!(
        decide(
            &changed_config,
            &item("done", 9),
            Some(&success),
            false,
            &original
        ),
        Decision::Reject(Error::Unauthorized)
    );
    let reused = Command {
        action: Action::ChangeStatus {
            target: StatusId("active".into()),
        },
        ..original.clone()
    };
    assert_eq!(
        decide(
            &changed_config,
            &item("done", 9),
            Some(&success),
            true,
            &reused
        ),
        Decision::Reject(Error::OperationIdReused)
    );
    assert_eq!(
        decide(&changed_config, &item("backlog", 2), None, true, &original),
        Decision::Reject(Error::ConfigurationConflict {
            current_revision: 5
        })
    );
    let wrong_item = SuccessfulCommand {
        item_id: WorkItemId("different".into()),
        ..success
    };
    assert_eq!(
        decide(
            &changed_config,
            &item("done", 9),
            Some(&wrong_item),
            true,
            &original
        ),
        Decision::Reject(Error::InvalidReference)
    );
}

#[test]
fn completion_and_archived_source_still_use_phase_semantics() {
    let config = configuration();
    let completion = decide(
        &config,
        &item("backlog", 2),
        None,
        true,
        &command(Action::ChangeStatus {
            target: StatusId("done".into()),
        }),
    );
    assert!(matches!(
        completion,
        Decision::Apply(WorkItemChange {
            phase: Phase::Done,
            effect: LifecycleEffect::Completion,
            ..
        })
    ));
    let from_archived = decide(
        &config,
        &item("old", 2),
        None,
        true,
        &command(Action::ChangeStatus {
            target: StatusId("done".into()),
        }),
    );
    assert!(matches!(
        from_archived,
        Decision::Apply(WorkItemChange {
            phase: Phase::Done,
            effect: LifecycleEffect::Completion,
            ..
        })
    ));
}

#[test]
fn returning_from_active_to_new_has_no_lifecycle_effect() {
    let result = decide(
        &configuration(),
        &item("active", 2),
        None,
        true,
        &command(Action::ChangeStatus {
            target: StatusId("ready".into()),
        }),
    );
    assert!(matches!(
        result,
        Decision::Apply(WorkItemChange {
            phase: Phase::New,
            effect: LifecycleEffect::None,
            ..
        })
    ));
}

#[test]
fn archived_workflow_allows_existing_items_to_progress_but_blocks_migration_into_it() {
    let mut config = configuration();
    config.workflows[0].archived = true;
    assert!(matches!(
        decide(
            &config,
            &item("backlog", 2),
            None,
            true,
            &command(Action::ChangeStatus {
                target: StatusId("active".into()),
            })
        ),
        Decision::Apply(_)
    ));
    let mut alternate_item = item("active", 2);
    alternate_item.workflow_id = WorkflowId("alternate".into());
    assert_eq!(
        decide(
            &config,
            &alternate_item,
            None,
            true,
            &command(Action::MigrateWorkflow {
                destination_workflow: WorkflowId("development".into()),
                destination_status: StatusId("ready".into()),
            })
        ),
        Decision::Reject(Error::ArchivedTarget)
    );
}

#[test]
fn item_version_cannot_overflow() {
    let mut current = item("backlog", u64::MAX);
    let mut request = command(Action::ChangeStatus {
        target: StatusId("ready".into()),
    });
    request.expected_item_version = current.version;
    assert_eq!(
        decide(&configuration(), &current, None, true, &request),
        Decision::Reject(Error::VersionOverflow)
    );
    current.archived = true;
    assert_eq!(
        decide(&configuration(), &current, None, true, &request),
        Decision::Reject(Error::ArchivedTarget)
    );
}

#[test]
fn references_archives_and_versions_are_checked() {
    let config = configuration();
    let current = item("backlog", 2);
    assert_eq!(
        decide(
            &config,
            &current,
            None,
            true,
            &command(Action::ChangeStatus {
                target: StatusId("old".into())
            })
        ),
        Decision::Reject(Error::ArchivedTarget)
    );
    assert_eq!(
        decide(
            &config,
            &current,
            None,
            true,
            &command(Action::ChangeStatus {
                target: StatusId("missing".into())
            })
        ),
        Decision::Reject(Error::InvalidReference)
    );
    let stale = Command {
        expected_item_version: 1,
        ..command(Action::ChangeStatus {
            target: StatusId("ready".into()),
        })
    };
    assert_eq!(
        decide(&config, &current, None, true, &stale),
        Decision::Reject(Error::ItemVersionConflict { current_version: 2 })
    );
    let mut other_project = current.clone();
    other_project.project_id = ProjectId("other".into());
    assert_eq!(
        decide(
            &config,
            &other_project,
            None,
            true,
            &command(Action::ChangeStatus {
                target: StatusId("ready".into())
            })
        ),
        Decision::Reject(Error::InvalidReference)
    );
}

#[test]
fn configuration_rejects_ambiguous_references_and_invalid_initial_phase() {
    let mut config = configuration();
    assert_eq!(config.validate(), Ok(()));
    config.statuses.push(config.statuses[0].clone());
    assert_eq!(config.validate(), Err(Error::InvalidConfiguration));
    assert_eq!(
        decide(
            &config,
            &item("backlog", 2),
            None,
            true,
            &command(Action::ChangeStatus {
                target: StatusId("ready".into())
            })
        ),
        Decision::Reject(Error::InvalidConfiguration)
    );
    config.statuses.pop();
    config.workflows[1].initial_status_id = StatusId("active".into());
    assert_eq!(config.validate(), Err(Error::InvalidConfiguration));
    config.workflows[1].initial_status_id = StatusId("backlog".into());
    config.workflows[0]
        .permitted_phase_changes
        .push(PhaseChange::new(Phase::Done, Phase::New));
    assert_eq!(config.validate(), Err(Error::InvalidConfiguration));
}

#[test]
fn archived_project_blocks_new_item_commands_but_preserves_recorded_replay() {
    let original = command(Action::ChangeStatus {
        target: StatusId("ready".into()),
    });
    let result = WorkItemChange {
        workflow_id: WorkflowId("development".into()),
        status_id: StatusId("ready".into()),
        phase: Phase::New,
        version: 3,
        effect: LifecycleEffect::None,
    };
    let recorded = SuccessfulCommand {
        item_id: WorkItemId("item".into()),
        request: original.clone(),
        result: result.clone(),
    };
    let mut config = configuration();
    config.project_archived = true;
    config.revision = 5;
    assert_eq!(
        decide(&config, &item("backlog", 2), None, true, &original),
        Decision::Reject(Error::ConfigurationConflict {
            current_revision: 5
        })
    );
    let fresh = Command {
        expected_configuration_revision: 5,
        ..original.clone()
    };
    assert_eq!(
        decide(&config, &item("backlog", 2), None, true, &fresh),
        Decision::Reject(Error::ArchivedProject)
    );
    assert_eq!(
        decide(
            &config,
            &item("backlog", 9),
            Some(&recorded),
            true,
            &original
        ),
        Decision::Replay(result)
    );
}
