//! Typed value and usage rules for project-defined WorkItem fields.

use std::collections::HashSet;

use crate::work_item::WorkItemTypeId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldKind {
    Text,
    Number,
    Boolean,
    Date,
    SingleChoice,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FieldId(pub String);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct OptionId(pub String);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldUsage {
    Hidden,
    Optional,
    Required,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoiceOption {
    pub id: OptionId,
    pub archived: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldDefinition {
    pub id: FieldId,
    pub owner_type: WorkItemTypeId,
    pub name: String,
    pub kind: FieldKind,
    pub usage: FieldUsage,
    pub archived: bool,
    pub options: Vec<ChoiceOption>,
}

impl FieldDefinition {
    pub fn validate(&self) -> Result<(), FieldError> {
        if self.id.0.is_empty()
            || self.owner_type.0.is_empty()
            || self.name.trim().is_empty()
            || self.name.len() > 256
        {
            return Err(FieldError::InvalidDefinition);
        }
        if self.kind != FieldKind::SingleChoice && !self.options.is_empty() {
            return Err(FieldError::InvalidDefinition);
        }
        let mut ids = HashSet::new();
        for option in &self.options {
            if option.id.0.is_empty() || !ids.insert(&option.id) {
                return Err(FieldError::InvalidDefinition);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CivilDate {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl CivilDate {
    pub fn new(year: u16, month: u8, day: u8) -> Result<Self, FieldError> {
        if year == 0 || year > 9999 || !(1..=12).contains(&month) {
            return Err(FieldError::InvalidDate);
        }
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let max_day = match month {
            2 if leap => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        };
        if day == 0 || day > max_day {
            return Err(FieldError::InvalidDate);
        }
        Ok(Self { year, month, day })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldValue {
    Text(String),
    /// Exact thousandths, with signed values for general numeric fields.
    Number(i64),
    Boolean(bool),
    Date(CivilDate),
    SingleChoice(OptionId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldEdit {
    Keep,
    Set(FieldValue),
    Clear,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldError {
    InvalidDefinition,
    WrongType,
    WrongValueKind,
    Hidden,
    Archived,
    Required,
    ExistingValueMissing,
    InvalidValue,
    UnknownOption,
    ArchivedOption,
    InvalidDate,
}

/// Apply one field edit. `Keep` preserves an existing hidden or archived value;
/// ordinary `Set` and `Clear` cannot alter either field state.
pub fn apply_edit(
    definition: &FieldDefinition,
    item_type: &WorkItemTypeId,
    current: Option<&FieldValue>,
    edit: FieldEdit,
) -> Result<Option<FieldValue>, FieldError> {
    definition.validate()?;
    if &definition.owner_type != item_type {
        return Err(FieldError::WrongType);
    }
    if matches!(edit, FieldEdit::Keep) {
        if let Some(value) = current {
            validate_value(definition, value, true)?;
        }
        if definition.usage == FieldUsage::Required && !valid_required(current) {
            return Err(FieldError::Required);
        }
        return Ok(current.cloned());
    }
    if definition.archived {
        return Err(FieldError::Archived);
    }
    if definition.usage == FieldUsage::Hidden {
        return Err(FieldError::Hidden);
    }
    let next = match edit {
        FieldEdit::Keep => unreachable!(),
        FieldEdit::Clear => None,
        FieldEdit::Set(value) => {
            validate_value(definition, &value, false)?;
            Some(value)
        }
    };
    if definition.usage == FieldUsage::Required && !valid_required(next.as_ref()) {
        return Err(FieldError::Required);
    }
    Ok(next)
}

/// A required-field change is valid only when every applicable existing item
/// has a value valid under the proposed definition. Storage must coordinate
/// this check with concurrent item writes.
pub fn change_usage(
    definition: &FieldDefinition,
    usage: FieldUsage,
    applicable_values: &[Option<FieldValue>],
) -> Result<(), FieldError> {
    definition.validate()?;
    if definition.archived {
        return Err(FieldError::Archived);
    }
    if usage != FieldUsage::Required {
        return Ok(());
    }
    for value in applicable_values {
        if !valid_required(value.as_ref()) {
            return Err(FieldError::ExistingValueMissing);
        }
        if let Some(value) = value {
            validate_value(definition, value, true)?;
        }
    }
    Ok(())
}

fn valid_required(value: Option<&FieldValue>) -> bool {
    match value {
        None => false,
        Some(FieldValue::Text(text)) => !text.trim().is_empty(),
        Some(_) => true,
    }
}

fn validate_value(
    definition: &FieldDefinition,
    value: &FieldValue,
    allow_archived_option: bool,
) -> Result<(), FieldError> {
    match (definition.kind, value) {
        (FieldKind::Text, FieldValue::Text(text)) => {
            if text.len() > 16 * 1024 {
                return Err(FieldError::InvalidValue);
            }
        }
        (FieldKind::Number, FieldValue::Number(number)) => {
            if number.unsigned_abs() > 999_999_999 {
                return Err(FieldError::InvalidValue);
            }
        }
        (FieldKind::Boolean, FieldValue::Boolean(_)) => {}
        (FieldKind::Date, FieldValue::Date(date)) => {
            CivilDate::new(date.year, date.month, date.day)?;
        }
        (FieldKind::SingleChoice, FieldValue::SingleChoice(id)) => {
            let Some(option) = definition.options.iter().find(|option| option.id == *id) else {
                return Err(FieldError::UnknownOption);
            };
            if option.archived && !allow_archived_option {
                return Err(FieldError::ArchivedOption);
            }
        }
        _ => return Err(FieldError::WrongValueKind),
    }
    Ok(())
}
