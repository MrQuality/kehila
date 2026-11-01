use rustls::pki_types::UnixTime;
use std::time::{Duration, Instant};

/// A fixed elapsed budget cannot be extended by a UTC rewind. The owning
/// listener stops irreversibly on the first failed observation.
#[derive(Clone, Copy)]
pub(super) struct Window {
    start: UnixTime,
    end: UnixTime,
    deadline: Instant,
}
impl Window {
    pub(super) fn new(
        start: UnixTime,
        end: UnixTime,
        utc: Option<UnixTime>,
        now: Instant,
    ) -> Option<Self> {
        let utc = utc?;
        if utc < start || utc >= end {
            return None;
        }
        let remaining = end.as_secs().checked_sub(utc.as_secs())?;
        let deadline = now.checked_add(Duration::from_secs(remaining.min(180)))?;
        Some(Self {
            start,
            end,
            deadline,
        })
    }
    pub(super) fn open(self, utc: Option<UnixTime>, now: Instant) -> bool {
        now < self.deadline && utc.is_some_and(|utc| utc >= self.start && utc < self.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn utc(seconds: u64) -> UnixTime {
        UnixTime::since_unix_epoch(Duration::from_secs(seconds))
    }
    #[test]
    fn exact_utc_and_elapsed_boundaries_never_extend_budget() {
        let now = Instant::now();
        assert!(Window::new(utc(100), utc(200), None, now).is_none());
        assert!(Window::new(utc(100), utc(200), Some(utc(99)), now).is_none());
        assert!(Window::new(utc(100), utc(200), Some(utc(200)), now).is_none());
        let window = Window::new(utc(100), utc(200), Some(utc(190)), now).unwrap();
        assert!(window.open(Some(utc(190)), now + Duration::from_secs(9)));
        assert!(!window.open(Some(utc(100)), now + Duration::from_secs(10)));
        assert!(!window.open(Some(utc(200)), now));
        assert!(!window.open(Some(utc(99)), now));
        assert!(!window.open(None, now));
        let lifetime = Window::new(utc(100), utc(1000), Some(utc(110)), now).unwrap();
        assert!(!lifetime.open(Some(utc(110)), now + Duration::from_secs(180)));
    }
}
