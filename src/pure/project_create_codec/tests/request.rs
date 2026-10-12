use project_create_codec::*;
use serde_json::Value;
use task_contract::project::EstimateUnit;
use task_contract::project_create::ProjectCreateCommand;
use task_contract::work_item::ProjectId;

fn command(v: &Value) -> ProjectCreateCommand {
    ProjectCreateCommand {
        operation_id: v["operation_id"].as_str().unwrap().into(),
        project_id: ProjectId(v["project_id"].as_str().unwrap().into()),
        name: v["name"].as_str().unwrap().into(),
        prefix: v["prefix"].as_str().unwrap().into(),
        estimate_unit: if v["estimate_unit"] == "hours" {
            EstimateUnit::Hours
        } else {
            EstimateUnit::Points
        },
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn fixture() -> ProjectCreateCommand {
    command(
        &serde_json::json!({"operation_id":"op","project_id":"project","name":"Project","prefix":"QA","estimate_unit":"hours"}),
    )
}

#[test]
fn frozen_request_bytes_and_fingerprints() {
    let cases: Value = serde_json::from_str(include_str!(
        "../../../../experiments/SP-003/fixtures/request-v1.json"
    ))
    .unwrap();
    for case in cases.as_array().unwrap() {
        let request = command(&case["request"]);
        let bytes = encode_request(&request, 1).unwrap();
        assert_eq!(hex(&bytes), case["hex"]);
        assert_eq!(hex(&fingerprint(&bytes)), case["sha256"]);
        assert_eq!(
            verify_request(1, &bytes, &fingerprint(&bytes)).unwrap(),
            request
        );
    }
}

#[test]
fn request_corruption_and_version_dispatch() {
    let bytes = encode_request(&fixture(), 1).unwrap();
    for end in 0..bytes.len() {
        assert!(decode_request(1, &bytes[..end]).is_err());
    }
    let mut altered = bytes.clone();
    altered.push(0);
    assert!(decode_request(1, &altered).is_err());
    let mut altered = bytes.clone();
    *altered.last_mut().unwrap() = 2;
    assert!(decode_request(1, &altered).is_err());
    let mut altered = bytes.clone();
    altered[b"kehila\0project_create\0\x00\x00\x00\x01".len() + 4] = 255;
    assert!(decode_request(1, &altered).is_err());
    assert_eq!(
        verify_request(1, &bytes, &[0; 32]),
        Err(Error::DigestMismatch)
    );
    assert_eq!(decode_request(2, &bytes), Err(Error::UnsupportedVersion(2)));
    assert_eq!(
        encode_request(&fixture(), 0),
        Err(Error::UnsupportedVersion(0))
    );
}

#[test]
fn historical_requests_are_lossless_and_bounded_independently_of_admission() {
    let mut request = fixture();
    for name in ["e\u{301}", "\u{e9}", " ", "historical\0value"] {
        request.name = name.into();
        let bytes = encode_request(&request, 1).unwrap();
        assert_eq!(decode_request(1, &bytes).unwrap(), request);
    }
    request.name = "x".repeat(MAX_STRING_BYTES);
    assert!(encode_request(&request, 1).is_ok());
    request.name.push('x');
    assert_eq!(encode_request(&request, 1), Err(Error::LimitExceeded));
    assert_eq!(
        decode_request(1, &vec![0; MAX_RECORD_BYTES + 1]),
        Err(Error::LimitExceeded)
    );
}

#[test]
fn native_admission_preserves_identifier_bytes_and_rejects_unpersistable_requests() {
    let mut request = fixture();
    for id in [" ".to_owned(), "\u{e9}".repeat(64), "x".repeat(128)] {
        request.project_id = ProjectId(id);
        assert_eq!(
            NativeRequest::new(request.clone()).unwrap().command(),
            &request
        );
    }
    for id in [
        "".to_owned(),
        "x".repeat(129),
        "\u{e9}".repeat(65),
        "a\0b".into(),
    ] {
        request.project_id = ProjectId(id);
        assert_eq!(
            NativeRequest::new(request.clone()),
            Err(Error::InvalidNativeRequest)
        );
    }
    request = fixture();
    for token in ["", " ", "a\0b", "\u{e9}"] {
        request.operation_id = token.into();
        assert_eq!(
            NativeRequest::new(request.clone()),
            Err(Error::InvalidNativeRequest)
        );
    }
    request = fixture();
    request.name = "x".repeat(257);
    assert_eq!(
        NativeRequest::new(request),
        Err(Error::InvalidNativeRequest)
    );
}
