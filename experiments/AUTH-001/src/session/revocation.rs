//! One fixed, lifetime-bounded revoke-other batch per initiating entry.
use super::*;

pub(super) enum Cause {
    Logout,
    Other(String),
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct Target {
    pub(crate) public_id: String,
    pub(crate) kind: Kind,
}

pub(super) struct Batch {
    deadline: Instant,
    targets: Vec<Target>,
}

impl Registry {
    pub(crate) fn revoke_others(
        &mut self,
        initiator: &Arc<Session>,
        snapshot: &Snapshot,
        now: Instant,
    ) -> Result<Vec<Target>, SessionError> {
        self.observe_generation(snapshot)?;
        if !self
            .entries
            .iter()
            .any(|entry| Arc::ptr_eq(entry, initiator))
        {
            return Err(SessionError::Unknown);
        }
        let mut current = initiator
            .state
            .lock()
            .map_err(|_| SessionError::Unavailable)?;
        if !initiator.current(snapshot, now, &current) || !matches!(current.phase, Phase::Active) {
            return Err(SessionError::Unknown);
        }
        if let Some(batch) = &mut current.pending {
            if now >= batch.deadline {
                // Drop fixed IDs at expiry; the old batch cannot be revived or
                // silently replaced by a new selection on a retry.
                batch.targets.clear();
                return Err(SessionError::Unknown);
            }
            return Ok(batch.targets.clone());
        }
        let mut targets = Vec::new();
        targets
            .try_reserve_exact(CAPACITY - 1)
            .map_err(|_| SessionError::Unavailable)?;
        for entry in &self.entries {
            if Arc::ptr_eq(entry, initiator) {
                continue;
            }
            let mut state = match entry.state.lock() {
                Ok(state) => state,
                Err(_) => {
                    self.live_runtime.stop();
                    return Err(SessionError::Unavailable);
                }
            };
            if !entry.current(snapshot, now, &state) || !matches!(state.phase, Phase::Active) {
                continue;
            }
            targets.push(Target {
                public_id: entry.public_id.clone(),
                kind: entry.kind,
            });
            state.phase = Phase::Revoked {
                at: now,
                audit_confirmed: false,
                was_published: true,
                cause: Cause::Other(initiator.public_id.clone()),
            };
        }
        current.pending = Some(Batch {
            deadline: current.idle_deadline.min(initiator.absolute_deadline),
            targets,
        });
        Ok(current
            .pending
            .as_ref()
            .ok_or(SessionError::Unavailable)?
            .targets
            .clone())
    }

    pub(crate) fn confirm_revoke_others(
        &mut self,
        initiator: &Arc<Session>,
        snapshot: &Snapshot,
        targets: &[Target],
        now: Instant,
    ) -> Result<(), SessionError> {
        if !self.live_runtime.is_live()
            || !self
                .entries
                .iter()
                .any(|entry| Arc::ptr_eq(entry, initiator))
        {
            return Err(SessionError::Unknown);
        }
        let mut current = initiator
            .state
            .lock()
            .map_err(|_| SessionError::Unavailable)?;
        if !initiator.current(snapshot, now, &current) || !matches!(current.phase, Phase::Active) {
            return Err(SessionError::Unknown);
        }
        let batch = current.pending.as_ref().ok_or(SessionError::Unknown)?;
        if now >= batch.deadline || batch.targets != targets {
            return Err(SessionError::Unknown);
        }
        for target in targets {
            // Reclaimed/expired targets stay absent. Confirming durable audit
            // must never recreate a lookup entry or overwrite first provenance.
            if let Some(entry) = self
                .entries
                .iter()
                .find(|entry| entry.public_id == target.public_id && entry.kind == target.kind)
            {
                let mut state = entry.state.lock().map_err(|_| SessionError::Unavailable)?;
                if let Phase::Revoked {
                    audit_confirmed,
                    cause: Cause::Other(original),
                    ..
                } = &mut state.phase
                {
                    if original == &initiator.public_id {
                        *audit_confirmed = true;
                    }
                }
            }
        }
        current.pending = None;
        Ok(())
    }
}
