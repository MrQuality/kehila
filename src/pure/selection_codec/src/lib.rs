//! Versioned current-work selection representations, independent of I/O.
//!
//! Historical reconstruction never evaluates current domain eligibility,
//! authentication or grants. The adapter owns those independent boundaries.
pub use project_create_codec::{fingerprint, Error};
use task_contract::current_work::{CurrentWork, SelectionResult};

mod request;
mod state;
pub use request::{decode_request, encode_request, verify_request, NativeRequest};
pub use request::{identifier as valid_identifier, token as valid_operation_token};

pub const MAX_WIRE_BYTES: usize = 4096;
pub const MAX_HISTORY_BYTES: usize = 16384;

/// Permanent transition representation. Its ordinal is independent of the
/// selection version: a successful no-op still has a distinct accepted event.
/// SQL attribution and typed adapter checks establish transition consistency.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    pub ordinal: u64,
    pub operation_id: String,
    pub before: CurrentWork,
    pub result: SelectionResult,
}

fn version(value: u32) -> Result<(), Error> {
    if value == 1 {
        Ok(())
    } else {
        Err(Error::UnsupportedVersion(value))
    }
}

pub fn encode_result(result: &SelectionResult, codec_version: u32) -> Result<Vec<u8>, Error> {
    version(codec_version)?;
    state::encode_result(result)
}

pub fn decode_result(codec_version: u32, bytes: &[u8]) -> Result<SelectionResult, Error> {
    version(codec_version)?;
    state::decode_result(bytes)
}

pub fn encode_event(event: &Event, codec_version: u32) -> Result<Vec<u8>, Error> {
    version(codec_version)?;
    state::encode_event(event)
}

pub fn decode_event(codec_version: u32, bytes: &[u8]) -> Result<Event, Error> {
    version(codec_version)?;
    state::decode_event(bytes)
}
