use selection_codec::{decode_request, encode_request, fingerprint, verify_request, Error};
use task_contract::{
    current_work::{SelectionCommand, UserId},
    relationship::EndpointIdentity,
    work_item::{ProjectId, WorkItemId},
};

fn command() -> SelectionCommand {
    SelectionCommand {
        user_id: UserId("owner".into()),
        operation_id: "0123456789abcdef0123456789abcdef".into(),
        expected_version: 7,
        requested: Some(EndpointIdentity {
            project_id: ProjectId("project".into()),
            item_id: WorkItemId("item".into()),
        }),
    }
}

#[test]
fn version_one_frame_is_exact_and_user_scoped() {
    let request = command();
    let bytes = encode_request(&request, 1).unwrap();
    let mut expected = b"kehila\0selection\0\0\0\0\x01".to_vec();
    for field in [&request.user_id.0, &request.operation_id] {
        expected.extend_from_slice(&(field.len() as u32).to_be_bytes());
        expected.extend_from_slice(field.as_bytes());
    }
    expected.extend_from_slice(&7u64.to_be_bytes());
    expected.push(1);
    for field in ["project", "item"] {
        expected.extend_from_slice(&(field.len() as u32).to_be_bytes());
        expected.extend_from_slice(field.as_bytes());
    }
    assert_eq!(bytes, expected);
    let digest: String = fingerprint(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(
        digest,
        "a72b12179c4d9db9c5901da14cec3c9ed424ef48ff520328d4fe329c936bb009"
    );
    assert_eq!(verify_request(1, &bytes, &fingerprint(&bytes)), Ok(request));
}

#[test]
fn clear_is_an_explicit_null_discriminant_and_versions_are_full_u64() {
    for version in [0, 1, u64::MAX] {
        let mut request = command();
        request.expected_version = version;
        request.requested = None;
        let bytes = encode_request(&request, 1).unwrap();
        assert_eq!(bytes.last(), Some(&0));
        assert_eq!(decode_request(1, &bytes), Ok(request));
    }
}

#[test]
fn every_identity_or_payload_change_changes_the_canonical_fingerprint() {
    let original = command();
    let bytes = encode_request(&original, 1).unwrap();
    let digest = fingerprint(&bytes);
    let mut variants = vec![original.clone(); 6];
    variants[0].user_id.0 = "another-owner".into();
    variants[1].operation_id = "1123456789abcdef0123456789abcdef".into();
    variants[2].expected_version += 1;
    variants[3].requested = None;
    variants[4]
        .requested
        .as_mut()
        .unwrap()
        .project_id
        .0
        .push('2');
    variants[5].requested.as_mut().unwrap().item_id.0.push('2');
    for request in variants {
        let changed = encode_request(&request, 1).unwrap();
        assert_ne!(fingerprint(&changed), digest);
        assert_eq!(
            verify_request(1, &changed, &digest),
            Err(Error::DigestMismatch)
        );
    }
}

#[test]
fn truncation_unknown_discriminants_invalid_utf8_and_trailing_bytes_reject() {
    let bytes = encode_request(&command(), 1).unwrap();
    for end in 0..bytes.len() {
        assert!(decode_request(1, &bytes[..end]).is_err());
    }
    let mut changed = bytes.clone();
    changed.push(0);
    assert!(decode_request(1, &changed).is_err());
    let mut changed = bytes.clone();
    changed[b"kehila\0selection\0\0\0\0\x01".len() + 4] = 0xff;
    assert!(decode_request(1, &changed).is_err());
    let mut clear = command();
    clear.requested = None;
    let mut changed = encode_request(&clear, 1).unwrap();
    *changed.last_mut().unwrap() = 2;
    assert!(decode_request(1, &changed).is_err());
    assert_eq!(decode_request(2, &bytes), Err(Error::UnsupportedVersion(2)));
    assert_eq!(
        encode_request(&command(), 0),
        Err(Error::UnsupportedVersion(0))
    );
}

#[test]
fn encoding_preserves_exact_ids_and_enforces_the_fixed_token_shape() {
    for value in [" ".to_owned(), "e\u{301}".into(), "\u{e9}".repeat(64)] {
        let mut request = command();
        request.user_id.0 = value.clone();
        request.requested.as_mut().unwrap().project_id.0 = value.clone();
        request.requested.as_mut().unwrap().item_id.0 = value;
        let bytes = encode_request(&request, 1).unwrap();
        assert_eq!(decode_request(1, &bytes), Ok(request));
    }
    for field in ["", "a\0b", &"x".repeat(129), &"\u{e9}".repeat(65)] {
        for which in 0..3 {
            let mut request = command();
            match which {
                0 => request.user_id.0 = field.into(),
                1 => request.requested.as_mut().unwrap().project_id.0 = field.into(),
                _ => request.requested.as_mut().unwrap().item_id.0 = field.into(),
            }
            assert!(encode_request(&request, 1).is_err());
        }
    }
    for token in [
        "",
        "op",
        "0123456789ABCDEF0123456789ABCDEF",
        &"0".repeat(33),
    ] {
        let mut request = command();
        request.operation_id = token.into();
        assert!(encode_request(&request, 1).is_err());
    }
}

#[test]
fn record_bounds_apply_before_length_directed_allocation() {
    assert_eq!(decode_request(1, &vec![0; 4097]), Err(Error::LimitExceeded));
    let mut bytes = b"kehila\0selection\0\0\0\0\x01".to_vec();
    bytes.extend_from_slice(&u32::MAX.to_be_bytes());
    assert!(decode_request(1, &bytes).is_err());
}
