use task_contract::field::{
    apply_edit, change_usage, ChoiceOption, CivilDate, FieldDefinition, FieldEdit, FieldError,
    FieldId, FieldUsage, FieldValue, OptionId,
};
use task_contract::project::FieldKind;
use task_contract::work_item::WorkItemTypeId;

fn text_field(usage: FieldUsage) -> FieldDefinition {
    FieldDefinition {
        id: FieldId("summary".into()),
        owner_type: WorkItemTypeId("task".into()),
        name: "Summary".into(),
        kind: FieldKind::Text,
        usage,
        archived: false,
        options: vec![],
    }
}

#[test]
fn hidden_field_retains_its_value_and_rejects_ordinary_writes() {
    let field = text_field(FieldUsage::Hidden);
    let old = Some(FieldValue::Text("retained".into()));
    assert_eq!(
        apply_edit(
            &field,
            &field.owner_type,
            old.as_ref(),
            FieldEdit::Set(FieldValue::Text("new".into()))
        ),
        Err(FieldError::Hidden)
    );
    assert_eq!(
        apply_edit(&field, &field.owner_type, old.as_ref(), FieldEdit::Clear),
        Err(FieldError::Hidden)
    );
    assert_eq!(
        apply_edit(&field, &field.owner_type, old.as_ref(), FieldEdit::Keep),
        Ok(old.clone())
    );
    assert_eq!(old, Some(FieldValue::Text("retained".into())));
}

#[test]
fn clearing_is_absence_and_required_text_rejects_whitespace() {
    let optional = text_field(FieldUsage::Optional);
    assert_eq!(
        apply_edit(
            &optional,
            &optional.owner_type,
            Some(&FieldValue::Text("old".into())),
            FieldEdit::Clear
        ),
        Ok(None)
    );
    let required = text_field(FieldUsage::Required);
    assert_eq!(
        apply_edit(&required, &required.owner_type, None, FieldEdit::Clear),
        Err(FieldError::Required)
    );
    assert_eq!(
        apply_edit(
            &required,
            &required.owner_type,
            None,
            FieldEdit::Set(FieldValue::Text(" \t ".into()))
        ),
        Err(FieldError::Required)
    );
    assert_eq!(
        apply_edit(
            &required,
            &required.owner_type,
            None,
            FieldEdit::Set(FieldValue::Text("Ready".into()))
        ),
        Ok(Some(FieldValue::Text("Ready".into())))
    );
    assert_eq!(
        apply_edit(&required, &required.owner_type, None, FieldEdit::Keep),
        Err(FieldError::Required)
    );
}

#[test]
fn required_change_checks_every_applicable_item() {
    let optional = text_field(FieldUsage::Optional);
    let valid = Some(FieldValue::Text("present".into()));
    assert_eq!(
        change_usage(&optional, FieldUsage::Required, &[valid.clone(), None]),
        Err(FieldError::ExistingValueMissing)
    );
    assert_eq!(
        change_usage(
            &optional,
            FieldUsage::Required,
            &[Some(FieldValue::Text("  ".into()))]
        ),
        Err(FieldError::ExistingValueMissing)
    );
    assert_eq!(
        change_usage(&optional, FieldUsage::Required, &[valid]),
        Ok(())
    );
    assert_eq!(change_usage(&optional, FieldUsage::Hidden, &[None]), Ok(()));
}

#[test]
fn typed_values_and_owner_are_checked() {
    let field = text_field(FieldUsage::Optional);
    assert_eq!(
        apply_edit(
            &field,
            &WorkItemTypeId("milestone".into()),
            None,
            FieldEdit::Set(FieldValue::Text("x".into()))
        ),
        Err(FieldError::WrongType)
    );
    assert_eq!(
        apply_edit(
            &field,
            &field.owner_type,
            None,
            FieldEdit::Set(FieldValue::Boolean(true))
        ),
        Err(FieldError::WrongValueKind)
    );
    let mut number = field;
    number.kind = FieldKind::Number;
    assert_eq!(
        apply_edit(
            &number,
            &number.owner_type,
            None,
            FieldEdit::Set(FieldValue::Number(-250))
        ),
        Ok(Some(FieldValue::Number(-250)))
    );
    assert_eq!(
        apply_edit(
            &number,
            &number.owner_type,
            None,
            FieldEdit::Set(FieldValue::Number(1_000_000_000))
        ),
        Err(FieldError::InvalidValue)
    );
}

#[test]
fn choice_identity_and_archival_govern_new_assignments() {
    let mut field = text_field(FieldUsage::Optional);
    field.kind = FieldKind::SingleChoice;
    field.options = vec![
        ChoiceOption {
            id: OptionId("open".into()),
            archived: false,
        },
        ChoiceOption {
            id: OptionId("old".into()),
            archived: true,
        },
    ];
    assert_eq!(
        apply_edit(
            &field,
            &field.owner_type,
            None,
            FieldEdit::Set(FieldValue::SingleChoice(OptionId("open".into())))
        ),
        Ok(Some(FieldValue::SingleChoice(OptionId("open".into()))))
    );
    assert_eq!(
        apply_edit(
            &field,
            &field.owner_type,
            None,
            FieldEdit::Set(FieldValue::SingleChoice(OptionId("old".into())))
        ),
        Err(FieldError::ArchivedOption)
    );
    assert_eq!(
        apply_edit(
            &field,
            &field.owner_type,
            None,
            FieldEdit::Set(FieldValue::SingleChoice(OptionId("unknown".into())))
        ),
        Err(FieldError::UnknownOption)
    );
}

#[test]
fn dates_validate_calendar_boundaries() {
    assert_eq!(CivilDate::new(2024, 2, 29).unwrap().day, 29);
    assert_eq!(CivilDate::new(2023, 2, 29), Err(FieldError::InvalidDate));
    assert_eq!(CivilDate::new(2026, 13, 1), Err(FieldError::InvalidDate));
}

#[test]
fn same_display_name_on_distinct_types_does_not_share_field_identity() {
    let task = text_field(FieldUsage::Optional);
    let mut milestone = task.clone();
    milestone.id = FieldId("milestone-summary".into());
    milestone.owner_type = WorkItemTypeId("milestone".into());
    assert_eq!(task.name, milestone.name);
    assert_ne!(task.id, milestone.id);
    assert_eq!(
        apply_edit(
            &milestone,
            &task.owner_type,
            None,
            FieldEdit::Set(FieldValue::Text("x".into()))
        ),
        Err(FieldError::WrongType)
    );
}
