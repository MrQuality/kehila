use task_contract::field::FieldValue;
use task_contract::payload::{within_field_payload_limit, MAX_FIELD_ENTRIES, MAX_TEXT_BYTES};

#[test]
fn field_payload_accepts_the_boundary_and_rejects_either_excess() {
    let at_limit = FieldValue::Text("x".repeat(MAX_TEXT_BYTES));
    assert!(within_field_payload_limit(MAX_FIELD_ENTRIES, [&at_limit]));
    assert!(!within_field_payload_limit(
        MAX_FIELD_ENTRIES + 1,
        [&at_limit]
    ));
    let one_more = FieldValue::Text("y".into());
    assert!(!within_field_payload_limit(2, [&at_limit, &one_more]));
}

#[test]
fn field_payload_counts_utf8_bytes_only_for_text_values() {
    let text = FieldValue::Text("é".repeat(MAX_TEXT_BYTES / 2));
    let number = FieldValue::Number(0);
    assert!(within_field_payload_limit(2, [&text, &number]));
    let extra = FieldValue::Text("z".into());
    assert!(!within_field_payload_limit(3, [&text, &number, &extra]));
}
