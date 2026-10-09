//! Bounded SP-003 prototype, not a production adapter or accepted wire contract.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use task_contract::field::*;
use task_contract::project::{EstimateUnit, Project};
use task_contract::project_create::*;
use task_contract::relationship::{Direction, RelationshipType, RelationshipTypeId};
use task_contract::work_item::*;

const HEADER: &[u8] = b"kehila\0project_create\0\x00\x00\x00\x01";
type Outcome<T> = Result<T, String>;

pub fn encode_request(command: &ProjectCreateCommand, version: u32) -> Outcome<Vec<u8>> {
    if version != 1 {
        return Err("unsupported request codec version".into());
    }
    let mut bytes = HEADER.to_vec();
    for field in [
        &command.operation_id,
        &command.project_id.0,
        &command.name,
        &command.prefix,
    ] {
        let length = u32::try_from(field.len()).map_err(|_| "string exceeds u32 length")?;
        bytes.extend(length.to_be_bytes());
        bytes.extend(field.as_bytes());
    }
    bytes.push(match command.estimate_unit {
        EstimateUnit::Hours => 0,
        EstimateUnit::Points => 1,
    });
    Ok(bytes)
}

pub fn decode_request(bytes: &[u8]) -> Outcome<ProjectCreateCommand> {
    let mut remaining = bytes
        .strip_prefix(HEADER)
        .ok_or("unsupported request header/version")?;
    let mut strings = Vec::new();
    for _ in 0..4 {
        let length_bytes = remaining.get(..4).ok_or("truncated length")?;
        let length =
            u32::from_be_bytes(length_bytes.try_into().map_err(|_| "invalid length")?) as usize;
        remaining = &remaining[4..];
        let field = remaining.get(..length).ok_or("truncated string")?;
        strings.push(
            std::str::from_utf8(field)
                .map_err(|_| "invalid UTF8")?
                .to_owned(),
        );
        remaining = &remaining[length..];
    }
    let estimate_unit = match remaining {
        [0] => EstimateUnit::Hours,
        [1] => EstimateUnit::Points,
        _ => return Err("invalid unit or trailing bytes".into()),
    };
    let [operation_id, project_id, name, prefix]: [String; 4] = strings
        .try_into()
        .map_err(|_| "invalid request field count")?;
    Ok(ProjectCreateCommand {
        operation_id,
        project_id: ProjectId(project_id),
        name,
        prefix,
        estimate_unit,
    })
}

pub fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn object(value: &Value, keys: &[&str]) -> Outcome<()> {
    let obj = value.as_object().ok_or("expected object")?;
    if obj.len() != keys.len() || keys.iter().any(|key| !obj.contains_key(*key)) {
        return Err("missing or unknown field".into());
    }
    Ok(())
}
fn string(value: &Value) -> Outcome<String> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or("expected string".into())
}
fn boolean(value: &Value) -> Outcome<bool> {
    value.as_bool().ok_or("expected boolean".into())
}
pub fn unsigned(value: &Value) -> Outcome<u64> {
    let s = value.as_str().ok_or("u64 requires decimal string")?;
    if s.is_empty() || (s.len() > 1 && s.starts_with('0')) || !s.bytes().all(|b| b.is_ascii_digit())
    {
        return Err("noncanonical u64".into());
    }
    s.parse().map_err(|_| "u64 overflow".into())
}
fn list<T>(value: &Value, decode: impl Fn(&Value) -> Outcome<T>) -> Outcome<Vec<T>> {
    value
        .as_array()
        .ok_or("expected array")?
        .iter()
        .map(decode)
        .collect()
}
fn optional(value: &Value) -> Outcome<Option<String>> {
    if value.is_null() {
        Ok(None)
    } else {
        string(value).map(Some)
    }
}
fn phase_label(phase: Phase) -> &'static str {
    match phase {
        Phase::New => "new",
        Phase::Active => "active",
        Phase::Done => "done",
    }
}
fn phase(value: &Value) -> Outcome<Phase> {
    match value.as_str() {
        Some("new") => Ok(Phase::New),
        Some("active") => Ok(Phase::Active),
        Some("done") => Ok(Phase::Done),
        _ => Err("invalid phase".into()),
    }
}
fn unit_label(unit: EstimateUnit) -> &'static str {
    match unit {
        EstimateUnit::Hours => "hours",
        EstimateUnit::Points => "points",
    }
}
fn unit(value: &Value) -> Outcome<EstimateUnit> {
    match value.as_str() {
        Some("hours") => Ok(EstimateUnit::Hours),
        Some("points") => Ok(EstimateUnit::Points),
        _ => Err("invalid estimate unit".into()),
    }
}
fn kind_label(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::Text => "text",
        FieldKind::Number => "number",
        FieldKind::Boolean => "boolean",
        FieldKind::Date => "date",
        FieldKind::SingleChoice => "single_choice",
    }
}
fn kind(value: &Value) -> Outcome<FieldKind> {
    match value.as_str() {
        Some("text") => Ok(FieldKind::Text),
        Some("number") => Ok(FieldKind::Number),
        Some("boolean") => Ok(FieldKind::Boolean),
        Some("date") => Ok(FieldKind::Date),
        Some("single_choice") => Ok(FieldKind::SingleChoice),
        _ => Err("invalid field kind".into()),
    }
}
fn origin_label(origin: FieldOrigin) -> &'static str {
    match origin {
        FieldOrigin::Application => "application",
        FieldOrigin::Project => "project",
    }
}
fn origin(value: &Value) -> Outcome<FieldOrigin> {
    match value.as_str() {
        Some("application") => Ok(FieldOrigin::Application),
        Some("project") => Ok(FieldOrigin::Project),
        _ => Err("invalid field origin".into()),
    }
}
fn usage_label(usage: FieldUsage) -> &'static str {
    match usage {
        FieldUsage::Hidden => "hidden",
        FieldUsage::Optional => "optional",
        FieldUsage::Required => "required",
    }
}
fn usage(value: &Value) -> Outcome<FieldUsage> {
    match value.as_str() {
        Some("hidden") => Ok(FieldUsage::Hidden),
        Some("optional") => Ok(FieldUsage::Optional),
        Some("required") => Ok(FieldUsage::Required),
        _ => Err("invalid field usage".into()),
    }
}
fn direction_label(direction: Direction) -> &'static str {
    match direction {
        Direction::Directed => "directed",
        Direction::Symmetric => "symmetric",
    }
}
fn direction(value: &Value) -> Outcome<Direction> {
    match value.as_str() {
        Some("directed") => Ok(Direction::Directed),
        Some("symmetric") => Ok(Direction::Symmetric),
        _ => Err("invalid direction".into()),
    }
}

fn encode_configuration(c: &Configuration) -> Value {
    json!({"project_id":c.project_id.0,"revision":c.revision.to_string(),"project_archived":c.project_archived,
        "statuses":c.statuses.iter().map(|s| json!({"id":s.id.0,"name":s.name,"group_id":s.group_id.as_ref().map(|id| &id.0),"phase":phase_label(s.phase),"archived":s.archived})).collect::<Vec<_>>(),
        "status_groups":c.status_groups.iter().map(|g| json!({"id":g.id.0,"name":g.name,"archived":g.archived})).collect::<Vec<_>>(),
        "workflows":c.workflows.iter().map(|w| json!({"id":w.id.0,"status_ids":w.status_ids.iter().map(|id| &id.0).collect::<Vec<_>>(),"initial_status_id":w.initial_status_id.0,"permitted_phase_changes":w.permitted_phase_changes.iter().map(|p| json!([phase_label(p.from),phase_label(p.to)])).collect::<Vec<_>>(),"archived":w.archived})).collect::<Vec<_>>(),
        "types":c.types.iter().map(|t| json!({"id":t.id.0,"permitted_workflows":t.permitted_workflows.iter().map(|id| &id.0).collect::<Vec<_>>(),"default_workflow_id":t.default_workflow_id.0,"title_field_id":t.title_field_id.as_ref().map(|id| &id.0),"archived":t.archived})).collect::<Vec<_>>(),
        "fields":c.fields.iter().map(|f| json!({"id":f.id.0,"owner_type":f.owner_type.0,"name":f.name,"kind":kind_label(f.kind),"origin":origin_label(f.origin),"usage":usage_label(f.usage),"archived":f.archived,"options":f.options.iter().map(|o| json!({"id":o.id.0,"name":o.name,"archived":o.archived})).collect::<Vec<_>>()})).collect::<Vec<_>>(),
        "relationship_types":c.relationship_types.iter().map(|r| json!({"owner_project_id":r.owner_project_id.0,"id":r.id.0,"name":r.name,"direction":direction_label(r.direction),"archived":r.archived})).collect::<Vec<_>>()})
}

fn decode_configuration(v: &Value) -> Outcome<Configuration> {
    object(
        v,
        &[
            "project_id",
            "revision",
            "project_archived",
            "statuses",
            "status_groups",
            "workflows",
            "types",
            "fields",
            "relationship_types",
        ],
    )?;
    Ok(Configuration {
        project_id: ProjectId(string(&v["project_id"])?),
        revision: unsigned(&v["revision"])?,
        project_archived: boolean(&v["project_archived"])?,
        statuses: list(&v["statuses"], |s| {
            object(s, &["id", "name", "group_id", "phase", "archived"])?;
            Ok(Status {
                id: StatusId(string(&s["id"])?),
                name: string(&s["name"])?,
                group_id: optional(&s["group_id"])?.map(StatusGroupId),
                phase: phase(&s["phase"])?,
                archived: boolean(&s["archived"])?,
            })
        })?,
        status_groups: list(&v["status_groups"], |g| {
            object(g, &["id", "name", "archived"])?;
            Ok(StatusGroup {
                id: StatusGroupId(string(&g["id"])?),
                name: string(&g["name"])?,
                archived: boolean(&g["archived"])?,
            })
        })?,
        workflows: list(&v["workflows"], |w| {
            object(
                w,
                &[
                    "id",
                    "status_ids",
                    "initial_status_id",
                    "permitted_phase_changes",
                    "archived",
                ],
            )?;
            Ok(Workflow {
                id: WorkflowId(string(&w["id"])?),
                status_ids: list(&w["status_ids"], |id| string(id).map(StatusId))?,
                initial_status_id: StatusId(string(&w["initial_status_id"])?),
                permitted_phase_changes: list(&w["permitted_phase_changes"], |p| {
                    let pair = p.as_array().ok_or("phase change requires array")?;
                    if pair.len() != 2 {
                        return Err("phase change requires pair".into());
                    }
                    Ok(PhaseChange::new(phase(&pair[0])?, phase(&pair[1])?))
                })?,
                archived: boolean(&w["archived"])?,
            })
        })?,
        types: list(&v["types"], |t| {
            object(
                t,
                &[
                    "id",
                    "permitted_workflows",
                    "default_workflow_id",
                    "title_field_id",
                    "archived",
                ],
            )?;
            Ok(WorkItemType {
                id: WorkItemTypeId(string(&t["id"])?),
                permitted_workflows: list(&t["permitted_workflows"], |id| {
                    string(id).map(WorkflowId)
                })?,
                default_workflow_id: WorkflowId(string(&t["default_workflow_id"])?),
                title_field_id: optional(&t["title_field_id"])?.map(FieldId),
                archived: boolean(&t["archived"])?,
            })
        })?,
        fields: list(&v["fields"], |f| {
            object(
                f,
                &[
                    "id",
                    "owner_type",
                    "name",
                    "kind",
                    "origin",
                    "usage",
                    "archived",
                    "options",
                ],
            )?;
            Ok(FieldDefinition {
                id: FieldId(string(&f["id"])?),
                owner_type: WorkItemTypeId(string(&f["owner_type"])?),
                name: string(&f["name"])?,
                kind: kind(&f["kind"])?,
                origin: origin(&f["origin"])?,
                usage: usage(&f["usage"])?,
                archived: boolean(&f["archived"])?,
                options: list(&f["options"], |o| {
                    object(o, &["id", "name", "archived"])?;
                    Ok(ChoiceOption {
                        id: OptionId(string(&o["id"])?),
                        name: string(&o["name"])?,
                        archived: boolean(&o["archived"])?,
                    })
                })?,
            })
        })?,
        relationship_types: list(&v["relationship_types"], |r| {
            object(
                r,
                &["owner_project_id", "id", "name", "direction", "archived"],
            )?;
            Ok(RelationshipType {
                owner_project_id: ProjectId(string(&r["owner_project_id"])?),
                id: RelationshipTypeId(string(&r["id"])?),
                name: string(&r["name"])?,
                direction: direction(&r["direction"])?,
                archived: boolean(&r["archived"])?,
            })
        })?,
    })
}

pub fn encode_result(result: &ProjectCreateResult) -> Value {
    let p = &result.project;
    json!({"codec_version":1,"kind":"project_create_result","value":{"seed_profile":match result.seed_profile { SeedProfile::M1V1=>"m1_v1" },"project":{"id":p.id.0,"name":p.name,"prefix":p.prefix,"configuration_revision":p.configuration_revision.to_string(),"next_sequence":p.next_sequence.to_string(),"archived":p.archived,"estimate_unit":unit_label(p.estimate_unit),"ever_estimated":p.ever_estimated},"configuration":encode_configuration(&result.configuration)}})
}
fn envelope(v: &Value, expected_kind: &str) -> Outcome<()> {
    object(v, &["codec_version", "kind", "value"])?;
    if v["codec_version"].as_u64() != Some(1) || v["kind"].as_str() != Some(expected_kind) {
        return Err("unsupported codec version/kind".into());
    }
    Ok(())
}
pub fn decode_result(v: &Value) -> Outcome<ProjectCreateResult> {
    envelope(v, "project_create_result")?;
    let v = &v["value"];
    object(v, &["seed_profile", "project", "configuration"])?;
    if v["seed_profile"].as_str() != Some("m1_v1") {
        return Err("unknown seed profile".into());
    }
    let p = &v["project"];
    object(
        p,
        &[
            "id",
            "name",
            "prefix",
            "configuration_revision",
            "next_sequence",
            "archived",
            "estimate_unit",
            "ever_estimated",
        ],
    )?;
    Ok(ProjectCreateResult {
        project: Project {
            id: ProjectId(string(&p["id"])?),
            name: string(&p["name"])?,
            prefix: string(&p["prefix"])?,
            configuration_revision: unsigned(&p["configuration_revision"])?,
            next_sequence: unsigned(&p["next_sequence"])?,
            archived: boolean(&p["archived"])?,
            estimate_unit: unit(&p["estimate_unit"])?,
            ever_estimated: boolean(&p["ever_estimated"])?,
        },
        configuration: decode_configuration(&v["configuration"])?,
        seed_profile: SeedProfile::M1V1,
    })
}
pub fn encode_snapshot(configuration: &Configuration) -> Value {
    json!({"codec_version":1,"kind":"configuration_snapshot","value":encode_configuration(configuration)})
}
pub fn decode_snapshot(v: &Value) -> Outcome<Configuration> {
    envelope(v, "configuration_snapshot")?;
    decode_configuration(&v["value"])
}

pub fn proposed_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.contains('\0')
}
