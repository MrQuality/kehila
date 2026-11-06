//! Historical storage validation only. No fresh mutation or transport authority.
use super::super::super::item_read::{HistoricalRecord, Stored};
use super::super::{project_from, SavedResult};
use super::*;
use std::collections::{HashMap, HashSet};
use task_contract::{
    field::{FieldEdit, FieldEntry},
    item_mutation::EstimateEdit,
    project::ReadableId,
};

// Direct edits retain the exact submitted Arc. Creation retains its server-owned
// locator; its canonical command is recovered only from verified stored bytes.
pub(crate) enum ReplayRequest {
    Create { project: String, handle: String },
    Edit(Arc<Canonical>),
}
impl ReplayRequest {
    fn project(&self) -> &str {
        match self {
            Self::Create { project, .. } => project,
            Self::Edit(value) => value.project_id(),
        }
    }
}
pub(crate) enum HistoryPreparation {
    // An observation only. This must never grant permission to execute an edit
    // from a reconciliation request or resurrect expired/retired identities.
    Absent,
    Resolved(Resolved),
    Ready(Box<HistoricalDraft>),
}
struct Payload {
    request: Vec<u8>,
    digest: [u8; 32],
    request_codec: u32,
    result: String,
    result_codec: u32,
    version: u64,
    readable: ReadableId,
    creation: i64,
}
pub(crate) struct HistoricalDraft {
    request: ReplayRequest,
    data: Metadata,
    scope: Scope,
    epoch: Epoch,
    live: Arc<RuntimeLiveness>,
    deadline: Instant,
    payload: Payload,
    revision: HistoricalRecord,
}
// Verified native data, NOT permission to deliver it. The serving owner must
// pair it privately with the complete admitted reply and obtain the final fresh
// replay-only protected check before handing any bytes to transport.
pub(crate) struct HistoricalAccepted {
    original: RetryIdentity,
    data: Metadata,
    result: String,
}
impl HistoricalAccepted {
    pub(crate) fn result_bytes(&self) -> &[u8] {
        self.result.as_bytes()
    }
}
fn invalid<T>() -> std::result::Result<T, PreparationError> {
    Err(PreparationError::InvalidProof)
}
fn check_time(
    live: &RuntimeLiveness,
    deadline: Instant,
) -> std::result::Result<(), PreparationError> {
    if !live.is_live() {
        return Err(PreparationError::Unavailable);
    }
    if Instant::now() >= deadline {
        return Err(PreparationError::Expired);
    }
    Ok(())
}
impl HistoricalDraft {
    pub(crate) fn admit(self) -> std::result::Result<HistoricalAccepted, PreparationError> {
        check_time(&self.live, self.deadline)?;
        let Self {
            request,
            data,
            scope,
            epoch,
            live,
            deadline,
            payload,
            revision,
        } = self;
        let stored = revision
            .decode()
            .map_err(|_| PreparationError::InvalidProof)?;
        let command = match request {
            ReplayRequest::Create { project, handle } => {
                let request = work_item_codec::verify_request(
                    payload.request_codec,
                    &payload.request,
                    &payload.digest,
                )
                .map_err(|_| PreparationError::InvalidProof)?;
                let Request::Create { project_id, .. } = &request else {
                    return invalid();
                };
                if project_id.0 != project {
                    return invalid();
                }
                let command = Arc::new(
                    Canonical::new(scope, request).map_err(|_| PreparationError::InvalidProof)?,
                );
                if command.bytes() != payload.request {
                    return invalid();
                }
                let identity = Identity::Create {
                    handle,
                    pending: data.pending.ok_or(PreparationError::InvalidProof)?,
                    expires: data.pending_expires.ok_or(PreparationError::InvalidProof)?,
                };
                (command, identity)
            }
            ReplayRequest::Edit(command) => {
                if command.scope != scope
                    || payload.request_codec != 1
                    || command.digest != payload.digest
                    || command.bytes() != payload.request
                {
                    return invalid();
                }
                (command, Identity::Edit)
            }
        };
        // Release the extra stored request copy before result/history comparison.
        // The result String and typed permanent revision remain live together;
        // these record bounds do not claim to bound allocator/decoder peak RSS.
        drop(payload.request);
        let result =
            work_item_codec::decode_result(payload.result_codec, payload.result.as_bytes())
                .map_err(|_| PreparationError::InvalidProof)?;
        verify_history(
            &command.0,
            &data,
            &stored,
            &result,
            payload.version,
            &payload.readable,
            payload.creation,
        )?;
        drop(stored);
        drop(result);
        let original = RetryIdentity {
            command: command.0,
            identity: command.1,
            epoch,
            live,
            deadline,
        };
        original
            .check_resolved(&Resolved {
                outcome: Disposition::Replay,
                data: data.clone(),
            })
            .map_err(|_| PreparationError::InvalidProof)?;
        check_time(&original.live, original.deadline)?;
        Ok(HistoricalAccepted {
            original,
            data,
            result: payload.result,
        })
    }
}
// This verifies historical identity and attribution; it deliberately never calls
// decide_create/edit or consults mutable project/configuration/item state.
fn verify_history(
    command: &Canonical,
    data: &Metadata,
    stored: &Stored,
    result: &SavedResult,
    version: u64,
    readable: &ReadableId,
    creation: i64,
) -> std::result::Result<(), PreparationError> {
    let revision = &stored.revision;
    let (project, item, token, family) = super::save::command_parts(&command.request);
    if data.operation != Some(stored.operation)
        || data.target.as_deref() != Some(item)
        || data.token.as_deref() != Some(token)
        || stored.operation <= 0
        || creation <= 0
        || revision.actor_id.0 != command.scope.actor
        || &revision.result != result
        || project_from(result).id.0 != project
        || version == 0
        || readable.project_id.0 != project
        || readable.item_id.0 != item
        || !task_contract::project::valid_prefix(&readable.prefix)
        || readable.sequence == 0
        || readable.sequence >= project_from(result).next_sequence
    {
        return invalid();
    }
    match (&command.request, result) {
        (Request::Create { command, .. }, SavedResult::Create(saved)) => {
            if family != "item_create"
                || version != 1
                || saved.item.version != 1
                || creation != stored.operation
                || saved.item.id.0 != item
                || saved.item.project_id.0 != project
                || saved.readable_id != *readable
                || saved.project.prefix != readable.prefix
                || Some(saved.project.next_sequence) != readable.sequence.checked_add(1)
                || saved.item.type_id != command.type_id
                || command
                    .workflow_id
                    .as_ref()
                    .is_some_and(|workflow| workflow != &saved.item.workflow_id)
                || revision.configuration_revision != command.expected_configuration_revision
                || saved.data.estimate != command.estimate
                || !same_values(&saved.data.values, &command.values)
            {
                return invalid();
            }
        }
        (Request::Edit { command, .. }, SavedResult::Edit(saved)) => {
            if family != "item_edit"
                || version != saved.item.version
                || saved.item.id.0 != item
                || saved.item.project_id.0 != project
                || Some(version) != command.expected_item_version.checked_add(1)
                || revision.configuration_revision != command.expected_configuration_revision
            {
                return invalid();
            }
            match command.estimate {
                EstimateEdit::Keep => (),
                EstimateEdit::Set(value) if saved.data.estimate == Some(value) => (),
                EstimateEdit::Clear if saved.data.estimate.is_none() => (),
                _ => return invalid(),
            }
            let mut fields = HashSet::with_capacity(command.fields.len());
            for field in &command.fields {
                if !fields.insert(&field.id) {
                    return invalid();
                }
                let value = saved
                    .data
                    .values
                    .iter()
                    .find(|entry| entry.id == field.id)
                    .map(|entry| &entry.value);
                match &field.edit {
                    FieldEdit::Keep => (),
                    FieldEdit::Set(expected) if value == Some(expected) => (),
                    FieldEdit::Clear if value.is_none() => (),
                    _ => return invalid(),
                }
            }
        }
        _ => return invalid(),
    }
    Ok(())
}
// Result order follows the historical configuration, not submitted order.
// Compare complete field maps without consulting today's configuration. Exact
// submitted order remains part of immutable canonical request identity.
fn same_values(saved: &[FieldEntry], submitted: &[FieldEntry]) -> bool {
    let saved_map: HashMap<_, _> = saved
        .iter()
        .map(|entry| (&entry.id, &entry.value))
        .collect();
    let submitted_map: HashMap<_, _> = submitted
        .iter()
        .map(|entry| (&entry.id, &entry.value))
        .collect();
    saved_map.len() == saved.len()
        && submitted_map.len() == submitted.len()
        && saved_map == submitted_map
}
impl Fenced<'_, ReadProof> {
    pub(crate) async fn prepare_item_history(
        &mut self,
        request: ReplayRequest,
    ) -> NativeResult<HistoryPreparation> {
        let lookup = match &request {
            ReplayRequest::Create { project, handle } => {
                self.lookup_item_create(project, handle).await?
            }
            ReplayRequest::Edit(command) => self.lookup_item_edit(command).await?,
        };
        match lookup.kind {
            Kind::Create | Kind::Edit => return Ok(HistoryPreparation::Absent),
            Kind::Resolved(outcome) if outcome != Disposition::Replay => {
                return Ok(HistoryPreparation::Resolved(Resolved {
                    outcome,
                    data: lookup.value,
                }))
            }
            Kind::Resolved(_) => (),
        }
        let data = lookup.value;
        let operation = data.operation.ok_or(NativeError::Integrity)?;
        let target = data.target.as_deref().ok_or(NativeError::Integrity)?;
        let token = data.token.as_deref().ok_or(NativeError::Integrity)?;
        let family = match &request {
            ReplayRequest::Create { .. } => "item_create",
            ReplayRequest::Edit(_) => "item_edit",
        };
        let a = self.authority();
        let actor = a.owner.clone();
        let installation = a.installation.clone();
        let project = request.project();
        let mut rows = self
            .rows(
                include_str!("replay/historical_operation.sql"),
                &[
                    &operation,
                    &actor,
                    &family,
                    &project,
                    &target,
                    &token,
                    &data.pending,
                ],
            )
            .await?;
        if rows.len() != 1 {
            return Err(NativeError::Integrity);
        }
        let row = rows.pop().ok_or(NativeError::Integrity)?;
        drop(rows);
        let payload = decode_payload(row, &data, &actor, project, target, token, family)?;
        let revision = self
            .historical_item_revision(project, target, payload.version)
            .await?;
        Ok(HistoryPreparation::Ready(Box::new(HistoricalDraft {
            request,
            data,
            scope: Scope {
                installation,
                actor,
            },
            epoch: Epoch::from(self.authority()),
            live: Arc::clone(&self.pending.live),
            deadline: self.deadline,
            payload,
            revision,
        })))
    }
}
fn decode_payload(
    row: Row,
    data: &Metadata,
    actor: &str,
    project: &str,
    item: &str,
    token: &str,
    family: &str,
) -> NativeResult<Payload> {
    let request_codec = u32::try_from(get::<i64>(&row, 8)?).map_err(|_| NativeError::Integrity)?;
    let result_codec = u32::try_from(get::<i64>(&row, 16)?).map_err(|_| NativeError::Integrity)?;
    let digest: [u8; 32] = get::<Vec<u8>>(&row, 9)?
        .try_into()
        .map_err(|_| NativeError::Integrity)?;
    let request = get::<Option<Vec<u8>>>(&row, 15)?.ok_or(NativeError::Integrity)?;
    let result = get::<Option<String>>(&row, 17)?.ok_or(NativeError::Integrity)?;
    let version = scalar(get(&row, 18)?)?;
    let readable = ReadableId {
        project_id: task_contract::work_item::ProjectId(project.into()),
        item_id: task_contract::work_item::WorkItemId(item.into()),
        prefix: get(&row, 20)?,
        sequence: scalar(get(&row, 21)?)?,
    };
    let creation = get::<i64>(&row, 22)?;
    if get::<String>(&row, 0)? != actor
        || get::<String>(&row, 1)? != family
        || get::<String>(&row, 2)? != "work_item"
        || get::<String>(&row, 3)? != project
        || get::<String>(&row, 4)? != item
        || get::<Option<String>>(&row, 5)?.is_some()
        || get::<Option<String>>(&row, 6)?.is_some()
        || get::<String>(&row, 7)? != token
        || Some(request_codec) != data.codec
        || request_codec == 0
        || result_codec == 0
        || get::<String>(&row, 10)? != family
        || get::<i64>(&row, 11)? != 1
        || Some(scalar(get(&row, 12)?)?) != data.origin
        || Some(scalar(get(&row, 13)?)?) != data.end
        || get::<bool>(&row, 14)?
        || !get::<bool>(&row, 19)?
        || !get::<bool>(&row, 23)?
        || request.is_empty()
        || request.len() > project_create_codec::MAX_RECORD_BYTES
        || result.is_empty()
        || result.len() > project_create_codec::MAX_RECORD_BYTES
        || version == 0
        || creation <= 0
    {
        return Err(NativeError::Integrity);
    }
    Ok(Payload {
        request,
        digest,
        request_codec,
        result,
        result_codec,
        version,
        readable,
        creation,
    })
}

#[cfg(test)]
mod tests;

mod release;
pub(crate) use release::{HistoryAuthorization, HistoryReleaseFailure};
