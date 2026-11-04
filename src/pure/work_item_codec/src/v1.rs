//! Frozen version-one shapes. Reconstruction never consults current configuration.
use super::*;
use project_create_codec::RecordBudget;
use serde_json::{json, Value};
use task_contract::{field::*, project::Estimate};
mod saved;
pub(super) use saved::{
    decode_result, decode_revision, result, result_preflight, revision, revision_preflight,
};

type Result<T> = std::result::Result<T, Error>;
fn malformed(reason: &'static str) -> Error {
    Error::Malformed(reason)
}
fn object(value: &Value, keys: &[&str]) -> Result<()> {
    let object = value.as_object().ok_or(malformed("expected object"))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(malformed("missing or unknown field"));
    }
    Ok(())
}
fn string(value: &Value) -> Result<String> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or(malformed("expected string"))
}
fn unsigned(value: &Value) -> Result<u64> {
    let text = value
        .as_str()
        .ok_or(malformed("expected unsigned string"))?;
    let number = text
        .parse::<u64>()
        .map_err(|_| malformed("unsigned integer"))?;
    if number.to_string() != text {
        return Err(malformed("noncanonical unsigned integer"));
    }
    Ok(number)
}
fn signed(value: &Value) -> Result<i64> {
    let text = value.as_str().ok_or(malformed("expected signed string"))?;
    let number = text
        .parse::<i64>()
        .map_err(|_| malformed("signed integer"))?;
    if number.to_string() != text {
        return Err(malformed("noncanonical signed integer"));
    }
    Ok(number)
}
fn optional_string(value: &Value) -> Result<Option<String>> {
    if value.is_null() {
        Ok(None)
    } else {
        string(value).map(Some)
    }
}
fn list<T>(value: &Value, decode: impl Fn(&Value) -> Result<T>) -> Result<Vec<T>> {
    value
        .as_array()
        .ok_or(malformed("expected array"))?
        .iter()
        .map(decode)
        .collect()
}
fn envelope(kind: &str, value: Value) -> Value {
    json!({"codec_version":1,"kind":kind,"value":value})
}
fn open<'a>(value: &'a Value, kind: &str) -> Result<&'a Value> {
    object(value, &["codec_version", "kind", "value"])?;
    if value["codec_version"].as_u64() != Some(1) || value["kind"].as_str() != Some(kind) {
        return Err(malformed("envelope version or kind"));
    }
    Ok(&value["value"])
}
fn estimate(value: Option<Estimate>) -> Value {
    value.map_or(Value::Null, |value| {
        Value::String(value.thousandths().to_string())
    })
}
fn decode_estimate(value: &Value) -> Result<Option<Estimate>> {
    if value.is_null() {
        return Ok(None);
    }
    Estimate::try_from_thousandths(unsigned(value)?)
        .map(Some)
        .map_err(|_| malformed("estimate range"))
}
fn field_value(value: &FieldValue) -> Value {
    match value {
        FieldValue::Text(text) => json!({"kind":"text","value":text}),
        FieldValue::Number(number) => json!({"kind":"number","value":number.to_string()}),
        FieldValue::Boolean(value) => json!({"kind":"boolean","value":value}),
        FieldValue::Date(date) => {
            json!({"kind":"date","value":format!("{:04}-{:02}-{:02}",date.year,date.month,date.day)})
        }
        FieldValue::SingleChoice(id) => json!({"kind":"single_choice","value":id.0}),
    }
}
fn decode_field_value(value: &Value) -> Result<FieldValue> {
    object(value, &["kind", "value"])?;
    let inner = &value["value"];
    Ok(match value["kind"].as_str() {
        Some("text") => FieldValue::Text(string(inner)?),
        Some("number") => FieldValue::Number(signed(inner)?),
        Some("boolean") => FieldValue::Boolean(inner.as_bool().ok_or(malformed("boolean"))?),
        Some("single_choice") => FieldValue::SingleChoice(OptionId(string(inner)?)),
        Some("date") => {
            let text = inner.as_str().ok_or(malformed("date string"))?;
            let bytes = text.as_bytes();
            if bytes.len() != 10
                || bytes[4] != b'-'
                || bytes[7] != b'-'
                || !bytes
                    .iter()
                    .enumerate()
                    .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
            {
                return Err(malformed("canonical date"));
            }
            let year = text[..4].parse().map_err(|_| malformed("year"))?;
            let month = text[5..7].parse().map_err(|_| malformed("month"))?;
            let day = text[8..].parse().map_err(|_| malformed("day"))?;
            FieldValue::Date(CivilDate::new(year, month, day).map_err(|_| malformed("date range"))?)
        }
        _ => return Err(malformed("field value kind")),
    })
}
fn field_preflight(value: &FieldValue, budget: &mut RecordBudget) -> Result<()> {
    match value {
        FieldValue::Text(text) => budget.text(text),
        FieldValue::SingleChoice(id) => budget.text(&id.0),
        FieldValue::Date(date) => {
            CivilDate::new(date.year, date.month, date.day).map_err(|_| malformed("date range"))?;
            Ok(())
        }
        _ => Ok(()),
    }
}
fn values(values: &[FieldEntry]) -> Value {
    Value::Array(
        values
            .iter()
            .map(|entry| json!({"id":entry.id.0,"value":field_value(&entry.value)}))
            .collect(),
    )
}
fn decode_values(value: &Value) -> Result<Vec<FieldEntry>> {
    list(value, |entry| {
        object(entry, &["id", "value"])?;
        Ok(FieldEntry {
            id: FieldId(string(&entry["id"])?),
            value: decode_field_value(&entry["value"])?,
        })
    })
}
fn values_preflight(values: &[FieldEntry], budget: &mut RecordBudget) -> Result<()> {
    budget.collection_length(values.len())?;
    for entry in values {
        budget.text(&entry.id.0)?;
        field_preflight(&entry.value, budget)?;
    }
    Ok(())
}
fn estimate_edit(edit: &EstimateEdit) -> Value {
    match edit {
        EstimateEdit::Keep => json!({"action":"keep"}),
        EstimateEdit::Clear => json!({"action":"clear"}),
        EstimateEdit::Set(value) => json!({"action":"set","value":estimate(Some(*value))}),
    }
}
fn decode_estimate_edit(value: &Value) -> Result<EstimateEdit> {
    match value["action"].as_str() {
        Some("keep") => {
            object(value, &["action"])?;
            Ok(EstimateEdit::Keep)
        }
        Some("clear") => {
            object(value, &["action"])?;
            Ok(EstimateEdit::Clear)
        }
        Some("set") => {
            object(value, &["action", "value"])?;
            Ok(EstimateEdit::Set(
                decode_estimate(&value["value"])?.ok_or(malformed("set estimate"))?,
            ))
        }
        _ => Err(malformed("estimate action")),
    }
}
fn field_edit(edit: &FieldEdit) -> Value {
    match edit {
        FieldEdit::Keep => json!({"action":"keep"}),
        FieldEdit::Clear => json!({"action":"clear"}),
        FieldEdit::Set(value) => json!({"action":"set","value":field_value(value)}),
    }
}
fn decode_field_edit(value: &Value) -> Result<FieldEdit> {
    match value["action"].as_str() {
        Some("keep") => {
            object(value, &["action"])?;
            Ok(FieldEdit::Keep)
        }
        Some("clear") => {
            object(value, &["action"])?;
            Ok(FieldEdit::Clear)
        }
        Some("set") => {
            object(value, &["action", "value"])?;
            Ok(FieldEdit::Set(decode_field_value(&value["value"])?))
        }
        _ => Err(malformed("field action")),
    }
}
pub(super) fn request_preflight(request: &Request, budget: &mut RecordBudget) -> Result<()> {
    match request {
        Request::Create {
            project_id,
            command: c,
        } => {
            for text in [&project_id.0, &c.operation_id, &c.item_id.0, &c.type_id.0] {
                budget.text(text)?;
            }
            if let Some(workflow) = &c.workflow_id {
                budget.text(&workflow.0)?;
            }
            values_preflight(&c.values, budget)
        }
        Request::Edit {
            project_id,
            item_id,
            command: c,
        } => {
            for text in [&project_id.0, &item_id.0, &c.operation_id] {
                budget.text(text)?;
            }
            budget.collection_length(c.fields.len())?;
            for field in &c.fields {
                budget.text(&field.id.0)?;
                if let FieldEdit::Set(value) = &field.edit {
                    field_preflight(value, budget)?;
                }
            }
            Ok(())
        }
    }
}
pub(super) fn request(request: &Request) -> Value {
    match request {
        Request::Create {
            project_id,
            command: c,
        } => envelope(
            "item_create_request",
            json!({
                "project_id":project_id.0,"operation_id":c.operation_id,"item_id":c.item_id.0,
                "type_id":c.type_id.0,"workflow_id":c.workflow_id.as_ref().map(|v|&v.0),
                "expected_configuration_revision":c.expected_configuration_revision.to_string(),
                "estimate":estimate(c.estimate),"values":values(&c.values)
            }),
        ),
        Request::Edit {
            project_id,
            item_id,
            command: c,
        } => envelope(
            "item_edit_request",
            json!({
                "project_id":project_id.0,"item_id":item_id.0,"operation_id":c.operation_id,
                "expected_configuration_revision":c.expected_configuration_revision.to_string(),
                "expected_item_version":c.expected_item_version.to_string(),"estimate":estimate_edit(&c.estimate),
                "fields":c.fields.iter().map(|f|json!({"id":f.id.0,"edit":field_edit(&f.edit)})).collect::<Vec<_>>()
            }),
        ),
    }
}
pub(super) fn decode_request(value: &Value) -> Result<Request> {
    match value["kind"].as_str() {
        Some("item_create_request") => {
            let c = open(value, "item_create_request")?;
            object(
                c,
                &[
                    "project_id",
                    "operation_id",
                    "item_id",
                    "type_id",
                    "workflow_id",
                    "expected_configuration_revision",
                    "estimate",
                    "values",
                ],
            )?;
            Ok(Request::Create {
                project_id: ProjectId(string(&c["project_id"])?),
                command: CreateCommand {
                    operation_id: string(&c["operation_id"])?,
                    item_id: WorkItemId(string(&c["item_id"])?),
                    type_id: WorkItemTypeId(string(&c["type_id"])?),
                    workflow_id: optional_string(&c["workflow_id"])?.map(WorkflowId),
                    expected_configuration_revision: unsigned(
                        &c["expected_configuration_revision"],
                    )?,
                    estimate: decode_estimate(&c["estimate"])?,
                    values: decode_values(&c["values"])?,
                },
            })
        }
        Some("item_edit_request") => {
            let c = open(value, "item_edit_request")?;
            object(
                c,
                &[
                    "project_id",
                    "item_id",
                    "operation_id",
                    "expected_configuration_revision",
                    "expected_item_version",
                    "estimate",
                    "fields",
                ],
            )?;
            Ok(Request::Edit {
                project_id: ProjectId(string(&c["project_id"])?),
                item_id: WorkItemId(string(&c["item_id"])?),
                command: EditCommand {
                    operation_id: string(&c["operation_id"])?,
                    expected_item_version: unsigned(&c["expected_item_version"])?,
                    expected_configuration_revision: unsigned(
                        &c["expected_configuration_revision"],
                    )?,
                    estimate: decode_estimate_edit(&c["estimate"])?,
                    fields: list(&c["fields"], |f| {
                        object(f, &["id", "edit"])?;
                        Ok(FieldUpdate {
                            id: FieldId(string(&f["id"])?),
                            edit: decode_field_edit(&f["edit"])?,
                        })
                    })?,
                },
            })
        }
        _ => Err(malformed("request family")),
    }
}
