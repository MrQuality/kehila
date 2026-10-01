//! Logical field-payload bounds for M1 item mutations.
//!
//! HTTP body and storage-record limits are separate adapter concerns.

use crate::field::FieldValue;

pub const MAX_FIELD_ENTRIES: usize = 128;
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;

/// Count supplied field entries, including Keep/Clear edits, and the UTF-8
/// bytes in supplied text values. Never truncate or round an oversized input.
pub fn within_field_payload_limit<'a>(
    field_count: usize,
    values: impl IntoIterator<Item = &'a FieldValue>,
) -> bool {
    if field_count > MAX_FIELD_ENTRIES {
        return false;
    }
    let total = values.into_iter().try_fold(0usize, |total, value| {
        let bytes = match value {
            FieldValue::Text(text) => text.len(),
            _ => 0,
        };
        total
            .checked_add(bytes)
            .filter(|sum| *sum <= MAX_TEXT_BYTES)
    });
    total.is_some()
}
