//! Versioned project-creation storage representations, independent of I/O.
//!
//! Decoding reconstructs historical values; it never re-runs current domain
//! acceptance. New native writes must enter through [`NativeRequest`].
mod json;
mod request;
pub use json::parse_json;
pub use request::{decode_request, encode_request, fingerprint, verify_request, NativeRequest};

/// Admission ceilings for codec resources, not product configuration policy.
pub const MAX_STRING_BYTES: usize = 64 * 1024;
pub const MAX_RECORD_BYTES: usize = 1024 * 1024;

/// Bounded diagnostics never contain record contents or caller-supplied values.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    UnsupportedVersion(u32),
    Malformed(&'static str),
    LimitExceeded,
    DigestMismatch,
    InvalidNativeRequest,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedVersion(v) => write!(f, "unsupported codec version {v}"),
            Self::Malformed(reason) => write!(f, "malformed storage record: {reason}"),
            Self::LimitExceeded => f.write_str("codec resource limit exceeded"),
            Self::DigestMismatch => f.write_str("request fingerprint mismatch"),
            Self::InvalidNativeRequest => f.write_str("request cannot enter native storage"),
        }
    }
}
impl std::error::Error for Error {}

/// Maximum value nesting depth; the root is depth zero.
pub const MAX_DEPTH: usize = 16;
/// Maximum entries per JSON array or object.
pub const MAX_COLLECTION_ENTRIES: usize = 1024;
/// Maximum JSON values, counting containers but excluding object keys.
pub const MAX_NODES: usize = 16384;

mod limits;
mod v1;
use task_contract::project_create::ProjectCreateResult;
use task_contract::work_item::Configuration;

fn stored_json(version: u32, bytes: &[u8]) -> Result<serde_json::Value, Error> {
    match version {
        1 => parse_json(bytes),
        other => Err(Error::UnsupportedVersion(other)),
    }
}

/// Reconstruct an entire saved success using its stored version. The embedded
/// version/kind must agree; schema errors are corruption, not new domain rejects.
pub fn decode_result(version: u32, bytes: &[u8]) -> Result<ProjectCreateResult, Error> {
    v1::decode_result(&stored_json(version, bytes)?).map_err(Error::Malformed)
}
/// Encode a complete trusted result. This validates resources, not business
/// acceptance; the caller must obtain the result from authoritative domain rules.
pub fn encode_result(result: &ProjectCreateResult) -> Result<Vec<u8>, Error> {
    limits::result(result)?;
    limits::serialize(&v1::encode_result(result))
}
/// Reconstruct historical configuration without consulting current definitions.
pub fn decode_snapshot(version: u32, bytes: &[u8]) -> Result<Configuration, Error> {
    v1::decode_snapshot(&stored_json(version, bytes)?).map_err(Error::Malformed)
}
/// Preserve complete ordered configuration, including empty collections.
pub fn encode_snapshot(configuration: &Configuration) -> Result<Vec<u8>, Error> {
    limits::configuration(configuration)?;
    limits::serialize(&v1::encode_snapshot(configuration))
}
