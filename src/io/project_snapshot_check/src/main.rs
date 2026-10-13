//! Host-side native test oracle. Process I/O stays outside the pure codec crate.
use project_create_codec::*;
use serde_json::{json, Value};
use std::io::{self, Read, Write};
use task_contract::project::EstimateUnit;
use task_contract::project_create::*;
use task_contract::work_item::ProjectId;

const FRAME_LIMIT: usize = (MAX_RECORD_BYTES + MAX_NODES * 2) * 4 + 4096;

fn fixture(label: &str) -> Result<ProjectCreateResult, &'static str> {
    let command = ProjectCreateCommand {
        operation_id: "snapshot-bridge".into(),
        project_id: ProjectId("snapshot-bridge".into()),
        name: "Bridge Ω é e\u{301}".into(),
        prefix: "SB".into(),
        estimate_unit: EstimateUnit::Hours,
    };
    let ProjectCreateDecision::Apply(mut r) = decide_project_create(false, None, true, &command)
    else {
        return Err("fixture decision failed");
    };
    let revision = match label {
        "zero" => 0,
        "wide" => (1 << 53) + 1,
        "maximum" => u64::MAX,
        "seed" | "ordered" | "escaped" | "limit" | "overflow" | "nul" => 1,
        _ => return Err("unknown fixture"),
    };
    r.project.configuration_revision = revision;
    r.project.next_sequence = revision;
    r.configuration.revision = revision;
    if label == "ordered" {
        r.configuration.statuses.reverse();
        r.configuration.fields.reverse();
        r.configuration.workflows[0].status_ids.reverse();
        r.project.estimate_unit = EstimateUnit::Points;
        r.project.archived = true;
        r.configuration.project_archived = true;
        r.project.ever_estimated = true;
    }
    if label == "escaped" {
        r.project.name = "comma,: quote\" slash\\ tab\t newline\n backspace\x08 control\x01".into();
    }
    if label == "nul" {
        r.configuration.fields[0].name.push('\0');
    }
    if label == "limit" || label == "overflow" {
        r.configuration.statuses = vec![r.configuration.statuses[0].clone(); 16];
        for status in &mut r.configuration.statuses {
            status.name = "x".repeat(65_500);
        }
        let mut low = 0;
        let mut high = MAX_STRING_BYTES;
        while low < high {
            let n = (low + high).div_ceil(2);
            r.configuration.statuses[15].name = "x".repeat(n);
            if encode_native_revision_snapshots(&r).is_ok() {
                low = n;
            } else {
                high = n - 1;
            }
        }
        r.configuration.statuses[15].name = "x".repeat(low + usize::from(label == "overflow"));
    }
    Ok(r)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(encoded, "{b:02x}").expect("String formatting cannot fail");
    }
    encoded
}
fn unhex(value: &Value) -> Result<Vec<u8>, &'static str> {
    let s = value.as_str().ok_or("missing hex string")?;
    if s.len() % 2 != 0 || s.len() > 2 * (MAX_RECORD_BYTES + 2 * MAX_NODES) {
        return Err("invalid hex length");
    }
    s.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digit = |b| match b {
                b'0'..=b'9' => Ok(b - b'0'),
                b'a'..=b'f' => Ok(b - b'a' + 10),
                _ => Err("invalid hex"),
            };
            Ok(digit(pair[0])? * 16 + digit(pair[1])?)
        })
        .collect()
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("expected fixture|verify and fixture name".into());
    }
    let r = fixture(&args[1])?;
    match args[0].as_str() {
        "fixture" => {
            let p = encode_project_snapshot(&r.project)?;
            let c = encode_snapshot(&r.configuration)?;
            let native = encode_native_revision_snapshots(&r);
            if args[1] == "nul" || args[1] == "overflow" {
                if native.is_ok() {
                    return Err("incompatible fixture admitted".into());
                }
            } else {
                native?;
            }
            let frame = serde_json::to_vec(&json!({
                "project": hex(&p), "configuration": hex(&c),
                "revision": r.configuration.revision.to_string(),
                "project_id": r.project.id.0, "codec_version": 1,
            }))?;
            if frame.len() > FRAME_LIMIT {
                return Err("fixture output overflow".into());
            }
            io::stdout().lock().write_all(&frame)?;
        }
        "verify" => {
            let mut bytes = Vec::new();
            io::stdin()
                .lock()
                .take(FRAME_LIMIT as u64 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() > FRAME_LIMIT {
                return Err("input frame overflow".into());
            }
            let frame: Value = serde_json::from_slice(&bytes)?;
            let object = frame.as_object().ok_or("expected row frame")?;
            if object.len() != 5 {
                return Err("unexpected row fields".into());
            }
            let id = ProjectId(
                frame["project_id"]
                    .as_str()
                    .ok_or("missing project ID")?
                    .into(),
            );
            let revision = frame["revision"]
                .as_str()
                .ok_or("missing revision")?
                .parse::<u64>()?;
            let version = u32::try_from(frame["codec_version"].as_u64().ok_or("missing version")?)?;
            let actual = verify_revision_snapshots(
                &id,
                revision,
                version,
                &unhex(&frame["project"])?,
                &unhex(&frame["configuration"])?,
            )?;
            if actual != (r.project, r.configuration) {
                return Err("typed fixture equality mismatch".into());
            }
            println!("PASS: exact recorded project/configuration pair");
        }
        _ => return Err("unknown command".into()),
    }
    Ok(())
}

fn main() {
    if run().is_err() {
        // Do not print record contents from serde/SQL errors.
        eprintln!("ERR_SNAPSHOT_ORACLE: fixture, frame, codec or equality failure");
        std::process::exit(3);
    }
}
