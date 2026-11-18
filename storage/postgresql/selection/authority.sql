-- Private helpers are callable only by the closed owner-defined capabilities.
CREATE FUNCTION kehila_auth.selection_fence(expected kehila_auth.authority)
RETURNS kehila_auth.authority LANGUAGE plpgsql VOLATILE
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE fresh kehila_auth.authority;
BEGIN
  IF expected.installation_id IS NULL OR expected.owner_actor_id IS NULL
     OR expected.runtime_generation IS NULL OR expected.auth_generation IS NULL
     OR expected.verifier_revision IS NULL THEN
    RAISE EXCEPTION 'complete selection authority required' USING ERRCODE='55000';
  END IF;
  fresh:=kehila_auth.fence(expected.installation_id,expected.runtime_generation,
    expected.auth_generation,expected.verifier_revision);
  IF fresh.owner_actor_id IS DISTINCT FROM expected.owner_actor_id THEN
    RAISE EXCEPTION 'selection subject is not the installed owner' USING ERRCODE='42501';
  END IF;
  RETURN fresh;
END;
$$;

CREATE FUNCTION kehila_auth.selection_project_grant(subject kehila.ident,project kehila.ident)
RETURNS void LANGUAGE plpgsql VOLATILE
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
BEGIN
  IF project IS NULL THEN RETURN; END IF;
  PERFORM FROM kehila.projects p WHERE p.project_id=project FOR NO KEY UPDATE;
  IF NOT FOUND THEN RAISE EXCEPTION 'selection target authority denied' USING ERRCODE='42501'; END IF;
  -- A separate statement observes the grant AFTER the potential project wait.
  PERFORM kehila_auth.require_item_project_grant(subject,project);
END;
$$;

-- Assumes the immediately enclosing closed capability owns installation/actor
-- fences. This private helper acquires project then core, never item/head locks.
CREATE FUNCTION kehila_auth.resolve_selection(fresh kehila_auth.authority,
  requested_token kehila.operation_token,request_version bigint,canonical_request bytea)
RETURNS TABLE(outcome text,operation_row_id bigint,current_version kehila.u64)
LANGUAGE plpgsql VOLATILE SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE core kehila.operations; event kehila.selection_events; stored bytea; at_ms kehila.u64;
BEGIN
  IF requested_token IS NULL OR requested_token !~ '^[0-9a-f]{32}$'
     OR request_version IS DISTINCT FROM 1::bigint OR canonical_request IS NULL
     OR octet_length(canonical_request) NOT BETWEEN 1 AND 4096 THEN
    RAISE EXCEPTION 'invalid selection request representation' USING ERRCODE='22023';
  END IF;
  SELECT o.* INTO core FROM kehila.operations o
    WHERE o.actor_id=fresh.owner_actor_id AND o.command_family='selection' AND o.target_kind='user'
      AND o.target_user_id=fresh.owner_actor_id AND o.target_project_id IS NULL
      AND o.target_item_id IS NULL AND o.target_relationship_id IS NULL AND o.token=requested_token;
  IF NOT FOUND THEN RETURN QUERY SELECT 'absent'::text,NULL::bigint,NULL::kehila.u64; RETURN; END IF;
  SELECT e.* INTO STRICT event FROM kehila.selection_events e WHERE e.operation_row_id=core.operation_row_id;
  IF event.user_id IS DISTINCT FROM fresh.owner_actor_id OR event.actor_id IS DISTINCT FROM fresh.owner_actor_id
     OR event.token IS DISTINCT FROM requested_token THEN
    RAISE EXCEPTION 'invalid original selection attribution'
      USING ERRCODE='23514',CONSTRAINT='selection_replay_attribution';
  END IF;
  -- The retained event, NEVER the retry payload or current selection, owns
  -- original authorization scope, including after payload retirement.
  PERFORM kehila_auth.selection_project_grant(fresh.owner_actor_id,event.selected_project_id);
  PERFORM FROM kehila.operations o WHERE o.operation_row_id=core.operation_row_id FOR SHARE;
  SELECT o.* INTO STRICT core FROM kehila.operations o WHERE o.operation_row_id=core.operation_row_id;
  IF core.request_codec_version<>request_version OR core.request_sha256 IS DISTINCT FROM sha256(canonical_request)
     OR canonical_request IS DISTINCT FROM kehila.selection_request_bytes(event.user_id,event.token,
       event.before_version,event.selected_project_id,event.selected_item_id) THEN
    RETURN QUERY SELECT 'operation_id_reused'::text,core.operation_row_id,event.selection_version; RETURN;
  END IF;
  IF core.payload_retired THEN
    RETURN QUERY SELECT 'replay_expired'::text,core.operation_row_id,event.selection_version; RETURN;
  END IF;
  SELECT CASE WHEN octet_length(p.request_bytes) BETWEEN 1 AND 4096 THEN p.request_bytes END
    INTO STRICT stored FROM kehila.operation_payloads p WHERE p.operation_row_id=core.operation_row_id;
  IF stored IS NULL THEN
    RAISE EXCEPTION 'invalid retained selection request' USING ERRCODE='23514',CONSTRAINT='selection_replay_payload';
  END IF;
  IF stored IS DISTINCT FROM canonical_request THEN
    RETURN QUERY SELECT 'operation_id_reused'::text,core.operation_row_id,event.selection_version; RETURN;
  END IF;
  -- Payload lookup can wait. Sample expiry only after the last payload access
  -- and exact identity comparison, immediately before releasing replay.
  at_ms:=kehila_auth.trusted_millis();
  IF at_ms<core.replay_origin_ms THEN
    RAISE EXCEPTION 'trusted clock precedes replay origin' USING ERRCODE='55000';
  END IF;
  IF at_ms>=core.replay_deadline_ms THEN
    RETURN QUERY SELECT 'replay_expired'::text,core.operation_row_id,event.selection_version; RETURN;
  END IF;
  RETURN QUERY SELECT 'replay'::text,core.operation_row_id,event.selection_version;
EXCEPTION WHEN NO_DATA_FOUND OR TOO_MANY_ROWS THEN
  RAISE EXCEPTION 'incoherent recorded selection'
    USING ERRCODE='23514',CONSTRAINT='selection_replay_attribution';
END;
$$;

CREATE FUNCTION kehila_auth.reconcile_selection(expected kehila_auth.authority,
  requested_token kehila.operation_token,request_version bigint,canonical_request bytea)
RETURNS TABLE(outcome text,operation_row_id bigint,current_version kehila.u64)
LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE fresh kehila_auth.authority;
BEGIN
  fresh:=kehila_auth.selection_fence(expected);
  RETURN QUERY SELECT * FROM kehila_auth.resolve_selection(fresh,requested_token,request_version,canonical_request);
END;
$$;

REVOKE ALL ON FUNCTION kehila_auth.selection_fence(kehila_auth.authority),
  kehila_auth.selection_project_grant(kehila.ident,kehila.ident),
  kehila_auth.resolve_selection(kehila_auth.authority,kehila.operation_token,bigint,bytea),
  kehila_auth.reconcile_selection(kehila_auth.authority,kehila.operation_token,bigint,bytea) FROM PUBLIC;
