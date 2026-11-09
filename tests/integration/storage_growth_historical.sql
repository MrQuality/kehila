-- Temporary owner-only structural fixture in a fresh synthetic database.
-- Historical origins are constructed on insertion, never rewritten. This does
-- not qualify elapsed real time, runtime retirement, or typed project seeding.
CREATE FUNCTION pg_temp.storage_project(seed jsonb) RETURNS void LANGUAGE plpgsql AS $$
DECLARE
  p jsonb := seed #> '{value,project}'; i jsonb := seed #> '{value,item}';
  pending_id bigint; op bigint; at_ms kehila.u64 := kehila_auth.trusted_millis();
  entry jsonb; ordinal integer := 0;
BEGIN
  IF EXISTS (SELECT FROM kehila.projects) OR seed->>'kind' <> 'item_create_result' THEN
    RAISE EXCEPTION 'fresh historical storage fixture required';
  END IF;
  INSERT INTO kehila_auth.pending_projects
    (installation_id,actor_id,intent_handle,token,name,prefix,estimate_unit,registered_ms,expires_ms)
    VALUES ('installation','owner',repeat('a',32),repeat('a',32),p->>'name',p->>'prefix',
      p->>'estimate_unit',at_ms,at_ms+86400000) RETURNING request_id INTO pending_id;
  INSERT INTO kehila.operations
    (actor_id,command_family,target_kind,target_project_id,token,request_codec_version,
     request_sha256,required_grant,grant_policy_version,replay_origin_ms,replay_deadline_ms)
    VALUES ('owner','project_create','project',p->>'id',repeat('a',32),1,sha256('\x01'::bytea),
      'project_create',1,at_ms,at_ms+7776000000) RETURNING operation_row_id INTO op;
  INSERT INTO kehila.operation_payloads (operation_row_id,request_bytes,result_codec_version,result)
    VALUES (op,'\x01',1,'{}');
  INSERT INTO kehila.projects
    (project_id,name,current_prefix,estimate_unit,configuration_revision,next_sequence,
     created_by,creation_operation_row_id,seed_profile)
    VALUES (p->>'id',p->>'name',p->>'prefix',p->>'estimate_unit',1,1,'owner',op,'m1_v1');
  INSERT INTO kehila.configuration_revisions
    (project_id,revision,snapshot_codec_version,configuration_snapshot,project_snapshot,
     actor_id,operation_row_id,command_family,target_kind) VALUES
    (p->>'id',1,1,'{}',jsonb_build_object('codec_version',1,'kind','project_snapshot',
      'value',p || jsonb_build_object('next_sequence','1','ever_estimated',false)),
      'owner',op,'project_create','project');
  INSERT INTO kehila.project_grants (project_id,actor_id,access_profile,active,version)
    VALUES (p->>'id','owner','initial_owner',true,1);
  INSERT INTO kehila.project_grant_events
    (project_id,actor_id,version,access_profile,active,granted_by,granting_operation_row_id,command_family)
    VALUES (p->>'id','owner',1,'initial_owner',true,'owner',op,'project_create');
  INSERT INTO kehila.statuses (project_id,status_id,name,phase,archived,position) VALUES
    (p->>'id',i->>'status_id','Storage status',(seed #>> '{value,phase}')::kehila.phase,false,0);
  INSERT INTO kehila.workflows (project_id,workflow_id,initial_status_id,initial_phase,archived,position)
    VALUES (p->>'id',i->>'workflow_id',i->>'status_id','new',false,0);
  INSERT INTO kehila.workflow_statuses (project_id,workflow_id,status_id,position)
    VALUES (p->>'id',i->>'workflow_id',i->>'status_id',0);
  INSERT INTO kehila.work_item_types (project_id,type_id,default_workflow_id,title_field_id,archived,position) VALUES
    (p->>'id',i->>'type_id',i->>'workflow_id',seed #>> '{value,data,values,0,id}',false,0);
  INSERT INTO kehila.type_workflows (project_id,type_id,workflow_id,position)
    VALUES (p->>'id',i->>'type_id',i->>'workflow_id',0);
  FOR entry IN SELECT value FROM jsonb_array_elements(seed #> '{value,data,values}') LOOP
    INSERT INTO kehila.fields (project_id,field_id,owner_type_id,name,value_kind,origin,usage,archived,position) VALUES
      (p->>'id',entry->>'id',i->>'type_id','Storage field '||ordinal,'text','application','optional',false,ordinal);
    ordinal := ordinal+1;
  END LOOP;
  UPDATE kehila_auth.pending_projects SET state='completed',target_project_id=p->>'id',operation_row_id=op
    WHERE request_id=pending_id;
  INSERT INTO kehila_auth.creation_intents
    (request_id,actor_id,command_family,intent_handle,target_kind,target_project_id,operation_row_id) VALUES
    (pending_id,'owner','project_create',repeat('a',32),'project',p->>'id',op);
END;
$$;

CREATE FUNCTION pg_temp.storage_revision(request bytea, saved jsonb, token text, version integer)
RETURNS void LANGUAGE plpgsql AS $$
DECLARE
  p jsonb := saved #> '{value,project}'; i jsonb := saved #> '{value,item}';
  pid text := p->>'id'; iid text := i->>'id'; family text;
  pending_id bigint; op bigint; previous_op bigint; state_version bigint;
  at_ms kehila.u64 := kehila_auth.trusted_millis(); entry jsonb;
BEGIN
  IF version NOT BETWEEN 1 AND 21 OR i->>'version' <> version::text
     OR octet_length(request) NOT BETWEEN 1 AND 1048576
     OR octet_length(saved::text)>1048576 THEN
    RAISE EXCEPTION 'historical storage fixture bound or version mismatch';
  END IF;
  SELECT operation_row_id, h.state_version::bigint INTO STRICT previous_op,state_version
    FROM kehila.project_operational_heads h WHERE project_id=pid;
  IF state_version<>version-1 THEN RAISE EXCEPTION 'historical storage fixture chain mismatch'; END IF;
  family := CASE WHEN version=1 THEN 'item_create' ELSE 'item_edit' END;
  IF version=1 THEN
    INSERT INTO kehila_auth.pending_items
      (installation_id,actor_id,project_id,intent_handle,token,target_item_id,
       request_codec_version,request_sha256,request_bytes,registered_ms,expires_ms)
      VALUES ('installation','owner',pid,repeat('b',32),token,iid,1,sha256(request),request,
        at_ms,at_ms+86400000) RETURNING request_id INTO pending_id;
  END IF;
  INSERT INTO kehila.operations
    (actor_id,command_family,target_kind,target_project_id,target_item_id,token,
     request_codec_version,request_sha256,required_grant,grant_policy_version,
     replay_origin_ms,replay_deadline_ms)
    VALUES ('owner',family,'work_item',pid,iid,token,1,sha256(request),family,1,0,7776000000)
    RETURNING operation_row_id INTO op;
  INSERT INTO kehila.operation_payloads (operation_row_id,request_bytes,result_codec_version,result)
    VALUES (op,request,1,saved);
  IF version=1 THEN
    INSERT INTO kehila.items (item_id,project_id,issued_prefix,issued_sequence,
      creation_request_id,creation_operation_row_id,current_version) VALUES (iid,pid,p->>'prefix',1,pending_id,op,1);
  END IF;
  INSERT INTO kehila.item_revisions
    (project_id,item_id,item_version,previous_item_version,configuration_revision,actor_id,
     command_family,token,operation_row_id,type_id,workflow_id,status_id,phase,archived,
     estimate_thousandths,estimate_unit,history_codec_version,history,project_state_version,
     previous_project_operation,resulting_next_sequence,resulting_ever_estimated)
    VALUES (pid,iid,version,CASE WHEN version=1 THEN NULL ELSE version-1 END,1,'owner',family,
      token,op,i->>'type_id',i->>'workflow_id',i->>'status_id',(saved #>> '{value,phase}')::kehila.phase,
      false,NULL,p->>'estimate_unit',1,
      jsonb_build_object('codec_version',1,'kind','item_revision','value',jsonb_build_object(
        'actor_id','owner','configuration_revision','1','project_state_version',version::text,
        'previous_project_operation',previous_op::text,'result',saved)),version,previous_op,2,false);
  IF version=1 THEN
    FOR entry IN SELECT value FROM jsonb_array_elements(saved #> '{value,data,values}') LOOP
      INSERT INTO kehila.item_field_uses
        (project_id,owner_type_id,field_id,first_item_id,first_item_version,first_operation_row_id)
        VALUES (pid,i->>'type_id',entry->>'id',iid,1,op);
    END LOOP;
    UPDATE kehila.projects SET next_sequence=2 WHERE project_id=pid;
    UPDATE kehila_auth.pending_items SET state='completed',operation_row_id=op WHERE request_id=pending_id;
  ELSE
    UPDATE kehila.items SET current_version=version WHERE item_id=iid;
  END IF;
  UPDATE kehila.project_operational_heads SET state_version=version,operation_row_id=op WHERE project_id=pid;
END;
$$;
