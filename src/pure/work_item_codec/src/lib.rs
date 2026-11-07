//! Pure versioned WorkItem representations. No authentication or database I/O.
pub use project_create_codec::{fingerprint, parse_json, Error};
use task_contract::{current_work::UserId, item_mutation::*, work_item::*};
mod v1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Request {
    Create {
        project_id: ProjectId,
        command: CreateCommand,
    },
    Edit {
        project_id: ProjectId,
        item_id: WorkItemId,
        command: EditCommand,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SavedResult {
    Create(CreateResult),
    Edit(EditResult),
}

/// Permanent history is independent of expiring replay payloads. The adapter
/// verifies every attribution and transition against protected database facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Revision {
    pub actor_id: UserId,
    pub configuration_revision: u64,
    pub project_state_version: u64,
    pub previous_project_operation: u64,
    pub result: SavedResult,
}

/// Checked native representation bytes, not evidence of domain acceptance.
#[derive(Debug, Eq, PartialEq)]
pub struct NativeRecord(Vec<u8>);
impl NativeRecord {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

fn version(version: u32) -> Result<(), Error> {
    if version == 1 {
        Ok(())
    } else {
        Err(Error::UnsupportedVersion(version))
    }
}
pub fn encode_request(request: &Request, codec_version: u32) -> Result<Vec<u8>, Error> {
    version(codec_version)?;
    v1::request_preflight(request, &mut project_create_codec::RecordBudget::default())?;
    project_create_codec::composition::encode_record(&v1::request(request))
}
pub fn admit_request(request: &Request) -> Result<NativeRecord, Error> {
    v1::request_preflight(request, &mut project_create_codec::RecordBudget::native())?;
    project_create_codec::composition::encode_native_record(&v1::request(request)).map(NativeRecord)
}
pub fn decode_request(codec_version: u32, bytes: &[u8]) -> Result<Request, Error> {
    version(codec_version)?;
    v1::decode_request(&parse_json(bytes)?)
}
/// Persist canonical encode_request bytes and fingerprint those bytes. Alternate
/// JSON spellings may decode equally but are not canonical stored request bytes.
pub fn verify_request(
    codec_version: u32,
    bytes: &[u8],
    digest: &[u8; 32],
) -> Result<Request, Error> {
    let request = decode_request(codec_version, bytes)?;
    let canonical = encode_request(&request, codec_version)?;
    if canonical != bytes || &fingerprint(&canonical) != digest {
        return Err(Error::DigestMismatch);
    }
    Ok(request)
}
pub fn encode_result(result: &SavedResult, codec_version: u32) -> Result<Vec<u8>, Error> {
    version(codec_version)?;
    v1::result_preflight(result, &mut project_create_codec::RecordBudget::default())?;
    project_create_codec::composition::encode_record(&v1::result(result)?)
}
pub fn decode_result(codec_version: u32, bytes: &[u8]) -> Result<SavedResult, Error> {
    version(codec_version)?;
    v1::decode_result(&parse_json(bytes)?)
}
pub fn admit_result(result: &SavedResult) -> Result<NativeRecord, Error> {
    v1::result_preflight(result, &mut project_create_codec::RecordBudget::native())?;
    project_create_codec::composition::encode_native_record(&v1::result(result)?).map(NativeRecord)
}
pub fn encode_revision(revision: &Revision, codec_version: u32) -> Result<Vec<u8>, Error> {
    version(codec_version)?;
    v1::revision_preflight(revision, &mut project_create_codec::RecordBudget::default())?;
    project_create_codec::composition::encode_record(&v1::revision(revision)?)
}
pub fn decode_revision(codec_version: u32, bytes: &[u8]) -> Result<Revision, Error> {
    version(codec_version)?;
    v1::decode_revision(&parse_json(bytes)?)
}
pub fn admit_revision(revision: &Revision) -> Result<NativeRecord, Error> {
    v1::revision_preflight(revision, &mut project_create_codec::RecordBudget::native())?;
    project_create_codec::composition::encode_native_record(&v1::revision(revision)?)
        .map(NativeRecord)
}
