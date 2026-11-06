//! Consumer-visible codes and diagnostic/uncertainty boundaries.
use super::*;
use task_contract::field::FieldError;
fn admitted() -> Catalogue {
    Catalogue::admit().unwrap()
}
fn wire(catalogue: &Catalogue, notice: Notice) -> (u16, serde_json::Value) {
    let encoded = catalogue.reply(notice);
    let (status, body) = encoded.test_parts();
    assert!(body.len() <= NOTICE_BYTES);
    (status, serde_json::from_slice(body).unwrap())
}
#[test]
fn scoped_catalogue_is_complete_small_and_contains_no_data_or_transport_authority() {
    let catalogue = admitted();
    let expected = [
        (403, "forbidden"),
        (404, "not_found"),
        (404, "pending_not_found"),
        (404, "operation_not_found"),
        (410, "pending_cancelled"),
        (410, "pending_expired"),
        (410, "replay_expired"),
        (409, "operation_conflict"),
        (409, "configuration_conflict"),
        (409, "item_conflict"),
        (422, "invalid_mutation"),
        (409, "archived_target"),
        (422, "representation_rejected"),
        (409, "mutation_state_exhausted"),
        (503, "attempt_exhausted"),
    ];
    assert_eq!(Notice::ALL.len(), expected.len());
    let mut total = 0;
    for (notice, (status, code)) in Notice::ALL.iter().copied().zip(expected) {
        let (actual, body) = wire(&catalogue, notice);
        assert_eq!(actual, status);
        assert_eq!(body, json!({"error":code}));
        assert_eq!(Policy::Scoped(notice).generic_error(), None);
        total += catalogue.reply(notice).encoded_len();
    }
    assert!(total <= 2048);
}
#[test]
fn selecting_an_already_admitted_notice_shares_the_same_encoded_bytes() {
    let catalogue = admitted();
    let first = catalogue.reply(Notice::ItemConflict);
    let second = catalogue.reply(Notice::ItemConflict);
    assert!(std::ptr::eq(
        first.test_parts().1.as_ptr(),
        second.test_parts().1.as_ptr()
    ));
    assert_eq!(first.test_parts().1, b"{\"error\":\"item_conflict\"}");
}
#[test]
fn conflicts_exhaustion_and_representation_rejection_have_distinct_bounded_codes() {
    let cases = [
        (
            Mutation::ConfigurationConflict {
                current_revision: u64::MAX,
            },
            Notice::ConfigurationConflict,
        ),
        (
            Mutation::ItemVersionConflict {
                current_version: u64::MAX,
            },
            Notice::ItemConflict,
        ),
        (Mutation::OperationIdReused, Notice::OperationConflict),
        (Mutation::VersionOverflow, Notice::StateExhausted),
        (
            Mutation::Project(ProjectError::SequenceExhausted),
            Notice::StateExhausted,
        ),
        (Mutation::ArchivedItem, Notice::Archived),
        (
            Mutation::Field(FieldError::InvalidDefinition),
            Notice::InvalidMutation,
        ),
    ];
    for (error, expected) in cases {
        assert_eq!(
            failure(&AttemptFailure::Admission(AdmissionFailure::Item(
                ItemAdmissionError::Domain(error)
            ))),
            Policy::Scoped(expected)
        );
    }
    assert_eq!(
        failure(&AttemptFailure::Exhausted),
        Policy::Scoped(Notice::AttemptExhausted)
    );
    assert_eq!(
        failure(&AttemptFailure::Admission(AdmissionFailure::Item(
            ItemAdmissionError::StateExhausted
        ))),
        Policy::Scoped(Notice::StateExhausted)
    );
    assert_eq!(
        failure(&AttemptFailure::Admission(AdmissionFailure::Representation)),
        Policy::Scoped(Notice::RepresentationRejected)
    );
    assert_eq!(
        failure(&AttemptFailure::Admission(AdmissionFailure::Item(
            ItemAdmissionError::Expired
        ))),
        Policy::Unavailable
    );
}
#[test]
fn unknown_commit_and_known_commit_with_egress_loss_only_expose_fixed_uncertainty() {
    for error in [
        AttemptFailure::Commit(DbError::UnknownCommit),
        AttemptFailure::CommittedUndeliverable(Error::Unauthorized),
        AttemptFailure::CommittedUndeliverable(Error::Unavailable),
        AttemptFailure::Write(ItemWriteFailure::Operation {
            primary: NativeError::Database(DbError::UnknownCommit),
            rollback: Some(DbError::Unavailable),
        }),
    ] {
        let policy = failure(&error);
        assert_eq!(policy, Policy::Uncertain);
        assert_eq!(
            policy.generic_error(),
            Some(Error::Project(crate::project::Failure::OutcomeUnknown))
        );
    }
}
#[test]
fn known_rollback_never_becomes_a_statement_that_the_operation_cannot_commit() {
    let clean = AttemptFailure::Write(ItemWriteFailure::Operation {
        primary: NativeError::Rejected(crate::project::Failure::Conflict),
        rollback: None,
    });
    assert_eq!(failure(&clean), Policy::Scoped(Notice::OperationConflict));
    let failed_cleanup = AttemptFailure::Write(ItemWriteFailure::Operation {
        primary: NativeError::Rejected(crate::project::Failure::Conflict),
        rollback: Some(DbError::Unavailable),
    });
    assert_eq!(failure(&failed_cleanup), Policy::Unavailable);
    assert_eq!(
        wire(&admitted(), Notice::OperationConflict).1,
        json!({"error":"operation_conflict"})
    );
    // No operation-rejected/rolled-back/absent flag or replacement identity.
}
#[test]
fn metadata_separates_pending_and_replay_expiry_and_cannot_fabricate_a_saved_result() {
    use ItemDisposition as D;
    assert_eq!(
        observed(D::Expired, false),
        Policy::Scoped(Notice::PendingExpired)
    );
    assert_eq!(
        observed(D::Expired, true),
        Policy::Scoped(Notice::ReplayExpired)
    );
    assert_eq!(
        observed(D::Missing, false),
        Policy::Scoped(Notice::PendingMissing)
    );
    assert_eq!(
        observed(D::Cancelled, false),
        Policy::Scoped(Notice::PendingCancelled)
    );
    assert_eq!(observed(D::Replay, true), Policy::Historical);
    assert_eq!(Policy::Historical.generic_error(), None);
    for disposition in [D::Missing, D::Cancelled] {
        assert_eq!(observed(disposition, true), Policy::Unavailable);
    }
    assert_eq!(observed(D::Replay, false), Policy::Unavailable);
    // Historical has neither a notice index, payload nor executable draft.
}
#[test]
fn diagnostics_are_retained_inside_reports_and_cannot_enter_the_wire_vocabulary() {
    let secret = "untrusted SQLSTATE SELECT private diagnostic\u{0}";
    let report = AttemptReport {
        result: Err(AttemptFailure::Write(ItemWriteFailure::Operation {
            primary: NativeError::Database(DbError::Rejected(secret.into())),
            rollback: None,
        })),
        notices: admitted(),
        origin: super::super::notice::test_origin(),
    };
    assert_eq!(report.policy(), Policy::Unavailable);
    assert!(report.notice().is_none());
    let Err(AttemptFailure::Write(ItemWriteFailure::Operation {
        primary: NativeError::Database(DbError::Rejected(retained)),
        rollback: None,
    })) = report.into_result()
    else {
        panic!("diagnostic erased")
    };
    assert_eq!(retained, secret);
    let catalogue = admitted();
    for kind in Notice::ALL {
        let encoded = catalogue.reply(*kind);
        assert!(!std::str::from_utf8(encoded.test_parts().1)
            .unwrap()
            .contains("private diagnostic"));
    }
    for error in [
        DbError::Constraint {
            code: "23505".into(),
            name: secret.into(),
        },
        DbError::Rejected("23505".into()),
        DbError::Rejected("40001".into()),
    ] {
        assert_eq!(native(&NativeError::Database(error)), Policy::Unavailable);
    }
}
#[test]
fn scoped_report_selects_prepared_notice_without_erasing_internal_failure() {
    let report = AttemptReport {
        result: Err(AttemptFailure::Admission(AdmissionFailure::Item(
            ItemAdmissionError::Domain(Mutation::ItemVersionConflict {
                current_version: u64::MAX,
            }),
        ))),
        notices: admitted(),
        origin: super::super::notice::test_origin(),
    };
    assert_eq!(report.policy(), Policy::Scoped(Notice::ItemConflict));
    assert_eq!(report.policy().generic_error(), None);
    let notice = report.notice().unwrap();
    assert_eq!(notice.test_parts().1, b"{\"error\":\"item_conflict\"}");
    assert!(matches!(
        report.into_result(),
        Err(AttemptFailure::Admission(AdmissionFailure::Item(
            ItemAdmissionError::Domain(Mutation::ItemVersionConflict {
                current_version: u64::MAX
            })
        )))
    ));
}

#[test]
fn ambiguous_native_state_cannot_claim_authentication_failure() {
    let raw = NativeError::Database(DbError::Rejected("55000".into()));
    assert_eq!(native(&raw), Policy::Unavailable);
    assert_eq!(
        failure(&AttemptFailure::Preparation {
            primary: raw,
            rollback: None
        }),
        Policy::Unavailable
    );
    let authenticated_source = crate::coordinator::fence_error(DbError::Rejected("55000".into()));
    assert_eq!(authenticated_source, Error::Unauthorized);
    assert_eq!(
        failure(&AttemptFailure::Owner(authenticated_source)),
        Policy::Unauthorized
    );
}

#[test]
fn replay_release_failure_keeps_cleanup_and_primary_policy_distinct() {
    use crate::postgres::transaction::ItemHistoryReleaseFailure as Release;
    let forbidden = || NativeError::Database(DbError::Rejected("42501".into()));
    assert_eq!(
        failure(&AttemptFailure::Release(Release::Operation {
            primary: forbidden(),
            rollback: None
        })),
        Policy::Scoped(Notice::Forbidden)
    );
    assert_eq!(
        failure(&AttemptFailure::Release(Release::Operation {
            primary: forbidden(),
            rollback: Some(DbError::Unavailable)
        })),
        Policy::Unavailable
    );
    assert_eq!(
        failure(&AttemptFailure::Release(Release::Rollback(
            DbError::Unavailable
        ))),
        Policy::Unavailable
    );
    assert_eq!(Policy::Replayed.generic_error(), None);
}
