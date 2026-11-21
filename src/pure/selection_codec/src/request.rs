use super::{fingerprint, version, Error, MAX_WIRE_BYTES};
use task_contract::{
    current_work::{SelectionCommand, UserId},
    relationship::EndpointIdentity,
    work_item::{ProjectId, WorkItemId},
};

const HEADER: &[u8] = b"kehila\0selection\0\0\0\0\x01";

pub fn identifier(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.contains('\0')
}
pub fn token(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn valid(command: &SelectionCommand) -> bool {
    identifier(&command.user_id.0)
        && token(&command.operation_id)
        && command.requested.as_ref().map_or(true, |target| {
            identifier(&target.project_id.0) && identifier(&target.item_id.0)
        })
}
fn append(bytes: &mut Vec<u8>, value: &str) {
    // Callers have already bounded the field to 128 bytes or a fixed token.
    bytes.extend_from_slice(&(value.len() as u32).to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

/// Exact version-one binary identity, without current domain acceptance.
pub fn encode_request(command: &SelectionCommand, codec_version: u32) -> Result<Vec<u8>, Error> {
    version(codec_version)?;
    if !valid(command) {
        return Err(Error::InvalidNativeRequest);
    }
    let mut bytes = Vec::with_capacity(512);
    bytes.extend_from_slice(HEADER);
    append(&mut bytes, &command.user_id.0);
    append(&mut bytes, &command.operation_id);
    bytes.extend_from_slice(&command.expected_version.to_be_bytes());
    match &command.requested {
        None => bytes.push(0),
        Some(target) => {
            bytes.push(1);
            append(&mut bytes, &target.project_id.0);
            append(&mut bytes, &target.item_id.0);
        }
    }
    Ok(bytes)
}

fn take<'a>(remaining: &mut &'a [u8], count: usize) -> Result<&'a [u8], Error> {
    let bytes = remaining
        .get(..count)
        .ok_or(Error::Malformed("truncated selection request"))?;
    *remaining = &remaining[count..];
    Ok(bytes)
}
fn string(remaining: &mut &[u8], maximum: usize) -> Result<String, Error> {
    let prefix = take(remaining, 4)?;
    let size = u32::from_be_bytes(
        prefix
            .try_into()
            .map_err(|_| Error::Malformed("selection length"))?,
    ) as usize;
    if size > maximum {
        return Err(Error::LimitExceeded);
    }
    let value = std::str::from_utf8(take(remaining, size)?)
        .map_err(|_| Error::Malformed("selection UTF8"))?;
    Ok(value.to_owned())
}

pub fn decode_request(codec_version: u32, bytes: &[u8]) -> Result<SelectionCommand, Error> {
    version(codec_version)?;
    if bytes.len() > MAX_WIRE_BYTES {
        return Err(Error::LimitExceeded);
    }
    let mut remaining = bytes
        .strip_prefix(HEADER)
        .ok_or(Error::Malformed("selection header"))?;
    let user_id = UserId(string(&mut remaining, 128)?);
    let operation_id = string(&mut remaining, 32)?;
    let expected_version = u64::from_be_bytes(
        take(&mut remaining, 8)?
            .try_into()
            .map_err(|_| Error::Malformed("selection version"))?,
    );
    let requested = match take(&mut remaining, 1)? {
        [0] => None,
        [1] => Some(EndpointIdentity {
            project_id: ProjectId(string(&mut remaining, 128)?),
            item_id: WorkItemId(string(&mut remaining, 128)?),
        }),
        _ => return Err(Error::Malformed("selection target discriminant")),
    };
    let command = SelectionCommand {
        user_id,
        operation_id,
        expected_version,
        requested,
    };
    if !remaining.is_empty() || !valid(&command) {
        return Err(Error::Malformed("selection request shape"));
    }
    Ok(command)
}

pub fn verify_request(
    codec_version: u32,
    bytes: &[u8],
    digest: &[u8; 32],
) -> Result<SelectionCommand, Error> {
    let command = decode_request(codec_version, bytes)?;
    if &fingerprint(bytes) != digest {
        return Err(Error::DigestMismatch);
    }
    Ok(command)
}
