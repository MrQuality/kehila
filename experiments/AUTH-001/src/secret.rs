//! Fixed-size random secrets; no formatting or serialization interfaces.
use data_encoding::BASE64URL_NOPAD;
use ring::{
    digest,
    rand::{SecureRandom, SystemRandom},
};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

#[derive(Debug, Eq, PartialEq)]
pub(super) enum SecretError {
    Unavailable,
    Invalid,
}

#[derive(Clone, Copy)]
pub(super) struct Digest([u8; 32]);

pub(super) struct Secret {
    encoded: Zeroizing<[u8; 43]>,
    digest: Digest,
}

impl Secret {
    pub(super) fn generate() -> Result<Self, SecretError> {
        let source = SystemRandom::new();
        Self::from_source(|buffer| source.fill(buffer).map_err(|_| ()))
    }

    fn from_source(mut fill: impl FnMut(&mut [u8]) -> Result<(), ()>) -> Result<Self, SecretError> {
        let mut raw = Zeroizing::new([0; 32]);
        fill(raw.as_mut_slice()).map_err(|_| SecretError::Unavailable)?;
        let mut encoded = Zeroizing::new([0; 43]);
        // Unpadded Base64url of exactly 32 bytes is exactly 43 ASCII bytes.
        BASE64URL_NOPAD.encode_mut(raw.as_slice(), encoded.as_mut_slice());
        Ok(Self {
            encoded,
            digest: Digest::of_raw(&raw),
        })
    }

    pub(super) fn encoded(&self) -> &[u8; 43] {
        &self.encoded
    }

    pub(super) fn digest(&self) -> Digest {
        self.digest
    }
}

impl Digest {
    fn of_raw(raw: &[u8; 32]) -> Self {
        let result = digest::digest(&digest::SHA256, raw);
        let mut bytes = [0; 32];
        // The selected SHA-256 algorithm has a fixed 32-byte output.
        bytes.copy_from_slice(result.as_ref());
        Self(bytes)
    }

    pub(super) fn from_encoded(encoded: &[u8]) -> Result<Self, SecretError> {
        if encoded.len() != 43 {
            return Err(SecretError::Invalid);
        }
        let mut raw = Zeroizing::new([0; 32]);
        let size = BASE64URL_NOPAD
            .decode_mut(encoded, raw.as_mut_slice())
            .map_err(|_| SecretError::Invalid)?;
        if size != 32 {
            return Err(SecretError::Invalid);
        }
        let mut canonical = Zeroizing::new([0; 43]);
        BASE64URL_NOPAD.encode_mut(raw.as_slice(), canonical.as_mut_slice());
        if canonical.as_slice() != encoded {
            return Err(SecretError::Invalid);
        }
        Ok(Self::of_raw(&raw))
    }

    pub(super) fn matches(&self, other: &Self) -> bool {
        bool::from(self.0.as_slice().ct_eq(other.0.as_slice()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_encoding_and_independent_sha256_vector() {
        let secret = Secret::from_source(|buffer| {
            buffer.fill(0);
            Ok(())
        })
        .unwrap();
        assert_eq!(secret.encoded(), &[b'A'; 43]);
        // SHA-256 of 32 zero bytes, independently checked with Python hashlib.
        assert_eq!(
            secret.digest.0,
            [
                0x66, 0x68, 0x7a, 0xad, 0xf8, 0x62, 0xbd, 0x77, 0x6c, 0x8f, 0xc1, 0x8b, 0x8e, 0x9f,
                0x8e, 0x20, 0x08, 0x97, 0x14, 0x85, 0x6e, 0xe2, 0x33, 0xb3, 0x90, 0x2a, 0x59, 0x1d,
                0x0d, 0x5f, 0x29, 0x25
            ]
        );
        assert!(secret
            .digest
            .matches(&Digest::from_encoded(secret.encoded()).unwrap()));
    }

    #[test]
    fn rejects_wrong_size_padding_invalid_alphabet_and_unused_bits() {
        for invalid in [
            vec![b'A'; 42],
            vec![b'A'; 44],
            vec![b'='; 43],
            vec![b'!'; 43],
            [vec![b'A'; 42], vec![b'B']].concat(),
        ] {
            assert!(matches!(
                Digest::from_encoded(&invalid),
                Err(SecretError::Invalid)
            ));
        }
    }

    #[test]
    fn failed_random_source_cannot_return_a_credential_or_fallback() {
        let mut calls = 0;
        let result = Secret::from_source(|buffer| {
            calls += 1;
            buffer.fill(7);
            Err(())
        });
        assert!(matches!(result, Err(SecretError::Unavailable)));
        assert_eq!(calls, 1);
    }

    #[test]
    fn real_os_source_and_digest_comparison_are_wired() {
        let first = Secret::generate().unwrap();
        let second = Secret::generate().unwrap();
        let first_digest = Digest::from_encoded(first.encoded()).unwrap();
        let second_digest = Digest::from_encoded(second.encoded()).unwrap();
        assert!(first.digest.matches(&first_digest));
        assert!(!first_digest.matches(&second_digest));
    }
}
