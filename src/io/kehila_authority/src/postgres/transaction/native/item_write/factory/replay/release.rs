//! Consumed final replay-only authorization. No packet or transaction escapes.
use super::*;
use crate::postgres::{transaction::ItemReplayProof, DbError};

const RELEASE: &str = concat!(
    columns!(),
    " FROM kehila_auth.authorize_item_replay(",
    proof!(),
    ",$6::text::kehila.ident,$7::text,$8::text::kehila.ident,$9::text::kehila.operation_token,",
    "$10::bigint,$11::bigint,$12::bytea,$13::text::kehila.u64,$14::text::kehila.u64,",
    "$15::bigint,$16::text::kehila.u64,$17::text::kehila_auth.intent_key) r LIMIT 2"
);

pub(crate) enum HistoryReleaseFailure {
    Operation {
        primary: NativeError,
        rollback: Option<DbError>,
    },
    Rollback(DbError),
}
pub(crate) enum HistoryAuthorization {
    Released(HistoryRelease),
    Expired(Resolved),
}
// This is an authorization observation only. The owning coordinator must hold
// the exact preadmitted reply paired with the consumed HistoricalAccepted; no
// generic caller can turn this view into permission for arbitrary reply bytes.
pub(crate) struct HistoryRelease {
    authority: Authority,
}
impl HistoryRelease {
    pub(crate) fn authority(&self) -> &Authority {
        &self.authority
    }
}
impl Fenced<'_, ItemReplayProof> {
    pub(crate) async fn authorize_item_history(
        mut self,
        packet: HistoricalAccepted,
    ) -> std::result::Result<HistoryAuthorization, HistoryReleaseFailure> {
        let checked = async {
            packet.original.check_owner(
                self.authority(),
                &self.pending.live,
                self.deadline,
                Instant::now(),
            )?;
            // The fixture can distinguish fresh-fence ordering from final authorization.
            #[cfg(test)]
            self.pending
                .live
                .probe
                .transaction_point(
                    0,
                    crate::lifecycle::probe::Point::ItemReplayFenced,
                    self.deadline,
                )
                .await;
            let command = &packet.original.command;
            let (project, item, token, family) =
                super::super::save::command_parts(&command.request);
            let (handle, pending, expires) = match &packet.original.identity {
                Identity::Create {
                    handle,
                    pending,
                    expires,
                } => (
                    Some(handle.as_str()),
                    Some(*pending),
                    Some(expires.to_string()),
                ),
                Identity::Edit => (None, None, None),
            };
            let operation = packet.data.operation.ok_or(NativeError::Integrity)?;
            let codec = i64::from(packet.data.codec.ok_or(NativeError::Integrity)?);
            let origin = packet
                .data
                .origin
                .ok_or(NativeError::Integrity)?
                .to_string();
            let end = packet.data.end.ok_or(NativeError::Integrity)?.to_string();
            let digest = command.digest.as_slice();
            let a = self.native_proof();
            let mut rows = self
                .rows(
                    RELEASE,
                    &[
                        &a[0], &a[1], &a[2], &a[3], &a[4], &project, &family, &item, &token,
                        &operation, &codec, &digest, &origin, &end, &pending, &expires, &handle,
                    ],
                )
                .await?;
            if rows.len() != 1 {
                return Err(NativeError::Integrity);
            }
            let row = rows.pop().ok_or(NativeError::Integrity)?;
            drop(rows);
            let (outcome, data) = decode_metadata(row, self.authority())?;
            if data != packet.data {
                return Err(NativeError::Integrity);
            }
            #[cfg(test)]
            {
                use crate::lifecycle::probe::ItemReplayObservation as Observation;
                let observed = match outcome.as_str() {
                    "authorized" => Some(Observation::Authorized),
                    "expired" => Some(Observation::Expired),
                    "before_origin" => Some(Observation::BeforeOrigin),
                    _ => None,
                };
                if let Some(observed) = observed {
                    self.pending.live.probe.record_item_replay(observed)
                }
            }
            match outcome.as_str() {
                "authorized" => Ok(None),
                "expired" => Ok(Some(Resolved {
                    outcome: Disposition::Expired,
                    data,
                })),
                "before_origin" => Err(NativeError::Database(DbError::Unavailable)),
                _ => Err(NativeError::Integrity),
            }
        }
        .await;
        // Hold only an actually authorized transaction for retirement/rollback
        // controls. This wait is absent from production and cannot manufacture
        // authorization for an expired, malformed or failed operation.
        #[cfg(test)]
        if matches!(&checked, Ok(None)) {
            self.pending
                .live
                .probe
                .transaction_point(
                    0,
                    crate::lifecycle::probe::Point::ItemReplayCleanup,
                    self.deadline,
                )
                .await;
        }
        // No encoding, CPU work or payload access follows the final DB sample.
        // Drop the redundant historical result bytes before releasing the fence.
        drop(packet);
        let a = self.authority();
        let authority = Authority {
            installation: a.installation.clone(),
            owner: a.owner.clone(),
            runtime: a.runtime,
            authentication: a.authentication,
            revision: a.revision,
        };
        #[cfg(test)]
        let observed = Arc::clone(&self.pending.live);
        let rollback = self.rollback().await.err();
        #[cfg(test)]
        if rollback.is_some() {
            observed
                .probe
                .record_item_replay(crate::lifecycle::probe::ItemReplayObservation::RollbackFailed)
        }
        match (checked, rollback) {
            (Err(primary), rollback) => Err(HistoryReleaseFailure::Operation { primary, rollback }),
            (Ok(_), Some(error)) => Err(HistoryReleaseFailure::Rollback(error)),
            (Ok(Some(resolved)), None) => Ok(HistoryAuthorization::Expired(resolved)),
            (Ok(None), None) => Ok(HistoryAuthorization::Released(HistoryRelease { authority })),
        }
    }
}
