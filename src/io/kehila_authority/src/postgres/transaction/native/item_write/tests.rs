//! Sealed unseen-command decisions only; no route or replay delivery proof.
use super::*;
use task_contract::{
    current_work::UserId,
    field::{FieldEntry, FieldValue},
    item_mutation::{CreateCommand, EditCommand, EstimateEdit},
    project::Estimate,
    work_item::WorkItemId,
};
fn fixture() -> (Observed, CreateCommand) {
    let seed = crate::project::Prepared::new(
        crate::native_ingress::Form {
            name: "Project".into(),
            prefix: "PR".into(),
            estimate_unit: task_contract::project::EstimateUnit::Hours,
        },
        "project".into(),
        "0".repeat(32),
    )
    .unwrap();
    let result = seed.result();
    let project = result.project.clone();
    let configuration = result.configuration.clone();
    let command = CreateCommand {
        operation_id: "1".repeat(32),
        expected_configuration_revision: 1,
        item_id: WorkItemId("item".into()),
        type_id: configuration.types[0].id.clone(),
        workflow_id: None,
        estimate: None,
        values: vec![],
    };
    (
        Observed {
            scope: Scope {
                installation: "installation".into(),
                actor: "owner".into(),
            },
            configuration_project: project.clone(),
            project,
            configuration,
            configuration_operation: 1,
            state_version: 0,
            state_operation: 1,
            existing: None,
        },
        command,
    )
}
fn prior_item(revision: Revision) -> Previous {
    let SavedResult::Create(created) = &revision.result else {
        panic!("fixture must start from creation")
    };
    Previous {
        operation: 2,
        readable: created.readable_id.clone(),
        revision,
    }
}
fn canonical(observed: &Observed, request: Request) -> std::sync::Arc<Canonical> {
    std::sync::Arc::new(Canonical::new(observed.scope.clone(), request).unwrap())
}
#[test]
fn both_configured_types_preserve_exact_command_and_issue_only_prepared_result() {
    for index in 0..2 {
        let (observed, mut command) = fixture();
        let kind = &observed.configuration.types[index];
        command.type_id = kind.id.clone();
        command.estimate = Some(Estimate::try_from_thousandths(0).unwrap());
        command.values = vec![FieldEntry {
            id: kind.title_field_id.clone().unwrap(),
            value: FieldValue::Text("  exact title  ".into()),
        }];
        let expected_type = kind.id.clone();
        let expected_workflow = kind.default_workflow_id.clone();
        let expected_status = observed
            .configuration
            .workflows
            .iter()
            .find(|workflow| workflow.id == expected_workflow)
            .unwrap()
            .initial_status_id
            .clone();
        let request = canonical(
            &observed,
            Request::Create {
                project_id: observed.project.id.clone(),
                command,
            },
        );
        let bytes = request.bytes().to_vec();
        let accepted = Accepted::new(request, observed).unwrap();
        let SavedResult::Create(result) = &accepted.revision().result else {
            panic!("wrong family")
        };
        assert_eq!(result.item.type_id, expected_type);
        assert_eq!(result.item.workflow_id, expected_workflow);
        assert_eq!(result.item.status_id, expected_status);
        assert_eq!(result.item.version, 1);
        assert_eq!(result.readable_id.sequence, 1);
        assert_eq!(result.phase, task_contract::work_item::Phase::New);
        assert!(result.project.ever_estimated);
        assert_eq!(result.data.estimate.unwrap().thousandths(), 0);
        assert_eq!(
            result.data.values[0].value,
            FieldValue::Text("  exact title  ".into())
        );
        assert_eq!(accepted.request_bytes(), bytes);
        let revision = work_item_codec::decode_revision(1, accepted.history_bytes()).unwrap();
        assert_eq!(revision, *accepted.revision());
        assert_eq!(revision.actor_id, UserId("owner".into()));
        assert_eq!(revision.project_state_version, 1);
        assert_eq!(revision.previous_project_operation, 1);
    }
}
#[test]
fn actor_and_installation_cannot_be_rebound_between_command_and_snapshot() {
    for actor in [false, true] {
        let (mut observed, command) = fixture();
        let request = canonical(
            &observed,
            Request::Create {
                project_id: observed.project.id.clone(),
                command,
            },
        );
        if actor {
            observed.scope.actor = "other".into()
        } else {
            observed.scope.installation = "other".into()
        };
        assert!(matches!(
            Accepted::new(request, observed),
            Err(PreparationError::InvalidProof)
        ));
    }
}
#[test]
fn request_project_must_match_observed_project() {
    let (observed, command) = fixture();
    let request = canonical(
        &observed,
        Request::Create {
            project_id: task_contract::work_item::ProjectId("other".into()),
            command,
        },
    );
    assert!(matches!(
        Accepted::new(request, observed),
        Err(PreparationError::InvalidProof)
    ));
}
#[test]
fn stale_configuration_is_domain_conflict_without_a_save_capability() {
    let (observed, mut command) = fixture();
    command.expected_configuration_revision = 2;
    let request = canonical(
        &observed,
        Request::Create {
            project_id: observed.project.id.clone(),
            command,
        },
    );
    assert!(matches!(
        Accepted::new(request, observed),
        Err(PreparationError::Domain(
            task_contract::item_mutation::ItemMutationError::ConfigurationConflict { .. }
        ))
    ));
}
#[test]
fn invalid_native_text_cannot_become_an_admitted_command() {
    let (observed, mut command) = fixture();
    command.values = vec![FieldEntry {
        id: observed.configuration.types[0]
            .title_field_id
            .clone()
            .unwrap(),
        value: FieldValue::Text("contains\0nul".into()),
    }];
    assert!(matches!(
        Canonical::new(
            observed.scope,
            Request::Create {
                project_id: observed.project.id,
                command
            }
        ),
        Err(PreparationError::Representation(_))
    ));
}
#[test]
fn operational_state_overflow_does_not_wrap_or_mint_a_packet() {
    let (mut observed, command) = fixture();
    observed.state_version = u64::MAX;
    let request = canonical(
        &observed,
        Request::Create {
            project_id: observed.project.id.clone(),
            command,
        },
    );
    assert!(matches!(
        Accepted::new(request, observed),
        Err(PreparationError::StateExhausted)
    ));
}
#[test]
fn clearing_zero_estimate_preserves_monotonic_project_history_and_item_lifecycle() {
    let (observed, mut command) = fixture();
    command.estimate = Some(Estimate::try_from_thousandths(0).unwrap());
    let request = canonical(
        &observed,
        Request::Create {
            project_id: observed.project.id.clone(),
            command,
        },
    );
    let created = Accepted::new(request, observed).unwrap();
    let previous = created.revision().clone();
    let SavedResult::Create(result) = &previous.result else {
        panic!()
    };
    let current = result.item.clone();
    let current_phase = result.phase;
    let current_sequence = result.project.next_sequence;
    let (mut observed, _) = fixture();
    observed.project = result.project.clone();
    observed.state_version = 1;
    observed.state_operation = 2;
    observed.existing = Some(prior_item(previous));
    let command = EditCommand {
        operation_id: "2".repeat(32),
        expected_item_version: 1,
        expected_configuration_revision: 1,
        estimate: EstimateEdit::Clear,
        fields: vec![],
    };
    let request = canonical(
        &observed,
        Request::Edit {
            project_id: observed.project.id.clone(),
            item_id: current.id.clone(),
            command,
        },
    );
    let edited = Accepted::new(request, observed).unwrap();
    let SavedResult::Edit(result) = &edited.revision().result else {
        panic!()
    };
    assert_eq!(result.item.version, 2);
    assert_eq!(result.item.type_id, current.type_id);
    assert_eq!(result.item.workflow_id, current.workflow_id);
    assert_eq!(result.item.status_id, current.status_id);
    assert_eq!(result.item.archived, current.archived);
    assert_eq!(result.phase, current_phase);
    assert_eq!(result.project.next_sequence, current_sequence);
    assert_eq!(result.data.estimate, None);
    assert!(result.project.ever_estimated);
    assert_eq!(edited.revision().project_state_version, 2);
    assert_eq!(edited.revision().previous_project_operation, 2);
}

pub(super) fn edit_fixture() -> (Observed, EditCommand) {
    let (observed, command) = fixture();
    let request = canonical(
        &observed,
        Request::Create {
            project_id: observed.project.id.clone(),
            command,
        },
    );
    let created = Accepted::new(request, observed).unwrap();
    let previous = created.revision().clone();
    let project = project_from(&previous.result).clone();
    let (mut observed, _) = fixture();
    observed.project = project;
    observed.state_version = 1;
    observed.state_operation = 2;
    observed.existing = Some(prior_item(previous));
    (
        observed,
        EditCommand {
            operation_id: "3".repeat(32),
            expected_item_version: 1,
            expected_configuration_revision: 1,
            estimate: EstimateEdit::Keep,
            fields: vec![],
        },
    )
}
#[test]
fn edit_target_must_be_the_observed_item() {
    let (observed, command) = edit_fixture();
    let request = canonical(
        &observed,
        Request::Edit {
            project_id: observed.project.id.clone(),
            item_id: WorkItemId("wrong-item".into()),
            command,
        },
    );
    assert!(matches!(
        Accepted::new(request, observed),
        Err(PreparationError::InvalidProof)
    ));
}
#[test]
fn stale_item_version_is_a_domain_conflict() {
    let (observed, mut command) = edit_fixture();
    command.expected_item_version = 2;
    let request = canonical(
        &observed,
        Request::Edit {
            project_id: observed.project.id.clone(),
            item_id: WorkItemId("item".into()),
            command,
        },
    );
    assert!(matches!(
        Accepted::new(request, observed),
        Err(PreparationError::Domain(
            task_contract::item_mutation::ItemMutationError::ItemVersionConflict {
                current_version: 1
            }
        ))
    ));
}
#[test]
fn create_and_edit_snapshot_shapes_cannot_be_interchanged() {
    let (_, create) = fixture();
    let (observed, _) = edit_fixture();
    let request = canonical(
        &observed,
        Request::Create {
            project_id: observed.project.id.clone(),
            command: create,
        },
    );
    assert!(matches!(
        Accepted::new(request, observed),
        Err(PreparationError::InvalidProof)
    ));
    let (mut observed, edit) = edit_fixture();
    observed.existing = None;
    let request = canonical(
        &observed,
        Request::Edit {
            project_id: observed.project.id.clone(),
            item_id: WorkItemId("item".into()),
            command: edit,
        },
    );
    assert!(matches!(
        Accepted::new(request, observed),
        Err(PreparationError::InvalidProof)
    ));
}

#[test]
fn native_result_is_retained_before_the_write_boundary() {
    let (observed, command) = fixture();
    let request = canonical(
        &observed,
        Request::Create {
            project_id: observed.project.id.clone(),
            command,
        },
    );
    let created = Accepted::new(request, observed).unwrap();
    assert_eq!(
        work_item_codec::decode_result(1, created.result_bytes()).unwrap(),
        created.revision().result
    );
    let (observed, command) = edit_fixture();
    let request = canonical(
        &observed,
        Request::Edit {
            project_id: observed.project.id.clone(),
            item_id: WorkItemId("item".into()),
            command,
        },
    );
    let edited = Accepted::new(request, observed).unwrap();
    assert_eq!(
        work_item_codec::decode_result(1, edited.result_bytes()).unwrap(),
        edited.revision().result
    );
    assert!(edited.retained_record_bytes().unwrap() <= 7 * project_create_codec::MAX_RECORD_BYTES);
}
#[test]
fn immutable_readable_identity_must_agree_with_the_verified_prior_creation() {
    for case in 0..3 {
        let (mut observed, command) = edit_fixture();
        let previous = observed.existing.as_mut().unwrap();
        match case {
            0 => previous.readable.sequence = 0,
            1 => previous.readable.sequence = observed.project.next_sequence,
            _ => previous.readable.prefix = "ZZ".into(),
        };
        let request = canonical(
            &observed,
            Request::Edit {
                project_id: observed.project.id.clone(),
                item_id: WorkItemId("item".into()),
                command,
            },
        );
        assert!(matches!(
            Accepted::new(request, observed),
            Err(PreparationError::InvalidProof)
        ));
    }
}
