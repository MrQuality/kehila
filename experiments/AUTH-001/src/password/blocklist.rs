//! Bounded digest-only blocklist admission. The reviewed corpus is supplied
//! externally; this module does not download or distribute password datasets.
use super::NormalizedPassword;
use ring::digest;
use zeroize::Zeroizing;

const MAX_ENTRIES: usize = 4096;
const POLICY_ONE_COUNT: usize = 2097;
pub(crate) const ENCODED_LEN: usize = POLICY_ONE_COUNT * 32;
const POLICY_ONE_SHA256: [u8; 32] = [
    0x4a, 0x2e, 0xc4, 0x3e, 0x1b, 0x73, 0x7b, 0xfa, 0xe0, 0x6e, 0xb3, 0xe4, 0xae, 0xc4, 0x69, 0x93,
    0x34, 0x4c, 0x28, 0xdd, 0x8a, 0xa9, 0x59, 0xf5, 0x01, 0x1c, 0x37, 0x8b, 0xad, 0x7c, 0x2f, 0xc8,
];

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum BlocklistError {
    InvalidAsset,
    Blocked,
}

pub(crate) struct Blocklist(Vec<[u8; 32]>);
pub(crate) struct AllowedPassword(NormalizedPassword);

impl AllowedPassword {
    pub(super) fn into_password(self) -> NormalizedPassword {
        self.0
    }
}

impl Blocklist {
    #[cfg(test)]
    pub(crate) fn synthetic_for_tests() -> Self {
        // Mechanics fixture only; never used to qualify actual corpus coverage.
        Self(vec![digest::digest(
            &digest::SHA256,
            b"blocked synthetic password",
        )
        .as_ref()
        .try_into()
        .expect("SHA-256 has a fixed 32-byte output")])
    }
    pub(crate) fn policy_one(bytes: &[u8]) -> Result<Self, BlocklistError> {
        Self::decode(bytes, POLICY_ONE_COUNT, &POLICY_ONE_SHA256)
    }
    // Expected count/hash are trusted, reviewed startup inputs, never request
    // fields. The file reader must cap the read before calling this decoder.
    fn decode(
        bytes: &[u8],
        expected_count: usize,
        expected_sha256: &[u8; 32],
    ) -> Result<Self, BlocklistError> {
        if expected_count == 0
            || expected_count > MAX_ENTRIES
            || bytes.len() != expected_count * 32
            || digest::digest(&digest::SHA256, bytes).as_ref() != expected_sha256
        {
            return Err(BlocklistError::InvalidAsset);
        }
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(expected_count)
            .map_err(|_| BlocklistError::InvalidAsset)?;
        for &entry in bytes.as_chunks::<32>().0 {
            if entries.last().is_some_and(|previous| previous >= &entry) {
                return Err(BlocklistError::InvalidAsset);
            }
            entries.push(entry);
        }
        Ok(Self(entries))
    }

    pub(crate) fn admit(
        &self,
        password: NormalizedPassword,
    ) -> Result<AllowedPassword, BlocklistError> {
        let hash = Zeroizing::new(
            digest::digest(&digest::SHA256, password.bytes())
                .as_ref()
                .try_into()
                .map_err(|_| BlocklistError::InvalidAsset)?,
        );
        if self.0.binary_search(&*hash).is_ok() {
            return Err(BlocklistError::Blocked);
        }
        Ok(AllowedPassword(password))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroize::Zeroizing;

    fn password(value: &str) -> NormalizedPassword {
        NormalizedPassword::new(Zeroizing::new(value.to_owned())).unwrap()
    }

    fn sha256(bytes: &[u8]) -> [u8; 32] {
        digest::digest(&digest::SHA256, bytes)
            .as_ref()
            .try_into()
            .unwrap()
    }

    fn fixture() -> (Vec<u8>, [u8; 32]) {
        let mut entries = [
            sha256(b"a common synthetic password"),
            sha256("a synthetic café password".as_bytes()),
        ];
        entries.sort_unstable();
        let bytes = entries.concat();
        let hash = sha256(&bytes);
        (bytes, hash)
    }

    #[test]
    fn exact_whole_normalized_password_is_blocked() {
        let (bytes, hash) = fixture();
        let blocklist = Blocklist::decode(&bytes, 2, &hash).unwrap();
        assert!(matches!(
            blocklist.admit(password("a common synthetic password")),
            Err(BlocklistError::Blocked)
        ));
        assert!(matches!(
            blocklist.admit(password("a synthetic cafe\u{301} password")),
            Err(BlocklistError::Blocked)
        ));
        let allowed = blocklist
            .admit(password("a common synthetic password with suffix"))
            .unwrap();
        assert_eq!(
            allowed.0.bytes(),
            b"a common synthetic password with suffix"
        );
        assert!(blocklist
            .admit(password("A common synthetic password"))
            .is_ok());
    }

    #[test]
    fn asset_count_length_and_hash_fail_closed() {
        let (bytes, hash) = fixture();
        for count in [0, 1, 3, MAX_ENTRIES + 1, usize::MAX] {
            assert!(matches!(
                Blocklist::decode(&bytes, count, &hash),
                Err(BlocklistError::InvalidAsset)
            ));
        }
        assert!(Blocklist::decode(&bytes[..63], 2, &hash).is_err());
        assert!(Blocklist::decode(&bytes, 2, &[0; 32]).is_err());
        let mut corrupt = bytes;
        corrupt[0] ^= 1;
        assert!(Blocklist::decode(&corrupt, 2, &hash).is_err());
    }

    #[test]
    fn unsorted_or_duplicate_assets_fail_even_with_matching_hash() {
        let duplicate = vec![1; 64];
        assert!(Blocklist::decode(&duplicate, 2, &sha256(&duplicate)).is_err());
        let mut unsorted = vec![2; 32];
        unsorted.extend_from_slice(&[1; 32]);
        assert!(Blocklist::decode(&unsorted, 2, &sha256(&unsorted)).is_err());
    }

    #[test]
    fn policy_one_cannot_be_replaced_by_a_self_consistent_synthetic_asset() {
        let (bytes, _) = fixture();
        assert!(matches!(
            Blocklist::policy_one(&bytes),
            Err(BlocklistError::InvalidAsset)
        ));
    }

    #[test]
    #[ignore = "requires the separately provisioned external policy-one corpus"]
    fn selected_external_corpus_matches_pin_and_admits_by_exact_password() {
        use std::io::Read;
        let path = std::env::var_os("AUTH001_BLOCKLIST_FIXTURE")
            .expect("external blocklist fixture must be explicitly selected");
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .unwrap()
            .take((ENCODED_LEN + 1) as u64)
            .read_to_end(&mut bytes)
            .unwrap();
        let list = Blocklist::policy_one(&bytes).unwrap();
        assert_eq!(list.0.len(), POLICY_ONE_COUNT);
        assert!(matches!(
            list.admit(password("123456789012345")),
            Err(BlocklistError::Blocked)
        ));
        assert!(list
            .admit(password("synthetic-fixture-unlisted-7e3d9742"))
            .is_ok());
    }
}
