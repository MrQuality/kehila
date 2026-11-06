-- Final historical-result authorization, after complete reply preparation.
-- Nonmutating protected READ COMMITTED: row locks make SQL read-only unsuitable.
-- This entry does not load payload bodies, run domain rules, or execute work.
CREATE FUNCTION kehila_auth.authorize_item_replay(
  expected kehila_auth.authority, requested_project kehila.ident, family text,
  requested_item kehila.ident, request_token kehila.operation_token,
  requested_operation bigint, request_codec bigint, request_digest bytea,
  original_origin kehila.u64, original_deadline kehila.u64,
  pending_id bigint, pending_expiry kehila.u64, handle kehila_auth.intent_key
) RETURNS kehila_auth.item_resolution LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE fresh kehila_auth.authority; core kehila.operations;
  resolution kehila_auth.item_resolution; at_ms kehila.u64;
BEGIN
  PERFORM kehila_auth.require_item_resolution_mode(true);
  IF family IS NULL OR family NOT IN ('item_create','item_edit')
     OR requested_operation IS NULL OR requested_operation<=0 OR requested_item IS NULL
     OR request_token IS NULL OR request_codec IS NULL OR request_codec NOT BETWEEN 1 AND 4294967295
     OR request_digest IS NULL OR octet_length(request_digest)<>32
     OR original_origin IS NULL OR original_deadline IS NULL
     OR original_deadline<>original_origin+7776000000
     OR (family='item_create' AND (pending_id IS NULL OR pending_id<=0 OR pending_expiry IS NULL OR handle IS NULL))
     OR (family='item_edit' AND (pending_id IS NOT NULL OR pending_expiry IS NOT NULL OR handle IS NOT NULL)) THEN
    RAISE EXCEPTION 'invalid prepared item replay identity' USING ERRCODE='22023';
  END IF;
  -- Fixed lock order: installation SHARE, actor NO KEY UPDATE, project,
  -- operation SHARE. Retirement takes installation/actor before operation UPDATE.
  fresh:=kehila_auth.item_write_fence(expected,requested_project);
  SELECT o.* INTO core FROM kehila.operations o
    WHERE o.operation_row_id=requested_operation AND o.actor_id=fresh.owner_actor_id
      AND o.command_family=family AND o.target_kind='work_item'
      AND o.target_project_id=requested_project AND o.target_item_id=requested_item
      AND o.target_user_id IS NULL AND o.target_relationship_id IS NULL AND o.token=request_token
    FOR SHARE;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'prepared item operation lost permanent identity'
      USING ERRCODE='23514', CONSTRAINT='item_release_identity';
  END IF;
  IF core.request_codec_version IS DISTINCT FROM request_codec
     OR core.request_sha256 IS DISTINCT FROM request_digest
     OR core.required_grant IS DISTINCT FROM family OR core.grant_policy_version IS DISTINCT FROM 1
     OR core.replay_origin_ms IS DISTINCT FROM original_origin
     OR core.replay_deadline_ms IS DISTINCT FROM original_deadline
     OR core.payload_retired IS NULL THEN
    RAISE EXCEPTION 'prepared item operation changed immutable identity'
      USING ERRCODE='23514', CONSTRAINT='item_release_identity';
  END IF;
  IF family='item_create' AND NOT EXISTS(SELECT FROM kehila_auth.pending_items p
      WHERE p.request_id=pending_id AND p.installation_id=fresh.installation_id
        AND p.actor_id=fresh.owner_actor_id AND p.project_id=requested_project
        AND p.command_family=family AND p.target_kind='work_item' AND p.intent_handle=handle
        AND p.token=request_token AND p.target_item_id=requested_item
        AND p.request_codec_version=request_codec AND p.request_sha256=request_digest
        AND p.expires_ms=pending_expiry AND p.state='completed' AND p.operation_row_id=requested_operation) THEN
    RAISE EXCEPTION 'prepared item creation lost immutable pending binding'
      USING ERRCODE='23514', CONSTRAINT='item_release_pending';
  END IF;
  IF EXISTS(SELECT FROM kehila.operation_payloads p WHERE p.operation_row_id=requested_operation)=core.payload_retired THEN
    RAISE EXCEPTION 'incoherent item replay retention'
      USING ERRCODE='23514', CONSTRAINT='item_release_retention';
  END IF;
  -- A new statement after every possible wait. The current initial_owner
  -- predicate represents the original Read/CreateItem/EditItem capability set.
  PERFORM kehila_auth.require_item_project_grant(fresh.owner_actor_id,requested_project);
  resolution.authority:=fresh; resolution.operation_row_id:=requested_operation;
  resolution.pending_request_id:=pending_id; resolution.target_item_id:=requested_item;
  resolution.token:=request_token; resolution.request_codec_version:=request_codec;
  resolution.replay_origin_ms:=original_origin; resolution.replay_deadline_ms:=original_deadline;
  resolution.pending_expires_ms:=pending_expiry;
  -- Last database observation, after all identity/grant/retention checks. The
  -- operation lock remains held through confirmed transaction completion.
  at_ms:=kehila_auth.trusted_millis();
  resolution.outcome:=CASE WHEN core.payload_retired THEN 'expired'
    WHEN at_ms<original_origin THEN 'before_origin'
    WHEN at_ms>=original_deadline THEN 'expired' ELSE 'authorized' END;
  RETURN resolution;
END;
$$;
REVOKE ALL ON FUNCTION kehila_auth.authorize_item_replay(
  kehila_auth.authority,kehila.ident,text,kehila.ident,kehila.operation_token,
  bigint,bigint,bytea,kehila.u64,kehila.u64,bigint,kehila.u64,kehila_auth.intent_key) FROM PUBLIC;
