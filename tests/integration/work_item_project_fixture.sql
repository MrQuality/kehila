-- Shared relational-only second-project fixture; no typed seed claim.
-- A minimal relational ProjectCreate for cross-project controls. Each caller
-- owns commit/rollback; this does not qualify typed project seed generation.
CREATE FUNCTION pg_temp.second_project() RETURNS void LANGUAGE plpgsql AS $$
DECLARE pending_id bigint; operation_id bigint; at_ms kehila.u64;
BEGIN
  at_ms := kehila_auth.trusted_millis();
  INSERT INTO kehila_auth.pending_projects(installation_id,actor_id,intent_handle,token,
    name,prefix,estimate_unit,registered_ms,expires_ms)
    VALUES ('installation','owner',repeat('c',32),repeat('c',32),'second','PS','hours',at_ms,at_ms+86400000)
    RETURNING request_id INTO pending_id;
  INSERT INTO kehila.operations(actor_id,command_family,target_kind,target_project_id,token,
    request_codec_version,request_sha256,required_grant,grant_policy_version,replay_origin_ms,replay_deadline_ms)
    VALUES ('owner','project_create','project','p2',repeat('c',32),1,sha256(convert_to('{}','UTF8')),
      'project_create',1,at_ms,at_ms+7776000000) RETURNING operation_row_id INTO operation_id;
  INSERT INTO kehila.operation_payloads VALUES(operation_id,convert_to('{}','UTF8'),1,'{}');
  INSERT INTO kehila.projects(project_id,name,current_prefix,estimate_unit,configuration_revision,
    next_sequence,created_by,creation_operation_row_id,seed_profile)
    VALUES ('p2','second','PS','hours',1,1,'owner',operation_id,'m1_v1');
  INSERT INTO kehila.configuration_revisions(project_id,revision,snapshot_codec_version,
    configuration_snapshot,project_snapshot,actor_id,operation_row_id,command_family)
    VALUES ('p2',1,1,'{}','{}','owner',operation_id,'project_create');
  INSERT INTO kehila.project_grants VALUES ('p2','owner','initial_owner',true,1);
  INSERT INTO kehila.project_grant_events(project_id,actor_id,version,access_profile,active,
    granted_by,granting_operation_row_id,command_family)
    VALUES ('p2','owner',1,'initial_owner',true,'owner',operation_id,'project_create');
  UPDATE kehila_auth.pending_projects SET state='completed',target_project_id='p2',operation_row_id=operation_id
    WHERE request_id=pending_id;
  INSERT INTO kehila_auth.creation_intents VALUES(pending_id,'owner','project_create',repeat('c',32),
    'project','p2',operation_id);
END;
$$;
