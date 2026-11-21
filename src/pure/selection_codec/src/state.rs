use super::{
    request::{identifier, token},
    *,
};
use serde_json::{json, Value};
use task_contract::{
    current_work::UserId,
    relationship::EndpointIdentity,
    work_item::{ProjectId, WorkItemId},
};

type Result<T> = std::result::Result<T, Error>;
fn malformed() -> Error {
    Error::Malformed("selection stored shape")
}
fn keys(value: &Value, expected: &[&str]) -> Result<()> {
    let object = value.as_object().ok_or_else(malformed)?;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(malformed());
    }
    Ok(())
}
fn id(value: &Value) -> Result<String> {
    let text = value
        .as_str()
        .filter(|text| identifier(text))
        .ok_or_else(malformed)?;
    Ok(text.to_owned())
}
fn unsigned(value: &Value) -> Result<u64> {
    let text = value.as_str().ok_or_else(malformed)?;
    let number = text.parse::<u64>().map_err(|_| malformed())?;
    if number.to_string() != text {
        return Err(malformed());
    }
    Ok(number)
}
fn state_value(state: &CurrentWork) -> Result<Value> {
    if !identifier(&state.user_id.0) {
        return Err(malformed());
    }
    let target = match &state.selected {
        None => Value::Null,
        Some(target) => {
            if !identifier(&target.project_id.0) || !identifier(&target.item_id.0) {
                return Err(malformed());
            }
            json!({"project_id":target.project_id.0,"item_id":target.item_id.0})
        }
    };
    Ok(json!({"user_id":state.user_id.0,"version":state.version.to_string(),"requested":target}))
}
fn read_state(value: &Value) -> Result<CurrentWork> {
    keys(value, &["user_id", "version", "requested"])?;
    let selected = if value["requested"].is_null() {
        None
    } else {
        let target = &value["requested"];
        keys(target, &["project_id", "item_id"])?;
        Some(EndpointIdentity {
            project_id: ProjectId(id(&target["project_id"])?),
            item_id: WorkItemId(id(&target["item_id"])?),
        })
    };
    Ok(CurrentWork {
        user_id: UserId(id(&value["user_id"])?),
        version: unsigned(&value["version"])?,
        selected,
    })
}
fn result_value(result: &SelectionResult) -> Result<Value> {
    Ok(json!({"codec_version":1,"kind":"selection_result","value":state_value(&result.next)?}))
}
fn open<'a>(value: &'a Value, kind: &str) -> Result<&'a Value> {
    keys(value, &["codec_version", "kind", "value"])?;
    if value["codec_version"].as_u64() != Some(1) || value["kind"].as_str() != Some(kind) {
        return Err(malformed());
    }
    Ok(&value["value"])
}
fn read_result(value: &Value) -> Result<SelectionResult> {
    Ok(SelectionResult {
        next: read_state(open(value, "selection_result")?)?,
    })
}
fn encode(value: Value, maximum: usize) -> Result<Vec<u8>> {
    // Fixed scalar preflight precedes construction; native canonical encoding
    // additionally rejects NUL and reserves PostgreSQL JSONB structural spacing.
    let bytes = project_create_codec::composition::encode_native_record(&value)?;
    if bytes.len() > maximum {
        return Err(Error::LimitExceeded);
    }
    Ok(bytes)
}
fn parse(bytes: &[u8], maximum: usize) -> Result<Value> {
    if bytes.len() > maximum {
        return Err(Error::LimitExceeded);
    }
    project_create_codec::parse_json(bytes)
}
pub(super) fn encode_result(result: &SelectionResult) -> Result<Vec<u8>> {
    encode(result_value(result)?, MAX_WIRE_BYTES)
}
pub(super) fn decode_result(bytes: &[u8]) -> Result<SelectionResult> {
    read_result(&parse(bytes, MAX_WIRE_BYTES)?)
}
pub(super) fn encode_event(event: &Event) -> Result<Vec<u8>> {
    if event.ordinal == 0 || !token(&event.operation_id) {
        return Err(malformed());
    }
    let value = json!({"codec_version":1,"kind":"selection_event","value":{
        "ordinal":event.ordinal.to_string(),"operation_id":event.operation_id,
        "before":state_value(&event.before)?,"result":result_value(&event.result)?
    }});
    encode(value, MAX_HISTORY_BYTES)
}
pub(super) fn decode_event(bytes: &[u8]) -> Result<Event> {
    let value = parse(bytes, MAX_HISTORY_BYTES)?;
    let event = open(&value, "selection_event")?;
    keys(event, &["ordinal", "operation_id", "before", "result"])?;
    let ordinal = unsigned(&event["ordinal"])?;
    let operation_id = event["operation_id"]
        .as_str()
        .filter(|text| token(text))
        .ok_or_else(malformed)?
        .to_owned();
    if ordinal == 0 {
        return Err(malformed());
    }
    Ok(Event {
        ordinal,
        operation_id,
        before: read_state(&event["before"])?,
        result: read_result(&event["result"])?,
    })
}
