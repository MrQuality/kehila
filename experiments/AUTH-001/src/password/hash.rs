//! Fixed-profile verifier admission and owned Argon2 workspace.
use super::NormalizedPassword;
use argon2::{
    password_hash::phc::{Output, PasswordHash, Salt},
    Algorithm, Argon2, Block, Params, Version,
};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, Zeroizing};

const PARAMETERS: &str = "m=65536,t=3,p=1";
const MEMORY_KIB: usize = 65536;

#[derive(Debug, Eq, PartialEq)]
pub(super) enum HashError {
    UnsupportedVerifier,
    Unavailable,
}

pub(super) struct Verifier {
    encoded: Zeroizing<String>,
    salt: [u8; 16],
    expected: Zeroizing<[u8; 32]>,
}

pub(super) struct Hasher {
    engine: Argon2<'static>,
    memory: Zeroizing<Vec<Block>>,
}

impl Verifier {
    pub(super) fn from_stored(encoded: Zeroizing<String>, policy: u32) -> Result<Self, HashError> {
        if policy != 1 || encoded.is_empty() || encoded.len() > 512 || !encoded.is_ascii() {
            return Err(HashError::UnsupportedVerifier);
        }
        let parsed = PasswordHash::new(&encoded).map_err(|_| HashError::UnsupportedVerifier)?;
        // PHC get() takes the first duplicate while Argon2 conversion takes the
        // last. Use the exact own-writer profile, never parsed allocation costs.
        if parsed.algorithm.as_str() != "argon2id"
            || parsed.version != Some(19)
            || parsed.params.as_str() != PARAMETERS
        {
            return Err(HashError::UnsupportedVerifier);
        }
        let salt: [u8; 16] = parsed
            .salt
            .as_ref()
            .ok_or(HashError::UnsupportedVerifier)?
            .as_ref()
            .try_into()
            .map_err(|_| HashError::UnsupportedVerifier)?;
        let output = parsed.hash.as_ref().ok_or(HashError::UnsupportedVerifier)?;
        if output.len() != 32 {
            return Err(HashError::UnsupportedVerifier);
        }
        let canonical = Zeroizing::new(parsed.to_string());
        if canonical.as_str() != encoded.as_str() {
            return Err(HashError::UnsupportedVerifier);
        }
        let mut expected = Zeroizing::new([0; 32]);
        expected.copy_from_slice(output.as_bytes());
        Ok(Self {
            encoded,
            salt,
            expected,
        })
    }

    pub(super) fn encoded(&self) -> &str {
        &self.encoded
    }
}

impl Hasher {
    pub(super) fn new() -> Result<Self, HashError> {
        let parameters =
            Params::new(MEMORY_KIB as u32, 3, 1, Some(32)).map_err(|_| HashError::Unavailable)?;
        let engine = Argon2::new(Algorithm::Argon2id, Version::V0x13, parameters);
        let mut memory = Zeroizing::new(Vec::new());
        memory
            .try_reserve_exact(MEMORY_KIB)
            .map_err(|_| HashError::Unavailable)?;
        memory.resize(MEMORY_KIB, Block::default());
        Ok(Self { engine, memory })
    }

    fn compute(
        &mut self,
        password: &NormalizedPassword,
        salt: &[u8; 16],
    ) -> Result<Zeroizing<[u8; 32]>, HashError> {
        let mut output = Zeroizing::new([0; 32]);
        let result = self.engine.hash_password_into_with_memory(
            password.bytes(),
            salt,
            output.as_mut_slice(),
            self.memory.as_mut_slice(),
        );
        // Keep allocation/length for sequential replacement, but wipe before
        // returning success or failure. Drop also wipes during unwinding.
        for block in self.memory.iter_mut() {
            block.zeroize();
        }
        result.map_err(|_| HashError::Unavailable)?;
        Ok(output)
    }

    pub(super) fn hash(
        &mut self,
        password: &NormalizedPassword,
        salt: [u8; 16],
    ) -> Result<Verifier, HashError> {
        let expected = self.compute(password, &salt)?;
        let phc = PasswordHash {
            algorithm: "argon2id".try_into().map_err(|_| HashError::Unavailable)?,
            version: Some(19),
            params: PARAMETERS.parse().map_err(|_| HashError::Unavailable)?,
            salt: Some(Salt::new(&salt).map_err(|_| HashError::Unavailable)?),
            hash: Some(Output::new(expected.as_slice()).map_err(|_| HashError::Unavailable)?),
        };
        let encoded = Zeroizing::new(phc.to_string());
        Ok(Verifier {
            encoded,
            salt,
            expected,
        })
    }

    pub(super) fn verify(
        &mut self,
        password: &NormalizedPassword,
        verifier: &Verifier,
    ) -> Result<bool, HashError> {
        let output = self.compute(password, &verifier.salt)?;
        Ok(bool::from(
            output.as_slice().ct_eq(verifier.expected.as_slice()),
        ))
    }

    #[cfg(test)]
    fn workspace_is_clear(&self) -> bool {
        self.memory
            .iter()
            .all(|block| block.as_ref().iter().all(|word| *word == 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PREFIX: &str = "$argon2id$v=19$m=65536,t=3,p=1$";

    fn fixture(salt_bytes: usize, output_bytes: usize) -> String {
        // The Base64 encoding of all-zero bytes consists entirely of 'A'.
        let encoded_len = |bytes: usize| (bytes * 8).div_ceil(6);
        format!(
            "{PREFIX}{}${}",
            "A".repeat(encoded_len(salt_bytes)),
            "A".repeat(encoded_len(output_bytes))
        )
    }

    fn stored(value: String) -> Result<Verifier, HashError> {
        Verifier::from_stored(Zeroizing::new(value), 1)
    }

    #[test]
    fn admits_only_explicit_version_one_profile_and_canonical_parameters() {
        let value = fixture(16, 32);
        assert!(stored(value.clone()).is_ok());
        for unsupported in [
            value.replace("argon2id", "argon2i"),
            value.replace("v=19", "v=16"),
            value.replace("$v=19", ""),
            value.replace("m=65536", "m=104857600"),
            value.replace("t=3", "t=4"),
            value.replace("p=1", "p=4"),
            value.replace("m=65536,t=3,p=1", "t=3,m=65536,p=1"),
            value.replace("p=1", "p=1,data=AAAA"),
            value.replace("p=1", "p=1,keyid=AAAA"),
        ] {
            assert!(matches!(
                stored(unsupported),
                Err(HashError::UnsupportedVerifier)
            ));
        }
        assert!(matches!(
            Verifier::from_stored(Zeroizing::new(value), 2),
            Err(HashError::UnsupportedVerifier)
        ));
    }

    #[test]
    fn duplicate_parameters_cannot_hide_later_expensive_or_different_values() {
        let value = fixture(16, 32);
        for parameters in [
            "m=65536,m=104857600,t=3,p=1",
            "m=104857600,m=65536,t=3,p=1",
            "m=65536,m=65536,t=3,p=1",
            "m=65536,t=3,t=30,p=1",
            "m=65536,t=3,p=1,p=4",
        ] {
            assert!(matches!(
                stored(value.replace("m=65536,t=3,p=1", parameters)),
                Err(HashError::UnsupportedVerifier)
            ));
        }
    }

    #[test]
    fn salt_output_and_encoded_bounds_fail_before_a_workspace_exists() {
        for value in [
            fixture(15, 32),
            fixture(17, 32),
            fixture(16, 31),
            fixture(16, 33),
            String::new(),
            "x".repeat(513),
            "é".repeat(32),
        ] {
            assert!(matches!(stored(value), Err(HashError::UnsupportedVerifier)));
        }
    }

    #[test]
    fn real_fixed_cost_hash_verifies_only_the_same_normalized_password() {
        let password = NormalizedPassword::new(Zeroizing::new("A\u{30a}".repeat(15))).unwrap();
        let equivalent = NormalizedPassword::new(Zeroizing::new("Å".repeat(15))).unwrap();
        let wrong =
            NormalizedPassword::new(Zeroizing::new("a different password".to_owned())).unwrap();
        let mut hasher = Hasher::new().unwrap();
        let verifier = hasher.hash(&password, [7; 16]).unwrap();
        assert!(hasher.workspace_is_clear());
        assert!(verifier.encoded().starts_with(PREFIX));
        let parsed = stored(verifier.encoded().to_owned()).unwrap();
        assert!(hasher.verify(&equivalent, &parsed).unwrap());
        assert!(hasher.workspace_is_clear());
        assert!(!hasher.verify(&wrong, &parsed).unwrap());
        assert!(hasher.workspace_is_clear());
    }

    #[test]
    fn independent_fixed_profile_vector_and_distinct_salts() {
        let password =
            NormalizedPassword::new(Zeroizing::new("a fixed test password".to_owned())).unwrap();
        let mut hasher = Hasher::new().unwrap();
        let first = hasher.hash(&password, [1; 16]).unwrap();
        // Argon2 C reference via argon2-cffi/bindings 25.1.0, explicitly
        // configured id/v19/m=65536/t=3/p=1/out32; not a Rust roundtrip oracle.
        assert_eq!(
            first.encoded(),
            concat!(
                "$argon2id$v=19$m=65536,t=3,p=1$AQEBAQEBAQEBAQEBAQEBAQ$",
                "6d50UfWIhEf6nFrBV/eh7X/wOi7ToEKiEY6e0UdWGa8"
            )
        );
        let second = hasher.hash(&password, [2; 16]).unwrap();
        assert_ne!(first.encoded(), second.encoded());
    }
}
