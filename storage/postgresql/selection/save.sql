CREATE FUNCTION kehila_auth.save_selection(expected kehila_auth.authority,
  requested_token kehila.operation_token,expected_version kehila.u64,
  canonical_request bytea,accepted_event jsonb)
RETURNS TABLE(outcome text,operation_row_id bigint,current_version kehila.u64)
LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE fresh kehila_auth.authority; resolved record; head kehila.current_work_selections;
  requested_project kehila.ident; requested_item kehila.ident; item_archived boolean; project_archived boolean;
  requested jsonb; ordinal kehila.u64; next_version kehila.u64; new_operation bigint;
  origin kehila.u64; saved jsonb; after_state jsonb; before_state jsonb;
BEGIN
  fresh:=kehila_auth.selection_fence(expected);
  SELECT * INTO STRICT resolved FROM kehila_auth.resolve_selection(fresh,requested_token,1,canonical_request);
  IF resolved.outcome<>'absent' THEN
    RETURN QUERY SELECT resolved.outcome,resolved.operation_row_id,resolved.current_version; RETURN;
  END IF;
  -- Historical resolution above must not depend on prospective state/envelope
  -- acceptance, including a command that became invalid after its success.
  IF expected_version IS NULL OR accepted_event IS NULL OR jsonb_typeof(accepted_event) IS DISTINCT FROM 'object'
     OR octet_length(accepted_event::text)>16384 OR accepted_event->>'kind' IS DISTINCT FROM 'selection_event'
     OR accepted_event->'codec_version' IS DISTINCT FROM '1'::jsonb THEN
    RAISE EXCEPTION 'invalid admitted selection event' USING ERRCODE='22023';
  END IF;
  requested:=accepted_event #>'{value,result,value,requested}';
  IF requested IS NULL OR jsonb_typeof(requested) NOT IN ('object','null') THEN
    RAISE EXCEPTION 'invalid selection target representation' USING ERRCODE='22023';
  END IF;
  requested_project:=requested->>'project_id'; requested_item:=requested->>'item_id';
  IF (requested_project IS NULL) IS DISTINCT FROM (requested_item IS NULL) THEN
    RAISE EXCEPTION 'incomplete selection target pair' USING ERRCODE='22023';
  END IF;
  PERFORM kehila_auth.selection_project_grant(fresh.owner_actor_id,requested_project);
  IF requested_project IS NOT NULL THEN
    PERFORM FROM kehila.items i WHERE i.project_id=requested_project AND i.item_id=requested_item FOR NO KEY UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION 'selection target authority denied' USING ERRCODE='42501'; END IF;
    SELECT p.archived,r.archived INTO STRICT project_archived,item_archived
      FROM kehila.projects p JOIN kehila.items i ON i.project_id=p.project_id
      JOIN kehila.item_revisions r ON r.project_id=i.project_id AND r.item_id=i.item_id AND r.item_version=i.current_version
      WHERE p.project_id=requested_project AND i.item_id=requested_item;
  END IF;
  SELECT h.* INTO head FROM kehila.current_work_selections h
    WHERE h.user_id=fresh.owner_actor_id FOR UPDATE;
  IF NOT FOUND THEN
    IF EXISTS(SELECT FROM kehila.selection_events e WHERE e.user_id=fresh.owner_actor_id) THEN
      RAISE EXCEPTION 'selection head missing' USING ERRCODE='23514',CONSTRAINT='selection_current_boundary';
    END IF;
    head.user_id:=fresh.owner_actor_id; head.selection_version:=0; head.event_ordinal:=0;
  END IF;
  IF head.selection_version<>expected_version THEN
    RETURN QUERY SELECT 'version_conflict'::text,NULL::bigint,head.selection_version; RETURN;
  END IF;
  IF project_archived IS TRUE OR item_archived IS TRUE THEN
    RETURN QUERY SELECT 'archived_target'::text,NULL::bigint,head.selection_version; RETURN;
  END IF;
  ordinal:=(accepted_event #>>'{value,ordinal}')::kehila.u64;
  IF ordinal IS NULL OR ordinal<>head.event_ordinal+1 THEN
    RETURN QUERY SELECT 'preparation_stale'::text,NULL::bigint,head.selection_version; RETURN;
  END IF;
  before_state:=kehila.selection_state_value(fresh.owner_actor_id,head.selection_version,
    head.selected_project_id,head.selected_item_id);
  IF accepted_event #>'{value,before}' IS DISTINCT FROM before_state THEN
    RAISE EXCEPTION 'selection prepared before-state disagrees'
      USING ERRCODE='23514',CONSTRAINT='selection_save_preparation';
  END IF;
  next_version:=head.selection_version+CASE WHEN (requested_project,requested_item)
    IS DISTINCT FROM (head.selected_project_id,head.selected_item_id) THEN 1 ELSE 0 END;
  after_state:=kehila.selection_state_value(fresh.owner_actor_id,next_version,requested_project,requested_item);
  saved:=jsonb_build_object('codec_version',1,'kind','selection_result','value',after_state);
  IF accepted_event IS DISTINCT FROM jsonb_build_object('codec_version',1,'kind','selection_event','value',
      jsonb_build_object('ordinal',ordinal::numeric(20,0)::text,'operation_id',requested_token,
        'before',before_state,'result',saved))
     OR canonical_request IS DISTINCT FROM kehila.selection_request_bytes(fresh.owner_actor_id,requested_token,
        expected_version,requested_project,requested_item) THEN
    RAISE EXCEPTION 'selection admitted request and result disagree'
      USING ERRCODE='23514',CONSTRAINT='selection_save_preparation';
  END IF;
  origin:=kehila_auth.trusted_millis();
  INSERT INTO kehila.operations(actor_id,command_family,target_kind,target_user_id,token,
    request_codec_version,request_sha256,required_grant,grant_policy_version,replay_origin_ms,replay_deadline_ms)
    VALUES(fresh.owner_actor_id,'selection','user',fresh.owner_actor_id,requested_token,1,
      sha256(canonical_request),'selection',1,origin,origin+7776000000)
    RETURNING kehila.operations.operation_row_id INTO new_operation;
  INSERT INTO kehila.operation_payloads(operation_row_id,request_bytes,result_codec_version,result)
    VALUES(new_operation,canonical_request,1,saved);
  INSERT INTO kehila.selection_events(user_id,event_ordinal,previous_event_ordinal,actor_id,command_family,
    target_kind,token,operation_row_id,before_version,before_project_id,before_item_id,selection_version,
    selected_project_id,selected_item_id,history_codec_version,history)
    VALUES(fresh.owner_actor_id,ordinal,CASE WHEN head.event_ordinal=0 THEN NULL ELSE head.event_ordinal END,
      fresh.owner_actor_id,'selection','user',requested_token,new_operation,head.selection_version,
      head.selected_project_id,head.selected_item_id,next_version,requested_project,requested_item,1,accepted_event);
  IF head.event_ordinal=0 THEN
    INSERT INTO kehila.current_work_selections(user_id,selection_version,selected_project_id,selected_item_id,event_ordinal)
      VALUES(fresh.owner_actor_id,next_version,requested_project,requested_item,ordinal);
  ELSE
    UPDATE kehila.current_work_selections SET selection_version=next_version,
      selected_project_id=requested_project,selected_item_id=requested_item,event_ordinal=ordinal
      WHERE user_id=fresh.owner_actor_id;
  END IF;
  RETURN QUERY SELECT 'accepted'::text,new_operation,next_version;
END;
$$;
REVOKE ALL ON FUNCTION kehila_auth.save_selection(kehila_auth.authority,kehila.operation_token,kehila.u64,bytea,jsonb)
  FROM PUBLIC;
