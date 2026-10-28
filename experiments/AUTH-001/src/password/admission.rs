//! Installation-wide allowances, without client-keyed maps or waiting queues.
use std::time::Instant;

const TOKEN_NANOS: u64 = 12_000_000_000; // Five tokens per minute.

pub(crate) struct Bucket {
    credit: u64,
    ceiling: u64,
    observed: Instant,
}

impl Bucket {
    pub(crate) fn new(now: Instant, burst: u8) -> Self {
        let ceiling = u64::from(burst) * TOKEN_NANOS;
        Self {
            credit: ceiling,
            ceiling,
            observed: now,
        }
    }

    pub(crate) fn charge(&mut self, now: Instant) -> bool {
        let Some(elapsed) = now.checked_duration_since(self.observed) else {
            return false;
        };
        // Cap before conversion, then saturate; even arbitrarily long idle
        // periods cannot overflow or bank more than the configured burst.
        let refill = elapsed.as_nanos().min(u128::from(self.ceiling)) as u64;
        self.credit = self.credit.saturating_add(refill).min(self.ceiling);
        self.observed = now;
        if self.credit < TOKEN_NANOS {
            return false;
        }
        self.credit -= TOKEN_NANOS;
        true
    }
}

struct PasswordAdmission {
    login: Bucket,
    management: Bucket,
}

impl PasswordAdmission {
    fn new(now: Instant) -> Self {
        Self {
            login: Bucket::new(now, 5),
            management: Bucket::new(now, 5),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn burst_and_fractional_refill_are_exact() {
        let now = Instant::now();
        let mut bucket = Bucket::new(now, 5);
        for _ in 0..5 {
            assert!(bucket.charge(now));
        }
        assert!(!bucket.charge(now));
        assert!(!bucket.charge(now + Duration::from_millis(11_999)));
        assert!(bucket.charge(now + Duration::from_secs(12)));
        assert!(!bucket.charge(now + Duration::from_secs(12)));
        assert!(bucket.charge(now + Duration::from_secs(24)));
    }

    #[test]
    fn idle_time_cannot_accumulate_beyond_the_burst() {
        let now = Instant::now();
        let mut bucket = Bucket::new(now, 2);
        let later = now + Duration::from_secs(3600);
        assert!(bucket.charge(later));
        assert!(bucket.charge(later));
        assert!(!bucket.charge(later));
        assert!(!bucket.charge(later + Duration::from_nanos(1)));
    }

    #[test]
    fn backwards_time_denies_without_rewinding_the_refill_clock() {
        let now = Instant::now();
        let mut bucket = Bucket::new(now, 2);
        assert!(bucket.charge(now));
        assert!(!bucket.charge(now - Duration::from_secs(12)));
        assert!(bucket.charge(now));
        assert!(!bucket.charge(now));
    }

    #[test]
    fn login_and_management_allowances_are_independent() {
        let now = Instant::now();
        let mut admission = PasswordAdmission::new(now);
        for _ in 0..5 {
            assert!(admission.login.charge(now));
        }
        assert!(!admission.login.charge(now));
        for _ in 0..5 {
            assert!(admission.management.charge(now));
        }
        assert!(!admission.management.charge(now));
    }
}
