-- Historical-only snapshot entries. No serving grants or fresh execution here.
CREATE TYPE kehila_auth.item_resolution AS (
  authority kehila_auth.authority,
  outcome text,
  operation_row_id bigint,
  pending_request_id bigint,
  target_item_id kehila.ident,
  token kehila.operation_token,
  request_codec_version bigint,
  replay_origin_ms kehila.u64,
  replay_deadline_ms kehila.u64,
  pending_expires_ms kehila.u64
);

-- Private inspector: core MUST be obtained by a trusted fully scoped query.
-- RR readers hold their snapshot; future writers hold core SHARE before this
-- call. It acquires no locks/fences and never returns an unverified saved result.
CREATE FUNCTION kehila_auth.inspect_item_replay(
  core kehila.operations, expected_codec bigint, expected_digest bytea, canonical_request bytea
) RETURNS text LANGUAGE plpgsql VOLATILE
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE has_payload boolean; payload record; at_ms kehila.u64;
BEGIN
  IF expected_codec IS NULL OR expected_codec NOT BETWEEN 1 AND 4294967295
     OR expected_digest IS NULL OR octet_length(expected_digest)<>32
     OR (canonical_request IS NOT NULL AND octet_length(canonical_request) NOT BETWEEN 1 AND 1048576) THEN
    RAISE EXCEPTION 'invalid admitted replay comparison' USING ERRCODE='22023';
  END IF;
  IF canonical_request IS NOT NULL AND sha256(canonical_request) IS DISTINCT FROM expected_digest THEN
    RAISE EXCEPTION 'inconsistent admitted replay digest' USING ERRCODE='22023';
  END IF;
  IF core.operation_row_id IS NULL OR core.operation_row_id<=0
     OR core.command_family IS NULL OR core.command_family NOT IN ('item_create','item_edit')
     OR core.target_kind IS DISTINCT FROM 'work_item' OR core.actor_id IS NULL
     OR core.target_project_id IS NULL OR core.target_item_id IS NULL
     OR core.target_user_id IS NOT NULL OR core.target_relationship_id IS NOT NULL OR core.token IS NULL
     OR core.required_grant IS DISTINCT FROM core.command_family OR core.grant_policy_version IS DISTINCT FROM 1
     OR core.request_codec_version IS NULL OR core.request_codec_version NOT BETWEEN 1 AND 4294967295
     OR core.request_sha256 IS NULL OR octet_length(core.request_sha256)<>32
     OR core.replay_origin_ms IS NULL OR core.replay_deadline_ms IS NULL OR core.payload_retired IS NULL
     OR core.replay_deadline_ms<>core.replay_origin_ms+7776000000 THEN
    RAISE EXCEPTION 'invalid permanent item operation'
      USING ERRCODE='23514', CONSTRAINT='item_replay_core';
  END IF;
  SELECT EXISTS(SELECT FROM kehila.operation_payloads p WHERE p.operation_row_id=core.operation_row_id)
    INTO has_payload;
  IF has_payload=core.payload_retired THEN
    RAISE EXCEPTION 'incoherent item replay retention'
      USING ERRCODE='23514', CONSTRAINT='item_replay_retention';
  END IF;
  -- Permanent identity still conflicts after retirement. A retired tombstone
  -- cannot become unseen or revive under a backwards clock.
  IF core.request_codec_version<>expected_codec OR core.request_sha256<>expected_digest THEN RETURN 'conflict'; END IF;
  IF core.payload_retired THEN RETURN 'expired'; END IF;
  IF canonical_request IS NULL THEN
    RAISE EXCEPTION 'live replay lacks exact request comparison'
      USING ERRCODE='23514', CONSTRAINT='item_replay_request';
  END IF;
  at_ms:=kehila_auth.trusted_millis();
  IF at_ms<core.replay_origin_ms THEN
    RAISE EXCEPTION 'trusted clock precedes replay origin' USING ERRCODE='55000';
  END IF;
  IF at_ms>=core.replay_deadline_ms THEN RETURN 'expired'; END IF;
  SELECT octet_length(p.request_bytes) AS request_count,
      CASE WHEN octet_length(p.request_bytes) BETWEEN 1 AND 1048576 THEN p.request_bytes END AS request_bytes,
      octet_length(p.result::text) AS result_count
    INTO payload FROM kehila.operation_payloads p WHERE p.operation_row_id=core.operation_row_id;
  IF NOT FOUND OR payload.request_count IS NULL OR payload.request_count NOT BETWEEN 1 AND 1048576
     OR payload.result_count IS NULL OR payload.result_count NOT BETWEEN 1 AND 1048576
     OR sha256(payload.request_bytes) IS DISTINCT FROM core.request_sha256 THEN
    RAISE EXCEPTION 'invalid bounded item replay payload'
      USING ERRCODE='23514', CONSTRAINT='item_replay_payload';
  END IF;
  IF payload.request_bytes IS DISTINCT FROM canonical_request THEN RETURN 'conflict'; END IF;
  -- Internal availability only. Rust must load/decode the bounded stored
  -- envelopes, verify permanent attribution and enforce final release eligibility.
  RETURN 'replay';
END;
$$;

-- Callers own the earlier authority/project boundary. This helper only checks
-- the private constant mode; it never acquires an earlier lock class.
CREATE FUNCTION kehila_auth.require_item_resolution_mode(writing boolean) RETURNS void
LANGUAGE plpgsql STABLE SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
BEGIN
  IF writing IS NULL OR (writing AND (current_setting('transaction_isolation')<>'read committed'
      OR current_setting('transaction_read_only')<>'off'))
     OR (NOT writing AND (current_setting('transaction_isolation')<>'repeatable read'
      OR current_setting('transaction_read_only')<>'on')) THEN
    RAISE EXCEPTION 'invalid item resolution transaction mode' USING ERRCODE='25000';
  END IF;
END;
$$;

CREATE FUNCTION kehila_auth.resolve_item_edit(
  fresh kehila_auth.authority, requested_project kehila.ident, requested_item kehila.ident,
  request_token kehila.operation_token, request_codec bigint, canonical_request bytea, writing boolean
) RETURNS kehila_auth.item_resolution LANGUAGE plpgsql VOLATILE
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE resolution kehila_auth.item_resolution; core kehila.operations; digest bytea;
BEGIN
  PERFORM kehila_auth.require_item_resolution_mode(writing);
  resolution.authority:=fresh;
  IF requested_item IS NULL OR request_token IS NULL OR canonical_request IS NULL
     OR octet_length(canonical_request) NOT BETWEEN 1 AND 1048576
     OR request_codec IS NULL OR request_codec NOT BETWEEN 1 AND 4294967295 THEN
    RAISE EXCEPTION 'invalid admitted edit reconciliation' USING ERRCODE='22023';
  END IF;
  digest:=sha256(canonical_request);
  resolution.target_item_id:=requested_item; resolution.token:=request_token;
  resolution.request_codec_version:=request_codec;
  IF writing THEN
    SELECT o.* INTO core FROM kehila.operations o
      WHERE o.actor_id=fresh.owner_actor_id AND o.command_family='item_edit'
        AND o.target_kind='work_item' AND o.target_project_id=requested_project AND o.target_item_id=requested_item
        AND o.target_user_id IS NULL AND o.target_relationship_id IS NULL AND o.token=request_token FOR SHARE;
  ELSE
    SELECT o.* INTO core FROM kehila.operations o
      WHERE o.actor_id=fresh.owner_actor_id AND o.command_family='item_edit'
        AND o.target_kind='work_item' AND o.target_project_id=requested_project AND o.target_item_id=requested_item
        AND o.target_user_id IS NULL AND o.target_relationship_id IS NULL AND o.token=request_token;
  END IF;
  IF NOT FOUND THEN resolution.outcome:='unseen'; RETURN resolution; END IF;
  resolution.outcome:=kehila_auth.inspect_item_replay(core,request_codec,digest,canonical_request);
  resolution.operation_row_id:=core.operation_row_id;
  resolution.request_codec_version:=core.request_codec_version;
  resolution.replay_origin_ms:=core.replay_origin_ms; resolution.replay_deadline_ms:=core.replay_deadline_ms;
  RETURN resolution;
END;
$$;

CREATE FUNCTION kehila_auth.resolve_item_create(
  fresh kehila_auth.authority, requested_project kehila.ident, handle kehila_auth.intent_key,
  canonical_echo bytea, writing boolean
) RETURNS kehila_auth.item_resolution LANGUAGE plpgsql VOLATILE
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE resolution kehila_auth.item_resolution; pending record; core kehila.operations; at_ms kehila.u64;
BEGIN
  PERFORM kehila_auth.require_item_resolution_mode(writing);
  resolution.authority:=fresh;
  resolution.outcome:='missing';
  IF writing THEN
    IF canonical_echo IS NULL OR octet_length(canonical_echo) NOT BETWEEN 1 AND 1048576 THEN
      RAISE EXCEPTION 'invalid admitted creation echo' USING ERRCODE='22023';
    END IF;
    PERFORM FROM kehila_auth.pending_items p
      WHERE p.installation_id=fresh.installation_id AND p.actor_id=fresh.owner_actor_id
        AND p.project_id=requested_project AND p.command_family='item_create' AND p.intent_handle=handle FOR UPDATE;
    IF NOT FOUND THEN RETURN resolution; END IF;
  END IF;
  SELECT p.request_id,p.target_item_id,p.token,p.request_codec_version,p.request_sha256,
      p.state,p.expires_ms,p.operation_row_id,octet_length(p.request_bytes) AS byte_count,
      CASE WHEN octet_length(p.request_bytes) BETWEEN 1 AND 1048576 THEN p.request_bytes END AS request_bytes
    INTO pending FROM kehila_auth.pending_items p
    WHERE p.installation_id=(resolution.authority).installation_id AND p.actor_id=(resolution.authority).owner_actor_id
      AND p.project_id=requested_project AND p.command_family='item_create' AND p.intent_handle=handle;
  IF NOT FOUND THEN RETURN resolution; END IF;
  IF pending.state IS NULL OR pending.state NOT IN ('prepared','completed','cancelled','expired')
     OR pending.request_codec_version IS NULL OR pending.request_codec_version NOT BETWEEN 1 AND 4294967295
     OR pending.request_sha256 IS NULL OR octet_length(pending.request_sha256)<>32
     OR (pending.state='prepared' AND pending.byte_count IS NULL)
     OR (pending.byte_count IS NOT NULL AND pending.byte_count NOT BETWEEN 1 AND 1048576)
     OR (pending.state IN ('cancelled','expired') AND pending.byte_count IS NOT NULL)
     OR ((pending.state='completed') IS DISTINCT FROM (pending.operation_row_id IS NOT NULL))
     OR (pending.request_bytes IS NOT NULL AND sha256(pending.request_bytes) IS DISTINCT FROM pending.request_sha256) THEN
    RAISE EXCEPTION 'invalid historical pending binding'
      USING ERRCODE='23514', CONSTRAINT='item_replay_pending';
  END IF;
  resolution.pending_request_id:=pending.request_id; resolution.target_item_id:=pending.target_item_id;
  resolution.token:=pending.token; resolution.request_codec_version:=pending.request_codec_version;
  resolution.pending_expires_ms:=pending.expires_ms;
  IF writing THEN
    SELECT o.* INTO core FROM kehila.operations o
      WHERE o.actor_id=fresh.owner_actor_id AND o.command_family='item_create'
        AND o.target_kind='work_item' AND o.target_project_id=requested_project AND o.target_item_id=pending.target_item_id
        AND o.target_user_id IS NULL AND o.target_relationship_id IS NULL AND o.token=pending.token FOR SHARE;
  ELSE
    SELECT o.* INTO core FROM kehila.operations o
      WHERE o.actor_id=fresh.owner_actor_id AND o.command_family='item_create'
        AND o.target_kind='work_item' AND o.target_project_id=requested_project AND o.target_item_id=pending.target_item_id
        AND o.target_user_id IS NULL AND o.target_relationship_id IS NULL AND o.token=pending.token;
  END IF;
  IF NOT FOUND THEN
    IF pending.state='completed' THEN
      RAISE EXCEPTION 'completed item command lacks permanent operation'
        USING ERRCODE='23514', CONSTRAINT='item_replay_pending';
    END IF;
    IF writing AND (pending.request_codec_version<>1 OR sha256(canonical_echo)<>pending.request_sha256
        OR (pending.request_bytes IS NOT NULL AND pending.request_bytes IS DISTINCT FROM canonical_echo)) THEN
      resolution.outcome:='conflict'; RETURN resolution;
    END IF;
    at_ms:=kehila_auth.trusted_millis();
    resolution.outcome:=CASE WHEN pending.state='prepared' AND pending.expires_ms<=at_ms THEN 'expired' ELSE pending.state END;
    RETURN resolution;
  END IF;
  IF pending.state<>'completed' OR pending.operation_row_id IS DISTINCT FROM core.operation_row_id
     OR pending.request_codec_version IS DISTINCT FROM core.request_codec_version
     OR pending.request_sha256 IS DISTINCT FROM core.request_sha256
     OR NOT EXISTS(SELECT FROM kehila.items i JOIN kehila.item_revisions r
       ON r.project_id=i.project_id AND r.item_id=i.item_id AND r.item_version=1
       WHERE i.project_id=requested_project AND i.item_id=pending.target_item_id
         AND i.creation_request_id=pending.request_id AND i.creation_operation_row_id=core.operation_row_id
         AND r.operation_row_id=core.operation_row_id AND r.actor_id=(resolution.authority).owner_actor_id
         AND r.command_family='item_create' AND r.token=pending.token)
     OR (NOT core.payload_retired AND pending.request_bytes IS NULL) THEN
    RAISE EXCEPTION 'incoherent successful item command binding'
      USING ERRCODE='23514', CONSTRAINT='item_replay_pending';
  END IF;
  IF writing THEN
    resolution.outcome:=kehila_auth.inspect_item_replay(core,1,sha256(canonical_echo),canonical_echo);
  ELSE
    resolution.outcome:=kehila_auth.inspect_item_replay(core,pending.request_codec_version,
      pending.request_sha256,pending.request_bytes);
    IF resolution.outcome='conflict' THEN
      RAISE EXCEPTION 'stored creation request disagrees with success'
        USING ERRCODE='23514', CONSTRAINT='item_replay_pending';
    END IF;
  END IF;
  resolution.operation_row_id:=core.operation_row_id;
  resolution.replay_origin_ms:=core.replay_origin_ms; resolution.replay_deadline_ms:=core.replay_deadline_ms;
  RETURN resolution;
END;
$$;

CREATE FUNCTION kehila_auth.reconcile_item_edit(
  expected kehila_auth.authority, requested_project kehila.ident, requested_item kehila.ident,
  request_token kehila.operation_token, request_codec bigint, canonical_request bytea
) RETURNS kehila_auth.item_resolution LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE fresh kehila_auth.authority;
BEGIN
  fresh:=kehila_auth.item_read_authority(expected,requested_project);
  RETURN kehila_auth.resolve_item_edit(fresh,requested_project,requested_item,request_token,request_codec,canonical_request,false);
END;
$$;
CREATE FUNCTION kehila_auth.reconcile_item_create(
  expected kehila_auth.authority, requested_project kehila.ident, handle kehila_auth.intent_key
) RETURNS kehila_auth.item_resolution LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE fresh kehila_auth.authority;
BEGIN
  fresh:=kehila_auth.item_read_authority(expected,requested_project);
  RETURN kehila_auth.resolve_item_create(fresh,requested_project,handle,NULL,false);
END;
$$;

REVOKE ALL ON FUNCTION kehila_auth.inspect_item_replay(kehila.operations,bigint,bytea,bytea),
  kehila_auth.require_item_resolution_mode(boolean),
  kehila_auth.resolve_item_edit(kehila_auth.authority,kehila.ident,kehila.ident,kehila.operation_token,bigint,bytea,boolean),
  kehila_auth.resolve_item_create(kehila_auth.authority,kehila.ident,kehila_auth.intent_key,bytea,boolean),
  kehila_auth.reconcile_item_edit(kehila_auth.authority,kehila.ident,kehila.ident,kehila.operation_token,bigint,bytea),
  kehila_auth.reconcile_item_create(kehila_auth.authority,kehila.ident,kehila_auth.intent_key) FROM PUBLIC;
