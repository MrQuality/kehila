//! Volatile candidate credentials, not database authorization verdicts.
use crate::lifecycle::RuntimeLiveness;
use crate::secret::{Digest, Secret};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
mod revocation;
use revocation::Cause;

const CAPACITY: usize = 64;
const IDLE: Duration = Duration::from_secs(30 * 60);
const ABSOLUTE: Duration = Duration::from_secs(12 * 60 * 60);

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct Binding {
    pub(crate) installation: String,
    pub(crate) owner: String,
    pub(crate) runtime_generation: u64,
    pub(crate) auth_generation: u64,
}

// The coordinator must obtain this from a fresh fenced SQL read. A value of
// this type cannot prove that provenance, active permissions or lock ownership.
pub(crate) struct Snapshot {
    pub(crate) binding: Binding,
    pub(crate) actor_active: bool,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Kind {
    Browser,
    Cli,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum SessionError {
    Invalid,
    Full,
    Unavailable,
    Unknown,
}

enum Phase {
    Unpublished,
    Active,
    Revoked {
        at: Instant,
        audit_confirmed: bool,
        was_published: bool,
        cause: Cause,
    },
    Retired,
}

struct State {
    phase: Phase,
    activity: Instant,
    idle_deadline: Instant,
    pending: Option<revocation::Batch>,
}

pub(crate) struct Session {
    live_runtime: Arc<RuntimeLiveness>,
    pub(crate) public_id: String,
    pub(crate) binding: Binding,
    pub(crate) kind: Kind,
    bearer: Digest,
    csrf: Option<Secret>,
    issued: Instant,
    absolute_deadline: Instant,
    state: Mutex<State>,
}

pub(crate) struct NewSession {
    pub(crate) public_id: String,
    pub(crate) kind: Kind,
    pub(crate) bearer: Digest,
    pub(crate) csrf: Option<Secret>,
}

pub(crate) struct Registry {
    live_runtime: Arc<RuntimeLiveness>,
    startup: Binding,
    entries: Vec<Arc<Session>>,
}

impl Registry {
    // Only fresh fenced/committed authority reaches this operation. A new
    // generation removes eligibility; existing credentials are never rebound.
    pub(crate) fn observe_generation(&mut self, snapshot: &Snapshot) -> Result<(), SessionError> {
        if !self.live_runtime.is_live()
            || !snapshot.actor_active
            || !self.startup.same_runtime(&snapshot.binding)
            || snapshot.binding.auth_generation < self.startup.auth_generation
        {
            return Err(SessionError::Unavailable);
        }
        if snapshot.binding.auth_generation > self.startup.auth_generation {
            for entry in &self.entries {
                if entry.binding.auth_generation != snapshot.binding.auth_generation {
                    entry.retire();
                }
            }
            self.entries
                .retain(|entry| entry.binding.auth_generation == snapshot.binding.auth_generation);
            self.startup.auth_generation = snapshot.binding.auth_generation;
        }
        if self.live_runtime.is_live() {
            Ok(())
        } else {
            Err(SessionError::Unavailable)
        }
    }
    pub(crate) fn new(
        startup: Binding,
        live_runtime: Arc<RuntimeLiveness>,
    ) -> Result<Self, SessionError> {
        if !startup.valid() {
            return Err(SessionError::Invalid);
        }
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(CAPACITY)
            .map_err(|_| SessionError::Unavailable)?;
        Ok(Self {
            startup,
            entries,
            live_runtime,
        })
    }

    pub(crate) fn stop(&mut self) {
        self.live_runtime.stop();
        self.entries.clear();
    }

    pub(crate) fn reserve(
        &mut self,
        new: NewSession,
        snapshot: &Snapshot,
        now: Instant,
    ) -> Result<Arc<Session>, SessionError> {
        if !self.live_runtime.is_live() {
            return Err(SessionError::Unavailable);
        }
        if !snapshot.actor_active
            || !snapshot.binding.valid()
            || !self.startup.same_runtime(&snapshot.binding)
            || self.startup.auth_generation != snapshot.binding.auth_generation
            || new.public_id.is_empty()
            || new.public_id.len() > 128
            || !new.public_id.bytes().all(|byte| byte.is_ascii_graphic())
            || (new.kind == Kind::Browser) != new.csrf.is_some()
            || new
                .csrf
                .as_ref()
                .is_some_and(|csrf| csrf.digest().matches(&new.bearer))
        {
            return Err(SessionError::Invalid);
        }
        // Mark captured references retired before removing their lookup slots.
        // No registry/state guard crosses an await or database operation.
        let mut expired = [false; CAPACITY];
        let mut duplicate = false;
        let mut oldest: Option<(Instant, String, usize)> = None;
        for (index, entry) in self.entries.iter().enumerate() {
            let mut state = entry.state.lock().map_err(|_| SessionError::Unavailable)?;
            if now >= state.idle_deadline
                || now >= entry.absolute_deadline
                || matches!(state.phase, Phase::Retired)
            {
                state.phase = Phase::Retired;
                expired[index] = true;
                continue;
            }
            duplicate |= entry.public_id == new.public_id || entry.bearer.matches(&new.bearer);
            if let Phase::Revoked {
                at,
                audit_confirmed: true,
                ..
            } = state.phase
            {
                let key = (at, entry.public_id.clone(), index);
                if oldest.as_ref().is_none_or(|previous| &key < previous) {
                    oldest = Some(key);
                }
            }
        }
        for index in (0..self.entries.len()).rev() {
            if expired[index] {
                self.entries.remove(index);
            }
        }
        if duplicate {
            return Err(SessionError::Invalid);
        }
        if self.entries.len() == CAPACITY {
            // Full after expiry cleanup means no indices shifted above.
            let index = oldest.ok_or(SessionError::Full)?.2;
            self.entries[index]
                .state
                .lock()
                .map_err(|_| SessionError::Unavailable)?
                .phase = Phase::Retired;
            self.entries.remove(index);
        }
        let idle_deadline = now.checked_add(IDLE).ok_or(SessionError::Unavailable)?;
        let absolute_deadline = now.checked_add(ABSOLUTE).ok_or(SessionError::Unavailable)?;
        let entry = Arc::new(Session {
            live_runtime: self.live_runtime.clone(),
            public_id: new.public_id,
            binding: snapshot.binding.clone(),
            kind: new.kind,
            bearer: new.bearer,
            csrf: new.csrf,
            issued: now,
            absolute_deadline,
            state: Mutex::new(State {
                phase: Phase::Unpublished,
                activity: now,
                idle_deadline,
                pending: None,
            }),
        });
        self.entries.push(entry.clone());
        Ok(entry)
    }

    pub(crate) fn publish(
        &mut self,
        entry: &Arc<Session>,
        snapshot: &Snapshot,
        now: Instant,
    ) -> Result<(), SessionError> {
        if !self.live_runtime.is_live() {
            return Err(SessionError::Unavailable);
        }
        if !self.entries.iter().any(|stored| Arc::ptr_eq(stored, entry)) {
            return Err(SessionError::Unknown);
        }
        let mut state = entry.state.lock().map_err(|_| SessionError::Unavailable)?;
        if !entry.current(snapshot, now, &state) || !matches!(state.phase, Phase::Unpublished) {
            return Err(SessionError::Unknown);
        }
        state.phase = Phase::Active;
        Ok(())
    }

    pub(crate) fn discard_unpublished(&mut self, entry: &Arc<Session>) -> Result<(), SessionError> {
        let index = self
            .entries
            .iter()
            .position(|stored| Arc::ptr_eq(stored, entry))
            .ok_or(SessionError::Unknown)?;
        let mut state = entry.state.lock().map_err(|_| SessionError::Unavailable)?;
        if !matches!(
            state.phase,
            Phase::Unpublished
                | Phase::Revoked {
                    was_published: false,
                    ..
                }
        ) {
            return Err(SessionError::Invalid);
        }
        state.phase = Phase::Retired;
        drop(state);
        self.entries.remove(index);
        Ok(())
    }

    pub(crate) fn lookup(&self, bearer: &Digest, kind: Kind, now: Instant) -> Option<Arc<Session>> {
        if !self.live_runtime.is_live() {
            return None;
        }
        for entry in &self.entries {
            if entry.kind != kind || !entry.bearer.matches(bearer) {
                continue;
            }
            let state = entry.state.lock().ok()?;
            if now >= entry.issued
                && now >= state.activity
                && now < state.idle_deadline
                && now < entry.absolute_deadline
                && matches!(
                    state.phase,
                    Phase::Active
                        | Phase::Revoked {
                            was_published: true,
                            cause: Cause::Logout,
                            ..
                        }
                )
            {
                return Some(entry.clone());
            }
        }
        None
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Binding {
    fn valid(&self) -> bool {
        [&self.installation, &self.owner]
            .iter()
            .all(|value| !value.is_empty() && value.len() <= 128 && !value.contains('\0'))
            && self.runtime_generation > 0
            && self.auth_generation > 0
    }
    fn same_runtime(&self, other: &Self) -> bool {
        self.installation == other.installation
            && self.owner == other.owner
            && self.runtime_generation == other.runtime_generation
    }
}

impl Session {
    // Delivery abandonment is terminal, including after local publication.
    // No bearer or retained reference can restore this entry.
    pub(crate) fn retire(&self) {
        match self.state.lock() {
            Ok(mut state) => state.phase = Phase::Retired,
            Err(_) => self.live_runtime.stop(),
        }
    }
    pub(crate) fn csrf_for_bootstrap(
        &self,
        snapshot: &Snapshot,
        now: Instant,
    ) -> Option<zeroize::Zeroizing<[u8; 43]>> {
        let state = self.state.lock().ok()?;
        if self.kind != Kind::Browser
            || !self.current(snapshot, now, &state)
            || !matches!(
                state.phase,
                Phase::Active
                    | Phase::Revoked {
                        was_published: true,
                        cause: Cause::Logout,
                        ..
                    }
            )
        {
            return None;
        }
        Some(zeroize::Zeroizing::new(*self.csrf.as_ref()?.encoded()))
    }
    fn current(&self, snapshot: &Snapshot, now: Instant, state: &State) -> bool {
        self.live_runtime.is_live()
            && snapshot.actor_active
            && snapshot.binding == self.binding
            && now >= self.issued
            && now >= state.activity
            && now < state.idle_deadline
            && now < self.absolute_deadline
    }
    pub(crate) fn eligible(&self, snapshot: &Snapshot, now: Instant) -> bool {
        self.state.lock().is_ok_and(|state| {
            self.current(snapshot, now, &state) && matches!(state.phase, Phase::Active)
        })
    }
    pub(crate) fn revoke_for_logout(
        &self,
        snapshot: &Snapshot,
        now: Instant,
    ) -> Result<(), SessionError> {
        let mut state = self.state.lock().map_err(|_| SessionError::Unavailable)?;
        if !self.current(snapshot, now, &state) {
            return Err(SessionError::Unknown);
        }
        match state.phase {
            Phase::Unpublished | Phase::Active => {
                let was_published = matches!(state.phase, Phase::Active);
                state.phase = Phase::Revoked {
                    at: now,
                    audit_confirmed: false,
                    was_published,
                    cause: Cause::Logout,
                };
                Ok(())
            }
            Phase::Revoked {
                cause: Cause::Logout,
                ..
            } => Ok(()),
            Phase::Revoked { .. } => Err(SessionError::Unknown),
            Phase::Retired => Err(SessionError::Unknown),
        }
    }
    // Called only after the coordinator confirms the matching fixed SQL audit.
    pub(crate) fn confirm_logout(
        &self,
        snapshot: &Snapshot,
        now: Instant,
    ) -> Result<(), SessionError> {
        let mut state = self.state.lock().map_err(|_| SessionError::Unavailable)?;
        if !self.current(snapshot, now, &state) {
            return Err(SessionError::Unknown);
        }
        if let Phase::Revoked {
            audit_confirmed,
            cause: Cause::Logout,
            ..
        } = &mut state.phase
        {
            *audit_confirmed = true;
            Ok(())
        } else {
            Err(SessionError::Unknown)
        }
    }
    pub(crate) fn logout_retry(&self, snapshot: &Snapshot, now: Instant) -> bool {
        self.state.lock().is_ok_and(|state| {
            self.current(snapshot, now, &state)
                && matches!(
                    state.phase,
                    Phase::Revoked {
                        was_published: true,
                        cause: Cause::Logout,
                        ..
                    }
                )
        })
    }
    fn successful_business(&self, snapshot: &Snapshot, now: Instant) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        if !self.current(snapshot, now, &state) || !matches!(state.phase, Phase::Active) {
            return false;
        }
        let Some(deadline) = now.checked_add(IDLE) else {
            return false;
        };
        state.activity = now;
        state.idle_deadline = deadline.min(self.absolute_deadline);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> Snapshot {
        Snapshot {
            binding: Binding {
                installation: "installation".into(),
                owner: "owner".into(),
                runtime_generation: 1,
                auth_generation: 1,
            },
            actor_active: true,
        }
    }

    fn candidate(public_id: &str) -> (NewSession, Digest) {
        let token = Secret::generate().unwrap();
        let digest = token.digest();
        (
            NewSession {
                public_id: public_id.into(),
                kind: Kind::Cli,
                bearer: digest,
                csrf: None,
            },
            digest,
        )
    }

    #[test]
    fn unpublished_entries_never_lookup_and_failed_issuance_retires_captured_reference() {
        let now = Instant::now();
        let snapshot = snapshot();
        let mut registry = Registry::new(snapshot.binding.clone(), RuntimeLiveness::new()).unwrap();
        let (new, digest) = candidate("session");
        let entry = registry.reserve(new, &snapshot, now).unwrap();
        assert!(registry.lookup(&digest, Kind::Cli, now).is_none());
        assert!(!entry.eligible(&snapshot, now));
        registry.discard_unpublished(&entry).unwrap();
        assert!(registry.entries.is_empty());
        assert!(registry.publish(&entry, &snapshot, now).is_err());
        assert!(!entry.eligible(&snapshot, now));
    }

    #[test]
    fn publication_kind_and_exact_idle_expiry_are_enforced() {
        let now = Instant::now();
        let snapshot = snapshot();
        let mut registry = Registry::new(snapshot.binding.clone(), RuntimeLiveness::new()).unwrap();
        let (new, digest) = candidate("session");
        let entry = registry.reserve(new, &snapshot, now).unwrap();
        registry.publish(&entry, &snapshot, now).unwrap();
        assert!(registry.lookup(&digest, Kind::Browser, now).is_none());
        assert!(registry.lookup(&digest, Kind::Cli, now).is_some());
        assert!(entry.eligible(&snapshot, now + IDLE - Duration::from_nanos(1)));
        assert!(!entry.eligible(&snapshot, now + IDLE));
        assert!(!entry.successful_business(&snapshot, now + IDLE));
    }

    #[test]
    fn revocation_is_seen_by_captured_reference_and_retains_only_logout_retry() {
        let now = Instant::now();
        let snapshot = snapshot();
        let mut registry = Registry::new(snapshot.binding.clone(), RuntimeLiveness::new()).unwrap();
        let (new, _) = candidate("session");
        let entry = registry.reserve(new, &snapshot, now).unwrap();
        registry.publish(&entry, &snapshot, now).unwrap();
        let captured = entry.clone();
        entry.revoke_for_logout(&snapshot, now).unwrap();
        assert!(!captured.eligible(&snapshot, now));
        assert!(captured.logout_retry(&snapshot, now));
        entry.confirm_logout(&snapshot, now).unwrap();
        assert!(!captured.successful_business(&snapshot, now));
        assert!(!captured.logout_retry(&snapshot, now + IDLE));
    }

    #[test]
    fn full_registry_never_evicts_active_or_unconfirmed_but_reclaims_confirmed_logout() {
        let now = Instant::now();
        let snapshot = snapshot();
        let mut registry = Registry::new(snapshot.binding.clone(), RuntimeLiveness::new()).unwrap();
        let mut captured = Vec::new();
        for index in 0..CAPACITY {
            let (new, _) = candidate(&format!("session-{index}"));
            let entry = registry.reserve(new, &snapshot, now).unwrap();
            registry.publish(&entry, &snapshot, now).unwrap();
            captured.push(entry);
        }
        assert!(matches!(
            registry.reserve(candidate("excess").0, &snapshot, now),
            Err(SessionError::Full)
        ));
        captured[0].revoke_for_logout(&snapshot, now).unwrap();
        assert!(matches!(
            registry.reserve(candidate("excess").0, &snapshot, now),
            Err(SessionError::Full)
        ));
        captured[0].confirm_logout(&snapshot, now).unwrap();
        registry
            .reserve(candidate("replacement").0, &snapshot, now)
            .unwrap();
        assert_eq!(registry.entries.len(), CAPACITY);
        assert!(!captured[0].logout_retry(&snapshot, now));
        assert!(!captured[0].eligible(&snapshot, now));
        assert!(captured[1].eligible(&snapshot, now));
    }

    #[test]
    fn confirmed_reclamation_orders_by_revocation_time_then_public_id() {
        let now = Instant::now();
        let snapshot = snapshot();
        let mut registry = Registry::new(snapshot.binding.clone(), RuntimeLiveness::new()).unwrap();
        let mut captured = Vec::new();
        for index in 0..CAPACITY {
            let id = match index {
                0 => "b".to_owned(),
                1 => "a".to_owned(),
                _ => format!("filler-{index}"),
            };
            let entry = registry.reserve(candidate(&id).0, &snapshot, now).unwrap();
            registry.publish(&entry, &snapshot, now).unwrap();
            captured.push(entry);
        }
        for entry in &captured[..2] {
            entry.revoke_for_logout(&snapshot, now).unwrap();
            entry.confirm_logout(&snapshot, now).unwrap();
        }
        registry
            .reserve(candidate("replacement").0, &snapshot, now)
            .unwrap();
        assert!(captured[0].logout_retry(&snapshot, now));
        assert!(!captured[1].logout_retry(&snapshot, now));
    }

    #[test]
    fn activity_can_extend_idle_but_never_absolute_expiry_or_revocation() {
        let now = Instant::now();
        let snapshot = snapshot();
        let mut registry = Registry::new(snapshot.binding.clone(), RuntimeLiveness::new()).unwrap();
        let entry = registry
            .reserve(candidate("session").0, &snapshot, now)
            .unwrap();
        registry.publish(&entry, &snapshot, now).unwrap();
        for step in 1..=24 {
            assert!(entry.successful_business(&snapshot, now + Duration::from_secs(step * 29 * 60)));
        }
        assert!(entry.eligible(&snapshot, now + ABSOLUTE - Duration::from_nanos(1)));
        assert!(!entry.eligible(&snapshot, now + ABSOLUTE));
        assert!(!entry.successful_business(&snapshot, now + ABSOLUTE));
    }

    #[test]
    fn generation_and_owner_changes_or_inactive_actor_deny_captured_reference() {
        let now = Instant::now();
        let snapshot = snapshot();
        let mut registry = Registry::new(snapshot.binding.clone(), RuntimeLiveness::new()).unwrap();
        let entry = registry
            .reserve(candidate("session").0, &snapshot, now)
            .unwrap();
        registry.publish(&entry, &snapshot, now).unwrap();
        for field in 0..5 {
            let mut changed = Snapshot {
                binding: snapshot.binding.clone(),
                actor_active: true,
            };
            match field {
                0 => changed.binding.auth_generation += 1,
                1 => changed.binding.runtime_generation += 1,
                2 => changed.binding.owner = "different".into(),
                3 => changed.binding.installation = "different".into(),
                _ => changed.actor_active = false,
            }
            assert!(!entry.eligible(&changed, now));
            assert!(!entry.successful_business(&changed, now));
        }
    }

    #[test]
    fn committed_generation_retires_captured_entries_and_reclaims_full_capacity() {
        let now = Instant::now();
        let old = snapshot();
        let mut registry = Registry::new(old.binding.clone(), RuntimeLiveness::new()).unwrap();
        let mut captured = Vec::new();
        for index in 0..CAPACITY {
            let entry = registry
                .reserve(candidate(&format!("old-{index}")).0, &old, now)
                .unwrap();
            registry.publish(&entry, &old, now).unwrap();
            captured.push(entry);
        }
        let mut fresh = snapshot();
        fresh.binding.auth_generation += 1;
        registry.observe_generation(&fresh).unwrap();
        for entry in captured {
            assert!(!entry.eligible(&old, now));
        }
        for index in 0..CAPACITY {
            registry
                .reserve(candidate(&format!("new-{index}")).0, &fresh, now)
                .unwrap();
        }
        assert!(registry
            .reserve(candidate("overflow").0, &fresh, now)
            .is_err());
    }

    #[test]
    fn expired_entries_are_reclaimed_and_old_references_cannot_be_resurrected() {
        let now = Instant::now();
        let snapshot = snapshot();
        let mut registry = Registry::new(snapshot.binding.clone(), RuntimeLiveness::new()).unwrap();
        let old = registry
            .reserve(candidate("old").0, &snapshot, now)
            .unwrap();
        registry.publish(&old, &snapshot, now).unwrap();
        registry
            .reserve(candidate("new").0, &snapshot, now + IDLE)
            .unwrap();
        assert_eq!(registry.entries.len(), 1);
        assert!(!old.eligible(&snapshot, now));
        assert!(!old.successful_business(&snapshot, now));
    }

    #[test]
    fn stopped_runtime_cannot_publish_or_accept_cached_or_new_credentials() {
        let now = Instant::now();
        let snapshot = snapshot();
        let live = RuntimeLiveness::new();
        let mut registry = Registry::new(snapshot.binding.clone(), live.clone()).unwrap();
        let (new, digest) = candidate("active");
        let active = registry.reserve(new, &snapshot, now).unwrap();
        registry.publish(&active, &snapshot, now).unwrap();
        let unpublished = registry
            .reserve(candidate("pending").0, &snapshot, now)
            .unwrap();
        live.stop();
        assert!(!active.eligible(&snapshot, now));
        assert!(registry.lookup(&digest, Kind::Cli, now).is_none());
        assert!(registry.publish(&unpublished, &snapshot, now).is_err());
        assert!(registry
            .reserve(candidate("new").0, &snapshot, now)
            .is_err());
        registry.stop();
        assert!(registry.entries.is_empty());
    }

    #[test]
    fn revoked_unpublished_entry_never_becomes_lookup_or_logout_eligible() {
        let now = Instant::now();
        let snapshot = snapshot();
        let mut registry = Registry::new(snapshot.binding.clone(), RuntimeLiveness::new()).unwrap();
        let (new, digest) = candidate("unpublished");
        let entry = registry.reserve(new, &snapshot, now).unwrap();
        entry.revoke_for_logout(&snapshot, now).unwrap();
        assert!(registry.lookup(&digest, Kind::Cli, now).is_none());
        assert!(!entry.logout_retry(&snapshot, now));
        assert!(registry.publish(&entry, &snapshot, now).is_err());
        registry.discard_unpublished(&entry).unwrap();
        assert!(registry.entries.is_empty());
    }

    #[test]
    fn browser_requires_distinct_csrf_and_bootstrap_never_renews_expiry() {
        let now = Instant::now();
        let snapshot = snapshot();
        let mut registry = Registry::new(snapshot.binding.clone(), RuntimeLiveness::new()).unwrap();
        let token = Secret::generate().unwrap();
        let digest = token.digest();
        let reused = NewSession {
            public_id: "reused".into(),
            kind: Kind::Browser,
            bearer: digest,
            csrf: Some(token),
        };
        assert!(matches!(
            registry.reserve(reused, &snapshot, now),
            Err(SessionError::Invalid)
        ));
        let csrf = Secret::generate().unwrap();
        let expected = zeroize::Zeroizing::new(*csrf.encoded());
        let new = NewSession {
            public_id: "browser".into(),
            kind: Kind::Browser,
            bearer: digest,
            csrf: Some(csrf),
        };
        let entry = registry.reserve(new, &snapshot, now).unwrap();
        assert!(entry.csrf_for_bootstrap(&snapshot, now).is_none());
        registry.publish(&entry, &snapshot, now).unwrap();
        assert!(
            entry.csrf_for_bootstrap(&snapshot, now).unwrap().as_slice() == expected.as_slice()
        );
        entry
            .revoke_for_logout(&snapshot, now + Duration::from_secs(60))
            .unwrap();
        assert!(
            entry
                .csrf_for_bootstrap(&snapshot, now + IDLE - Duration::from_nanos(1))
                .unwrap()
                .as_slice()
                == expected.as_slice()
        );
        assert!(entry.csrf_for_bootstrap(&snapshot, now + IDLE).is_none());
    }

    #[test]
    fn cli_has_no_csrf_bootstrap_and_mixed_kind_state_is_rejected() {
        let now = Instant::now();
        let snapshot = snapshot();
        let mut registry = Registry::new(snapshot.binding.clone(), RuntimeLiveness::new()).unwrap();
        let (mut new, _) = candidate("mixed");
        new.csrf = Some(Secret::generate().unwrap());
        assert!(matches!(
            registry.reserve(new, &snapshot, now),
            Err(SessionError::Invalid)
        ));
        let entry = registry
            .reserve(candidate("cli").0, &snapshot, now)
            .unwrap();
        registry.publish(&entry, &snapshot, now).unwrap();
        assert!(entry.csrf_for_bootstrap(&snapshot, now).is_none());
    }

    #[test]
    fn dropping_registry_invalidates_active_and_retained_browser_references() {
        let now = Instant::now();
        let snapshot = snapshot();
        let mut registry = Registry::new(snapshot.binding.clone(), RuntimeLiveness::new()).unwrap();
        let active = registry
            .reserve(candidate("active").0, &snapshot, now)
            .unwrap();
        registry.publish(&active, &snapshot, now).unwrap();
        let token = Secret::generate().unwrap();
        let browser = registry
            .reserve(
                NewSession {
                    public_id: "browser".into(),
                    kind: Kind::Browser,
                    bearer: token.digest(),
                    csrf: Some(Secret::generate().unwrap()),
                },
                &snapshot,
                now,
            )
            .unwrap();
        registry.publish(&browser, &snapshot, now).unwrap();
        browser.revoke_for_logout(&snapshot, now).unwrap();
        assert!(active.eligible(&snapshot, now));
        assert!(browser.logout_retry(&snapshot, now));
        assert!(browser.csrf_for_bootstrap(&snapshot, now).is_some());
        drop(registry);
        assert!(!active.eligible(&snapshot, now));
        assert!(!active.successful_business(&snapshot, now));
        assert!(!browser.logout_retry(&snapshot, now));
        assert!(browser.csrf_for_bootstrap(&snapshot, now).is_none());
    }

    #[test]
    fn duplicate_live_public_id_or_bearer_cannot_allocate_another_identity() {
        let now = Instant::now();
        let snapshot = snapshot();
        let mut registry = Registry::new(snapshot.binding.clone(), RuntimeLiveness::new()).unwrap();
        let (new, digest) = candidate("original");
        let original = registry.reserve(new, &snapshot, now).unwrap();
        registry.publish(&original, &snapshot, now).unwrap();
        assert!(matches!(
            registry.reserve(candidate("original").0, &snapshot, now),
            Err(SessionError::Invalid)
        ));
        for kind in [Kind::Cli, Kind::Browser] {
            let csrf = (kind == Kind::Browser).then(|| Secret::generate().unwrap());
            let duplicate = NewSession {
                public_id: "different".into(),
                kind,
                bearer: digest,
                csrf,
            };
            assert!(matches!(
                registry.reserve(duplicate, &snapshot, now),
                Err(SessionError::Invalid)
            ));
        }
        assert_eq!(registry.entries.len(), 1);
        assert!(original.eligible(&snapshot, now));
        assert!(registry.lookup(&digest, Kind::Browser, now).is_none());
    }
    fn published(
        registry: &mut Registry,
        snapshot: &Snapshot,
        id: &str,
        now: Instant,
    ) -> Arc<Session> {
        let entry = registry.reserve(candidate(id).0, snapshot, now).unwrap();
        registry.publish(&entry, snapshot, now).unwrap();
        entry
    }

    #[test]
    fn revoke_other_batch_is_bounded_and_never_grants_own_logout_retry() {
        let now = Instant::now();
        let snapshot = snapshot();
        let mut registry = Registry::new(snapshot.binding.clone(), RuntimeLiveness::new()).unwrap();
        let initiator = published(&mut registry, &snapshot, "initiator", now);
        let targets: Vec<_> = (0..CAPACITY - 1)
            .map(|index| published(&mut registry, &snapshot, &format!("other-{index}"), now))
            .collect();
        let batch = registry.revoke_others(&initiator, &snapshot, now).unwrap();
        assert_eq!(batch.len(), 63);
        assert!(initiator.eligible(&snapshot, now));
        for entry in &targets {
            assert!(!entry.eligible(&snapshot, now));
            assert!(!entry.logout_retry(&snapshot, now));
            assert!(registry.lookup(&entry.bearer, entry.kind, now).is_none());
            assert!(entry.revoke_for_logout(&snapshot, now).is_err());
        }
        assert!(matches!(
            registry.reserve(candidate("excess").0, &snapshot, now),
            Err(SessionError::Full)
        ));
        registry
            .confirm_revoke_others(&initiator, &snapshot, &batch, now)
            .unwrap();
        let replacement = published(&mut registry, &snapshot, "replacement", now);
        assert!(replacement.eligible(&snapshot, now));
        assert!(initiator.eligible(&snapshot, now));
        assert!(targets.iter().all(|entry| !entry.eligible(&snapshot, now)));
    }

    #[test]
    fn pending_batch_freezes_targets_and_deadline_despite_later_activity() {
        let now = Instant::now();
        let snapshot = snapshot();
        let mut registry = Registry::new(snapshot.binding.clone(), RuntimeLiveness::new()).unwrap();
        let initiator = published(&mut registry, &snapshot, "initiator", now);
        let original = published(&mut registry, &snapshot, "original", now);
        let batch = registry.revoke_others(&initiator, &snapshot, now).unwrap();
        assert_eq!(batch.len(), 1);
        let later = now + IDLE / 2;
        assert!(initiator.successful_business(&snapshot, later));
        let new = published(&mut registry, &snapshot, "new", later);
        assert!(
            registry
                .revoke_others(&initiator, &snapshot, later)
                .unwrap()
                == batch
        );
        assert!(!original.eligible(&snapshot, later));
        assert!(new.eligible(&snapshot, later));
        assert!(initiator.eligible(&snapshot, now + IDLE));
        assert!(matches!(
            registry.revoke_others(&initiator, &snapshot, now + IDLE),
            Err(SessionError::Unknown)
        ));
        assert!(registry
            .confirm_revoke_others(&initiator, &snapshot, &batch, now + IDLE)
            .is_err());
        assert!(new.eligible(&snapshot, now + IDLE));
    }

    #[test]
    fn another_batch_cannot_replace_an_existing_revocation_initiator() {
        let now = Instant::now();
        let snapshot = snapshot();
        let mut registry = Registry::new(snapshot.binding.clone(), RuntimeLiveness::new()).unwrap();
        let first = published(&mut registry, &snapshot, "first", now);
        let target = published(&mut registry, &snapshot, "target", now);
        registry.revoke_others(&first, &snapshot, now).unwrap();
        let second = published(&mut registry, &snapshot, "second", now);
        let batch = registry.revoke_others(&second, &snapshot, now).unwrap();
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].public_id, "first");
        assert!(
            matches!(&target.state.lock().unwrap().phase, Phase::Revoked { cause: Cause::Other(original), .. } if original == "first")
        );
        assert!(!target.logout_retry(&snapshot, now));
    }
}
