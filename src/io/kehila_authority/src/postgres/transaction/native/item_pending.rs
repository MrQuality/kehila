//! Fixed pending-creation entries; no domain execution or saved-result loading.
use super::*;
use crate::item_pending::Registration;
use serde_json::{json, Value};

pub(crate) struct ItemPending {
    pub(super) project: String,
    pub(super) id: i64,
    pub(super) handle: String,
    pub(super) token: String,
    pub(super) target: String,
    pub(super) codec: u32,
    pub(super) digest: [u8; 32],
    pub(super) registered: u64,
    pub(super) expires: u64,
    pub(super) state: State,
    pub(super) operation: Option<i64>,
}
pub(super) struct StoredRequest {
    pub(super) request: work_item_codec::Request,
    pub(super) bytes: Vec<u8>,
}
pub(super) struct DecodedPending {
    pub(super) entry: ItemPending,
    pub(super) request: Option<StoredRequest>,
}

impl ItemPending {
    #[cfg(test)]
    pub(crate) fn id(&self) -> i64 {
        self.id
    }
    pub(crate) fn value(&self) -> Value {
        json!({"project_id":self.project,"request_id":self.id.to_string(),
            "intent_handle":self.handle,"token":self.token,"target_item_id":self.target,
            "registered_ms":self.registered.to_string(),"expires_ms":self.expires.to_string(),
            "state":match self.state { State::Prepared=>"prepared",State::Completed=>"completed",
                State::Cancelled=>"cancelled",State::Expired=>"expired"}})
    }
}

macro_rules! summary {
    () => {
        "SELECT p.request_id,p.intent_handle::text,p.token::text,p.target_item_id::text,\
     p.request_codec_version,p.request_sha256,p.registered_ms::text,p.expires_ms::text,\
     p.state,p.operation_row_id"
    };
}
const REGISTER: &str =
    concat!(summary!(), " FROM kehila_auth.register_item(",proof!(),
    ",$6::text::kehila.ident,$7::text::kehila_auth.intent_key,$8::text::kehila_auth.intent_key,\
    $9::text::kehila.ident,1::bigint,$10::bytea) p LIMIT 2");
const CANCEL: &str = concat!(
    summary!(),
    " FROM kehila_auth.cancel_item(",
    proof!(),
    ",$6::text::kehila.ident,$7::text::kehila_auth.intent_key) p LIMIT 2"
);
const LIST: &str = concat!(
    summary!(),
    " FROM kehila_auth.list_pending_items(",
    proof!(),
    ",$6::text::kehila.ident,$7::bigint) p ORDER BY p.request_id DESC LIMIT 17"
);
const OPEN: &str = concat!(
    summary!(),
    ",CASE WHEN octet_length(p.request_bytes)<=1048576 THEN p.request_bytes END \
    FROM kehila_auth.open_pending_item(",
    proof!(),
    ",$6::text::kehila.ident,$7::text::kehila_auth.intent_key) p LIMIT 2"
);

fn pending(row: &Row, project: &str) -> Result<ItemPending> {
    let id: i64 = get(row, 0)?;
    let handle: String = get(row, 1)?;
    let token: String = get(row, 2)?;
    let target: String = get(row, 3)?;
    let codec = u32::try_from(get::<i64>(row, 4)?).map_err(|_| NativeError::Integrity)?;
    let digest: [u8; 32] = get::<Vec<u8>>(row, 5)?
        .try_into()
        .map_err(|_| NativeError::Integrity)?;
    let registered = scalar(get(row, 6)?)?;
    let expires = scalar(get(row, 7)?)?;
    let state = match get::<String>(row, 8)?.as_str() {
        "prepared" => State::Prepared,
        "completed" => State::Completed,
        "cancelled" => State::Cancelled,
        "expired" => State::Expired,
        _ => return Err(NativeError::Integrity),
    };
    let operation: Option<i64> = get(row, 9)?;
    if id <= 0
        || !key(&handle)
        || token.is_empty()
        || token.len() > 128
        || !token.bytes().all(|b| (b'!'..=b'~').contains(&b))
        || target.is_empty()
        || target.len() > 128
        || target.contains('\0')
        || codec != 1
        || !expires
            .checked_sub(registered)
            .is_some_and(|span| (60_000..=604_800_000).contains(&span))
        || match state {
            State::Completed => !operation.is_some_and(|id| id > 0),
            _ => operation.is_some(),
        }
    {
        return Err(NativeError::Integrity);
    }
    Ok(ItemPending {
        project: project.into(),
        id,
        handle,
        token,
        target,
        codec,
        digest,
        registered,
        expires,
        state,
        operation,
    })
}

impl<P> Fenced<'_, P> {
    async fn pending_core(&mut self, value: &ItemPending) -> Result<Option<bool>> {
        let Some(operation) = value.operation else {
            return Ok(None);
        };
        // Permanent attribution and retention shape only. EXISTS does not fetch or
        // decode saved content; retirement never gates command inspection.
        let rows = self
            .rows(
                "SELECT actor_id::text,command_family,target_kind,target_project_id::text,\
          target_item_id::text,target_user_id::text,target_relationship_id::text,token::text,\
          request_codec_version,request_sha256,required_grant,grant_policy_version,\
          replay_origin_ms::text,replay_deadline_ms::text,o.payload_retired, \
          EXISTS(SELECT FROM kehila.operation_payloads p WHERE p.operation_row_id=o.operation_row_id) \
          FROM kehila.operations o WHERE o.operation_row_id=$1::bigint LIMIT 2",
                &[&operation],
            )
            .await?;
        if rows.len() != 1 {
            return Err(NativeError::Integrity);
        }
        let row = &rows[0];
        let origin = scalar(get(row, 12)?)?;
        let end = scalar(get(row, 13)?)?;
        let retired: bool = get(row, 14)?;
        let has_payload: bool = get(row, 15)?;
        if get::<String>(row, 0)? != self.authority.owner
            || get::<String>(row, 1)? != "item_create"
            || get::<String>(row, 2)? != "work_item"
            || get::<String>(row, 3)? != value.project
            || get::<String>(row, 4)? != value.target
            || get::<Option<String>>(row, 5)?.is_some()
            || get::<Option<String>>(row, 6)?.is_some()
            || get::<String>(row, 7)? != value.token
            || get::<i64>(row, 8)? != i64::from(value.codec)
            || get::<Vec<u8>>(row, 9)?.as_slice() != value.digest.as_slice()
            || get::<String>(row, 10)? != "item_create"
            || get::<i32>(row, 11)? != 1
            || origin.checked_add(task_contract::operation::REPLAY_PERIOD_MS) != Some(end)
            || retired == has_payload
        {
            return Err(NativeError::Integrity);
        }
        Ok(Some(retired))
    }
}

impl Fenced<'_, SessionProof> {
    pub(crate) async fn register_item(
        &mut self,
        registration: &Registration,
    ) -> Result<ItemPending> {
        let a = self.native_proof();
        let rows = self
            .rows(
                REGISTER,
                &[
                    &a[0],
                    &a[1],
                    &a[2],
                    &a[3],
                    &a[4],
                    &registration.project(),
                    &registration.handle(),
                    &registration.token(),
                    &registration.target(),
                    &registration.bytes(),
                ],
            )
            .await?;
        if rows.len() != 1 {
            return Err(NativeError::Integrity);
        }
        let value = pending(&rows[0], registration.project())?;
        if value.state != State::Prepared
            || value.handle != registration.handle()
            || value.token != registration.token()
            || value.target != registration.target()
            || value.digest != work_item_codec::fingerprint(registration.bytes())
        {
            return Err(NativeError::Integrity);
        }
        Ok(value)
    }

    pub(crate) async fn cancel_item(&mut self, project: &str, handle: &str) -> Result<ItemPending> {
        let a = self.native_proof();
        let mut rows = self
            .rows(
                CANCEL,
                &[&a[0], &a[1], &a[2], &a[3], &a[4], &project, &handle],
            )
            .await?;
        if rows.len() > 1 {
            return Err(NativeError::Integrity);
        }
        let row = rows.pop().ok_or(NativeError::Rejected(Failure::Absent))?;
        let value = pending(&row, project)?;
        if value.handle != handle || value.state == State::Prepared {
            return Err(NativeError::Integrity);
        }
        drop(row);
        let _ = self.pending_core(&value).await?;
        Ok(value)
    }
}

impl Fenced<'_, ReadProof> {
    pub(crate) async fn list_pending_items(
        &mut self,
        project: &str,
        before: Option<i64>,
    ) -> Result<Value> {
        let policy = self
            .rows(
                "SELECT page_limit FROM kehila_auth.item_pending_policy WHERE singleton LIMIT 2",
                &[],
            )
            .await?;
        if policy.len() != 1 {
            return Err(NativeError::Integrity);
        }
        let limit = get::<i32>(&policy[0], 0)?;
        if !(1..=16).contains(&limit) {
            return Err(NativeError::Integrity);
        }
        drop(policy);
        let a = self.native_proof();
        let rows = self
            .rows(
                LIST,
                &[&a[0], &a[1], &a[2], &a[3], &a[4], &project, &before],
            )
            .await?;
        if rows.len() > limit as usize + 1 {
            return Err(NativeError::Integrity);
        }
        let mut entries = rows
            .iter()
            .map(|row| pending(row, project))
            .collect::<Result<Vec<_>>>()?;
        drop(rows);
        if entries.windows(2).any(|pair| pair[0].id <= pair[1].id)
            || before.is_some_and(|cursor| entries.first().is_some_and(|entry| entry.id >= cursor))
        {
            return Err(NativeError::Integrity);
        }
        let before = if entries.len() > limit as usize {
            entries.pop();
            entries.last().map(|entry| entry.id.to_string())
        } else {
            None
        };
        for entry in &entries {
            let _ = self.pending_core(entry).await?;
        }
        Ok(
            json!({"entries":entries.iter().map(ItemPending::value).collect::<Vec<_>>(),"before":before}),
        )
    }

    pub(super) async fn read_pending_item(
        &mut self,
        project: &str,
        handle: &str,
    ) -> Result<DecodedPending> {
        let a = self.native_proof();
        let mut rows = self
            .rows(
                OPEN,
                &[&a[0], &a[1], &a[2], &a[3], &a[4], &project, &handle],
            )
            .await?;
        if rows.len() > 1 {
            return Err(NativeError::Integrity);
        }
        let row = rows.pop().ok_or(NativeError::Rejected(Failure::Absent))?;
        let entry = pending(&row, project)?;
        let bytes: Option<Vec<u8>> = get(&row, 10)?;
        // A Row owns the original bytea. Release it before canonical request
        // verification allocates another complete encoding.
        drop(row);
        if entry.handle != handle
            || bytes.as_ref().is_some_and(|bytes| {
                bytes.is_empty() || bytes.len() > project_create_codec::MAX_RECORD_BYTES
            })
            || (entry.state == State::Prepared && bytes.is_none())
            || (matches!(entry.state, State::Cancelled | State::Expired) && bytes.is_some())
        {
            return Err(NativeError::Integrity);
        }
        let retired = self.pending_core(&entry).await?;
        if entry.state == State::Completed && bytes.is_none() && retired != Some(true) {
            return Err(NativeError::Integrity);
        }
        let request = if let Some(bytes) = bytes {
            let request = work_item_codec::verify_request(entry.codec, &bytes, &entry.digest)
                .map_err(|_| NativeError::Integrity)?;
            let work_item_codec::Request::Create {
                project_id,
                command,
            } = &request
            else {
                return Err(NativeError::Integrity);
            };
            if project_id.0 != entry.project
                || command.operation_id != entry.token
                || command.item_id.0 != entry.target
            {
                return Err(NativeError::Integrity);
            }
            Some(StoredRequest { request, bytes })
        } else {
            None
        };
        Ok(DecodedPending { entry, request })
    }
    pub(crate) async fn open_pending_item(&mut self, project: &str, handle: &str) -> Result<Value> {
        let DecodedPending { entry, request } = self.read_pending_item(project, handle).await?;
        let request = if let Some(StoredRequest { request, bytes }) = request {
            // Historical inspection neither regenerates native envelopes nor
            // applies current domain rules. Keep its original copy lifetime.
            drop(bytes);
            let encoded = work_item_codec::encode_request(&request, entry.codec)
                .map_err(|_| NativeError::Integrity)?;
            drop(request);
            let value =
                project_create_codec::parse_json(&encoded).map_err(|_| NativeError::Integrity)?;
            drop(encoded);
            value
        } else {
            Value::Null
        };
        Ok(json!({"entry":entry.value(),"request":request}))
    }
}
