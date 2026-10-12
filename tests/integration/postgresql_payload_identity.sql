-- Dedicated regression: the old core has no event scheduled when only its
-- payload is moved. The NEW core's deferred shape is valid without the guard.
DO $$
DECLARE original_id bigint; moved_id bigint; rejected boolean := false;
        actual_constraint text;
BEGIN
  INSERT INTO kehila.operations
    (actor_id,command_family,target_kind,target_project_id,token,
     request_codec_version,request_sha256,required_grant,grant_policy_version,
     replay_origin_ms,replay_deadline_ms)
  VALUES ('actor','project_create','project','identity-original','identity',1,
          decode(repeat('02',32),'hex'),'project_create',1,0,7776000000)
  RETURNING operation_row_id INTO original_id;
  INSERT INTO kehila.operation_payloads VALUES (original_id,'\x02',1,'{}');
  SET CONSTRAINTS ALL IMMEDIATE;
  SET CONSTRAINTS ALL DEFERRED;
  BEGIN
    INSERT INTO kehila.operations
      (actor_id,command_family,target_kind,target_project_id,token,
       request_codec_version,request_sha256,required_grant,grant_policy_version,
       replay_origin_ms,replay_deadline_ms)
    VALUES ('actor','project_create','project','identity-moved','identity',1,
            decode(repeat('02',32),'hex'),'project_create',1,0,7776000000)
    RETURNING operation_row_id INTO moved_id;
    UPDATE kehila.operation_payloads SET operation_row_id = moved_id
      WHERE operation_row_id = original_id;
    SET CONSTRAINTS ALL IMMEDIATE;
  EXCEPTION WHEN check_violation THEN
    GET STACKED DIAGNOSTICS actual_constraint = CONSTRAINT_NAME;
    IF actual_constraint <> 'operation_payload_identity_immutable' THEN RAISE; END IF;
    rejected := true;
  END;
  IF NOT rejected THEN
    RAISE EXCEPTION 'identity regression did not reject reassignment';
  END IF;
END;
$$;
