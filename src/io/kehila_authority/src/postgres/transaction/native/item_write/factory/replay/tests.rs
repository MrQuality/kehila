//! Pure historical identity controls; real SQL/clock/retirement checks are separate.
use super::*;
use task_contract::{
    field::{FieldId, FieldValue},
    work_item::WorkItemId,
};

fn fixture(edit: bool) -> (Arc<Canonical>, Metadata, Stored, ReadableId, i64) {
    let admitted = if edit {
        super::super::tests::edit_draft()
    } else {
        super::super::tests::draft()
    }
    .admit()
    .unwrap();
    let packet = admitted.packet;
    let readable = match &packet.revision.result {
        SavedResult::Create(value) => value.readable_id.clone(),
        SavedResult::Edit(_) => packet.observed.existing.as_ref().unwrap().readable.clone(),
    };
    let operation = if edit { 3 } else { 2 };
    let (_, target, token, _) = super::super::save::command_parts(&packet.command.request);
    let data = Metadata {
        operation: Some(operation),
        target: Some(target.into()),
        codec: Some(1),
        token: Some(token.into()),
        pending: if edit { None } else { Some(17) },
        pending_expires: if edit { None } else { Some(86400000) },
        origin: Some(100),
        end: Some(100 + task_contract::operation::REPLAY_PERIOD_MS),
    };
    (
        packet.command,
        data,
        Stored {
            operation,
            revision: packet.revision,
        },
        readable,
        2,
    )
}
fn version(result: &SavedResult) -> u64 {
    match result {
        SavedResult::Create(value) => value.item.version,
        SavedResult::Edit(value) => value.item.version,
    }
}
#[test]
fn historical_create_and_edit_validate_without_any_current_domain_input() {
    for edit in [false, true] {
        let (command, data, stored, readable, creation) = fixture(edit);
        let saved = stored.revision.result.clone();
        assert!(verify_history(
            &command,
            &data,
            &stored,
            &saved,
            version(&saved),
            &readable,
            creation
        )
        .is_ok());
        // No mutable Project/Configuration/current item head is accepted by the
        // verifier. Their later changes cannot participate in this decision.
    }
}
#[test]
fn historical_payload_cannot_be_swapped_across_attributed_operations() {
    for field in 0..6 {
        let (command, mut data, mut stored, mut readable, creation) = fixture(false);
        let saved = stored.revision.result.clone();
        match field {
            0 => data.operation = Some(stored.operation + 1),
            1 => data.target = Some("other".into()),
            2 => data.token = Some("other".into()),
            3 => stored.revision.actor_id.0 = "other".into(),
            4 => readable.item_id = WorkItemId("other".into()),
            _ => readable.sequence += 1,
        }
        assert!(verify_history(
            &command,
            &data,
            &stored,
            &saved,
            version(&saved),
            &readable,
            creation
        )
        .is_err());
    }
}
#[test]
fn result_must_equal_the_complete_permanent_revision() {
    let (command, data, stored, readable, creation) = fixture(true);
    let mut saved = stored.revision.result.clone();
    let SavedResult::Edit(value) = &mut saved else {
        panic!()
    };
    value.project.name.push_str(" changed payload");
    assert!(verify_history(
        &command,
        &data,
        &stored,
        &saved,
        version(&saved),
        &readable,
        creation
    )
    .is_err());
}
#[test]
fn historical_creation_must_match_request_and_immutable_readable_identity() {
    for field in 0..5 {
        let (command, data, mut stored, readable, mut creation) = fixture(false);
        let SavedResult::Create(value) = &mut stored.revision.result else {
            panic!()
        };
        match field {
            0 => value.item.type_id.0 = "other".into(),
            1 => stored.revision.configuration_revision += 1,
            2 => {
                value.data.estimate =
                    Some(task_contract::project::Estimate::try_from_thousandths(0).unwrap())
            }
            3 => value.item.version += 1,
            _ => creation += 1,
        }
        let saved = stored.revision.result.clone();
        assert!(verify_history(
            &command,
            &data,
            &stored,
            &saved,
            version(&saved),
            &readable,
            creation
        )
        .is_err());
    }
}
#[test]
fn edit_requires_original_expected_version_and_configuration() {
    for configuration in [false, true] {
        let (command, data, mut stored, readable, creation) = fixture(true);
        if configuration {
            stored.revision.configuration_revision += 1
        } else {
            let SavedResult::Edit(value) = &mut stored.revision.result else {
                panic!()
            };
            value.item.version += 1;
        }
        let saved = stored.revision.result.clone();
        assert!(verify_history(
            &command,
            &data,
            &stored,
            &saved,
            version(&saved),
            &readable,
            creation
        )
        .is_err());
    }
}
#[test]
fn initial_fields_compare_by_identity_while_preserving_exact_values() {
    let a = FieldEntry {
        id: FieldId("a".into()),
        value: FieldValue::Text("  exact\r\n?  ".into()),
    };
    let b = FieldEntry {
        id: FieldId("b".into()),
        value: FieldValue::Text("second".into()),
    };
    assert!(same_values(
        &[a.clone(), b.clone()],
        &[b.clone(), a.clone()]
    ));
    let mut changed = a.clone();
    changed.value = FieldValue::Text("exact".into());
    assert!(!same_values(&[a.clone(), b.clone()], &[changed, b.clone()]));
    assert!(!same_values(&[a.clone(), b], &[a.clone(), a]));
}
#[test]
fn admission_time_check_does_not_renew_deadline_or_runtime() {
    let live = RuntimeLiveness::new();
    assert!(matches!(
        check_time(&live, Instant::now()),
        Err(PreparationError::Expired)
    ));
    let future = Instant::now() + std::time::Duration::from_secs(5);
    live.stop();
    assert!(matches!(
        check_time(&live, future),
        Err(PreparationError::Unavailable)
    ));
}
