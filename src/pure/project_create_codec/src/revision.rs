//! Fixed revision-row relationships and PostgreSQL JSONB admission, without I/O.
use crate::{decode_project_snapshot, decode_snapshot, limits, v1, Error};
use task_contract::project::Project;
use task_contract::project_create::ProjectCreateResult;
use task_contract::work_item::{Configuration, ProjectId};

fn coherent(
    id: &ProjectId,
    revision: u64,
    p: &Project,
    c: &Configuration,
) -> Result<(), &'static str> {
    if &p.id != id || &c.project_id != id {
        return Err("snapshot project identity mismatch");
    }
    if p.configuration_revision != revision || c.revision != revision {
        return Err("snapshot revision mismatch");
    }
    if p.archived != c.project_archived {
        return Err("snapshot archival mismatch");
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
    coherent(project_id, revision, &project, &configuration).map_err(Error::Malformed)?;
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
    )
    .map_err(Error::InvalidNativeSnapshot)?;
    limits::native_project(&result.project)?;
    limits::native_configuration(&result.configuration)?;
    Ok(NativeRevisionSnapshots {
        project: limits::serialize_native(&v1::encode_project_snapshot(&result.project))?,
        configuration: limits::serialize_native(&v1::encode_snapshot(&result.configuration))?,
    })
}
