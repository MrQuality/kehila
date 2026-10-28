//! Password-only normalization boundary; domain codec strings do not use it.
use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

mod hash;
mod worker;

#[derive(Debug, Eq, PartialEq)]
enum InputError {
    InputTooLarge,
    TooShort,
    TooLong,
}

// No Debug, Display, serialization or implicit string conversion for secrets.
struct NormalizedPassword(Zeroizing<String>);

impl NormalizedPassword {
    fn new(input: Zeroizing<String>) -> Result<Self, InputError> {
        if input.len() > 1024 {
            return Err(InputError::InputTooLarge);
        }
        // 128 Unicode scalars occupy at most 512 UTF-8 bytes. Allocate before
        // collecting secrets, so growth cannot leave an old output allocation.
        let mut normalized = Zeroizing::new(String::with_capacity(512));
        let mut count = 0;
        for character in input.nfc() {
            if count == 128 {
                return Err(InputError::TooLong);
            }
            normalized.push(character);
            count += 1;
        }
        if count < 15 {
            return Err(InputError::TooShort);
        }
        Ok(Self(normalized))
    }

    fn bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(value: impl Into<String>) -> Zeroizing<String> {
        Zeroizing::new(value.into())
    }

    #[test]
    fn credential_policy_one_pins_unicode_normalization_tables() {
        // Unassigned scalars can acquire normalization mappings in later
        // Unicode versions. A table upgrade needs credential-policy review.
        assert_eq!(unicode_normalization::UNICODE_VERSION, (18, 0, 0));
    }

    #[test]
    fn nfc_composition_is_password_only_and_counts_normalized_scalars() {
        let password = NormalizedPassword::new(input("A\u{30a}".repeat(15))).unwrap();
        assert_eq!(password.bytes(), "Å".repeat(15).as_bytes());
        assert!(matches!(
            NormalizedPassword::new(input("A\u{30a}".repeat(14))),
            Err(InputError::TooShort)
        ));
    }

    #[test]
    fn preserves_spaces_case_and_compatibility_characters() {
        for original in ["  Left Middle Right  ".to_owned(), "ﬃ".repeat(15)] {
            let password = NormalizedPassword::new(input(original.clone())).unwrap();
            assert_eq!(password.bytes(), original.as_bytes());
        }
    }

    #[test]
    fn minimum_and_maximum_count_unicode_scalars_instead_of_utf8_bytes() {
        assert!(NormalizedPassword::new(input("a".repeat(15))).is_ok());
        let password = NormalizedPassword::new(input("😀".repeat(128))).unwrap();
        assert_eq!(password.bytes().len(), 512);
        assert!(matches!(
            NormalizedPassword::new(input("😀".repeat(129))),
            Err(InputError::TooLong)
        ));
        assert!(matches!(
            NormalizedPassword::new(input("a".repeat(14))),
            Err(InputError::TooShort)
        ));
    }

    #[test]
    fn raw_byte_ceiling_precedes_normalization_and_character_checks() {
        assert!(matches!(
            NormalizedPassword::new(input("a".repeat(1025))),
            Err(InputError::InputTooLarge)
        ));
        assert!(matches!(
            NormalizedPassword::new(input("a".repeat(1024))),
            Err(InputError::TooLong)
        ));
    }

    #[test]
    fn canonical_expansion_cannot_bypass_normalized_output_limit() {
        let password = NormalizedPassword::new(input("\u{344}".repeat(64))).unwrap();
        let normalized = std::str::from_utf8(password.bytes()).unwrap();
        assert_eq!(normalized.chars().count(), 128);
        assert!(matches!(
            NormalizedPassword::new(input("\u{344}".repeat(65))),
            Err(InputError::TooLong)
        ));
    }
}
