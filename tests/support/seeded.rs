// Reproducible test input exploration, not a security randomness source.
pub struct Generator(u64);

impl Generator {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next(&mut self) -> u64 {
        // SplitMix64: mix the output so low-byte consumers do not see an LCG's
        // short low-bit cycles. This remains deterministic, non-cryptographic input.
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^ (value >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::Generator;

    #[test]
    fn splitmix_zero_seed_matches_reference_vector() {
        let mut generator = Generator::new(0);
        assert_eq!(generator.next(), 0xe220a8397b1dcdaf);
        assert_eq!(generator.next(), 0x6e789e6aa1b965f4);
        assert_eq!(generator.next(), 0x06c45d188009454f);
    }

    #[test]
    fn selected_seed_bytes_do_not_repeat_the_raw_lcg_cycle() {
        for seed in [1, 0x5eed, 0xa11ce, 0xf022] {
            let mut generator = Generator::new(seed);
            let bytes: Vec<u8> = (0..512).map(|_| generator.next() as u8).collect();
            assert_ne!(bytes[..256], bytes[256..]);
            assert!(bytes.windows(2).any(|pair| pair[0] % 2 == pair[1] % 2));
            let offsets: std::collections::BTreeSet<_> =
                bytes.iter().map(|byte| byte % 8).collect();
            assert_eq!(offsets.len(), 8);
        }
    }
}
