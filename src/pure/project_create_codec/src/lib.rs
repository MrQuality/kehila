//! Versioned project-creation storage representations, independent of I/O.
//!
//! Decoding reconstructs historical values; it never re-runs current domain
//! acceptance. New native writes must enter through [`NativeRequest`].
mod request;
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
