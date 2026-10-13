//! Fixed revision-row relationships and PostgreSQL JSONB admission, without I/O.
use crate::{decode_project_snapshot, decode_snapshot, limits, v1, Error, MAX_RECORD_BYTES};
use serde_json::Value;
use task_contract::project::Project;
use task_contract::project_create::ProjectCreateResult;
use task_contract::work_item::{Configuration, ProjectId};

fn coherent(id: &ProjectId, revision: u64, p: &Project, c: &Configuration) -> Result<(), Error> {
    if &p.id != id || &c.project_id != id {
        return Err(Error::Malformed("snapshot project identity mismatch"));
    }
    if p.configuration_revision != revision || c.revision != revision {
        return Err(Error::Malformed("snapshot revision mismatch"));
    }
    if p.archived != c.project_archived {
        return Err(Error::Malformed("snapshot archival mismatch"));
    }
    Ok(())
}

/// Decode a recorded pair under one stored version and verify fixed row/pair
/// relationships. Does not validate current business rules, current SQL scalar
/// admission, authenticity or equality to a mutable current Project projection.
pub fn verify_revision_snapshots(
    project_id: &ProjectId,
    revision: u64,
    codec_version: u32,
    project_bytes: &[u8],
    configuration_bytes: &[u8],
) -> Result<(Project, Configuration), Error> {
    let project = decode_project_snapshot(codec_version, project_bytes)?;
    let configuration = decode_snapshot(codec_version, configuration_bytes)?;
    coherent(project_id, revision, &project, &configuration)?;
    Ok((project, configuration))
}

/// Pair encoded from one result, admitted for bounded PostgreSQL 16 JSONB text
/// readback. Private bytes prevent mutation after admission. This is not proof of
/// business acceptance, SQL row validity, authentication or transactional writes.
#[derive(Debug)]
pub struct NativeRevisionSnapshots {
    project: Vec<u8>,
    configuration: Vec<u8>,
}
impl NativeRevisionSnapshots {
    pub fn codec_version(&self) -> u32 {
        1
    }
    pub fn project(&self) -> &[u8] {
        &self.project
    }
    pub fn configuration(&self) -> &[u8] {
        &self.configuration
    }
}

// PostgreSQL 16 jsonb text output adds one space after every colon/comma.
// These typed envelopes contain only strings, booleans, null and integer version
// 1: string escaping has the same byte length as serde_json's compact output.
// Count structure, not punctuation inside strings. Reject NUL (including nested
// identifiers/names), which PostgreSQL text/jsonb cannot represent.
fn jsonb_spaces(value: &Value) -> Result<usize, Error> {
    match value {
        Value::String(s) if s.contains('\0') => Err(Error::Malformed("JSONB cannot store NUL")),
        Value::Object(fields) => {
            let mut spaces = fields.len() + fields.len().saturating_sub(1);
            for child in fields.values() {
                spaces += jsonb_spaces(child)?;
            }
            Ok(spaces)
        }
        Value::Array(values) => {
            let mut spaces = values.len().saturating_sub(1);
            for child in values {
                spaces += jsonb_spaces(child)?;
            }
            Ok(spaces)
        }
        _ => Ok(0),
    }
}

fn admit_jsonb(value: &Value) -> Result<Vec<u8>, Error> {
    let bytes = limits::serialize(value)?;
    if jsonb_spaces(value)? > MAX_RECORD_BYTES - bytes.len() {
        return Err(Error::LimitExceeded);
    }
    Ok(bytes)
}

/// Admit generated snapshots before database access. The caller must supply a
/// trusted accepted result and independently enforce SQL/actor/routing contracts.
/// Reserves JSONB output whitespace under the existing 1 MiB read ceiling and
/// rejects NUL without changing historical codec acceptance.
pub fn encode_native_revision_snapshots(
    result: &ProjectCreateResult,
) -> Result<NativeRevisionSnapshots, Error> {
    coherent(
        &result.project.id,
        result.project.configuration_revision,
        &result.project,
        &result.configuration,
    )?;
    limits::project(&result.project)?;
    limits::configuration(&result.configuration)?;
    Ok(NativeRevisionSnapshots {
        project: admit_jsonb(&v1::encode_project_snapshot(&result.project))?,
        configuration: admit_jsonb(&v1::encode_snapshot(&result.configuration))?,
    })
}
