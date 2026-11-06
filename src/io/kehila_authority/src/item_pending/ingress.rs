//! Closed text-form and locator shapes, before application/entropy admission.
use super::Form;
use serde_json::{Map, Value};
use task_contract::{
    field::{FieldEntry, FieldId, FieldValue},
    project::Estimate,
    work_item::{ProjectId, WorkItemTypeId, WorkflowId},
};

#[derive(Clone, Copy)]
pub(crate) enum Route {
    Register,
    List,
    Open,
    Cancel,
}
impl Route {
    pub(crate) fn from_path(path: &str) -> Option<Self> {
        Some(match path {
            "/items/pending/register" => Self::Register,
            "/items/pending/list" => Self::List,
            "/items/pending/open" => Self::Open,
            "/items/pending/cancel" => Self::Cancel,
            _ => return None,
        })
    }
    pub(crate) fn body_limit(self) -> usize {
        match self {
            Self::Register => project_create_codec::MAX_RECORD_BYTES,
            _ => crate::http_auth::BODY_LIMIT,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Input {
    Register(Form),
    List {
        project: String,
        before: Option<i64>,
    },
    Open {
        project: String,
        handle: String,
    },
    Cancel {
        project: String,
        handle: String,
    },
}

#[derive(Debug)]
pub(crate) struct Invalid;

pub(super) fn key(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn object(value: Value) -> Result<Map<String, Value>, Invalid> {
    match value {
        Value::Object(value) => Ok(value),
        _ => Err(Invalid),
    }
}
fn string(value: Value, limit: usize) -> Result<String, Invalid> {
    match value {
        Value::String(value) if value.len() <= limit && !value.contains('\0') => Ok(value),
        _ => Err(Invalid),
    }
}
fn take(object: &mut Map<String, Value>, name: &str) -> Result<Value, Invalid> {
    object.remove(name).ok_or(Invalid)
}
fn id(value: Value) -> Result<String, Invalid> {
    let value = string(value, 128)?;
    if value.is_empty() {
        Err(Invalid)
    } else {
        Ok(value)
    }
}
fn unsigned(value: Value) -> Result<u64, Invalid> {
    let text = string(value, 20)?;
    let number = text.parse::<u64>().map_err(|_| Invalid)?;
    if number.to_string() != text {
        Err(Invalid)
    } else {
        Ok(number)
    }
}

fn form(project: String, object: &mut Map<String, Value>) -> Result<Form, Invalid> {
    let kind = WorkItemTypeId(id(take(object, "type_id")?)?);
    let workflow = match take(object, "workflow_id")? {
        Value::Null => None,
        value => Some(WorkflowId(id(value)?)),
    };
    let configuration = unsigned(take(object, "expected_configuration_revision")?)?;
    if configuration == 0 {
        return Err(Invalid);
    }
    let estimate = match take(object, "estimate")? {
        Value::Null => None,
        value => Some(Estimate::try_from_thousandths(unsigned(value)?).map_err(|_| Invalid)?),
    };
    let Value::Array(entries) = take(object, "values")? else {
        return Err(Invalid);
    };
    if entries.len() > project_create_codec::MAX_COLLECTION_ENTRIES {
        return Err(Invalid);
    }
    let mut seen = std::collections::HashSet::new();
    let mut values = Vec::with_capacity(entries.len());
    for entry in entries {
        let mut entry = self::object(entry)?;
        let field = id(take(&mut entry, "id")?)?;
        let value = string(
            take(&mut entry, "value")?,
            project_create_codec::MAX_STRING_BYTES,
        )?;
        if !entry.is_empty() || !seen.insert(field.clone()) {
            return Err(Invalid);
        }
        values.push(FieldEntry {
            id: FieldId(field),
            value: FieldValue::Text(value),
        });
    }
    Ok(Form {
        project: ProjectId(project),
        kind,
        workflow,
        configuration,
        estimate,
        values,
    })
}

pub(crate) fn decode(route: Route, bytes: &[u8]) -> Result<Input, Invalid> {
    if bytes.len() > route.body_limit() {
        return Err(Invalid);
    }
    let mut object = self::object(project_create_codec::parse_json(bytes).map_err(|_| Invalid)?)?;
    let project = id(take(&mut object, "project_id")?)?;
    let input = match route {
        Route::Register => Input::Register(form(project, &mut object)?),
        Route::List => {
            let before = match take(&mut object, "before")? {
                Value::Null => None,
                value => {
                    let number = unsigned(value)?;
                    Some(
                        i64::try_from(number)
                            .ok()
                            .filter(|n| *n > 0)
                            .ok_or(Invalid)?,
                    )
                }
            };
            Input::List { project, before }
        }
        Route::Open | Route::Cancel => {
            let handle = string(take(&mut object, "intent_handle")?, 32)?;
            if !key(&handle) {
                return Err(Invalid);
            }
            match route {
                Route::Open => Input::Open { project, handle },
                _ => Input::Cancel { project, handle },
            }
        }
    };
    if !object.is_empty() {
        return Err(Invalid);
    }
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn valid() -> Value {
        serde_json::json!({"project_id":"p","type_id":"task","workflow_id":null,
            "expected_configuration_revision":"1","estimate":"0",
            "values":[{"id":"title","value":"  "}]})
    }
    #[test]
    fn form_is_text_intent_without_caller_generated_identity_or_authority() {
        let Input::Register(form) =
            decode(Route::Register, &serde_json::to_vec(&valid()).unwrap()).unwrap()
        else {
            panic!()
        };
        assert_eq!(form.estimate.unwrap().thousandths(), 0);
        assert_eq!(form.values[0].value, FieldValue::Text("  ".into()));
        for name in [
            "actor_id",
            "token",
            "operation_id",
            "intent_handle",
            "item_id",
            "configuration",
        ] {
            let mut body = valid();
            body[name] = Value::String("caller".into());
            assert!(
                decode(Route::Register, &serde_json::to_vec(&body).unwrap()).is_err(),
                "{name}"
            );
        }
        for field in [
            "project_id",
            "type_id",
            "workflow_id",
            "expected_configuration_revision",
            "estimate",
            "values",
        ] {
            let mut body = valid();
            body.as_object_mut().unwrap().remove(field);
            assert!(
                decode(Route::Register, &serde_json::to_vec(&body).unwrap()).is_err(),
                "{field}"
            );
        }
    }
    #[test]
    fn original_duplicates_nested_unknowns_and_noncanonical_numbers_are_rejected() {
        assert!(decode(Route::Open, br#"{"project_id":"p","project_id":"q","intent_handle":"00000000000000000000000000000000"}"#).is_err());
        for estimate in [
            serde_json::json!(0),
            serde_json::json!("01"),
            serde_json::json!("1000000000"),
        ] {
            let mut body = valid();
            body["estimate"] = estimate;
            assert!(decode(Route::Register, &serde_json::to_vec(&body).unwrap()).is_err());
        }
        let mut body = valid();
        body["values"][0]["extra"] = Value::Null;
        assert!(decode(Route::Register, &serde_json::to_vec(&body).unwrap()).is_err());
        let mut body = valid();
        let duplicate = body["values"][0].clone();
        body["values"].as_array_mut().unwrap().push(duplicate);
        assert!(decode(Route::Register, &serde_json::to_vec(&body).unwrap()).is_err());
    }
}
