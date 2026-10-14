//! Host-side native test oracle. Process I/O stays outside the pure codec crate.
use project_create_codec::*;
use serde::Deserialize;
use serde_json::json;
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
        // Representation-stress fixture, deliberately outside live domain/SQL
        // scalar acceptance. This frozen shape adds 283 JSONB formatting spaces.
        r.configuration.statuses[15].name.clear();
        let base = encode_snapshot(&r.configuration)
            .map_err(|_| "fixture encoding failed")?
            .len();
        let padding = MAX_RECORD_BYTES - base - 283;
        if padding + usize::from(label == "overflow") > MAX_STRING_BYTES {
            return Err("boundary fixture cannot fit string ceiling");
        }
        r.configuration.statuses[15].name = "x".repeat(padding + usize::from(label == "overflow"));
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
fn unhex(s: &str) -> Result<Vec<u8>, &'static str> {
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodedFrame {
    project: String,
    configuration: String,
    project_id: String,
    revision: String,
    codec_version: u32,
}

struct RowFrame {
    project: Vec<u8>,
    configuration: Vec<u8>,
    project_id: ProjectId,
    revision: u64,
    codec_version: u32,
}

type OracleResult<T> = Result<T, Box<dyn std::error::Error>>;

fn parse_frame(bytes: &[u8]) -> OracleResult<RowFrame> {
    if bytes.len() > FRAME_LIMIT {
        return Err("input frame overflow".into());
    }
    // Struct deserialization rejects duplicate fields before their next value,
    // as well as unknown/missing fields. This IPC frame has its own hex budget;
    // the contained storage bytes still enter the production bounded decoders.
    let frame: EncodedFrame = serde_json::from_slice(bytes)?;
    if frame.revision.is_empty()
        || !frame.revision.bytes().all(|b| b.is_ascii_digit())
        || (frame.revision.len() > 1 && frame.revision.starts_with('0'))
    {
        return Err("noncanonical row revision".into());
    }
    Ok(RowFrame {
        project: unhex(&frame.project)?,
        configuration: unhex(&frame.configuration)?,
        project_id: ProjectId(frame.project_id),
        revision: frame.revision.parse()?,
        codec_version: frame.codec_version,
    })
}

enum Expectation<'a> {
    Exact(&'a ProjectCreateResult),
    Rejected(Error),
}

fn check_frame(bytes: &[u8], expectation: Expectation<'_>) -> OracleResult<()> {
    let frame = parse_frame(bytes)?;
    let actual = verify_revision_snapshots(
        &frame.project_id,
        frame.revision,
        frame.codec_version,
        &frame.project,
        &frame.configuration,
    );
    match expectation {
        Expectation::Rejected(error) => {
            if actual.err() != Some(error) {
                return Err("expected production codec rejection absent".into());
            }
        }
        Expectation::Exact(expected) => {
            let (project, configuration) = actual?;
            if project != expected.project || configuration != expected.configuration {
                return Err("typed fixture equality mismatch".into());
            }
        }
    }
    Ok(())
}

fn rejection(mode: &str) -> Option<Error> {
    match mode {
        "reject_identity" => Some(Error::Malformed("snapshot project identity mismatch")),
        "reject_revision" => Some(Error::Malformed("snapshot revision mismatch")),
        "reject_archival" => Some(Error::Malformed("snapshot archival mismatch")),
        "reject_envelope" => Some(Error::Malformed("unsupported codec version/kind")),
        "reject_version" => Some(Error::UnsupportedVersion(2)),
        "reject_limit" => Some(Error::LimitExceeded),
        _ => None,
    }
}

fn run() -> OracleResult<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("expected fixture|verify|reject_* and fixture name".into());
    }
    match args[0].as_str() {
        "fixture" => {
            let r = fixture(&args[1])?;
            let native = encode_native_revision_snapshots(&r);
            let (p, c) = match args[1].as_str() {
                "nul" | "overflow" => {
                    let expected = if args[1] == "nul" {
                        Error::InvalidNativeSnapshot("JSONB cannot store NUL")
                    } else {
                        Error::LimitExceeded
                    };
                    if native.err() != Some(expected) {
                        return Err("expected native admission rejection absent".into());
                    }
                    (
                        encode_project_snapshot(&r.project)?,
                        encode_snapshot(&r.configuration)?,
                    )
                }
                _ => {
                    let admitted = native?;
                    (
                        admitted.project().to_vec(),
                        admitted.configuration().to_vec(),
                    )
                }
            };
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
        mode if mode == "verify" || rejection(mode).is_some() => {
            let mut bytes = Vec::new();
            io::stdin()
                .lock()
                .take(FRAME_LIMIT as u64 + 1)
                .read_to_end(&mut bytes)?;
            if let Some(error) = rejection(mode) {
                // Framing/equality failures cannot satisfy this negative oracle.
                check_frame(&bytes, Expectation::Rejected(error))?;
                println!("PASS: expected production codec rejection: {mode}");
            } else {
                let expected = fixture(&args[1])?;
                check_frame(&bytes, Expectation::Exact(&expected))?;
                println!("PASS: exact recorded project/configuration pair");
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn frame(r: &ProjectCreateResult) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "project": hex(&encode_project_snapshot(&r.project).unwrap()),
            "configuration": hex(&encode_snapshot(&r.configuration).unwrap()),
            "revision": r.configuration.revision.to_string(),
            "project_id": r.project.id.0, "codec_version": 1,
        }))
        .unwrap()
    }

    #[test]
    fn frame_rejects_duplicates_unknown_missing_fields_and_noncanonical_revisions() {
        let valid = frame(&fixture("seed").unwrap());
        assert!(parse_frame(&valid).is_ok());
        let duplicate = [b"{\"revision\":\"999\",".as_slice(), &valid[1..]].concat();
        assert!(parse_frame(&duplicate).is_err());
        for revision in ["+1", "01", "-1", "1.0", "18446744073709551616"] {
            let mut bad: Value = serde_json::from_slice(&valid).unwrap();
            bad["revision"] = json!(revision);
            assert!(parse_frame(&serde_json::to_vec(&bad).unwrap()).is_err());
        }
        let mut bad: Value = serde_json::from_slice(&valid).unwrap();
        bad["extra"] = json!(1);
        assert!(parse_frame(&serde_json::to_vec(&bad).unwrap()).is_err());
        for key in [
            "project",
            "configuration",
            "project_id",
            "revision",
            "codec_version",
        ] {
            let mut missing: Value = serde_json::from_slice(&valid).unwrap();
            missing.as_object_mut().unwrap().remove(key);
            assert!(parse_frame(&serde_json::to_vec(&missing).unwrap()).is_err());
        }
        assert!(parse_frame(&vec![b' '; FRAME_LIMIT + 1]).is_err());
        for label in ["zero", "wide", "maximum"] {
            assert!(parse_frame(&frame(&fixture(label).unwrap())).is_ok());
        }
    }

    #[test]
    fn rejection_requires_the_expected_codec_error_not_equality_or_frame_failure() {
        let expected = fixture("seed").unwrap();
        let mut changed = expected.clone();
        changed.project.name.push('x'); // Coherent, but unequal to the fixture.
        let rejection = || Expectation::Rejected(Error::Malformed("snapshot revision mismatch"));
        assert!(check_frame(&frame(&changed), rejection()).is_err());
        assert!(check_frame(b"{}", rejection()).is_err());
        changed.project.configuration_revision = 2;
        assert!(check_frame(&frame(&changed), rejection()).is_ok());
        assert!(check_frame(&frame(&changed), Expectation::Exact(&expected)).is_err());
        assert!(check_frame(&frame(&expected), Expectation::Exact(&expected)).is_ok());
    }

    #[test]
    fn deterministic_representation_stress_fixtures_bracket_native_admission() {
        assert!(encode_native_revision_snapshots(&fixture("limit").unwrap()).is_ok());
        assert_eq!(
            encode_native_revision_snapshots(&fixture("overflow").unwrap()).err(),
            Some(Error::LimitExceeded)
        );
    }
}
