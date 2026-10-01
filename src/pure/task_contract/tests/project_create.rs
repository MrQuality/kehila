use task_contract::error_code::{Code, StableCode};
use task_contract::field::{FieldKind, FieldOrigin, FieldUsage};
use task_contract::project::EstimateUnit;
use task_contract::project_create::{
    decide_project_create, ProjectCreateCommand, ProjectCreateDecision, ProjectCreateError,
    SuccessfulProjectCreate,
};
use task_contract::work_item::{Phase, ProjectId};

fn command() -> ProjectCreateCommand {
    ProjectCreateCommand {
        operation_id: "create-project".into(),
        project_id: ProjectId("project".into()),
        name: "Example".into(),
        prefix: "EX".into(),
        estimate_unit: EstimateUnit::Points,
    }
}

#[test]
fn create_seeds_a_valid_minimal_project_with_protected_optional_fields() {
    let ProjectCreateDecision::Apply(result) = decide_project_create(false, None, true, &command())
    else {
        panic!("valid creation must apply")
    };
    assert_eq!(result.project.configuration_revision, 1);
    assert_eq!(result.configuration.revision, 1);
    assert_eq!(result.project.next_sequence, 1);
    assert!(!result.project.archived);
    assert!(!result.project.ever_estimated);
    assert_eq!(result.project.estimate_unit, EstimateUnit::Points);
    assert_eq!(result.configuration.project_id, result.project.id);
    assert_eq!(result.configuration.validate(), Ok(()));
    assert_eq!(
        result
            .configuration
            .types
            .iter()
            .map(|kind| kind.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["task", "milestone"]
    );
    assert_eq!(result.configuration.workflows.len(), 1);
    assert_eq!(
        result
            .configuration
            .statuses
            .iter()
            .map(|status| status.phase)
            .collect::<Vec<_>>(),
        vec![Phase::New, Phase::Active, Phase::Done]
    );
    assert_eq!(
        result.configuration.workflows[0]
            .permitted_phase_changes
            .len(),
        5
    );
    assert_eq!(result.configuration.fields.len(), 4);
    for kind in &result.configuration.types {
        let fields: Vec<_> = result
            .configuration
            .fields
            .iter()
            .filter(|field| field.owner_type == kind.id)
            .collect();
        assert_eq!(fields.len(), 2);
        assert_eq!(kind.title_field_id.as_ref(), Some(&fields[0].id));
        assert_eq!(
            kind.default_workflow_id,
            result.configuration.workflows[0].id
        );
        for field in fields {
            assert_eq!(field.origin, FieldOrigin::Application);
            assert_eq!(field.kind, FieldKind::Text);
            assert_eq!(field.usage, FieldUsage::Optional);
        }
    }
}

#[test]
fn creation_replays_recorded_seed_before_identity_uniqueness_but_after_authorization() {
    let request = command();
    let ProjectCreateDecision::Apply(result) = decide_project_create(false, None, true, &request)
    else {
        panic!("valid creation")
    };
    let recorded = SuccessfulProjectCreate {
        request: request.clone(),
        result: result.clone(),
    };
    assert_eq!(
        decide_project_create(true, Some(&recorded), true, &request),
        ProjectCreateDecision::Replay(result)
    );
    assert_eq!(
        decide_project_create(true, Some(&recorded), false, &request),
        ProjectCreateDecision::Reject(ProjectCreateError::Unauthorized)
    );
    let mut changed = request.clone();
    changed.name = "Other".into();
    assert_eq!(
        decide_project_create(true, Some(&recorded), true, &changed),
        ProjectCreateDecision::Reject(ProjectCreateError::OperationIdReused)
    );
    changed.project_id = ProjectId("different".into());
    assert_eq!(
        decide_project_create(false, Some(&recorded), true, &changed),
        ProjectCreateDecision::Reject(ProjectCreateError::InvalidReference)
    );
    assert_eq!(
        decide_project_create(true, None, true, &request),
        ProjectCreateDecision::Reject(ProjectCreateError::ProjectAlreadyExists)
    );
    assert_eq!(
        ProjectCreateError::ProjectAlreadyExists.code(),
        Code::ProjectAlreadyExists
    );
}

#[test]
fn creation_rejects_invalid_identity_operation_name_and_prefix() {
    for (mut request, error) in [
        (command(), ProjectCreateError::InvalidReference),
        (command(), ProjectCreateError::InvalidOperation),
        (command(), ProjectCreateError::InvalidName),
        (command(), ProjectCreateError::InvalidPrefix),
    ] {
        match error {
            ProjectCreateError::InvalidReference => request.project_id.0.clear(),
            ProjectCreateError::InvalidOperation => request.operation_id.clear(),
            ProjectCreateError::InvalidName => request.name = " \n".into(),
            ProjectCreateError::InvalidPrefix => request.prefix = "bad".into(),
            _ => unreachable!(),
        }
        assert_eq!(
            decide_project_create(false, None, true, &request),
            ProjectCreateDecision::Reject(error)
        );
    }
    for name in ["a".repeat(257), "Project\0name".into()] {
        let request = ProjectCreateCommand { name, ..command() };
        assert_eq!(
            decide_project_create(false, None, true, &request),
            ProjectCreateDecision::Reject(ProjectCreateError::InvalidName)
        );
    }
}

#[test]
fn creation_applies_shared_operation_token_bounds() {
    let mut request = command();
    request.operation_id = "x".repeat(128);
    assert!(matches!(
        decide_project_create(false, None, true, &request),
        ProjectCreateDecision::Apply(_)
    ));
    for token in ["x".repeat(129), "a b".into(), "é".into()] {
        request.operation_id = token;
        assert_eq!(
            decide_project_create(false, None, true, &request),
            ProjectCreateDecision::Reject(ProjectCreateError::InvalidOperation)
        );
    }
}
