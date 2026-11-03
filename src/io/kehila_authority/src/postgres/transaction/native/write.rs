use super::*;
use task_contract::operation::REPLAY_PERIOD_MS;
use task_contract::{project_create::SeedProfile, work_item::Phase};

pub(super) fn phase(value: Phase) -> &'static str {
    match value {
        Phase::New => "new",
        Phase::Active => "active",
        Phase::Done => "done",
    }
}
fn sql_position(value: usize) -> Result<i16> {
    i16::try_from(value).map_err(|_| NativeError::Integrity)
}

impl Fenced<'_, SessionProof> {
    pub(crate) async fn write_project(
        &mut self,
        request_id: i64,
        prepared: &Prepared,
    ) -> Result<i64> {
        let command = prepared.command();
        let result = prepared.result();
        let p = &result.project;
        let c = &result.configuration;
        // Session fencing alone is insufficient for creation. Reopen through
        // the SQL project fence before any permanent write, even if a caller
        // has already performed its lookup in this transaction.
        let pending = self
            .open_project(request_id, false)
            .await?
            .ok_or(NativeError::Rejected(Failure::Absent))?;
        match pending.state {
            State::Prepared => {}
            State::Completed => return Err(NativeError::Rejected(Failure::Conflict)),
            State::Cancelled => return Err(NativeError::Rejected(Failure::Cancelled)),
            State::Expired => return Err(NativeError::Rejected(Failure::Expired)),
        }
        let form = pending.form.as_ref().ok_or(NativeError::Integrity)?;
        if pending.token != command.operation_id
            || form.name != command.name
            || form.prefix != command.prefix
            || form.estimate_unit != command.estimate_unit
        {
            return Err(NativeError::Rejected(Failure::Conflict));
        }
        let actor = self.authority.owner.clone();
        let id = &p.id.0;
        let unit = project::unit_name(p.estimate_unit);
        let seed = match result.seed_profile {
            SeedProfile::M1V1 => "m1_v1",
        };
        let revision = p.configuration_revision.to_string();
        let sequence = p.next_sequence.to_string();
        let request = prepared.request_bytes();
        let digest = prepared.digest().as_slice();
        let saved =
            std::str::from_utf8(prepared.result_bytes()).map_err(|_| NativeError::Integrity)?;
        let project_snapshot = std::str::from_utf8(prepared.snapshots().project())
            .map_err(|_| NativeError::Integrity)?;
        let configuration_snapshot = std::str::from_utf8(prepared.snapshots().configuration())
            .map_err(|_| NativeError::Integrity)?;
        // Called only after authoritative lookup/acceptance. No seed generation,
        // hashing or entropy work follows this replay-origin sampling point.
        let origin = self.trusted_millis().await?;
        let deadline = origin
            .checked_add(REPLAY_PERIOD_MS)
            .ok_or(NativeError::Integrity)?
            .to_string();
        let origin = origin.to_string();
        let row=self.run("INSERT INTO kehila.operations \
          (actor_id,command_family,target_kind,target_project_id,token,request_codec_version,request_sha256,\
           required_grant,grant_policy_version,replay_origin_ms,replay_deadline_ms) \
          VALUES ($1::text::kehila.ident,'project_create','project',$2::text::kehila.ident,\
           $3::text::kehila.operation_token,1,$4::bytea,'project_create',1,$5::text::kehila.u64,$6::text::kehila.u64) \
          RETURNING operation_row_id",&[&actor,id,&command.operation_id,&digest,&origin,&deadline]).await?;
        let operation: i64 = get(&row, 0)?;
        if operation <= 0 {
            return Err(NativeError::Integrity);
        }
        self.run("INSERT INTO kehila.operation_payloads(operation_row_id,request_bytes,result_codec_version,result) \
          VALUES ($1::bigint,$2::bytea,1,$3::text::jsonb) RETURNING 1",&[&operation,&request,&saved]).await?;
        self.run("INSERT INTO kehila.projects \
          (project_id,name,current_prefix,estimate_unit,configuration_revision,next_sequence,ever_estimated,archived,\
           created_by,creation_operation_row_id,seed_profile) \
          VALUES ($1::text::kehila.ident,$2::text,$3::text::kehila.project_prefix,$4::text,\
           $5::text::kehila.u64,$6::text::kehila.u64,$7::boolean,$8::boolean,$9::text::kehila.ident,$10::bigint,$11::text) \
          RETURNING 1",&[id,&p.name,&p.prefix,&unit,&revision,&sequence,&p.ever_estimated,&p.archived,&actor,&operation,&seed]).await?;
        self.run("INSERT INTO kehila.configuration_revisions \
          (project_id,revision,snapshot_codec_version,configuration_snapshot,project_snapshot,actor_id,operation_row_id,command_family) \
          VALUES ($1::text::kehila.ident,$2::text::kehila.u64,1,$3::text::jsonb,$4::text::jsonb,\
           $5::text::kehila.ident,$6::bigint,'project_create') RETURNING 1",
          &[id,&revision,&configuration_snapshot,&project_snapshot,&actor,&operation]).await?;
        self.run(
            "INSERT INTO kehila.project_grants VALUES \
          ($1::text::kehila.ident,$2::text::kehila.ident,'initial_owner',true,1) RETURNING 1",
            &[id, &actor],
        )
        .await?;
        self.run("INSERT INTO kehila.project_grant_events \
          (project_id,actor_id,version,access_profile,active,granted_by,granting_operation_row_id,command_family) \
          VALUES ($1::text::kehila.ident,$2::text::kehila.ident,1,'initial_owner',true,$2::text::kehila.ident,$3::bigint,'project_create') \
          RETURNING 1",&[id,&actor,&operation]).await?;
        for (index, status) in c.statuses.iter().enumerate() {
            let position = sql_position(index)?;
            let phase = phase(status.phase);
            self.run("INSERT INTO kehila.statuses VALUES \
              ($1::text::kehila.ident,$2::text::kehila.ident,$3::text,$4::text::kehila.phase,$5::boolean,$6::smallint) RETURNING 1",
              &[id,&status.id.0,&status.name,&phase,&status.archived,&position]).await?;
        }
        for (index, workflow) in c.workflows.iter().enumerate() {
            let position = sql_position(index)?;
            self.run("INSERT INTO kehila.workflows \
              (project_id,workflow_id,initial_status_id,archived,position) VALUES \
              ($1::text::kehila.ident,$2::text::kehila.ident,$3::text::kehila.ident,$4::boolean,$5::smallint) RETURNING 1",
              &[id,&workflow.id.0,&workflow.initial_status_id.0,&workflow.archived,&position]).await?;
            for (index, status) in workflow.status_ids.iter().enumerate() {
                let position = sql_position(index)?;
                self.run("INSERT INTO kehila.workflow_statuses VALUES \
                  ($1::text::kehila.ident,$2::text::kehila.ident,$3::text::kehila.ident,$4::smallint) RETURNING 1",
                  &[id,&workflow.id.0,&status.0,&position]).await?;
            }
            for (index, change) in workflow.permitted_phase_changes.iter().enumerate() {
                let position = sql_position(index)?;
                let from = phase(change.from);
                let to = phase(change.to);
                self.run("INSERT INTO kehila.workflow_phase_changes VALUES \
                  ($1::text::kehila.ident,$2::text::kehila.ident,$3::text::kehila.phase,$4::text::kehila.phase,$5::smallint) RETURNING 1",
                  &[id,&workflow.id.0,&from,&to,&position]).await?;
            }
        }
        for (index, kind) in c.types.iter().enumerate() {
            let position = sql_position(index)?;
            let title = kind.title_field_id.as_ref().map(|id| id.0.as_str());
            self.run("INSERT INTO kehila.work_item_types VALUES \
              ($1::text::kehila.ident,$2::text::kehila.ident,$3::text::kehila.ident,$4::text::kehila.ident,$5::boolean,$6::smallint) RETURNING 1",
              &[id,&kind.id.0,&kind.default_workflow_id.0,&title,&kind.archived,&position]).await?;
            for (index, workflow) in kind.permitted_workflows.iter().enumerate() {
                let position = sql_position(index)?;
                self.run("INSERT INTO kehila.type_workflows VALUES \
                  ($1::text::kehila.ident,$2::text::kehila.ident,$3::text::kehila.ident,$4::smallint) RETURNING 1",
                  &[id,&kind.id.0,&workflow.0,&position]).await?;
            }
        }
        for (index, field) in c.fields.iter().enumerate() {
            let position = sql_position(index)?;
            self.run(
                "INSERT INTO kehila.fields VALUES \
              ($1::text::kehila.ident,$2::text::kehila.ident,$3::text::kehila.ident,$4::text,\
               'text','application','optional',false,$5::smallint) RETURNING 1",
                &[id, &field.id.0, &field.owner_type.0, &field.name, &position],
            )
            .await?;
        }
        Ok(operation)
    }
}
