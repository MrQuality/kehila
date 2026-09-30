use task_contract::project::{EstimateUnit, Project};
use task_contract::project_admin::{
    decide_project_metadata, ProjectMetadataCommand, ProjectMetadataDecision, ProjectMetadataError,
};
use task_contract::work_item::{Configuration, ProjectId};

fn current() -> (Project, Configuration) {
    let id = ProjectId("project".into());
    (
        Project {
            id: id.clone(),
            name: "Old".into(),
            prefix: "OLD".into(),
            configuration_revision: 3,
            next_sequence: 12,
            archived: false,
            estimate_unit: EstimateUnit::Hours,
            ever_estimated: false,
        },
        Configuration {
            project_id: id,
            revision: 3,
            project_archived: false,
            statuses: vec![],
            workflows: vec![],
            types: vec![],
            fields: vec![],
            relationship_types: vec![],
        },
    )
}

fn command() -> ProjectMetadataCommand {
    ProjectMetadataCommand {
        operation_id: "op".into(),
        expected_revision: 3,
        project_id: ProjectId("project".into()),
        name: "New".into(),
        prefix: "NEW".into(),
        estimate_unit: EstimateUnit::Points,
    }
}

#[test]
fn one_metadata_change_advances_project_and_configuration_together() {
    let (project, config) = current();
    let result = match decide_project_metadata(&project, &config, None, true, &command()) {
        ProjectMetadataDecision::Apply(result) => result,
        other => panic!("unexpected {other:?}"),
    };
    assert_eq!(result.project.configuration_revision, 4);
    assert_eq!(result.configuration.revision, 4);
    assert_eq!(result.project.next_sequence, 12);
    assert_eq!(result.project.estimate_unit, EstimateUnit::Points);
}

#[test]
fn unit_lock_and_invalid_metadata_are_rejected() {
    let (mut project, config) = current();
    project.ever_estimated = true;
    assert_eq!(
        decide_project_metadata(&project, &config, None, true, &command()),
        ProjectMetadataDecision::Reject(ProjectMetadataError::EstimateUnitLocked)
    );
    project.ever_estimated = false;
    let mut invalid = command();
    invalid.name = " ".into();
    assert_eq!(
        decide_project_metadata(&project, &config, None, true, &invalid),
        ProjectMetadataDecision::Reject(ProjectMetadataError::InvalidName)
    );
}

#[test]
fn replay_is_authorized_before_stale_revision_and_archival_checks() {
    let (project, config) = current();
    let cmd = command();
    let result = match decide_project_metadata(&project, &config, None, true, &cmd) {
        ProjectMetadataDecision::Apply(result) => result,
        other => panic!("unexpected {other:?}"),
    };
    let recorded = (cmd.clone(), result.clone());
    let mut archived = project.clone();
    archived.archived = true;
    archived.configuration_revision = 5;
    assert_eq!(
        decide_project_metadata(&archived, &config, Some(&recorded), true, &cmd),
        ProjectMetadataDecision::Replay(result)
    );
    assert_eq!(
        decide_project_metadata(&archived, &config, Some(&recorded), false, &cmd),
        ProjectMetadataDecision::Reject(ProjectMetadataError::Unauthorized)
    );
    let mut changed = cmd.clone();
    changed.name = "Different".into();
    assert_eq!(
        decide_project_metadata(&archived, &config, Some(&recorded), true, &changed),
        ProjectMetadataDecision::Reject(ProjectMetadataError::OperationIdReused)
    );
}
