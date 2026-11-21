use selection_codec::{
    decode_request, decode_result, encode_request, encode_result, fingerprint, verify_request,
    Error,
};
use task_contract::{
    current_work::{CurrentWork, SelectionCommand, SelectionResult, UserId},
    relationship::EndpointIdentity,
    work_item::{ProjectId, WorkItemId},
};

#[path = "../../../../tests/support/seeded.rs"]
mod seeded;

fn text(generator: &mut seeded::Generator) -> String {
    let length = 1 + generator.next() as usize % 24;
    (0..length)
        .map(|_| match generator.next() % 5 {
            0 => ['"', '\\', '\n', '\t', '\u{301}'][generator.next() as usize % 5],
            1 => ['א', 'ع', '中', '🦀', '\u{202e}'][generator.next() as usize % 5],
            _ => char::from_u32(1 + (generator.next() % 0x10ffff) as u32).unwrap_or('x'),
        })
        .collect()
}

#[test]
fn seeded_unicode_binary_and_json_roundtrips_preserve_exact_identity_and_u64() {
    for seed in [1, 0x5eed, 0xa11ce] {
        let mut generator = seeded::Generator::new(seed);
        for case in 0..256 {
            let version = match case % 4 {
                0 => 0,
                1 => u64::MAX,
                _ => generator.next(),
            };
            let command = SelectionCommand {
                user_id: UserId(text(&mut generator)),
                operation_id: format!("{:016x}{:016x}", generator.next(), generator.next()),
                expected_version: version,
                requested: (case % 3 != 0).then(|| EndpointIdentity {
                    project_id: ProjectId(text(&mut generator)),
                    item_id: WorkItemId(text(&mut generator)),
                }),
            };
            let wire = encode_request(&command, 1).unwrap();
            assert_eq!(
                decode_request(1, &wire),
                Ok(command.clone()),
                "seed {seed} case {case}"
            );
            assert_eq!(
                verify_request(1, &wire, &fingerprint(&wire)),
                Ok(command.clone())
            );
            let result = SelectionResult {
                next: CurrentWork {
                    user_id: command.user_id.clone(),
                    version,
                    selected: if version == 0 {
                        None
                    } else {
                        command.requested.clone()
                    },
                },
            };
            let json = encode_result(&result, 1).unwrap();
            assert_eq!(decode_result(1, &json), Ok(result.clone()));
            let decoded: serde_json::Value = serde_json::from_slice(&json).unwrap();
            assert_eq!(decoded["value"]["version"], version.to_string());
            assert_eq!(decoded["value"]["user_id"], command.user_id.0);

            // Keep the accepted header/envelope so generated cases reach the
            // intended UTF-8, length and discriminant checks, not just header rejection.
            let header = b"kehila\0selection\0\0\0\0\x01".len();
            let version_offset = header + 4 + command.user_id.0.len() + 4 + 32;
            let mut changed_version = wire.clone();
            changed_version[version_offset + generator.next() as usize % 8] ^= 1;
            let changed = decode_request(1, &changed_version).unwrap();
            assert_ne!(changed.expected_version, command.expected_version);
            assert_eq!(changed.user_id, command.user_id);
            assert_eq!(encode_request(&changed, 1).unwrap(), changed_version);

            let mut invalid_length = wire.clone();
            invalid_length[header..header + 4].copy_from_slice(&u32::MAX.to_be_bytes());
            assert_eq!(
                decode_request(1, &invalid_length),
                Err(Error::LimitExceeded)
            );
            let mut invalid_utf8 = wire.clone();
            invalid_utf8[header + 4] = 0xff;
            assert_eq!(
                decode_request(1, &invalid_utf8),
                Err(Error::Malformed("selection UTF8"))
            );
            let mut invalid_discriminant = wire.clone();
            invalid_discriminant[version_offset + 8] = 2 + (generator.next() % 254) as u8;
            assert_eq!(
                decode_request(1, &invalid_discriminant),
                Err(Error::Malformed("selection target discriminant"))
            );

            // A truncated frame must not decode as another valid identity.
            let cut = generator.next() as usize % wire.len();
            assert!(decode_request(1, &wire[..cut]).is_err());
            let mut trailing = wire.clone();
            trailing.push((generator.next() & 255) as u8);
            assert!(decode_request(1, &trailing).is_err());

            // Keep the valid envelope; vary only the decimal representation.
            let mut noncanonical = decoded;
            noncanonical["value"]["version"] = serde_json::json!(format!("0{version}"));
            assert!(decode_result(1, &serde_json::to_vec(&noncanonical).unwrap()).is_err());
            let mut invalid = command;
            invalid.user_id.0.push('\0');
            assert!(encode_request(&invalid, 1).is_err());
        }
    }
}

#[test]
fn bounded_arbitrary_byte_smoke_and_unknown_json_shapes_do_not_panic() {
    let mut generator = seeded::Generator::new(0xf022);
    for case in 0..1024 {
        let bytes: Vec<u8> = (0..case % 513).map(|_| generator.next() as u8).collect();
        let _ = decode_request(1, &bytes);
        let json =
            serde_json::to_vec(&serde_json::json!({"untrusted": text(&mut generator)})).unwrap();
        assert!(decode_result(1, &json).is_err());
        let _ = decode_result(1, &bytes);
    }
}
