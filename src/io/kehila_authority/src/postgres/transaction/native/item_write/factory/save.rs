//! Consumes one protected transaction and one admitted mutation packet.
//! Only fresh provisional acceptance can enter the private payload verifier.
use super::super::{project_from, NativeRecord, SavedResult};
use super::*;
use crate::postgres::{transaction::ItemWriteProof, DbError};

const CREATE_SAVE: &str = concat!(
    columns!(),
    " FROM kehila_auth.save_item_create(",
    proof!(),
    ",$6::text::kehila.ident,$7::text::kehila_auth.intent_key,$8::bytea,",
    "ROW($9::text::jsonb,$10::text::kehila.u64,$11::bigint,$12::text::jsonb,",
    "$13::text::jsonb,$14::text::kehila.u64,$15::bigint,$16::text::kehila.u64,",
    "$17::bigint,$18::text::jsonb)::kehila_auth.item_preparation,$19::text::jsonb) r LIMIT 2"
);
const EDIT_SAVE: &str =
    concat!(columns!(),
    " FROM kehila_auth.save_item_edit(", proof!(),
    ",$6::text::kehila.ident,$7::text::kehila.ident,$8::text::kehila.operation_token,$9::bytea,",
    "ROW($10::text::jsonb,$11::text::kehila.u64,$12::bigint,$13::text::jsonb,",
    "$14::text::jsonb,$15::text::kehila.u64,$16::bigint,$17::text::kehila.u64,",
    "$18::bigint,$19::text::jsonb)::kehila_auth.item_preparation,$20::text::jsonb) r LIMIT 2");
const ACCEPTED_OPERATION: &str = include_str!("save/accepted_operation.sql");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WriteConflict {
    Configuration,
    Item,
}

pub(crate) enum WriteFailure {
    Operation {
        primary: NativeError,
        rollback: Option<DbError>,
    },
    Rollback(DbError),
}

pub(crate) enum SaveOutcome<'a> {
    Ready(ValidatedWrite<'a>),
    Reprepare(RetryIdentity),
    Conflict(WriteConflict),
    Resolved(Resolved),
}

struct NativeSuccess {
    operation: i64,
    result: NativeRecord,
}

// No unrestricted transaction or second write entry is exposed by this type.
// Its owner must perform the final session check before consuming commit.
pub(crate) struct ValidatedWrite<'a> {
    transaction: Fenced<'a, ItemWriteProof>,
    success: NativeSuccess,
}

// Native-storage admission only: this does not admit an API wire envelope.
pub(crate) struct Committed {
    authority: Authority,
    success: NativeSuccess,
}
impl Committed {
    pub(crate) fn authority(&self) -> &Authority {
        &self.authority
    }
    pub(crate) fn operation(&self) -> i64 {
        self.success.operation
    }
    pub(crate) fn result_bytes(&self) -> &[u8] {
        self.success.result.as_bytes()
    }
}
impl ValidatedWrite<'_> {
    pub(crate) async fn rollback(self) -> std::result::Result<(), DbError> {
        self.transaction.rollback().await
    }
    pub(crate) async fn commit(self) -> std::result::Result<Committed, DbError> {
        let Self {
            transaction,
            success,
        } = self;
        let authority = transaction.commit().await?;
        Ok(Committed { authority, success })
    }
}

// Constructed only by the accepted branch of the fixed save response.
struct FreshAcceptance {
    operation: i64,
    data: Metadata,
}
enum Stage {
    Accepted(FreshAcceptance),
    Reprepare,
    Conflict(WriteConflict),
    Resolved(Resolved),
}

pub(super) fn command_parts(request: &Request) -> (&str, &str, &str, &str) {
    match request {
        Request::Create {
            project_id,
            command,
        } => (
            &project_id.0,
            &command.item_id.0,
            &command.operation_id,
            "item_create",
        ),
        Request::Edit {
            project_id,
            item_id,
            command,
        } => (
            &project_id.0,
            &item_id.0,
            &command.operation_id,
            "item_edit",
        ),
    }
}
fn utf8(bytes: &[u8]) -> NativeResult<&str> {
    std::str::from_utf8(bytes).map_err(|_| NativeError::Integrity)
}

fn classify(outcome: &str, data: Metadata, admitted: &Admitted) -> NativeResult<Stage> {
    let (_, target, token, _) = command_parts(&admitted.packet.command.request);
    if outcome == "missing" {
        if !matches!(&admitted.original.identity, Identity::Create { .. })
            || data.operation.is_some()
            || data.pending.is_some()
            || data.pending_expires.is_some()
            || data.target.is_some()
            || data.token.is_some()
            || data.codec.is_some()
        {
            return Err(NativeError::Integrity);
        }
        return Ok(Stage::Resolved(Resolved {
            outcome: Disposition::Missing,
            data,
        }));
    }
    let historical_codec_conflict = outcome == "conflict"
        && data.operation.is_some()
        && matches!(&admitted.original.identity, Identity::Edit)
        && data.codec.is_some_and(|version| version > 0);
    if data.target.as_deref() != Some(target)
        || data.token.as_deref() != Some(token)
        || (!historical_codec_conflict && data.codec != Some(1))
    {
        return Err(NativeError::Integrity);
    }
    match &admitted.original.identity {
        Identity::Create {
            pending, expires, ..
        } => {
            if data.pending != Some(*pending) || data.pending_expires != Some(*expires) {
                return Err(NativeError::Integrity);
            }
        }
        Identity::Edit => {
            if data.pending.is_some() || data.pending_expires.is_some() {
                return Err(NativeError::Integrity);
            }
        }
    }
    match outcome {
        "accepted" => {
            if data.operation.is_none() || data.origin.is_none() || data.end.is_none() {
                return Err(NativeError::Integrity);
            }
            Ok(Stage::Accepted(FreshAcceptance {
                operation: data.operation.ok_or(NativeError::Integrity)?,
                data,
            }))
        }
        "reprepare" | "configuration_conflict" | "item_conflict" => {
            if data.operation.is_some() {
                return Err(NativeError::Integrity);
            }
            Ok(match outcome {
                "reprepare" => Stage::Reprepare,
                "configuration_conflict" => Stage::Conflict(WriteConflict::Configuration),
                _ => Stage::Conflict(WriteConflict::Item),
            })
        }
        "replay" | "expired" | "cancelled" | "conflict" => {
            let disposition = match outcome {
                "replay" => Disposition::Replay,
                "expired" => Disposition::Expired,
                "cancelled" => Disposition::Cancelled,
                _ => Disposition::Conflict,
            };
            if (outcome == "replay" && data.operation.is_none())
                || (outcome == "cancelled"
                    && (data.operation.is_some()
                        || !matches!(&admitted.original.identity, Identity::Create { .. })))
                || (outcome == "expired"
                    && data.operation.is_none()
                    && !matches!(&admitted.original.identity, Identity::Create { .. }))
            {
                return Err(NativeError::Integrity);
            }
            Ok(Stage::Resolved(Resolved {
                outcome: disposition,
                data,
            }))
        }
        _ => Err(NativeError::Integrity),
    }
}

impl<'a> Fenced<'a, ItemWriteProof> {
    pub(crate) async fn save_item(
        mut self,
        admitted: Admitted,
    ) -> std::result::Result<SaveOutcome<'a>, WriteFailure> {
        let stage = async {
            admitted.check_owner(
                self.authority(),
                &self.pending.live,
                self.deadline,
                Instant::now(),
            )?;
            let stage = self.save_packet(&admitted).await?;
            if let Stage::Accepted(data) = &stage {
                // This path is unreachable from a historical resolution.
                self.verify_fresh_payload(&admitted, data).await?;
                self.verify_item_reload(&admitted, data).await?;
                self.force_project_constraints().await?;
            }
            Ok::<Stage, NativeError>(stage)
        }
        .await;
        match stage {
            Ok(Stage::Accepted(accepted)) => {
                let operation = accepted.operation;
                let result = admitted.packet.into_result();
                Ok(SaveOutcome::Ready(ValidatedWrite {
                    transaction: self,
                    success: NativeSuccess { operation, result },
                }))
            }
            Ok(Stage::Reprepare) => {
                let Admitted { packet, original } = admitted;
                drop(packet);
                self.rollback().await.map_err(WriteFailure::Rollback)?;
                Ok(SaveOutcome::Reprepare(original))
            }
            Ok(Stage::Conflict(conflict)) => {
                drop(admitted);
                self.rollback().await.map_err(WriteFailure::Rollback)?;
                Ok(SaveOutcome::Conflict(conflict))
            }
            Ok(Stage::Resolved(resolved)) => {
                drop(admitted);
                self.rollback().await.map_err(WriteFailure::Rollback)?;
                Ok(SaveOutcome::Resolved(resolved))
            }
            Err(primary) => {
                drop(admitted);
                let rollback = self.rollback().await.err();
                Err(WriteFailure::Operation { primary, rollback })
            }
        }
    }

    async fn save_packet(&mut self, admitted: &Admitted) -> NativeResult<Stage> {
        let packet = &admitted.packet;
        let observed = &packet.observed;
        let (project, item, token, _) = command_parts(&packet.command.request);
        let a = self.native_proof();
        let request = packet.request_bytes();
        let project_json = utf8(&packet.project_bytes)?;
        let configuration_json = utf8(&packet.configuration_bytes)?;
        let configuration_project = utf8(&packet.configuration_project_bytes)?;
        let history = utf8(packet.history_bytes())?;
        let configuration_revision = observed.configuration.revision.to_string();
        let state_version = observed.state_version.to_string();
        let item_version =
            observed
                .existing
                .as_ref()
                .map(|previous| match &previous.revision.result {
                    SavedResult::Create(result) => result.item.version.to_string(),
                    SavedResult::Edit(result) => result.item.version.to_string(),
                });
        let item_operation = observed
            .existing
            .as_ref()
            .map(|previous| previous.operation);
        let item_history = packet
            .existing_bytes
            .as_ref()
            .map(|bytes| utf8(bytes.as_bytes()))
            .transpose()?;
        let mut rows = match &admitted.original.identity {
            Identity::Create { handle, .. } => {
                self.rows(
                    CREATE_SAVE,
                    &[
                        &a[0],
                        &a[1],
                        &a[2],
                        &a[3],
                        &a[4],
                        &project,
                        &handle,
                        &request,
                        &project_json,
                        &configuration_revision,
                        &observed.configuration_operation,
                        &configuration_json,
                        &configuration_project,
                        &state_version,
                        &observed.state_operation,
                        &item_version,
                        &item_operation,
                        &item_history,
                        &history,
                    ],
                )
                .await?
            }
            Identity::Edit => {
                self.rows(
                    EDIT_SAVE,
                    &[
                        &a[0],
                        &a[1],
                        &a[2],
                        &a[3],
                        &a[4],
                        &project,
                        &item,
                        &token,
                        &request,
                        &project_json,
                        &configuration_revision,
                        &observed.configuration_operation,
                        &configuration_json,
                        &configuration_project,
                        &state_version,
                        &observed.state_operation,
                        &item_version,
                        &item_operation,
                        &item_history,
                        &history,
                    ],
                )
                .await?
            }
        };
        if rows.len() != 1 {
            return Err(NativeError::Integrity);
        }
        let row = rows.pop().ok_or(NativeError::Integrity)?;
        drop(rows);
        let (outcome, data) = decode_metadata(row, self.authority())?;
        classify(&outcome, data, admitted)
    }

    async fn verify_fresh_payload(
        &mut self,
        admitted: &Admitted,
        accepted: &FreshAcceptance,
    ) -> NativeResult<()> {
        let operation = accepted.operation;
        let data = &accepted.data;
        let packet = &admitted.packet;
        let (project, item, token, family) = command_parts(&packet.command.request);
        let mut rows = self.rows(ACCEPTED_OPERATION, &[&operation]).await?;
        if rows.len() != 1 {
            return Err(NativeError::Integrity);
        }
        let row = rows.pop().ok_or(NativeError::Integrity)?;
        drop(rows);
        if get::<String>(&row, 0)? != packet.command.scope.actor
            || get::<String>(&row, 1)? != family
            || get::<String>(&row, 2)? != "work_item"
            || get::<String>(&row, 3)? != project
            || get::<String>(&row, 4)? != item
            || get::<Option<String>>(&row, 5)?.is_some()
            || get::<Option<String>>(&row, 6)?.is_some()
            || get::<String>(&row, 7)? != token
            || get::<i64>(&row, 8)? != 1
            || get::<Vec<u8>>(&row, 9)?.as_slice() != packet.command.digest.as_slice()
            || get::<String>(&row, 10)? != family
            || get::<i64>(&row, 11)? != 1
            || Some(scalar(get(&row, 12)?)?) != data.origin
            || Some(scalar(get(&row, 13)?)?) != data.end
            || get::<bool>(&row, 14)?
            || get::<i64>(&row, 16)? != 1
            || get::<i64>(&row, 18)? != operation
        {
            return Err(NativeError::Integrity);
        }
        {
            let request: Option<Vec<u8>> = get(&row, 15)?;
            if request.as_deref() != Some(packet.request_bytes()) {
                return Err(NativeError::Integrity);
            }
        }
        {
            let result: Option<String> = get(&row, 17)?;
            let result = result.ok_or(NativeError::Integrity)?;
            let decoded = work_item_codec::decode_result(1, result.as_bytes())
                .map_err(|_| NativeError::Integrity)?;
            if decoded != packet.revision.result {
                return Err(NativeError::Integrity);
            }
        }
        // All driver/request/result temporaries die here, before current/history.
        Ok(())
    }

    async fn verify_item_reload(
        &mut self,
        admitted: &Admitted,
        accepted: &FreshAcceptance,
    ) -> NativeResult<()> {
        let packet = &admitted.packet;
        let (project, item, _, _) = command_parts(&packet.command.request);
        let operation = accepted.operation;
        let current = self.reload_item_project(project).await?;
        if current.project != *project_from(&packet.revision.result)
            || current.configuration != packet.observed.configuration
            || current.configuration_project != packet.observed.configuration_project
            || current.configuration_operation != packet.observed.configuration_operation
            || current.state_version != packet.revision.project_state_version
            || current.state_operation != operation
        {
            return Err(NativeError::Integrity);
        }
        let (stored, readable) = self.current_item_record(&current.project, item).await?;
        let expected_readable = match &packet.revision.result {
            SavedResult::Create(result) => &result.readable_id,
            SavedResult::Edit(_) => {
                &packet
                    .observed
                    .existing
                    .as_ref()
                    .ok_or(NativeError::Integrity)?
                    .readable
            }
        };
        if stored.operation != operation
            || stored.revision != packet.revision
            || &readable != expected_readable
        {
            return Err(NativeError::Integrity);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
