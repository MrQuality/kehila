//! Bounded anonymous browser state. Claims are owned across SQL work, never
//! registry guards; only confirmed rollback may restore a claimed entry.
use crate::{
    lifecycle::RuntimeLiveness,
    password::admission::Bucket,
    secret::{Digest, Secret},
};
use std::{
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};

const CAPACITY: usize = 32;
const TTL: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Eq, PartialEq)]
enum PreError {
    Busy,
    Full,
    Expired,
    Unavailable,
    Unknown,
    Invalid,
}

enum Phase {
    Live,
    Claimed(u64),
    Consumed,
}

struct PreSession {
    ticket: u64,
    issued: Instant,
    deadline: Instant,
    bearer: Digest,
    csrf: Secret,
    phase: Mutex<Phase>,
    live_runtime: Arc<RuntimeLiveness>,
}

enum Slot {
    Reserved {
        ticket: u64,
        issued: Instant,
        deadline: Instant,
    },
    Ready(Arc<PreSession>),
}

struct Inner {
    slots: Vec<Slot>,
    next: u64,
    issuance: Bucket,
}
struct Pool {
    inner: Arc<Mutex<Inner>>,
    live_runtime: Arc<RuntimeLiveness>,
}

struct Reservation {
    ticket: u64,
    issued: Instant,
    deadline: Instant,
    inner: Weak<Mutex<Inner>>,
    live_runtime: Arc<RuntimeLiveness>,
    finished: bool,
}

struct Claim {
    entry: Arc<PreSession>,
    id: u64,
    finished: bool,
}

impl Pool {
    fn new(live_runtime: Arc<RuntimeLiveness>, now: Instant) -> Result<Self, PreError> {
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(CAPACITY)
            .map_err(|_| PreError::Unavailable)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(Inner {
                slots,
                next: 0,
                issuance: Bucket::new(now, 2),
            })),
            live_runtime,
        })
    }
    fn reserve(&self, now: Instant) -> Result<Reservation, PreError> {
        if !self.live_runtime.is_live() {
            return Err(PreError::Unavailable);
        }
        let deadline = now.checked_add(TTL).ok_or(PreError::Unavailable)?;
        let mut inner = self.inner.lock().map_err(|_| PreError::Unavailable)?;
        for index in (0..inner.slots.len()).rev() {
            let remove = match &inner.slots[index] {
                Slot::Reserved { deadline, .. } => now >= *deadline,
                Slot::Ready(entry) => {
                    let mut phase = entry.phase.lock().map_err(|_| PreError::Unavailable)?;
                    if now >= entry.deadline {
                        *phase = Phase::Consumed;
                    }
                    matches!(*phase, Phase::Consumed)
                }
            };
            if remove {
                inner.slots.remove(index);
            }
        }
        if inner.slots.len() == CAPACITY {
            return Err(PreError::Full);
        }
        let ticket = inner.next.checked_add(1).ok_or(PreError::Unavailable)?;
        if !inner.issuance.charge(now) {
            return Err(PreError::Busy);
        }
        inner.next = ticket;
        inner.slots.push(Slot::Reserved {
            ticket,
            issued: now,
            deadline,
        });
        Ok(Reservation {
            ticket,
            issued: now,
            deadline,
            inner: Arc::downgrade(&self.inner),
            live_runtime: self.live_runtime.clone(),
            finished: false,
        })
    }
    fn reuse(&self, bearer: &Digest, now: Instant) -> Option<Arc<PreSession>> {
        if !self.live_runtime.is_live() {
            return None;
        }
        let inner = self.inner.lock().ok()?;
        for slot in &inner.slots {
            if let Slot::Ready(entry) = slot {
                if entry.bearer.matches(bearer) && entry.reusable(now) {
                    return Some(entry.clone());
                }
            }
        }
        None
    }
    fn claim(&self, entry: &Arc<PreSession>, now: Instant) -> Result<Claim, PreError> {
        if !self.live_runtime.is_live() {
            return Err(PreError::Unavailable);
        }
        let mut inner = self.inner.lock().map_err(|_| PreError::Unavailable)?;
        if !inner.slots.iter().any(|slot| matches!(slot, Slot::Ready(stored) if stored.ticket == entry.ticket && Arc::ptr_eq(stored, entry))) {
            return Err(PreError::Unknown);
        }
        let mut phase = entry.phase.lock().map_err(|_| PreError::Unavailable)?;
        if !entry.current(now) {
            return Err(PreError::Expired);
        }
        match *phase {
            Phase::Claimed(_) => return Err(PreError::Busy),
            Phase::Consumed => return Err(PreError::Unknown),
            Phase::Live => {}
        }
        let id = inner.next.checked_add(1).ok_or(PreError::Unavailable)?;
        inner.next = id;
        *phase = Phase::Claimed(id);
        Ok(Claim {
            entry: entry.clone(),
            id,
            finished: false,
        })
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        self.live_runtime.stop();
        if let Ok(mut inner) = self.inner.lock() {
            inner.slots.clear();
        }
    }
}

impl Reservation {
    fn finish(
        mut self,
        bearer: Digest,
        csrf: Secret,
        now: Instant,
    ) -> Result<Arc<PreSession>, PreError> {
        // complete's guards are released before self's fallback Drop can run.
        let result = self.complete(bearer, csrf, now);
        self.finished = result.is_ok();
        result
    }
    fn complete(
        &self,
        bearer: Digest,
        csrf: Secret,
        now: Instant,
    ) -> Result<Arc<PreSession>, PreError> {
        if !self.live_runtime.is_live() {
            return Err(PreError::Unavailable);
        }
        if now < self.issued || now >= self.deadline {
            return Err(PreError::Expired);
        }
        if csrf.digest().matches(&bearer) {
            return Err(PreError::Invalid);
        }
        let owner = self.inner.upgrade().ok_or(PreError::Unknown)?;
        let mut inner = owner.lock().map_err(|_| PreError::Unavailable)?;
        let index = inner.slots.iter().position(|slot| matches!(slot, Slot::Reserved { ticket, issued, deadline } if *ticket == self.ticket && *issued == self.issued && *deadline == self.deadline)).ok_or(PreError::Unknown)?;
        if inner
            .slots
            .iter()
            .any(|slot| matches!(slot, Slot::Ready(entry) if entry.bearer.matches(&bearer)))
        {
            return Err(PreError::Invalid);
        }
        let entry = Arc::new(PreSession {
            ticket: self.ticket,
            issued: self.issued,
            deadline: self.deadline,
            bearer,
            csrf,
            phase: Mutex::new(Phase::Live),
            live_runtime: self.live_runtime.clone(),
        });
        inner.slots[index] = Slot::Ready(entry.clone());
        Ok(entry)
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        if let Some(owner) = self.inner.upgrade() {
            if let Ok(mut inner) = owner.lock() {
                inner.slots.retain(
                    |slot| !matches!(slot, Slot::Reserved { ticket, .. } if *ticket == self.ticket),
                );
            }
        }
    }
}

impl PreSession {
    fn current(&self, now: Instant) -> bool {
        self.live_runtime.is_live() && now >= self.issued && now < self.deadline
    }
    fn reusable(&self, now: Instant) -> bool {
        self.current(now)
            && self
                .phase
                .lock()
                .is_ok_and(|phase| !matches!(*phase, Phase::Consumed))
    }
    fn csrf(&self, now: Instant) -> Option<zeroize::Zeroizing<[u8; 43]>> {
        let phase = self.phase.lock().ok()?;
        if !self.current(now) || matches!(*phase, Phase::Consumed) {
            return None;
        }
        Some(zeroize::Zeroizing::new(*self.csrf.encoded()))
    }
}

impl Claim {
    fn commit_confirmed(mut self, now: Instant) -> Result<(), PreError> {
        self.resolve(now, false)
    }
    fn rollback_confirmed(mut self, now: Instant) -> Result<(), PreError> {
        self.resolve(now, true)
    }
    fn resolve(&mut self, now: Instant, rollback: bool) -> Result<(), PreError> {
        let mut phase = self.entry.phase.lock().map_err(|_| PreError::Unavailable)?;
        if !matches!(*phase, Phase::Claimed(id) if id == self.id) {
            return Err(PreError::Unknown);
        }
        let current = self.entry.current(now);
        *phase = if current && rollback {
            Phase::Live
        } else {
            Phase::Consumed
        };
        self.finished = true;
        if current {
            Ok(())
        } else {
            Err(PreError::Expired)
        }
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        if let Ok(mut phase) = self.entry.phase.lock() {
            if matches!(*phase, Phase::Claimed(id) if id == self.id) {
                *phase = Phase::Consumed;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool(now: Instant) -> Pool {
        Pool::new(RuntimeLiveness::new(), now).unwrap()
    }
    fn ready(pool: &Pool, now: Instant) -> (Arc<PreSession>, Digest) {
        let reservation = pool.reserve(now).unwrap();
        let digest = Secret::generate().unwrap().digest();
        (
            reservation
                .finish(digest, Secret::generate().unwrap(), now)
                .unwrap(),
            digest,
        )
    }

    #[test]
    fn abandoned_randomness_reservation_frees_slot_without_refunding_allowance() {
        let now = Instant::now();
        let pool = pool(now);
        let first = pool.reserve(now).unwrap();
        assert_eq!(pool.inner.lock().unwrap().slots.len(), 1);
        drop(first);
        assert!(pool.inner.lock().unwrap().slots.is_empty());
        let _second = pool.reserve(now).unwrap();
        assert!(matches!(pool.reserve(now), Err(PreError::Busy)));
        assert!(pool.reserve(now + Duration::from_secs(12)).is_ok());
    }

    #[test]
    fn reuse_keeps_csrf_and_deadline_even_during_a_claim_without_issuance_charge() {
        let now = Instant::now();
        let pool = pool(now);
        let (entry, digest) = ready(&pool, now);
        let expected = entry.csrf(now).unwrap();
        let claim = pool.claim(&entry, now).unwrap();
        for _ in 0..10 {
            let reused = pool.reuse(&digest, now + Duration::from_secs(1)).unwrap();
            assert!(Arc::ptr_eq(&reused, &entry));
            assert!(reused.csrf(now).unwrap().as_slice() == expected.as_slice());
        }
        assert!(matches!(pool.claim(&entry, now), Err(PreError::Busy)));
        claim.rollback_confirmed(now).unwrap();
        assert!(pool.reserve(now).is_ok()); // The second issuance allowance remains.
        assert!(pool.reuse(&digest, now + TTL).is_none());
    }

    #[test]
    fn confirmed_rollback_allows_retry_and_confirmed_commit_consumes_once() {
        let now = Instant::now();
        let pool = pool(now);
        let (entry, digest) = ready(&pool, now);
        pool.claim(&entry, now)
            .unwrap()
            .rollback_confirmed(now)
            .unwrap();
        pool.claim(&entry, now)
            .unwrap()
            .commit_confirmed(now)
            .unwrap();
        assert!(pool.reuse(&digest, now).is_none());
        assert!(entry.csrf(now).is_none());
        assert!(pool.claim(&entry, now).is_err());
    }

    #[test]
    fn abandoned_claim_consumes_old_state_and_requires_another_bounded_allocation() {
        let now = Instant::now();
        let pool = pool(now);
        let (entry, digest) = ready(&pool, now);
        drop(pool.claim(&entry, now).unwrap());
        assert!(pool.reuse(&digest, now).is_none());
        assert!(entry.csrf(now).is_none());
        let _new = ready(&pool, now);
        assert!(matches!(pool.reserve(now), Err(PreError::Busy)));
    }

    #[test]
    fn expired_claim_cannot_restore_removed_or_replacement_state() {
        let now = Instant::now();
        let pool = pool(now);
        let (entry, digest) = ready(&pool, now);
        let claim = pool.claim(&entry, now).unwrap();
        let replacement = pool.reserve(now + TTL).unwrap();
        assert!(claim.rollback_confirmed(now + TTL).is_err());
        assert!(entry.csrf(now).is_none()); // Even a stale timestamp cannot revive it.
        assert!(pool.reuse(&digest, now).is_none());
        replacement
            .finish(
                Secret::generate().unwrap().digest(),
                Secret::generate().unwrap(),
                now + TTL,
            )
            .unwrap();
    }

    #[test]
    fn delayed_reservation_completion_never_replaces_a_new_slot() {
        let now = Instant::now();
        let pool = pool(now);
        let stale = pool.reserve(now).unwrap();
        let current = pool.reserve(now + TTL).unwrap();
        assert!(stale
            .finish(
                Secret::generate().unwrap().digest(),
                Secret::generate().unwrap(),
                now + TTL
            )
            .is_err());
        assert_eq!(pool.inner.lock().unwrap().slots.len(), 1);
        current
            .finish(
                Secret::generate().unwrap().digest(),
                Secret::generate().unwrap(),
                now + TTL,
            )
            .unwrap();
    }

    #[test]
    fn owner_drop_denies_captured_entry_claim_and_unfinished_allocation() {
        let now = Instant::now();
        let pool = pool(now);
        let (entry, _) = ready(&pool, now);
        let claim = pool.claim(&entry, now).unwrap();
        let pending = pool.reserve(now).unwrap();
        drop(pool);
        assert!(entry.csrf(now).is_none());
        assert!(claim.rollback_confirmed(now).is_err());
        assert!(pending
            .finish(
                Secret::generate().unwrap().digest(),
                Secret::generate().unwrap(),
                now
            )
            .is_err());
    }

    #[test]
    fn hard_capacity_counts_reserved_slots_before_randomness() {
        let now = Instant::now();
        let pool = pool(now);
        // Defensive capacity oracle, independent of the stricter normal rate.
        let mut inner = pool.inner.lock().unwrap();
        for ticket in 1..=CAPACITY as u64 {
            inner.slots.push(Slot::Reserved {
                ticket,
                issued: now,
                deadline: now + TTL,
            });
        }
        inner.next = CAPACITY as u64;
        drop(inner);
        assert!(matches!(pool.reserve(now), Err(PreError::Full)));
        pool.inner.lock().unwrap().slots.pop();
        assert!(pool.reserve(now).is_ok());
    }

    #[test]
    fn competing_threads_cannot_both_own_the_same_login_claim() {
        use std::{sync::mpsc, thread};
        let now = Instant::now();
        let pool = pool(now);
        let (entry, _) = ready(&pool, now);
        let (report, reports) = mpsc::sync_channel(2);
        thread::scope(|scope| {
            let mut releases = Vec::new();
            let mut handles = Vec::new();
            for _ in 0..2 {
                let (release, wait) = mpsc::sync_channel(1);
                releases.push(release);
                let report = report.clone();
                let pool = &pool;
                let entry = &entry;
                handles.push(scope.spawn(move || {
                    let result = pool.claim(entry, now);
                    let outcome = match &result {
                        Ok(_) => Ok(()),
                        Err(PreError::Busy) => Err(PreError::Busy),
                        Err(_) => Err(PreError::Unknown),
                    };
                    let _ = report.send(outcome);
                    // Keep the winner's claim until both attempts are observed.
                    let _ = wait.recv_timeout(Duration::from_secs(5));
                    drop(result);
                }));
            }
            drop(report);
            let first = reports.recv_timeout(Duration::from_secs(5)).unwrap();
            let second = reports.recv_timeout(Duration::from_secs(5)).unwrap();
            for release in releases {
                let _ = release.send(());
            }
            for handle in handles {
                handle.join().unwrap();
            }
            assert!(matches!(
                (first, second),
                (Ok(()), Err(PreError::Busy)) | (Err(PreError::Busy), Ok(()))
            ));
        });
        assert!(entry.csrf(now).is_none()); // Abandoned winner is consumed.
    }

    #[test]
    fn delayed_randomness_completion_preserves_the_reservation_deadline() {
        let now = Instant::now();
        let pool = pool(now);
        let reservation = pool.reserve(now).unwrap();
        let entry = reservation
            .finish(
                Secret::generate().unwrap().digest(),
                Secret::generate().unwrap(),
                now + Duration::from_secs(20),
            )
            .unwrap();
        assert!(entry.csrf(now + TTL - Duration::from_nanos(1)).is_some());
        assert!(entry.csrf(now + TTL).is_none());
    }
}
