use super::*;
use project_create_codec::composition::{project_from_value, project_value};
use task_contract::project::Project;

fn phase(value: Phase) -> &'static str {
    match value {
        Phase::New => "new",
        Phase::Active => "active",
        Phase::Done => "done",
    }
}
fn decode_phase(value: &Value) -> Result<Phase> {
    match value.as_str() {
        Some("new") => Ok(Phase::New),
        Some("active") => Ok(Phase::Active),
        Some("done") => Ok(Phase::Done),
        _ => Err(malformed("phase")),
    }
}
fn boolean(value: &Value) -> Result<bool> {
    value.as_bool().ok_or(malformed("boolean"))
}
fn item(item: &WorkItem) -> Value {
    json!({"id":item.id.0,"project_id":item.project_id.0,"type_id":item.type_id.0,
        "workflow_id":item.workflow_id.0,"status_id":item.status_id.0,
        "version":item.version.to_string(),"archived":item.archived})
}
fn decode_item(value: &Value) -> Result<WorkItem> {
    object(
        value,
        &[
            "id",
            "project_id",
            "type_id",
            "workflow_id",
            "status_id",
            "version",
            "archived",
        ],
    )?;
    Ok(WorkItem {
        id: WorkItemId(string(&value["id"])?),
        project_id: ProjectId(string(&value["project_id"])?),
        type_id: WorkItemTypeId(string(&value["type_id"])?),
        workflow_id: WorkflowId(string(&value["workflow_id"])?),
        status_id: StatusId(string(&value["status_id"])?),
        version: unsigned(&value["version"])?,
        archived: boolean(&value["archived"])?,
    })
}
fn data(value: &ItemData) -> Value {
    json!({"estimate":estimate(value.estimate),"values":values(&value.values)})
}
fn decode_data(value: &Value) -> Result<ItemData> {
    object(value, &["estimate", "values"])?;
    Ok(ItemData {
        estimate: decode_estimate(&value["estimate"])?,
        values: decode_values(&value["values"])?,
    })
}
fn common(result: &SavedResult) -> (&Project, &WorkItem, &ItemData, Phase) {
    match result {
        SavedResult::Create(r) => (&r.project, &r.item, &r.data, r.phase),
        SavedResult::Edit(r) => (&r.project, &r.item, &r.data, r.phase),
    }
}
pub(crate) fn result_preflight(result: &SavedResult, budget: &mut RecordBudget) -> Result<()> {
    let (p, item, data, _) = common(result);
    for text in [
        &p.id.0,
        &p.name,
        &p.prefix,
        &item.id.0,
        &item.project_id.0,
        &item.type_id.0,
        &item.workflow_id.0,
        &item.status_id.0,
    ] {
        budget.text(text)?;
    }
    values_preflight(&data.values, budget)?;
    if let SavedResult::Create(r) = result {
        for text in [
            &r.readable_id.project_id.0,
            &r.readable_id.item_id.0,
            &r.readable_id.prefix,
        ] {
            budget.text(text)?;
        }
    }
    Ok(())
}
pub(crate) fn result(result: &SavedResult) -> Result<Value> {
    let (p, item_value, item_data, item_phase) = common(result);
    let mut value = json!({"project":project_value(p)?,"item":item(item_value),"data":data(item_data),"phase":phase(item_phase)});
    let kind = match result {
        SavedResult::Create(r) => {
            value["readable_id"] = json!({"project_id":r.readable_id.project_id.0,"item_id":r.readable_id.item_id.0,
                "prefix":r.readable_id.prefix,"sequence":r.readable_id.sequence.to_string()});
            "item_create_result"
        }
        SavedResult::Edit(_) => "item_edit_result",
    };
    Ok(envelope(kind, value))
}
pub(crate) fn decode_result(value: &Value) -> Result<SavedResult> {
    let kind = value["kind"].as_str().ok_or(malformed("result kind"))?;
    if kind != "item_create_result" && kind != "item_edit_result" {
        return Err(malformed("result family"));
    }
    let result = open(value, kind)?;
    if kind == "item_create_result" {
        object(result, &["project", "item", "data", "phase", "readable_id"])?;
    } else {
        object(result, &["project", "item", "data", "phase"])?;
    }
    let project = project_from_value(&result["project"])?;
    let item = decode_item(&result["item"])?;
    let data = decode_data(&result["data"])?;
    let phase = decode_phase(&result["phase"])?;
    if kind == "item_edit_result" {
        return Ok(SavedResult::Edit(EditResult {
            project,
            item,
            data,
            phase,
        }));
    }
    let id = &result["readable_id"];
    object(id, &["project_id", "item_id", "prefix", "sequence"])?;
    Ok(SavedResult::Create(CreateResult {
        project,
        item,
        data,
        phase,
        readable_id: task_contract::project::ReadableId {
            project_id: ProjectId(string(&id["project_id"])?),
            item_id: WorkItemId(string(&id["item_id"])?),
            prefix: string(&id["prefix"])?,
            sequence: unsigned(&id["sequence"])?,
        },
    }))
}
pub(crate) fn revision_preflight(revision: &Revision, budget: &mut RecordBudget) -> Result<()> {
    budget.text(&revision.actor_id.0)?;
    result_preflight(&revision.result, budget)
}
pub(crate) fn revision(revision: &Revision) -> Result<Value> {
    Ok(envelope(
        "item_revision",
        json!({
            "actor_id":revision.actor_id.0,
            "configuration_revision":revision.configuration_revision.to_string(),
            "project_state_version":revision.project_state_version.to_string(),
            "previous_project_operation":revision.previous_project_operation.to_string(),"result":result(&revision.result)?
        }),
    ))
}
pub(crate) fn decode_revision(value: &Value) -> Result<Revision> {
    let value = open(value, "item_revision")?;
    object(
        value,
        &[
            "actor_id",
            "configuration_revision",
            "project_state_version",
            "previous_project_operation",
            "result",
        ],
    )?;
    Ok(Revision {
        actor_id: UserId(string(&value["actor_id"])?),
        configuration_revision: unsigned(&value["configuration_revision"])?,
        project_state_version: unsigned(&value["project_state_version"])?,
        previous_project_operation: unsigned(&value["previous_project_operation"])?,
        result: decode_result(&value["result"])?,
    })
}
