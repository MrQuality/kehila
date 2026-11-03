use super::*;
use project_create_codec::{
    decode_result, encode_request, fingerprint, verify_request, MAX_RECORD_BYTES,
};
use task_contract::project_create::{ProjectCreateCommand, ProjectCreateResult};
use task_contract::{
    operation::{check_replay_window, OperationError},
    work_item::ProjectId,
};

const _: () = assert!(MAX_RECORD_BYTES == 1048576);
pub(crate) struct Saved {
    pub(crate) request: ProjectCreateCommand,
    pub(crate) result: ProjectCreateResult,
    pub(crate) result_bytes: Vec<u8>,
}
pub(crate) struct Replay {
    pub(crate) entry: Pending,
    pub(crate) saved: Option<Saved>,
}

fn replay_available(origin: u64, at: u64, retired: bool, enforce_expiry: bool) -> Result<bool> {
    if retired {
        return Ok(false);
    }
    match check_replay_window(origin, at) {
        Ok(()) => Ok(true),
        Err(OperationError::ReplayExpired) if !enforce_expiry => Ok(true),
        Err(OperationError::ReplayExpired) => Ok(false),
        Err(OperationError::InvalidOperation) => Err(NativeError::Database(DbError::Unavailable)),
        Err(_) => Err(NativeError::Integrity),
    }
}

impl Fenced<'_, SessionProof> {
    pub(crate) async fn saved_project(
        &mut self,
        request_id: i64,
        enforce_expiry: bool,
        echo: Option<&ProjectCreateCommand>,
    ) -> Result<Replay> {
        // Always obtain current creation authority and scoped saved state here;
        // a caller cannot turn an active SessionProof into replay permission.
        let entry = self
            .open_project(request_id, false)
            .await?
            .ok_or(NativeError::Rejected(Failure::Absent))?;
        let saved = self.saved_opened(&entry, enforce_expiry, echo).await?;
        Ok(Replay { entry, saved })
    }
    // None means logically expired/retired. The caller retains the permanent
    // pending/project identity; this path never generates a seed or fresh target.
    async fn saved_opened(
        &mut self,
        pending: &Pending,
        enforce_expiry: bool,
        echo: Option<&ProjectCreateCommand>,
    ) -> Result<Option<Saved>> {
        if pending.state != State::Completed {
            return Err(NativeError::Integrity);
        }
        let operation = pending.operation.ok_or(NativeError::Integrity)?;
        let target = pending.project.as_ref().ok_or(NativeError::Integrity)?;
        let mapping=self.rows("SELECT actor_id::text,command_family,intent_handle::text,target_kind,\
          target_project_id::text,operation_row_id FROM kehila_auth.creation_intents WHERE request_id=$1::bigint LIMIT 1",
          &[&pending.id]).await?;
        if mapping.len() != 1 {
            return Err(NativeError::Integrity);
        }
        let m = &mapping[0];
        if get::<String>(m, 0)? != self.authority.owner
            || get::<String>(m, 1)? != "project_create"
            || get::<String>(m, 2)? != pending.handle
            || get::<String>(m, 3)? != "project"
            || get::<String>(m, 4)? != *target
            || get::<i64>(m, 5)? != operation
        {
            return Err(NativeError::Integrity);
        }
        let core=self.rows("SELECT actor_id::text,command_family,target_kind,target_project_id::text,\
          target_item_id::text,target_user_id::text,target_relationship_id::text,token::text,\
          request_codec_version,request_sha256,required_grant,grant_policy_version,replay_origin_ms::text,\
          replay_deadline_ms::text,payload_retired FROM kehila.lock_operation($1::bigint)",&[&operation]).await?;
        if core.len() != 1 {
            return Err(NativeError::Integrity);
        }
        let core = &core[0];
        let retired: bool = get(core, 14)?;
        let origin = scalar(get(core, 12)?)?;
        let deadline = scalar(get(core, 13)?)?;
        if get::<String>(core, 0)? != self.authority.owner
            || get::<String>(core, 1)? != "project_create"
            || get::<String>(core, 2)? != "project"
            || get::<Option<String>>(core, 3)?.as_ref() != Some(target)
            || get::<Option<String>>(core, 4)?.is_some()
            || get::<Option<String>>(core, 5)?.is_some()
            || get::<Option<String>>(core, 6)?.is_some()
            || get::<String>(core, 7)? != pending.token
            || get::<String>(core, 10)? != "project_create"
            || get::<i32>(core, 11)? != 1
            || origin.checked_add(7_776_000_000) != Some(deadline)
            || pending.form.is_none() != retired
        {
            return Err(NativeError::Integrity);
        }
        let request_version: i64 = get(core, 8)?;
        let request_version = u32::try_from(request_version).map_err(|_| NativeError::Integrity)?;
        let digest: Vec<u8> = get(core, 9)?;
        let digest: [u8; 32] = digest.try_into().map_err(|_| NativeError::Integrity)?;
        // This is bounded historical comparison, not prospective native/seed
        // preparation. The permanent target replaces any fresh candidate ID.
        if let Some(form) = &pending.form {
            let original = ProjectCreateCommand {
                operation_id: pending.token.clone(),
                project_id: ProjectId(target.clone()),
                name: form.name.clone(),
                prefix: form.prefix.clone(),
                estimate_unit: form.estimate_unit,
            };
            let bytes =
                encode_request(&original, request_version).map_err(|_| NativeError::Integrity)?;
            if fingerprint(&bytes) != digest {
                return Err(NativeError::Integrity);
            }
        }
        if let Some(echo) = echo {
            let compared = ProjectCreateCommand {
                project_id: ProjectId(target.clone()),
                ..echo.clone()
            };
            if let Some(form) = &pending.form {
                if compared.operation_id != pending.token
                    || compared.name != form.name
                    || compared.prefix != form.prefix
                    || compared.estimate_unit != form.estimate_unit
                {
                    return Err(NativeError::Rejected(Failure::Conflict));
                }
            }
            let bytes =
                encode_request(&compared, request_version).map_err(|_| NativeError::Integrity)?;
            if fingerprint(&bytes) != digest {
                return Err(NativeError::Rejected(Failure::Conflict));
            }
        }
        let payload_presence=self.run("SELECT EXISTS(SELECT FROM kehila.operation_payloads WHERE operation_row_id=$1::bigint)",
            &[&operation]).await?;
        if get::<bool>(&payload_presence, 0)? == retired {
            return Err(NativeError::Integrity);
        }
        let at = self.trusted_millis().await?;
        if !replay_available(origin, at, retired, enforce_expiry)? {
            return Ok(None);
        }
        let rows=self.rows("SELECT \
          CASE WHEN octet_length(request_bytes)<=1048576 THEN request_bytes END, result_codec_version,\
          CASE WHEN octet_length(result::text)<=1048576 THEN result::text END \
          FROM kehila.operation_payloads WHERE operation_row_id=$1::bigint LIMIT 1",&[&operation]).await?;
        if rows.len() != 1 {
            return Err(NativeError::Integrity);
        }
        let row = &rows[0];
        let request_bytes: Vec<u8> =
            get::<Option<Vec<u8>>>(row, 0)?.ok_or(NativeError::Integrity)?;
        let result_bytes: String = get::<Option<String>>(row, 2)?.ok_or(NativeError::Integrity)?;
        if request_bytes.len() > MAX_RECORD_BYTES || result_bytes.len() > MAX_RECORD_BYTES {
            return Err(NativeError::Integrity);
        }
        let result_version: i64 = get(row, 1)?;
        let result_version = u32::try_from(result_version).map_err(|_| NativeError::Integrity)?;
        let request = verify_request(request_version, &request_bytes, &digest)
            .map_err(|_| NativeError::Integrity)?;
        let result = decode_result(result_version, result_bytes.as_bytes())
            .map_err(|_| NativeError::Integrity)?;
        let form = pending.form.as_ref().ok_or(NativeError::Integrity)?;
        if request.project_id.0 != *target
            || request.operation_id != pending.token
            || request.name != form.name
            || request.prefix != form.prefix
            || request.estimate_unit != form.estimate_unit
            || result.project.id != request.project_id
            || result.project.name != request.name
            || result.project.prefix != request.prefix
            || result.project.estimate_unit != request.estimate_unit
            || result.configuration.project_id != request.project_id
            || result.configuration.revision != result.project.configuration_revision
        {
            return Err(NativeError::Integrity);
        }
        // Historical typed envelopes are checked against their immutable row;
        // current domain acceptance/seed generation is deliberately absent.
        let (project, configuration) = self
            .revision_pair(target, result.project.configuration_revision, operation)
            .await?;
        if project != result.project || configuration != result.configuration {
            return Err(NativeError::Integrity);
        }
        Ok(Some(Saved {
            request,
            result,
            result_bytes: result_bytes.into_bytes(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backwards_clock_cannot_release_full_results_in_either_mode_or_revive_retired_results() {
        for enforce in [true, false] {
            assert!(matches!(
                replay_available(100, 99, false, enforce),
                Err(NativeError::Database(DbError::Unavailable))
            ));
            assert!(matches!(
                replay_available(100, 99, true, enforce),
                Ok(false)
            ));
        }
        assert!(matches!(replay_available(100, 100, false, true), Ok(true)));
        assert!(matches!(
            replay_available(100, 7_776_000_100, false, true),
            Ok(false)
        ));
        // Only coherence verification of a newly written result ignores expiry;
        // it still uses the same before-origin rejection above.
        assert!(matches!(
            replay_available(100, 7_776_000_100, false, false),
            Ok(true)
        ));
    }
}
