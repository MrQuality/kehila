use crate::{Error, MAX_RECORD_BYTES, MAX_STRING_BYTES};
use sha2::{Digest, Sha256};
use task_contract::project::EstimateUnit;
use task_contract::project_create::{validate_project_create_input, ProjectCreateCommand};
use task_contract::work_item::ProjectId;

const HEADER: &[u8] = b"kehila\0project_create\0\x00\x00\x00\x01";

fn version(version: u32) -> Result<(), Error> {
    match version {
        1 => Ok(()),
        other => Err(Error::UnsupportedVersion(other)),
    }
}

/// Encode exact typed fields without normalization or current-domain validation.
pub fn encode_request(
    command: &ProjectCreateCommand,
    codec_version: u32,
) -> Result<Vec<u8>, Error> {
    version(codec_version)?;
    let fields = [
        &command.operation_id,
        &command.project_id.0,
        &command.name,
        &command.prefix,
    ];
    let mut size = HEADER.len() + 17;
    for field in fields {
        if field.len() > MAX_STRING_BYTES {
            return Err(Error::LimitExceeded);
        }
        size = size.checked_add(field.len()).ok_or(Error::LimitExceeded)?;
    }
    if size > MAX_RECORD_BYTES {
        return Err(Error::LimitExceeded);
    }
    let mut bytes = Vec::with_capacity(size);
    bytes.extend_from_slice(HEADER);
    for field in fields {
        let length = u32::try_from(field.len()).map_err(|_| Error::LimitExceeded)?;
        bytes.extend_from_slice(&length.to_be_bytes());
        bytes.extend_from_slice(field.as_bytes());
    }
    bytes.push(match command.estimate_unit {
        EstimateUnit::Hours => 0,
        EstimateUnit::Points => 1,
    });
    Ok(bytes)
}

fn take_string(remaining: &mut &[u8]) -> Result<String, Error> {
    let prefix = remaining
        .get(..4)
        .ok_or(Error::Malformed("truncated length"))?;
    // get(..4) above proves the fixed-size conversion.
    let length = u32::from_be_bytes(prefix.try_into().map_err(|_| Error::Malformed("length"))?);
    let length = usize::try_from(length).map_err(|_| Error::LimitExceeded)?;
    if length > MAX_STRING_BYTES {
        return Err(Error::LimitExceeded);
    }
    let field = remaining
        .get(4..4 + length)
        .ok_or(Error::Malformed("truncated string"))?;
    let value = std::str::from_utf8(field)
        .map_err(|_| Error::Malformed("UTF8"))?
        .to_owned();
    *remaining = &remaining[4 + length..];
    Ok(value)
}

/// Dispatch using the stored version and verify the embedded framing version.
pub fn decode_request(codec_version: u32, bytes: &[u8]) -> Result<ProjectCreateCommand, Error> {
    version(codec_version)?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(Error::LimitExceeded);
    }
    let mut remaining = bytes
        .strip_prefix(HEADER)
        .ok_or(Error::Malformed("request header"))?;
    let operation_id = take_string(&mut remaining)?;
    let project_id = ProjectId(take_string(&mut remaining)?);
    let name = take_string(&mut remaining)?;
    let prefix = take_string(&mut remaining)?;
    let estimate_unit = match remaining {
        [0] => EstimateUnit::Hours,
        [1] => EstimateUnit::Points,
        _ => return Err(Error::Malformed("unit or trailing bytes")),
    };
    Ok(ProjectCreateCommand {
        operation_id,
        project_id,
        name,
        prefix,
        estimate_unit,
    })
}

/// SHA-256 of the complete versioned request representation; not an authenticator.
pub fn fingerprint(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Reconstruct a retained request and require agreement with its permanent core.
pub fn verify_request(
    codec_version: u32,
    bytes: &[u8],
    expected: &[u8; 32],
) -> Result<ProjectCreateCommand, Error> {
    let request = decode_request(codec_version, bytes)?;
    if &fingerprint(bytes) != expected {
        return Err(Error::DigestMismatch);
    }
    Ok(request)
}

/// A command checked for native scalar storage and current creation validation.
/// This does not authenticate an actor, allocate a target or authorize creation.
#[derive(Debug, PartialEq, Eq)]
pub struct NativeRequest(ProjectCreateCommand);
impl NativeRequest {
    pub fn new(command: ProjectCreateCommand) -> Result<Self, Error> {
        let id = &command.project_id.0;
        if id.is_empty()
            || id.len() > 128
            || id.contains('\0')
            || validate_project_create_input(&command).is_err()
        {
            return Err(Error::InvalidNativeRequest);
        }
        Ok(Self(command))
    }
    pub fn command(&self) -> &ProjectCreateCommand {
        &self.0
    }
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        encode_request(&self.0, 1)
    }
}
