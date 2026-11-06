//! Closed text/estimate mutation inputs. Reconciliation carries the same identity.
use serde_json::{Map, Value};
use task_contract::{
    field::{FieldEdit, FieldId, FieldValue},
    item_mutation::{EditCommand, EstimateEdit, FieldUpdate},
    project::Estimate,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Route {
    ExecuteCreate,
    ReconcileCreate,
    Edit,
    ReconcileEdit,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Action {
    Execute,
    Reconcile,
}
impl Route {
    pub(crate) fn from_path(path: &str) -> Option<Self> {
        match path {
            "/items/pending/execute" => Some(Self::ExecuteCreate),
            "/items/pending/reconcile" => Some(Self::ReconcileCreate),
            "/items/edit" => Some(Self::Edit),
            "/items/reconcile" => Some(Self::ReconcileEdit),
            _ => None,
        }
    }
    pub(crate) fn body_limit(self) -> usize {
        match self {
            Self::Edit | Self::ReconcileEdit => project_create_codec::MAX_RECORD_BYTES,
            Self::ExecuteCreate | Self::ReconcileCreate => crate::http_auth::BODY_LIMIT,
        }
    }
    fn action(self) -> Action {
        match self {
            Self::ExecuteCreate | Self::Edit => Action::Execute,
            _ => Action::Reconcile,
        }
    }
}
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Request {
    Create {
        project: String,
        handle: String,
    },
    Edit {
        project: String,
        item: String,
        command: EditCommand,
    },
}
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Input {
    pub(crate) action: Action,
    pub(crate) request: Request,
}
#[derive(Debug)]
pub(crate) struct Invalid;
fn object(value: Value) -> Result<Map<String, Value>, Invalid> {
    match value {
        Value::Object(value) => Ok(value),
        _ => Err(Invalid),
    }
}
fn take(value: &mut Map<String, Value>, key: &str) -> Result<Value, Invalid> {
    value.remove(key).ok_or(Invalid)
}
fn string(value: Value, limit: usize) -> Result<String, Invalid> {
    match value {
        Value::String(value) if value.len() <= limit && !value.contains('\0') => Ok(value),
        _ => Err(Invalid),
    }
}
fn id(value: Value) -> Result<String, Invalid> {
    let value = string(value, 128)?;
    if value.is_empty() {
        Err(Invalid)
    } else {
        Ok(value)
    }
}
fn key(value: Value) -> Result<String, Invalid> {
    let value = string(value, 32)?;
    if value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(value)
    } else {
        Err(Invalid)
    }
}
fn unsigned(value: Value) -> Result<u64, Invalid> {
    let text = string(value, 20)?;
    let value = text.parse::<u64>().map_err(|_| Invalid)?;
    if value.to_string() == text {
        Ok(value)
    } else {
        Err(Invalid)
    }
}
fn positive(value: Value) -> Result<u64, Invalid> {
    let value = unsigned(value)?;
    if value == 0 {
        Err(Invalid)
    } else {
        Ok(value)
    }
}
fn estimate(value: Value) -> Result<EstimateEdit, Invalid> {
    let mut value = object(value)?;
    let action = string(take(&mut value, "action")?, 5)?;
    let result = match action.as_str() {
        "keep" => EstimateEdit::Keep,
        "clear" => EstimateEdit::Clear,
        "set" => EstimateEdit::Set(
            Estimate::try_from_thousandths(unsigned(take(&mut value, "value")?)?)
                .map_err(|_| Invalid)?,
        ),
        _ => return Err(Invalid),
    };
    if value.is_empty() {
        Ok(result)
    } else {
        Err(Invalid)
    }
}
fn fields(value: Value) -> Result<Vec<FieldUpdate>, Invalid> {
    let Value::Array(values) = value else {
        return Err(Invalid);
    };
    if values.len() > project_create_codec::MAX_COLLECTION_ENTRIES {
        return Err(Invalid);
    }
    let mut seen = std::collections::HashSet::with_capacity(values.len());
    let mut updates = Vec::with_capacity(values.len());
    for value in values {
        let mut value = object(value)?;
        let id = id(take(&mut value, "id")?)?;
        if !seen.insert(id.clone()) {
            return Err(Invalid);
        }
        let action = string(take(&mut value, "action")?, 5)?;
        let edit = match action.as_str() {
            "keep" => FieldEdit::Keep,
            "clear" => FieldEdit::Clear,
            "set" => FieldEdit::Set(FieldValue::Text(string(
                take(&mut value, "value")?,
                project_create_codec::MAX_STRING_BYTES,
            )?)),
            _ => return Err(Invalid),
        };
        if !value.is_empty() {
            return Err(Invalid);
        }
        updates.push(FieldUpdate {
            id: FieldId(id),
            edit,
        });
    }
    Ok(updates)
}
pub(crate) fn decode(route: Route, bytes: &[u8]) -> Result<Input, Invalid> {
    if bytes.len() > route.body_limit() {
        return Err(Invalid);
    }
    let mut value = object(project_create_codec::parse_json(bytes).map_err(|_| Invalid)?)?;
    let project = id(take(&mut value, "project_id")?)?;
    let request = match route {
        Route::ExecuteCreate | Route::ReconcileCreate => Request::Create {
            project,
            handle: key(take(&mut value, "intent_handle")?)?,
        },
        Route::Edit | Route::ReconcileEdit => Request::Edit {
            project,
            item: id(take(&mut value, "item_id")?)?,
            command: EditCommand {
                operation_id: key(take(&mut value, "operation_id")?)?,
                expected_item_version: positive(take(&mut value, "expected_item_version")?)?,
                expected_configuration_revision: positive(take(
                    &mut value,
                    "expected_configuration_revision",
                )?)?,
                estimate: estimate(take(&mut value, "estimate")?)?,
                fields: fields(take(&mut value, "fields")?)?,
            },
        },
    };
    if !value.is_empty() {
        return Err(Invalid);
    }
    Ok(Input {
        action: route.action(),
        request,
    })
}
#[cfg(test)]
mod tests;
