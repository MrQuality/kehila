//! Version-1 text decoding only. The result is not a TLS credential.
mod admission;

use data_encoding::BASE64;
use zeroize::Zeroizing;

const MAX_ENCODED: usize = 65_536;

#[derive(Debug, PartialEq, Eq)]
enum BundleDecodeError {
    TooLarge,
    Framing,
    Base64,
    Allocation,
}

struct DecodedBundle {
    certificate: Zeroizing<Vec<u8>>,
    private_key: Zeroizing<Vec<u8>>,
}

fn decode_bundle(input: &[u8]) -> Result<DecodedBundle, BundleDecodeError> {
    if input.len() > MAX_ENCODED {
        return Err(BundleDecodeError::TooLarge);
    }
    // Finish all framing checks without allocation, before inspecting base64 semantics.
    let mut lines = Lines { remaining: input };
    let certificate = lines.block(b"-----BEGIN CERTIFICATE-----", b"-----END CERTIFICATE-----")?;
    let private_key = lines.block(b"-----BEGIN PRIVATE KEY-----", b"-----END PRIVATE KEY-----")?;
    while let Some(line) = lines.next()? {
        if !exterior(line.text) {
            return Err(BundleDecodeError::Framing);
        }
    }
    Ok(DecodedBundle {
        certificate: certificate.decode()?,
        private_key: private_key.decode()?,
    })
}

struct Line<'a> {
    text: &'a [u8],
    terminated: bool,
}

struct Lines<'a> {
    remaining: &'a [u8],
}

fn exterior(line: &[u8]) -> bool {
    line.iter().all(|byte| matches!(byte, b' ' | b'\t'))
}

impl<'a> Lines<'a> {
    fn next(&mut self) -> Result<Option<Line<'a>>, BundleDecodeError> {
        if self.remaining.is_empty() {
            return Ok(None);
        }
        let end = self
            .remaining
            .iter()
            .position(|byte| *byte == b'\n')
            .unwrap_or(self.remaining.len());
        let terminated = end < self.remaining.len();
        let mut text = &self.remaining[..end];
        self.remaining = &self.remaining[end + usize::from(terminated)..];
        if terminated {
            text = text.strip_suffix(b"\r").unwrap_or(text);
        }
        if !text.is_ascii() || text.contains(&b'\r') {
            return Err(BundleDecodeError::Framing);
        }
        Ok(Some(Line { text, terminated }))
    }

    fn block(&mut self, begin: &[u8], end: &[u8]) -> Result<Payload<'a>, BundleDecodeError> {
        loop {
            let line = self.next()?.ok_or(BundleDecodeError::Framing)?;
            if line.text == begin && line.terminated {
                break;
            }
            if !exterior(line.text) {
                return Err(BundleDecodeError::Framing);
            }
        }
        let payload_start = self.remaining;
        let mut symbols = 0_usize;
        loop {
            let line_start = self.remaining.len();
            let line = self.next()?.ok_or(BundleDecodeError::Framing)?;
            if line.text == end && symbols != 0 {
                return Ok(Payload {
                    wrapped: &payload_start[..payload_start.len() - line_start],
                    symbols,
                });
            }
            if !line.terminated
                || !(1..=64).contains(&line.text.len())
                || !line
                    .text
                    .iter()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'='))
            {
                return Err(BundleDecodeError::Framing);
            }
            symbols = symbols
                .checked_add(line.text.len())
                .ok_or(BundleDecodeError::Framing)?;
        }
    }
}

struct Payload<'a> {
    wrapped: &'a [u8],
    symbols: usize,
}

fn buffer(size: usize) -> Result<Zeroizing<Vec<u8>>, BundleDecodeError> {
    // Wrap before reservation/writes so partial work is wiped on ordinary unwinding too.
    let mut bytes = Zeroizing::new(Vec::new());
    bytes
        .try_reserve_exact(size)
        .map_err(|_| BundleDecodeError::Allocation)?;
    Ok(bytes)
}

impl Payload<'_> {
    fn decode(self) -> Result<Zeroizing<Vec<u8>>, BundleDecodeError> {
        let mut joined = buffer(self.symbols)?;
        joined.extend(
            self.wrapped
                .iter()
                .copied()
                .filter(|byte| !matches!(byte, b'\r' | b'\n')),
        );
        // BASE64 accepts concatenated padded messages; the profile does not.
        if joined.len() % 4 != 0 {
            return Err(BundleDecodeError::Base64);
        }
        if let Some(first) = joined.iter().position(|byte| *byte == b'=') {
            if joined.len() - first > 2 || joined[first..].iter().any(|byte| *byte != b'=') {
                return Err(BundleDecodeError::Base64);
            }
        }
        let size = BASE64
            .decode_len(joined.len())
            .map_err(|_| BundleDecodeError::Base64)?;
        let mut decoded = buffer(size)?;
        decoded.resize(size, 0); // Reservation above ensures this does not grow capacity.
        let written = BASE64
            .decode_mut(&joined, &mut decoded)
            .map_err(|_| BundleDecodeError::Base64)?;
        decoded.truncate(written);
        Ok(decoded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle(certificate: &str, key: &str) -> Vec<u8> {
        format!(
            "-----BEGIN CERTIFICATE-----\n{certificate}\n-----END CERTIFICATE-----\n\
             -----BEGIN PRIVATE KEY-----\n{key}\n-----END PRIVATE KEY-----"
        )
        .into_bytes()
    }

    fn reject(input: &[u8], expected: BundleDecodeError) {
        match decode_bundle(input) {
            Err(actual) => assert_eq!(actual, expected),
            Ok(_) => panic!("invalid bundle decoded"),
        }
    }

    #[test]
    fn exact_decoded_bytes_and_wrapping() {
        for (encoded, expected) in [
            ("TWFu", b"Man".as_slice()),
            ("TWE=", b"Ma".as_slice()),
            ("TQ==", b"M".as_slice()),
            ("TQ=\n=", b"M".as_slice()),
            ("T\nW\nF\nu", b"Man".as_slice()),
        ] {
            let decoded = decode_bundle(&bundle(encoded, encoded)).unwrap();
            assert_eq!(decoded.certificate.as_slice(), expected);
            assert_eq!(decoded.private_key.as_slice(), expected);
        }
        let decoded = decode_bundle(&bundle(&"A".repeat(64), "TQ==")).unwrap();
        assert_eq!(decoded.certificate.as_slice(), &[0; 48]);
    }

    #[test]
    fn every_allowed_line_width_preserves_bytes() {
        let payload = "TWFu".repeat(16);
        for width in 1..=64 {
            let wrapped = payload
                .as_bytes()
                .chunks(width)
                .map(|line| std::str::from_utf8(line).unwrap())
                .collect::<Vec<_>>()
                .join("\n");
            let decoded = decode_bundle(&bundle(&wrapped, &wrapped)).unwrap();
            assert_eq!(decoded.certificate.as_slice(), b"Man".repeat(16));
            assert_eq!(decoded.private_key.as_slice(), b"Man".repeat(16));
        }
    }

    #[test]
    fn every_truncated_prefix_is_rejected() {
        let input = bundle("TQ==", "TWE=");
        for length in 0..input.len() {
            assert!(decode_bundle(&input[..length]).is_err());
        }
    }

    #[test]
    fn exterior_whitespace_line_endings_and_end_at_eof() {
        let input = String::from_utf8(bundle("TQ==", "TWE=")).unwrap();
        for input in [
            input.clone(),
            input.replace('\n', "\r\n"),
            format!(" \t\n\r\n{input}\r\n \t"),
            input.replace(
                "-----\n-----BEGIN PRIVATE",
                "-----\n \t\r\n-----BEGIN PRIVATE",
            ),
        ] {
            let decoded = decode_bundle(input.as_bytes()).unwrap();
            assert_eq!(decoded.certificate.as_slice(), b"M");
            assert_eq!(decoded.private_key.as_slice(), b"Ma");
        }
    }

    #[test]
    fn exact_limit_and_size_before_framing() {
        let mut input = bundle("TQ==", "TQ==");
        input.push(b'\n');
        input.resize(MAX_ENCODED, b' ');
        assert!(decode_bundle(&input).is_ok());
        input.push(b' ');
        reject(&input, BundleDecodeError::TooLarge);
        reject(&vec![0xff; MAX_ENCODED + 1], BundleDecodeError::TooLarge);
    }

    #[test]
    fn forbidden_material_and_labels_are_framing_errors() {
        let valid = String::from_utf8(bundle("TQ==", "TQ==")).unwrap();
        for input in [
            format!("\u{feff}{valid}"),
            format!("comment\n{valid}"),
            format!("{valid}\ncomment"),
            format!(" {valid}"),
            format!("{valid}\n{valid}"),
            valid.replace("BEGIN CERTIFICATE-----", "BEGIN CERTIFICATE----- "),
            valid.replace("END PRIVATE KEY-----", "END PRIVATE KEY----- "),
            valid.replace("PRIVATE KEY", "ENCRYPTED PRIVATE KEY"),
            valid.replace("PRIVATE KEY", "RSA PRIVATE KEY"),
            valid.replace("PRIVATE KEY", "EC PRIVATE KEY"),
            valid.replace("CERTIFICATE", "UNKNOWN"),
            valid.replace('\n', "\r"),
            valid.replace("-----\nTQ", "-----TQ"),
            valid.replace("TQ==\n-----END", "TQ==-----END"),
            valid.replace("END CERTIFICATE", "END PRIVATE KEY"),
            valid.replace("BEGIN CERTIFICATE", "END CERTIFICATE"),
            valid.replace("BEGIN PRIVATE KEY", "BEGIN CERTIFICATE"),
            valid.replace(
                "CERTIFICATE-----\nTQ==",
                "CERTIFICATE-----\n-----BEGIN CERTIFICATE-----\nTQ==",
            ),
            valid.replace(
                "-----END CERTIFICATE-----",
                "-----END CERTIFICATE-----\n-----END CERTIFICATE-----",
            ),
            valid.replace("-----END PRIVATE KEY-----", ""),
        ] {
            reject(input.as_bytes(), BundleDecodeError::Framing);
        }
        let reversed = valid
            .replace("CERTIFICATE", "TEMP")
            .replace("PRIVATE KEY", "CERTIFICATE")
            .replace("TEMP", "PRIVATE KEY");
        reject(reversed.as_bytes(), BundleDecodeError::Framing);
    }

    #[test]
    fn payload_framing_applies_to_both_blocks() {
        for payload in [
            "".to_owned(),
            "\nTQ==".to_owned(),
            "TQ==\n".to_owned(),
            "T Q==".to_owned(),
            "TQ==\t".to_owned(),
            "TQ-_".to_owned(),
            "TQé=".to_owned(),
            "TQ\r==".to_owned(),
            "Header: value\nTQ==".to_owned(),
            "A".repeat(65),
        ] {
            reject(&bundle(&payload, "TQ=="), BundleDecodeError::Framing);
            reject(&bundle("TQ==", &payload), BundleDecodeError::Framing);
        }
        reject(&[], BundleDecodeError::Framing);
        reject(b" \t\r\n", BundleDecodeError::Framing);
    }

    #[test]
    fn canonical_base64_and_partial_failure() {
        for payload in ["TQ", "TQ===", "TR==", "TWF=", "TQ==TQ==", "=TQ=", "===="] {
            reject(&bundle(payload, "TQ=="), BundleDecodeError::Base64);
            reject(&bundle("TQ==", payload), BundleDecodeError::Base64);
        }
    }

    #[test]
    fn whole_framing_precedes_payload_decoding() {
        reject(&bundle("TR==", "T Q=="), BundleDecodeError::Framing);
        let mut input = bundle("TR==", "TQ==");
        input.extend_from_slice(b"\nforbidden");
        reject(&input, BundleDecodeError::Framing);
    }

    #[test]
    fn errors_expose_only_categories() {
        for (error, expected) in [
            (BundleDecodeError::TooLarge, "TooLarge"),
            (BundleDecodeError::Framing, "Framing"),
            (BundleDecodeError::Base64, "Base64"),
            (BundleDecodeError::Allocation, "Allocation"),
        ] {
            assert_eq!(format!("{error:?}"), expected);
        }
    }
}
