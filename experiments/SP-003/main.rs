//! Host oracle: JSON input/output is experimental and does not expose a route.
mod codec;

use codec::*;
use serde_json::{json, Value};
use std::io::{self, Read};
use task_contract::project::EstimateUnit;
use task_contract::project_create::*;
use task_contract::work_item::ProjectId;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(value: &str) -> Result<Vec<u8>, String> {
    if value.len() % 2 != 0 || !value.is_ascii() {
        return Err("invalid hex".into());
    }
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).map_err(|_| "invalid hex".into()))
        .collect()
}
fn command(v: &Value) -> Result<ProjectCreateCommand, String> {
    let text = |key: &str| {
        v[key]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("missing string {key}"))
    };
    Ok(ProjectCreateCommand {
        operation_id: text("operation_id")?,
        project_id: ProjectId(text("project_id")?),
        name: text("name")?,
        prefix: text("prefix")?,
        estimate_unit: match v["estimate_unit"].as_str() {
            Some("hours") => EstimateUnit::Hours,
            Some("points") => EstimateUnit::Points,
            _ => return Err("invalid unit".into()),
        },
    })
}
fn request_json(c: &ProjectCreateCommand) -> Value {
    json!({"operation_id":c.operation_id,"project_id":c.project_id.0,"name":c.name,"prefix":c.prefix,"estimate_unit":match c.estimate_unit { EstimateUnit::Hours=>"hours",EstimateUnit::Points=>"points" }})
}
fn execute(action: &str, v: &Value) -> Result<Value, String> {
    match action {
        "request" => {
            let c = command(v)?;
            let bytes = encode_request(&c, 1)?;
            Ok(
                json!({"hex":hex(&bytes),"sha256":hex(&digest(&bytes)),"request":request_json(&decode_request(&bytes)?)}),
            )
        }
        "decode-request" => Ok(request_json(&decode_request(&unhex(
            v.as_str().ok_or("expected hex string")?,
        )?)?)),
        "result" => Ok(encode_result(&decode_result(v)?)),
        "snapshot" => Ok(encode_snapshot(&decode_snapshot(v)?)),
        "fixture" => {
            let c = command(v)?;
            let ProjectCreateDecision::Apply(r) = decide_project_create(false, None, true, &c)
            else {
                return Err("pure command rejected".into());
            };
            Ok(json!({"result":encode_result(&r),"snapshot":encode_snapshot(&r.configuration)}))
        }
        "id" => {
            let id = v.as_str().ok_or("expected id string")?;
            let c = ProjectCreateCommand {
                operation_id: "op".into(),
                project_id: ProjectId(id.into()),
                name: "Project".into(),
                prefix: "QA".into(),
                estimate_unit: EstimateUnit::Hours,
            };
            Ok(
                json!({"proposed":proposed_id(id),"pure_create":matches!(decide_project_create(false,None,true,&c),ProjectCreateDecision::Apply(_))}),
            )
        }
        _ => Err("unknown action".into()),
    }
}
fn main() {
    let result = (|| {
        let args: Vec<_> = std::env::args().collect();
        if args.len() != 2 {
            return Err("expected exactly one action".into());
        }
        let mut input = String::new();
        io::stdin()
            .take(2 * 1024 * 1024 + 1)
            .read_to_string(&mut input)
            .map_err(|_| "stdin read failed")?;
        if input.len() > 2 * 1024 * 1024 {
            return Err("input exceeds spike budget".into());
        }
        let value: Value = serde_json::from_str(&input).map_err(|_| "invalid JSON")?;
        execute(&args[1], &value)
    })();
    match result {
        Ok(v) => println!("{v}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests;
