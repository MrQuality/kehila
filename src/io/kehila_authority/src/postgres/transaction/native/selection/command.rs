//! Consumed selection witnesses. Only authorized snapshots construct drafts;
//! only fresh, distinct write/replay transactions consume admitted packets.
use super::*;
use crate::lifecycle::RuntimeLiveness;
use crate::postgres::transaction::{SelectionReplayProof, SelectionWriteProof};
use crate::selection::{Canonical, PreparationError, Prepared};
use std::sync::Arc;
use task_contract::{current_work::SelectionError, relationship::Endpoint};

#[derive(Clone)]
pub(crate) struct Identity {
    command: Arc<Canonical>,
    epoch: [u64; 3],
    live: Arc<RuntimeLiveness>,
    deadline: Instant,
    recorded: Option<(i64, u64)>,
}
impl Identity {
    fn check(
        &self,
        authority: &Authority,
        live: &Arc<RuntimeLiveness>,
        deadline: Instant,
    ) -> Result<()> {
        if self.command.installation() != authority.installation
            || self.command.request().user_id.0 != authority.owner
            || self.epoch
                != [
                    authority.runtime,
                    authority.authentication,
                    authority.revision,
                ]
            || !Arc::ptr_eq(&self.live, live)
            || self.deadline != deadline
        {
            return Err(NativeError::Integrity);
        }
        self.check_live()
    }
    fn check_live(&self) -> Result<()> {
        if !self.live.is_live() {
            return Err(DbError::Unavailable.into());
        }
        if Instant::now() >= self.deadline {
            return Err(DbError::Expired.into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Resolution {
    Absent,
    Expired,
    Reused,
    Conflict(u64),
    Archived,
    InvalidTarget,
    Exhausted,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PacketKind {
    Fresh,
    Historical,
    Notice(Resolution),
}

enum Observation {
    Absent,
    Fresh {
        head: ReadDraft,
        target: Option<Endpoint>,
    },
    Historical {
        operation: i64,
        event: Row,
        retired: bool,
        payload: Option<Row>,
    },
}
pub(crate) struct CommandDraft {
    identity: Identity,
    observation: Observation,
}
struct History {
    operation: i64,
    version: u64,
    same_identity: bool,
    retired: bool,
    reply: Option<Vec<u8>>,
}
impl History {
    fn resolve(
        &self,
        status: &str,
        operation: Option<i64>,
        version: Option<u64>,
    ) -> Result<Option<Resolution>> {
        if operation != Some(self.operation) || version != Some(self.version) {
            return Err(NativeError::Integrity);
        }
        match status {
            "replay" if self.same_identity && !self.retired && self.reply.is_some() => Ok(None),
            "replay_expired" if self.same_identity => Ok(Some(Resolution::Expired)),
            "operation_id_reused" if !self.same_identity => Ok(Some(Resolution::Reused)),
            _ => Err(NativeError::Integrity),
        }
    }
}
enum Admitted {
    Fresh(Box<Prepared>),
    Historical(History),
    Notice(Resolution),
}
pub(crate) struct CommandPacket {
    identity: Identity,
    admitted: Admitted,
}

fn representation(error: PreparationError) -> NativeError {
    match error {
        PreparationError::InvalidProof | PreparationError::Representation(_) => {
            NativeError::Integrity
        }
        _ => NativeError::Database(DbError::Unavailable),
    }
}
impl CommandDraft {
    pub(crate) fn admit(self) -> Result<CommandPacket> {
        self.identity.check_live()?;
        let command = &self.identity.command;
        let admitted = match self.observation {
            Observation::Absent => Admitted::Notice(Resolution::Absent),
            Observation::Fresh { head, target } => {
                let (current, ordinal, _) = head.verify()?;
                match command.prepare(&current, ordinal, target.as_ref()) {
                    Ok(value) => Admitted::Fresh(Box::new(value)),
                    Err(PreparationError::Domain(error)) => Admitted::Notice(match error {
                        SelectionError::VersionConflict { current_version } => {
                            Resolution::Conflict(current_version)
                        }
                        SelectionError::ArchivedTarget => Resolution::Archived,
                        SelectionError::InvalidReference => Resolution::InvalidTarget,
                        SelectionError::VersionOverflow => return Err(DbError::Unavailable.into()),
                        _ => return Err(NativeError::Integrity),
                    }),
                    Err(error) => return Err(representation(error)),
                }
            }
            Observation::Historical {
                operation,
                event,
                retired,
                payload,
            } => {
                let retained = verified_event(
                    command.installation(),
                    &command.request().user_id.0,
                    operation,
                    &event,
                )?;
                if retained.operation_id != command.request().operation_id {
                    return Err(NativeError::Integrity);
                }
                if self
                    .identity
                    .recorded
                    .is_some_and(|value| value != (operation, retained.result.next.version))
                {
                    return Err(NativeError::Integrity);
                }
                let original = Canonical::admit(
                    command.installation(),
                    &command.request().user_id.0,
                    task_contract::current_work::SelectionCommand {
                        user_id: retained.before.user_id.clone(),
                        operation_id: retained.operation_id.clone(),
                        expected_version: retained.before.version,
                        requested: retained.result.next.selected.clone(),
                    },
                )
                .map_err(representation)?;
                let reply = match payload {
                    Some(row) if !retired => {
                        if get::<i64>(&row, 0)? != operation
                            || get::<Option<Vec<u8>>>(&row, 1)?.as_deref() != Some(original.bytes())
                        {
                            return Err(NativeError::Integrity);
                        }
                        let codec = u32::try_from(get::<i64>(&row, 2)?)
                            .map_err(|_| NativeError::Integrity)?;
                        let result =
                            get::<Option<String>>(&row, 3)?.ok_or(NativeError::Integrity)?;
                        let history =
                            get::<Option<String>>(&event, 13)?.ok_or(NativeError::Integrity)?;
                        let decoded = original
                            .reconstruct(1, history.as_bytes(), codec, result.as_bytes())
                            .map_err(representation)?;
                        if original.bytes() == command.bytes() {
                            Some(decoded.reply().to_vec())
                        } else {
                            None
                        }
                    }
                    None if retired => None,
                    _ => return Err(NativeError::Integrity),
                };
                Admitted::Historical(History {
                    operation,
                    version: retained.result.next.version,
                    same_identity: original.bytes() == command.bytes(),
                    retired,
                    reply,
                })
            }
        };
        self.identity.check_live()?;
        Ok(CommandPacket {
            identity: self.identity,
            admitted,
        })
    }
}
impl CommandPacket {
    pub(crate) fn kind(&self) -> PacketKind {
        match &self.admitted {
            Admitted::Fresh(_) => PacketKind::Fresh,
            Admitted::Historical(_) => PacketKind::Historical,
            Admitted::Notice(value) => PacketKind::Notice(*value),
        }
    }
    pub(crate) fn reply(&self) -> Option<&[u8]> {
        match &self.admitted {
            Admitted::Fresh(value) => Some(value.reply()),
            Admitted::Historical(value) => value.reply.as_deref(),
            Admitted::Notice(_) => None,
        }
    }
}

impl Fenced<'_, ReadProof> {
    pub(crate) async fn prepare_selection(
        &mut self,
        command: Arc<Canonical>,
        original: Option<Identity>,
        reconcile: bool,
    ) -> Result<CommandDraft> {
        let identity = original.unwrap_or_else(|| Identity {
            command: Arc::clone(&command),
            epoch: [
                self.authority.runtime,
                self.authority.authentication,
                self.authority.revision,
            ],
            live: Arc::clone(&self.pending.live),
            deadline: self.deadline,
            recorded: None,
        });
        identity.check(self.authority(), &self.pending.live, self.deadline)?;
        if !Arc::ptr_eq(&command, &identity.command) {
            return Err(NativeError::Integrity);
        }
        let user = command.request().user_id.0.as_str();
        let token = command.request().operation_id.as_str();
        let mut cores = self.rows("SELECT operation_row_id,payload_retired FROM kehila.operations \
            WHERE actor_id=$1::text::kehila.ident AND command_family='selection' AND target_kind='user' \
            AND target_user_id=$1::text::kehila.ident AND target_project_id IS NULL AND target_item_id IS NULL \
            AND target_relationship_id IS NULL AND token=$2::text::kehila.operation_token LIMIT 2",&[&user,&token]).await?;
        if cores.len() > 1 {
            return Err(NativeError::Integrity);
        }
        let observation = if let Some(core) = cores.pop() {
            let operation: i64 = get(&core, 0)?;
            let retired: bool = get(&core, 1)?;
            let mut events = self
                .rows(include_str!("read_event.sql"), &[&operation])
                .await?;
            if events.len() != 1 {
                return Err(NativeError::Integrity);
            }
            let event = events.remove(0);
            // Original retained scope owns authority before any comparison,
            // payload access or outcome, even for retirement or changed retries.
            if let Some(project) = get::<Option<String>>(&event, 10)? {
                self.authorize_item_scope(&project).await?;
            }
            let mut payloads = if retired {
                Vec::new()
            } else {
                self.rows("SELECT operation_row_id, \
                CASE WHEN octet_length(request_bytes) BETWEEN 1 AND 4096 THEN request_bytes END, \
                result_codec_version,CASE WHEN octet_length(result::text)<=4096 THEN result::text END \
                FROM kehila.operation_payloads WHERE operation_row_id=$1::bigint LIMIT 2",&[&operation]).await?
            };
            if payloads.len() > 1 {
                return Err(NativeError::Integrity);
            }
            Observation::Historical {
                operation,
                event,
                retired,
                payload: payloads.pop(),
            }
        } else if identity.recorded.is_some() {
            return Err(NativeError::Integrity);
        } else if reconcile {
            Observation::Absent
        } else {
            let target = if let Some(target) = &command.request().requested {
                self.authorize_item_scope(&target.project_id.0).await?;
                let mut rows = self.rows("SELECT p.archived,r.archived, \
                    i.current_version::numeric(20,0)::text, \
                    (SELECT max(v.item_version)::numeric(20,0)::text FROM kehila.item_revisions v \
                      WHERE v.project_id=i.project_id AND v.item_id=i.item_id) \
                    FROM kehila.projects p JOIN kehila.items i USING(project_id) \
                    JOIN kehila.item_revisions r ON r.project_id=i.project_id AND r.item_id=i.item_id \
                      AND r.item_version=i.current_version \
                    WHERE i.project_id=$1::text::kehila.ident AND i.item_id=$2::text::kehila.ident LIMIT 2",
                    &[&target.project_id.0,&target.item_id.0]).await?;
                if rows.len() > 1 {
                    return Err(NativeError::Integrity);
                }
                match rows.pop() {
                    Some(row) => {
                        let version = scalar(get(&row, 2)?)?;
                        if version == 0
                            || get::<Option<String>>(&row, 3)?.as_deref()
                                != Some(version.to_string().as_str())
                        {
                            return Err(NativeError::Integrity);
                        }
                        Some(Endpoint {
                            project_id: target.project_id.clone(),
                            item_id: target.item_id.clone(),
                            project_archived: get(&row, 0)?,
                            item_archived: get(&row, 1)?,
                        })
                    }
                    None => None,
                }
            } else {
                None
            };
            Observation::Fresh {
                head: self.selection_head().await?,
                target,
            }
        };
        Ok(CommandDraft {
            identity,
            observation,
        })
    }
}

pub(crate) enum SelectionFailure {
    Operation {
        primary: NativeError,
        rollback: Option<DbError>,
    },
    Rollback(DbError),
}
pub(crate) enum SaveOutcome<'a> {
    Ready(ValidatedWrite<'a>),
    Reprepare(Identity),
    Historical(Identity),
    Notice(Resolution),
}
pub(crate) struct ValidatedWrite<'a> {
    transaction: Fenced<'a, SelectionWriteProof>,
}
enum SaveStage {
    Accepted,
    Stale,
    Historical(i64, u64),
    Notice(Resolution),
}
impl ValidatedWrite<'_> {
    pub(crate) async fn rollback(self) -> std::result::Result<(), DbError> {
        self.transaction.rollback().await
    }
    pub(crate) async fn commit(self) -> std::result::Result<Authority, DbError> {
        self.transaction.commit().await
    }
}

const RESOLVE:&str = concat!("SELECT outcome,operation_row_id,current_version::numeric(20,0)::text FROM kehila_auth.reconcile_selection(",proof!(),
    ",$6::text::kehila.operation_token,1::bigint,$7::bytea) LIMIT 2");
const SAVE:&str = concat!("SELECT outcome,operation_row_id,current_version::numeric(20,0)::text FROM kehila_auth.save_selection(",proof!(),
    ",$6::text::kehila.operation_token,$7::text::kehila.u64,$8::bytea,$9::text::jsonb) LIMIT 2");

impl<'a> Fenced<'a, SelectionWriteProof> {
    pub(crate) async fn save_selection(
        mut self,
        packet: CommandPacket,
    ) -> std::result::Result<SaveOutcome<'a>, SelectionFailure> {
        let checked = async {
            packet
                .identity
                .check(self.authority(), &self.pending.live, self.deadline)?;
            let Admitted::Fresh(prepared) = &packet.admitted else {
                return Err(NativeError::Integrity);
            };
            let command = &packet.identity.command;
            let a = self.native_proof();
            let expected = command.request().expected_version.to_string();
            let bytes = command.bytes();
            let history = prepared.history();
            let mut rows = self
                .rows(
                    SAVE,
                    &[
                        &a[0],
                        &a[1],
                        &a[2],
                        &a[3],
                        &a[4],
                        &command.request().operation_id,
                        &expected,
                        &bytes,
                        &history,
                    ],
                )
                .await?;
            if rows.len() != 1 {
                return Err(NativeError::Integrity);
            }
            let row = rows.remove(0);
            let status: String = get(&row, 0)?;
            let operation: Option<i64> = get(&row, 1)?;
            let version = get::<Option<String>>(&row, 2)?.map(scalar).transpose()?;
            match status.as_str() {
                "accepted" => {
                    let operation = operation
                        .filter(|value| *value > 0)
                        .ok_or(NativeError::Integrity)?;
                    if version != Some(prepared.event().result.next.version) {
                        return Err(NativeError::Integrity);
                    }
                    self.verify_selection_save(operation, &packet).await?;
                    self.force_project_constraints().await?;
                    Ok(SaveStage::Accepted)
                }
                "preparation_stale"
                    if operation.is_none()
                        && version == Some(command.request().expected_version) =>
                {
                    Ok(SaveStage::Stale)
                }
                "version_conflict" if operation.is_none() => Ok(SaveStage::Notice(
                    Resolution::Conflict(version.ok_or(NativeError::Integrity)?),
                )),
                "archived_target"
                    if operation.is_none()
                        && version == Some(command.request().expected_version) =>
                {
                    Ok(SaveStage::Notice(Resolution::Archived))
                }
                "replay" | "replay_expired" | "operation_id_reused"
                    if operation.is_some_and(|value| value > 0) && version.is_some() =>
                {
                    Ok(SaveStage::Historical(
                        operation.ok_or(NativeError::Integrity)?,
                        version.ok_or(NativeError::Integrity)?,
                    ))
                }
                _ => Err(NativeError::Integrity),
            }
        }
        .await;
        match checked {
            Ok(SaveStage::Accepted) => Ok(SaveOutcome::Ready(ValidatedWrite { transaction: self })),
            Ok(stage) => {
                self.rollback().await.map_err(SelectionFailure::Rollback)?;
                Ok(match stage {
                    SaveStage::Stale => SaveOutcome::Reprepare(packet.identity),
                    SaveStage::Historical(operation, version) => {
                        let mut identity = packet.identity;
                        identity.recorded = Some((operation, version));
                        SaveOutcome::Historical(identity)
                    }
                    SaveStage::Notice(value) => SaveOutcome::Notice(value),
                    SaveStage::Accepted => unreachable!(),
                })
            }
            Err(primary) => Err(SelectionFailure::Operation {
                primary,
                rollback: self.rollback().await.err(),
            }),
        }
    }
    async fn verify_selection_save(
        &mut self,
        operation: i64,
        packet: &CommandPacket,
    ) -> Result<()> {
        let Admitted::Fresh(prepared) = &packet.admitted else {
            return Err(NativeError::Integrity);
        };
        let command = &packet.identity.command;
        let result = std::str::from_utf8(prepared.result()).map_err(|_| NativeError::Integrity)?;
        let history = prepared.history();
        let bytes = command.bytes();
        let row = self
            .run(
                include_str!("verify_save.sql"),
                &[
                    &operation,
                    &command.request().user_id.0,
                    &command.request().operation_id,
                    &bytes,
                    &result,
                    &history,
                ],
            )
            .await?;
        if !get::<bool>(&row, 0)? {
            return Err(NativeError::Integrity);
        }
        Ok(())
    }
}
impl Fenced<'_, SelectionReplayProof> {
    pub(crate) async fn release_selection(
        mut self,
        packet: CommandPacket,
    ) -> std::result::Result<Option<Resolution>, SelectionFailure> {
        let checked = async {
            packet
                .identity
                .check(self.authority(), &self.pending.live, self.deadline)?;
            let Admitted::Historical(history) = &packet.admitted else {
                return Err(NativeError::Integrity);
            };
            let command = &packet.identity.command;
            let a = self.native_proof();
            let bytes = command.bytes();
            let mut rows = self
                .rows(
                    RESOLVE,
                    &[
                        &a[0],
                        &a[1],
                        &a[2],
                        &a[3],
                        &a[4],
                        &command.request().operation_id,
                        &bytes,
                    ],
                )
                .await?;
            if rows.len() != 1 {
                return Err(NativeError::Integrity);
            }
            let row = rows.remove(0);
            history.resolve(
                &get::<String>(&row, 0)?,
                get(&row, 1)?,
                get::<Option<String>>(&row, 2)?.map(scalar).transpose()?,
            )
        }
        .await;
        drop(packet);
        let rollback = self.rollback().await.err();
        match (checked, rollback) {
            (Err(primary), rollback) => Err(SelectionFailure::Operation { primary, rollback }),
            (Ok(_), Some(error)) => Err(SelectionFailure::Rollback(error)),
            (Ok(value), None) => Ok(value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn authority() -> Authority {
        Authority {
            installation: "installation".into(),
            owner: "owner".into(),
            runtime: 1,
            authentication: 2,
            revision: 3,
        }
    }
    fn identity(live: Arc<RuntimeLiveness>, deadline: Instant) -> Identity {
        Identity {
            command: Arc::new(
                Canonical::admit(
                    "installation",
                    "owner",
                    task_contract::current_work::SelectionCommand {
                        user_id: UserId("owner".into()),
                        operation_id: "a".repeat(32),
                        expected_version: 0,
                        requested: None,
                    },
                )
                .unwrap(),
            ),
            epoch: [1, 2, 3],
            live,
            deadline,
            recorded: None,
        }
    }
    #[test]
    fn command_witness_rejects_other_authority_or_execution_context() {
        let live = RuntimeLiveness::new();
        let deadline = Instant::now() + std::time::Duration::from_secs(60);
        let original = identity(Arc::clone(&live), deadline);
        assert!(original.check(&authority(), &live, deadline).is_ok());
        for field in 0..5 {
            let mut other = authority();
            match field {
                0 => other.installation.push('x'),
                1 => other.owner.push('x'),
                2 => other.runtime += 1,
                3 => other.authentication += 1,
                _ => other.revision += 1,
            };
            assert!(matches!(
                original.check(&other, &live, deadline),
                Err(NativeError::Integrity)
            ));
        }
        assert!(matches!(
            original.check(&authority(), &RuntimeLiveness::new(), deadline),
            Err(NativeError::Integrity)
        ));
        assert!(matches!(
            original.check(
                &authority(),
                &live,
                deadline + std::time::Duration::from_millis(1)
            ),
            Err(NativeError::Integrity)
        ));
    }
    #[test]
    fn command_witness_cannot_revive_after_stop_or_deadline() {
        let live = RuntimeLiveness::new();
        let expired = identity(
            Arc::clone(&live),
            Instant::now() - std::time::Duration::from_millis(1),
        );
        assert!(matches!(
            expired.check_live(),
            Err(NativeError::Database(DbError::Expired))
        ));
        let active = identity(
            Arc::clone(&live),
            Instant::now() + std::time::Duration::from_secs(60),
        );
        live.stop();
        assert!(matches!(
            active.check_live(),
            Err(NativeError::Database(DbError::Unavailable))
        ));
    }
    #[test]
    fn historical_release_rejects_different_operation_version_and_impossible_statuses() {
        let mut history = History {
            operation: 7,
            version: 2,
            same_identity: true,
            retired: false,
            reply: Some(b"admitted".to_vec()),
        };
        assert!(matches!(
            history.resolve("replay", Some(7), Some(2)),
            Ok(None)
        ));
        assert!(matches!(
            history.resolve("replay_expired", Some(7), Some(2)),
            Ok(Some(Resolution::Expired))
        ));
        for (status, operation, version) in [
            ("replay", Some(8), Some(2)),
            ("replay", Some(7), Some(3)),
            ("absent", None, None),
            ("operation_id_reused", Some(7), Some(2)),
        ] {
            assert!(matches!(
                history.resolve(status, operation, version),
                Err(NativeError::Integrity)
            ));
        }
        history.retired = true;
        assert!(matches!(
            history.resolve("replay", Some(7), Some(2)),
            Err(NativeError::Integrity)
        ));
        assert!(matches!(
            history.resolve("replay_expired", Some(7), Some(2)),
            Ok(Some(Resolution::Expired))
        ));
        history.same_identity = false;
        assert!(matches!(
            history.resolve("replay_expired", Some(7), Some(2)),
            Err(NativeError::Integrity)
        ));
        assert!(matches!(
            history.resolve("operation_id_reused", Some(7), Some(2)),
            Ok(Some(Resolution::Reused))
        ));
    }
}
