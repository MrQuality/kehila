//! Fixed scoped-notice release. No payload, replacement scope or renewed time.
use super::replies::Notice;
use super::*;
#[derive(Clone)]
pub(super) struct NoticeOrigin {
    project: String,
    entry: Arc<Session>,
    deadline: Instant,
}
impl NoticeOrigin {
    pub(super) fn capture(
        intent: &Intent,
        entry: Arc<Session>,
        deadline: Instant,
    ) -> Result<Self, Error> {
        let project = intent.project();
        // Bound the owned copy even for an invalid internal caller. This check
        // establishes no grant and does not replace edge/domain admission.
        if project.is_empty() || project.len() > 128 || project.contains('\0') {
            return Err(Error::Invalid);
        }
        Ok(Self {
            project: project.to_owned(),
            entry,
            deadline,
        })
    }
}
// Consumers cannot provide bytes, alter scope or replace the original session.
pub(in crate::coordinator) struct ScopedNotice {
    origin: NoticeOrigin,
    reply: EncodedReply,
}
impl ScopedNotice {
    pub(super) fn from_catalogue(
        kind: Notice,
        origin: NoticeOrigin,
        catalogue: &Catalogue,
    ) -> Self {
        Self {
            origin,
            reply: catalogue.reply(kind),
        }
    }
    #[cfg(test)]
    pub(in crate::coordinator) fn test_parts(&self) -> (u16, &[u8]) {
        self.reply.test_parts()
    }
}
// Diagnostics remain typed; every public failure suppresses the original
// notice. This distinct type cannot recursively select another scoped notice.
pub(in crate::coordinator) enum NoticeFailure {
    Owner(Error),
    Eligibility {
        primary: Error,
        rollback: Option<DbError>,
    },
    Scope {
        primary: NativeError,
        rollback: Option<DbError>,
    },
    Cleanup(DbError),
}
impl std::fmt::Debug for NoticeFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Owner(error) => f.debug_tuple("Owner").field(error).finish(),
            Self::Eligibility { primary, rollback } => f
                .debug_struct("Eligibility")
                .field("primary", primary)
                .field("rollback", rollback)
                .finish(),
            Self::Scope { primary, rollback } => f
                .debug_struct("Scope")
                .field("primary", primary)
                .field("rollback", rollback)
                .finish(),
            Self::Cleanup(error) => f.debug_tuple("Cleanup").field(error).finish(),
        }
    }
}
impl NoticeFailure {
    pub(in crate::coordinator) fn generic_error(&self) -> Error {
        match self {
            Self::Owner(error @ (Error::Unauthorized | Error::UnknownCredential | Error::Busy)) => {
                *error
            }
            Self::Eligibility {
                primary,
                rollback: None,
            } => match primary {
                Error::Project(crate::project::Failure::Forbidden) => *primary,
                _ => Error::Unauthorized,
            },
            Self::Scope {
                primary: NativeError::Rejected(crate::project::Failure::Forbidden),
                rollback: None,
            } => Error::Project(crate::project::Failure::Forbidden),
            Self::Scope {
                primary: NativeError::Database(DbError::Rejected(code)),
                rollback: None,
            } if code == "42501" => Error::Project(crate::project::Failure::Forbidden),
            _ => Error::Unavailable,
        }
    }
}
enum Denied {
    Eligibility(Error),
    Scope(NativeError),
}
impl Engine {
    pub(in crate::coordinator) async fn deliver_item_notice(
        &self,
        credential: &Credential,
        notice: ScopedNotice,
    ) -> Result<NativeDelivery, NoticeFailure> {
        let ScopedNotice { origin, reply } = notice;
        let deadline = origin.deadline;
        self.check(deadline).map_err(NoticeFailure::Owner)?;
        let candidate = self.candidate(credential).map_err(NoticeFailure::Owner)?;
        if !Arc::ptr_eq(&candidate, &origin.entry) {
            return Err(NoticeFailure::Owner(Error::Unauthorized));
        }
        let fresh = {
            let mut database = tokio::time::timeout_at(
                tokio::time::Instant::from_std(deadline),
                self.database.lock(),
            )
            .await
            .map_err(|_| NoticeFailure::Owner(Error::Unavailable))?;
            let binding = &origin.entry.binding;
            let mut transaction = database
                .read_snapshot(
                    &binding.installation,
                    binding.runtime_generation,
                    binding.auth_generation,
                    deadline,
                )
                .await
                .map_err(|error| NoticeFailure::Owner(fence_error(error)))?;
            let fresh = snapshot(transaction.authority());
            let checked = match self.project_eligibility(&origin.entry, credential, &fresh) {
                Err(error) => Err(Denied::Eligibility(error)),
                Ok(()) => transaction
                    .authorize_item_scope(&origin.project)
                    .await
                    .map_err(Denied::Scope),
            };
            let rollback = transaction.rollback().await.err();
            if let Err(Denied::Scope(primary)) = &checked {
                self.item_native_failure(primary)
            }
            if rollback.is_some() {
                self.live.fail(crate::lifecycle::FatalCause::OwnedExecution)
            }
            match (checked, rollback) {
                (Err(Denied::Eligibility(primary)), rollback) => {
                    return Err(NoticeFailure::Eligibility { primary, rollback })
                }
                (Err(Denied::Scope(primary)), rollback) => {
                    return Err(NoticeFailure::Scope { primary, rollback })
                }
                (Ok(()), Some(error)) => return Err(NoticeFailure::Cleanup(error)),
                (Ok(()), None) => fresh,
            }
        };
        // The RR transaction and database guard have ended before body construction.
        self.check(deadline).map_err(NoticeFailure::Owner)?;
        self.project_eligibility(&origin.entry, credential, &fresh)
            .map_err(|primary| NoticeFailure::Eligibility {
                primary,
                rollback: None,
            })?;
        self.native_delivery_encoded(origin.entry, fresh, reply, deadline)
            .map_err(NoticeFailure::Owner)
    }
    #[cfg(test)]
    pub(in crate::coordinator) fn notice_fixture(
        &self,
        credential: &Credential,
        project: &str,
        deadline: Instant,
    ) -> Result<ScopedNotice, Error> {
        let entry = self.candidate(credential)?;
        let origin = NoticeOrigin::capture(
            &Intent::Create {
                project: project.into(),
                handle: "fixture".into(),
            },
            entry,
            deadline,
        )?;
        Ok(ScopedNotice::from_catalogue(
            Notice::ItemConflict,
            origin,
            &Catalogue::admit()?,
        ))
    }
}
#[cfg(test)]
pub(super) fn test_origin() -> NoticeOrigin {
    let live = RuntimeLiveness::new();
    let binding = Binding {
        installation: "installation".into(),
        owner: "owner".into(),
        runtime_generation: 1,
        auth_generation: 2,
    };
    let fresh = Snapshot {
        binding: binding.clone(),
        actor_active: true,
    };
    let mut registry = Registry::new(binding, live).unwrap();
    let bearer = Secret::generate().unwrap();
    let csrf = Secret::generate().unwrap();
    let now = Instant::now();
    let entry = registry
        .reserve(
            NewSession {
                public_id: "notice-unit".into(),
                kind: Kind::Browser,
                bearer: bearer.digest(),
                csrf: Some(csrf),
            },
            &fresh,
            now,
        )
        .unwrap();
    registry.publish(&entry, &fresh, now).unwrap();
    NoticeOrigin::capture(
        &Intent::Create {
            project: "project".into(),
            handle: "handle".into(),
        },
        entry,
        now + WORK_DEADLINE,
    )
    .unwrap()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notice_capture_cannot_follow_changed_project_or_renewed_deadline() {
        let original = test_origin();
        let deadline = original.deadline;
        let entry = Arc::clone(&original.entry);
        let mut intent = Intent::Create {
            project: "original".into(),
            handle: "handle".into(),
        };
        let captured = NoticeOrigin::capture(&intent, entry, deadline).unwrap();
        let Intent::Create { project, .. } = &mut intent else {
            panic!()
        };
        *project = "other-authorized-project".into();
        let notice = ScopedNotice::from_catalogue(
            Notice::ItemConflict,
            captured,
            &Catalogue::admit().unwrap(),
        );
        assert_eq!(notice.origin.project, "original");
        assert_eq!(notice.origin.deadline, deadline);
        assert!(Arc::ptr_eq(&notice.origin.entry, &original.entry));
    }
    #[test]
    fn oversized_or_invalid_original_scope_cannot_be_copied_into_a_notice() {
        let original = test_origin();
        for project in [String::new(), "x".repeat(129), "bad\0project".into()] {
            assert!(matches!(
                NoticeOrigin::capture(
                    &Intent::Create {
                        project,
                        handle: "handle".into()
                    },
                    Arc::clone(&original.entry),
                    original.deadline
                ),
                Err(Error::Invalid)
            ));
        }
    }
}
