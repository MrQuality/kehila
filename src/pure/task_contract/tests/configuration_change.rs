use task_contract::configuration_change::{validate_change, ConfigurationChangeError};
use task_contract::work_item::{
    Configuration, Phase, ProjectId, Status, StatusId, WorkItem, WorkItemId, WorkItemType,
    WorkItemTypeId, Workflow, WorkflowId,
};

fn config() -> Configuration {
    Configuration {
        project_id: ProjectId("project".into()),
        revision: 3,
        project_archived: false,
        statuses: vec![
            Status {
                id: StatusId("new".into()),
                phase: Phase::New,
                archived: false,
            },
            Status {
                id: StatusId("ready".into()),
                phase: Phase::New,
                archived: false,
            },
            Status {
                id: StatusId("done".into()),
                phase: Phase::Done,
                archived: false,
            },
        ],
        workflows: vec![Workflow {
            id: WorkflowId("flow".into()),
            status_ids: vec![
                StatusId("new".into()),
                StatusId("ready".into()),
                StatusId("done".into()),
            ],
            initial_status_id: StatusId("new".into()),
            permitted_phase_changes: vec![],
            archived: false,
        }],
        types: vec![WorkItemType {
            id: WorkItemTypeId("task".into()),
            permitted_workflows: vec![WorkflowId("flow".into())],
            default_workflow_id: WorkflowId("flow".into()),
            archived: false,
        }],
    }
}

fn item(status: &str) -> WorkItem {
    WorkItem {
        id: WorkItemId("item".into()),
        project_id: ProjectId("project".into()),
        type_id: WorkItemTypeId("task".into()),
        workflow_id: WorkflowId("flow".into()),
        status_id: StatusId(status.into()),
        version: 1,
        archived: false,
    }
}

fn next(previous: &Configuration) -> Configuration {
    let mut proposed = previous.clone();
    proposed.revision += 1;
    proposed
}

#[test]
fn removing_membership_or_type_permission_requires_prior_item_migration() {
    let previous = config();
    let mut proposed = next(&previous);
    proposed.workflows[0]
        .status_ids
        .retain(|id| id.0 != "ready");
    assert_eq!(
        validate_change(&previous, &proposed, 3, &[item("ready")], true),
        Err(ConfigurationChangeError::MigrationRequired {
            item_id: WorkItemId("item".into())
        })
    );
    assert_eq!(
        validate_change(&previous, &proposed, 3, &[item("new")], true),
        Ok(())
    );

    let mut proposed = next(&previous);
    let mut alternative = proposed.workflows[0].clone();
    alternative.id = WorkflowId("other".into());
    proposed.workflows.push(alternative);
    proposed.types[0].permitted_workflows = vec![WorkflowId("other".into())];
    proposed.types[0].default_workflow_id = WorkflowId("other".into());
    assert_eq!(
        validate_change(&previous, &proposed, 3, &[item("new")], true),
        Err(ConfigurationChangeError::MigrationRequired {
            item_id: WorkItemId("item".into())
        })
    );
}

#[test]
fn archiving_initial_status_or_default_workflow_requires_replacement() {
    let previous = config();
    let mut proposed = next(&previous);
    proposed.statuses[0].archived = true;
    assert_eq!(
        validate_change(&previous, &proposed, 3, &[], true),
        Err(ConfigurationChangeError::InvalidConfiguration)
    );
    proposed.workflows[0].initial_status_id = StatusId("ready".into());
    assert_eq!(validate_change(&previous, &proposed, 3, &[], true), Ok(()));

    let mut proposed = next(&previous);
    proposed.workflows[0].archived = true;
    assert_eq!(
        validate_change(&previous, &proposed, 3, &[], true),
        Err(ConfigurationChangeError::InvalidConfiguration)
    );
    proposed.types[0].archived = true;
    assert_eq!(validate_change(&previous, &proposed, 3, &[], true), Ok(()));
}

#[test]
fn an_in_use_status_cannot_be_reinterpreted_as_a_different_phase() {
    let previous = config();
    let mut proposed = next(&previous);
    proposed.statuses[2].phase = Phase::New;
    assert_eq!(
        validate_change(&previous, &proposed, 3, &[item("done")], true),
        Err(ConfigurationChangeError::MigrationRequired {
            item_id: WorkItemId("item".into())
        })
    );
}

#[test]
fn configuration_edits_require_authorization_revision_and_active_project() {
    let previous = config();
    let proposed = next(&previous);
    assert_eq!(
        validate_change(&previous, &proposed, 3, &[], false),
        Err(ConfigurationChangeError::Unauthorized)
    );
    assert_eq!(
        validate_change(&previous, &proposed, 2, &[], true),
        Err(ConfigurationChangeError::RevisionConflict {
            current_revision: 3
        })
    );
    let mut archived = previous.clone();
    archived.project_archived = true;
    assert_eq!(
        validate_change(&archived, &proposed, 3, &[], true),
        Err(ConfigurationChangeError::ArchivedProject)
    );
}
