-- Structural fixture only: no authenticated actor or complete typed M1 seed.
-- Install after the existing SP-005 role/guard assertions.
SET ROLE kehila_owner;
CREATE FUNCTION sp005.fixture_create(actor text, target text, token text, handle text)
RETURNS bigint LANGUAGE plpgsql SECURITY INVOKER
SET search_path=pg_catalog,sp005,kehila,pg_temp AS $$
DECLARE operation bigint; permitted kehila.actors;
BEGIN
  -- Repeated by every attempt, including a fresh-target retry after rollback.
  SELECT a.* INTO STRICT permitted FROM kehila.lock_actor(actor::kehila.ident) a;
  IF NOT permitted.active OR NOT permitted.can_create_project THEN
    RAISE EXCEPTION 'fixture creation authority denied' USING ERRCODE='42501';
  END IF;
  IF EXISTS (SELECT FROM sp005.creation_intents m
    WHERE m.actor_id=actor AND m.command_family='project_create' AND m.intent_handle=handle) THEN
    RAISE EXCEPTION 'fixture intent already completed';
  END IF;
  INSERT INTO kehila.operations
    (actor_id,command_family,target_kind,target_project_id,token,request_codec_version,
     request_sha256,required_grant,grant_policy_version,replay_origin_ms,replay_deadline_ms)
  VALUES(actor,'project_create','project',target,token,1,decode(repeat('01',32),'hex'),
    'project_create',1,200,7776000200) RETURNING operation_row_id INTO operation;
  INSERT INTO kehila.operation_payloads VALUES(operation,'\x01',1,'{}');
  INSERT INTO kehila.projects
    (project_id,name,current_prefix,estimate_unit,configuration_revision,next_sequence,
     created_by,creation_operation_row_id,seed_profile)
  VALUES(target,'Concurrent structure fixture','CF','hours',1,1,actor,operation,'m1_v1');
  INSERT INTO kehila.configuration_revisions VALUES
    (target,1,1,'{}','{}',actor,operation,'project_create','project');
  INSERT INTO kehila.project_grants VALUES(target,actor,'initial_owner',true,1);
  INSERT INTO kehila.project_grant_events
    (project_id,actor_id,version,access_profile,active,granted_by,granting_operation_row_id,command_family)
  VALUES(target,actor,1,'initial_owner',true,actor,operation,'project_create');
  INSERT INTO kehila.statuses VALUES(target,'new','New','new',false,0),(target,'done','Done','done',false,1);
  INSERT INTO kehila.workflows VALUES(target,'flow','new','new',false,0);
  INSERT INTO kehila.workflow_statuses VALUES(target,'flow','new',0),(target,'flow','done',1);
  INSERT INTO kehila.workflow_phase_changes VALUES(target,'flow','new','done',0);
  INSERT INTO kehila.work_item_types VALUES(target,'task','flow','title',false,0);
  INSERT INTO kehila.type_workflows VALUES(target,'task','flow',0);
  INSERT INTO kehila.fields VALUES(target,'title','task','Title','text','application','optional',false,0);
  INSERT INTO sp005.creation_intents VALUES(actor,'project_create',handle,'project',target,operation);
  SET CONSTRAINTS ALL IMMEDIATE;
  RETURN operation;
END $$;
REVOKE ALL ON FUNCTION sp005.fixture_create(text,text,text,text) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION sp005.fixture_create(text,text,text,text) TO kehila_creator;
RESET ROLE;
