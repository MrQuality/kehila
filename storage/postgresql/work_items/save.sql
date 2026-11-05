-- Private preparation proof. Each JSON record has its own 1 MiB ceiling; the
-- caller also owns aggregate admission/capacity and the original deadline.
CREATE TYPE kehila_auth.item_preparation AS (
  project_snapshot jsonb,
  configuration_revision kehila.u64,
  configuration_operation bigint,
  configuration_snapshot jsonb,
  configuration_project_snapshot jsonb,
  state_version kehila.u64,
  state_operation bigint,
  item_version kehila.u64,
  item_operation bigint,
  item_history jsonb
);

-- Called only by the two fixed entries, after their earlier fences/resolution.
-- This maps one admitted pure history/result; it never applies sparse edits.
-- The transaction owner admits exactly one command against committed project/
-- item rows, with no earlier writes to them. Retry/reprepare requires full
-- rollback. This is not a batching entry; runtime ownership remains required.
CREATE FUNCTION kehila_auth.persist_item_history(
  requested_project kehila.ident, family text, canonical_request bytea,
  prepared kehila_auth.item_preparation, accepted_history jsonb, resolved kehila_auth.item_resolution
) RETURNS kehila_auth.item_resolution LANGUAGE plpgsql VOLATILE
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE
  creating boolean:=family='item_create'; project kehila.projects; head kehila.project_operational_heads;
  fenced_head kehila.project_operational_heads;
  item kehila.items; revision record; configuration record; current_snapshot jsonb;
  saved jsonb; data_values jsonb; field jsonb; new_version kehila.u64; new_state kehila.u64;
  predecessor bigint; new_sequence kehila.u64; new_estimated boolean; new_estimate integer;
  item_type kehila.ident; item_workflow kehila.ident; item_status kehila.ident; item_phase kehila.phase;
  item_archived boolean; origin kehila.u64; deadline kehila.u64; digest bytea; new_operation bigint;
BEGIN
  IF family IS NULL OR family NOT IN ('item_create','item_edit')
     OR resolved.operation_row_id IS NOT NULL OR resolved.target_item_id IS NULL OR resolved.token IS NULL
     OR (resolved.authority).owner_actor_id IS NULL
     OR resolved.outcome IS DISTINCT FROM (CASE WHEN creating THEN 'prepared' ELSE 'unseen' END)
     OR creating IS DISTINCT FROM (resolved.pending_request_id IS NOT NULL) THEN
    RAISE EXCEPTION 'invalid fresh item persistence context'
      USING ERRCODE='23514', CONSTRAINT='item_save_context';
  END IF;
  IF prepared.project_snapshot IS NULL OR jsonb_typeof(prepared.project_snapshot) IS DISTINCT FROM 'object'
     OR prepared.project_snapshot->>'codec_version' IS DISTINCT FROM '1'
     OR jsonb_typeof(prepared.project_snapshot->'codec_version') IS DISTINCT FROM 'number'
     OR prepared.project_snapshot->>'kind' IS DISTINCT FROM 'project_snapshot'
     OR prepared.configuration_revision IS NULL OR prepared.configuration_operation IS NULL OR prepared.configuration_operation<=0
     OR prepared.configuration_snapshot IS NULL OR prepared.configuration_project_snapshot IS NULL
     OR prepared.state_version IS NULL OR prepared.state_operation IS NULL OR prepared.state_operation<=0
     OR octet_length(prepared.project_snapshot::text)>1048576
     OR octet_length(prepared.configuration_snapshot::text)>1048576
     OR octet_length(prepared.configuration_project_snapshot::text)>1048576
     OR (creating AND (prepared.item_version IS NOT NULL OR prepared.item_operation IS NOT NULL OR prepared.item_history IS NOT NULL))
     OR (NOT creating AND (prepared.item_version IS NULL OR prepared.item_version<1 OR prepared.item_operation IS NULL
       OR prepared.item_operation<=0 OR prepared.item_history IS NULL OR octet_length(prepared.item_history::text)>1048576))
     OR canonical_request IS NULL OR octet_length(canonical_request) NOT BETWEEN 1 AND 1048576
     OR accepted_history IS NULL OR jsonb_typeof(accepted_history) IS DISTINCT FROM 'object'
     OR octet_length(accepted_history::text)>1048576
     OR accepted_history->>'codec_version' IS DISTINCT FROM '1'
     OR jsonb_typeof(accepted_history->'codec_version') IS DISTINCT FROM 'number'
     OR accepted_history->>'kind' IS DISTINCT FROM 'item_revision' THEN
    RAISE EXCEPTION 'invalid admitted item packet' USING ERRCODE='22023';
  END IF;
  -- Revision insertion references this external predecessor core. Prehold its
  -- SHARE fence before any item lock; relying on the later FK KEY SHARE would
  -- introduce an earlier lock class after item acquisition and permit retirement.
  SELECT h.* INTO fenced_head FROM kehila.project_operational_heads h WHERE h.project_id=requested_project;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'protected predecessor head unavailable'
      USING ERRCODE='23514', CONSTRAINT='item_save_predecessor';
  END IF;
  PERFORM FROM kehila.operations o JOIN kehila.projects p ON p.project_id=requested_project
    WHERE o.operation_row_id=fenced_head.operation_row_id AND o.target_project_id=requested_project
      AND o.target_user_id IS NULL AND o.target_relationship_id IS NULL
      AND o.required_grant=o.command_family AND o.grant_policy_version=1
      AND ((fenced_head.state_version=0 AND o.operation_row_id=p.creation_operation_row_id
          AND o.command_family='project_create' AND o.target_kind='project' AND o.target_item_id IS NULL
          AND o.actor_id=p.created_by)
        OR (fenced_head.state_version>0 AND o.command_family IN ('item_create','item_edit')
          AND o.target_kind='work_item' AND EXISTS(SELECT FROM kehila.item_revisions r
            WHERE r.project_id=requested_project AND r.project_state_version=fenced_head.state_version
              AND r.operation_row_id=o.operation_row_id AND r.item_id=o.target_item_id
              AND r.actor_id=o.actor_id AND r.command_family=o.command_family AND r.token=o.token)))
    FOR SHARE OF o;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'incoherent protected predecessor core'
      USING ERRCODE='23514', CONSTRAINT='item_save_predecessor';
  END IF;
  IF creating THEN
    IF EXISTS(SELECT FROM kehila.items i WHERE i.item_id=resolved.target_item_id) THEN
      RAISE EXCEPTION 'reserved item target already exists without recorded success'
        USING ERRCODE='23514', CONSTRAINT='item_save_context';
    END IF;
  ELSE
    SELECT i.* INTO item FROM kehila.items i
      WHERE i.project_id=requested_project AND i.item_id=resolved.target_item_id FOR NO KEY UPDATE;
    IF NOT FOUND THEN resolved.outcome:='item_conflict'; RETURN resolved; END IF;
    SELECT r.operation_row_id,r.archived,
        CASE WHEN octet_length(r.history::text)<=1048576 THEN r.history END AS history
      INTO revision FROM kehila.item_revisions r
      WHERE r.project_id=requested_project AND r.item_id=item.item_id AND r.item_version=item.current_version;
    IF NOT FOUND OR revision.history IS NULL THEN
      RAISE EXCEPTION 'current item revision unavailable'
        USING ERRCODE='23514', CONSTRAINT='item_save_reference';
    END IF;
  END IF;
  -- These statements follow every possible pending/core/item wait. Earlier
  -- installation/actor/project locks remain owned; no helper re-fences here.
  SELECT p.* INTO project FROM kehila.projects p WHERE p.project_id=requested_project;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'protected project unavailable' USING ERRCODE='23514', CONSTRAINT='item_save_reference';
  END IF;
  SELECT c.operation_row_id,c.snapshot_codec_version,
      CASE WHEN octet_length(c.configuration_snapshot::text)<=1048576 THEN c.configuration_snapshot END AS configuration_snapshot,
      CASE WHEN octet_length(c.project_snapshot::text)<=1048576 THEN c.project_snapshot END AS project_snapshot
    INTO configuration FROM kehila.configuration_revisions c
    WHERE c.project_id=requested_project AND c.revision=project.configuration_revision;
  IF NOT FOUND OR configuration.snapshot_codec_version<>1
     OR configuration.configuration_snapshot IS NULL OR configuration.project_snapshot IS NULL THEN
    RAISE EXCEPTION 'protected configuration unavailable' USING ERRCODE='23514', CONSTRAINT='item_save_reference';
  END IF;
  SELECT h.* INTO head FROM kehila.project_operational_heads h WHERE h.project_id=requested_project;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'protected operational head unavailable' USING ERRCODE='23514', CONSTRAINT='item_save_reference';
  END IF;
  current_snapshot:=jsonb_build_object('codec_version',1,'kind','project_snapshot','value',jsonb_build_object(
    'id',project.project_id,'name',project.name,'prefix',project.current_prefix,'estimate_unit',project.estimate_unit,
    'configuration_revision',project.configuration_revision::numeric(20,0)::text,
    'next_sequence',project.next_sequence::numeric(20,0)::text,'ever_estimated',project.ever_estimated,'archived',project.archived));
  IF project.archived OR project.configuration_revision<>prepared.configuration_revision
     OR configuration.operation_row_id IS DISTINCT FROM prepared.configuration_operation
     OR configuration.configuration_snapshot IS DISTINCT FROM prepared.configuration_snapshot
     OR configuration.project_snapshot IS DISTINCT FROM prepared.configuration_project_snapshot
     OR (current_snapshot #- '{value,next_sequence}' #- '{value,ever_estimated}')
       IS DISTINCT FROM (prepared.project_snapshot #- '{value,next_sequence}' #- '{value,ever_estimated}') THEN
    resolved.outcome:='configuration_conflict'; RETURN resolved;
  END IF;
  IF NOT creating THEN
    IF item.current_version<>prepared.item_version OR revision.archived
       OR revision.operation_row_id IS DISTINCT FROM prepared.item_operation OR revision.history IS DISTINCT FROM prepared.item_history THEN
      resolved.outcome:='item_conflict'; RETURN resolved;
    END IF;
  END IF;
  IF head.state_version<>fenced_head.state_version OR head.operation_row_id<>fenced_head.operation_row_id
     OR head.state_version<>prepared.state_version OR head.operation_row_id<>prepared.state_operation
     OR current_snapshot #>'{value,next_sequence}' IS DISTINCT FROM prepared.project_snapshot #>'{value,next_sequence}'
     OR current_snapshot #>'{value,ever_estimated}' IS DISTINCT FROM prepared.project_snapshot #>'{value,ever_estimated}' THEN
    resolved.outcome:='reprepare'; RETURN resolved;
  END IF;
  saved:=accepted_history #>'{value,result}';
  data_values:=saved #>'{value,data,values}';
  IF jsonb_typeof(saved) IS DISTINCT FROM 'object' OR octet_length(saved::text)>1048576
     OR saved->>'codec_version' IS DISTINCT FROM '1' OR jsonb_typeof(saved->'codec_version') IS DISTINCT FROM 'number'
     OR saved->>'kind' IS DISTINCT FROM family||'_result' OR jsonb_typeof(data_values) IS DISTINCT FROM 'array' THEN
    RAISE EXCEPTION 'invalid admitted item result' USING ERRCODE='22023';
  END IF;
  IF jsonb_array_length(data_values)>1024 THEN
    RAISE EXCEPTION 'accepted field collection exceeds storage bound' USING ERRCODE='22023';
  END IF;
  -- Decode only fixed persistence fields. Immediate/deferred structural guards
  -- independently verify their agreement with the unchanged admitted envelope.
  new_version:=(saved #>>'{value,item,version}')::kehila.u64;
  new_state:=(accepted_history #>>'{value,project_state_version}')::kehila.u64;
  -- A malformed envelope must not route a later FK lock to an unfenced core.
  -- The immediate history guard compares the unchanged envelope with this fact.
  predecessor:=fenced_head.operation_row_id;
  new_sequence:=(saved #>>'{value,project,next_sequence}')::kehila.u64;
  new_estimated:=(saved #>>'{value,project,ever_estimated}')::boolean;
  new_estimate:=(saved #>>'{value,data,estimate}')::integer;
  item_type:=(saved #>>'{value,item,type_id}')::kehila.ident;
  item_workflow:=(saved #>>'{value,item,workflow_id}')::kehila.ident;
  item_status:=(saved #>>'{value,item,status_id}')::kehila.ident;
  item_phase:=(saved #>>'{value,phase}')::kehila.phase;
  item_archived:=(saved #>>'{value,item,archived}')::boolean;
  digest:=sha256(canonical_request);
  -- No application decisions, entropy or envelope generation follow this
  -- acceptance clock. Database integrity checks remain part of persistence.
  origin:=kehila_auth.trusted_millis();
  deadline:=(origin+7776000000)::kehila.u64;
  INSERT INTO kehila.operations(actor_id,command_family,target_kind,target_project_id,target_item_id,token,
    request_codec_version,request_sha256,required_grant,grant_policy_version,replay_origin_ms,replay_deadline_ms)
    VALUES((resolved.authority).owner_actor_id,family,'work_item',requested_project,resolved.target_item_id,resolved.token,
      1,digest,family,1,origin,deadline) RETURNING operation_row_id INTO new_operation;
  INSERT INTO kehila.operation_payloads VALUES(new_operation,canonical_request,1,saved);
  IF creating THEN
    INSERT INTO kehila.items VALUES(resolved.target_item_id,requested_project,project.current_prefix,project.next_sequence,
      resolved.pending_request_id,new_operation,1);
  END IF;
  INSERT INTO kehila.item_revisions(project_id,item_id,item_version,previous_item_version,configuration_revision,
    actor_id,command_family,token,operation_row_id,type_id,workflow_id,status_id,phase,archived,estimate_thousandths,
    estimate_unit,history_codec_version,history,project_state_version,previous_project_operation,resulting_next_sequence,resulting_ever_estimated)
    VALUES(requested_project,resolved.target_item_id,new_version,CASE WHEN creating THEN NULL ELSE item.current_version END,
      project.configuration_revision,(resolved.authority).owner_actor_id,family,resolved.token,new_operation,
      item_type,item_workflow,item_status,item_phase,item_archived,new_estimate,project.estimate_unit,1,accepted_history,
      new_state,predecessor,new_sequence,new_estimated);
  FOR field IN SELECT value FROM jsonb_array_elements(data_values) LOOP
    INSERT INTO kehila.item_field_uses VALUES(requested_project,item_type,(field->>'id')::kehila.ident,
      resolved.target_item_id,new_version,new_operation) ON CONFLICT DO NOTHING;
  END LOOP;
  IF NOT creating THEN
    UPDATE kehila.items SET current_version=new_version WHERE project_id=requested_project AND item_id=resolved.target_item_id;
  END IF;
  UPDATE kehila.projects SET next_sequence=new_sequence,ever_estimated=new_estimated WHERE project_id=requested_project;
  UPDATE kehila.project_operational_heads SET state_version=new_state,operation_row_id=new_operation WHERE project_id=requested_project;
  IF creating THEN
    UPDATE kehila_auth.pending_items SET state='completed',operation_row_id=new_operation WHERE request_id=resolved.pending_request_id;
  END IF;
  resolved.outcome:='accepted'; resolved.operation_row_id:=new_operation;
  resolved.replay_origin_ms:=origin; resolved.replay_deadline_ms:=deadline;
  RETURN resolved;
END;
$$;

CREATE FUNCTION kehila_auth.save_item_create(
  expected kehila_auth.authority, requested_project kehila.ident, handle kehila_auth.intent_key,
  canonical_request bytea, prepared kehila_auth.item_preparation, accepted_history jsonb
) RETURNS kehila_auth.item_resolution LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE fresh kehila_auth.authority; resolved kehila_auth.item_resolution;
BEGIN
  PERFORM kehila_auth.require_item_resolution_mode(true);
  fresh:=kehila_auth.item_write_fence(expected,requested_project);
  resolved:=kehila_auth.resolve_item_create(fresh,requested_project,handle,canonical_request,true);
  IF resolved.outcome<>'prepared' THEN RETURN resolved; END IF;
  RETURN kehila_auth.persist_item_history(requested_project,'item_create',canonical_request,prepared,accepted_history,resolved);
END;
$$;
CREATE FUNCTION kehila_auth.save_item_edit(
  expected kehila_auth.authority, requested_project kehila.ident, requested_item kehila.ident, request_token kehila.operation_token,
  canonical_request bytea, prepared kehila_auth.item_preparation, accepted_history jsonb
) RETURNS kehila_auth.item_resolution LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE fresh kehila_auth.authority; resolved kehila_auth.item_resolution;
BEGIN
  PERFORM kehila_auth.require_item_resolution_mode(true);
  fresh:=kehila_auth.item_write_fence(expected,requested_project);
  resolved:=kehila_auth.resolve_item_edit(fresh,requested_project,requested_item,request_token,1,canonical_request,true);
  IF resolved.outcome<>'unseen' THEN RETURN resolved; END IF;
  RETURN kehila_auth.persist_item_history(requested_project,'item_edit',canonical_request,prepared,accepted_history,resolved);
END;
$$;
REVOKE ALL ON FUNCTION kehila_auth.persist_item_history(kehila.ident,text,bytea,kehila_auth.item_preparation,jsonb,kehila_auth.item_resolution),
  kehila_auth.save_item_create(kehila_auth.authority,kehila.ident,kehila_auth.intent_key,bytea,kehila_auth.item_preparation,jsonb),
  kehila_auth.save_item_edit(kehila_auth.authority,kehila.ident,kehila.ident,kehila.operation_token,bytea,kehila_auth.item_preparation,jsonb) FROM PUBLIC;
