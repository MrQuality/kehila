-- Fixed creation-intent entries. Installed with fresh v3; execution stays
-- owner-only until the corresponding runtime profile/application path is ready.
CREATE TABLE kehila_auth.item_pending_policy (
  singleton boolean PRIMARY KEY CHECK(singleton),
  prepared_limit integer NOT NULL CHECK(prepared_limit BETWEEN 1 AND 32),
  metadata_limit integer NOT NULL CHECK(metadata_limit BETWEEN 1 AND 64),
  page_limit integer NOT NULL CHECK(page_limit BETWEEN 1 AND 16),
  lifetime_seconds integer NOT NULL CHECK(lifetime_seconds BETWEEN 60 AND 86400)
);
INSERT INTO kehila_auth.item_pending_policy VALUES(true,32,64,16,86400);

-- These predicates are part of containment: never scan permanent success
-- history or another project's terminal rows just to count a bounded set.
CREATE INDEX pending_item_prepared_owner ON kehila_auth.pending_items
  (installation_id,actor_id,request_id) WHERE state='prepared';
CREATE INDEX pending_item_project_never_created ON kehila_auth.pending_items
  (installation_id,actor_id,project_id,request_id) WHERE state<>'completed';

CREATE TYPE kehila_auth.pending_item_summary AS (
  request_id bigint,
  intent_handle kehila_auth.intent_key,
  token kehila.operation_token,
  target_item_id kehila.ident,
  request_codec_version bigint,
  request_sha256 bytea,
  registered_ms kehila.u64,
  expires_ms kehila.u64,
  state text,
  operation_row_id bigint
);

CREATE FUNCTION kehila_auth.register_item(
  expected kehila_auth.authority, requested_project kehila.ident,
  handle kehila_auth.intent_key, request_token kehila_auth.intent_key,
  target_item kehila.ident, codec bigint, canonical_request bytea
) RETURNS SETOF kehila_auth.pending_item_summary
LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE
  fresh kehila_auth.authority; policy kehila_auth.item_pending_policy;
  selected_ids bigint[]; selected_id bigint; pending_state text;
  pending_expiry kehila.u64; at_ms kehila.u64;
BEGIN
  -- Trusted backend identity is generated before canonical encoding, under the
  -- original request deadline. This entry never deduplicates content or retries.
  IF codec IS DISTINCT FROM 1 OR canonical_request IS NULL
     OR octet_length(canonical_request) NOT BETWEEN 1 AND 1048576
     OR handle IS NULL OR request_token IS NULL OR target_item IS NULL THEN
    RAISE EXCEPTION 'invalid admitted item registration' USING ERRCODE='22023';
  END IF;
  fresh := kehila_auth.item_write_fence(expected,requested_project);
  SELECT p.* INTO STRICT policy FROM kehila_auth.item_pending_policy p WHERE p.singleton;
  SELECT array_agg(b.request_id ORDER BY b.request_id) INTO selected_ids FROM
    (SELECT p.request_id FROM kehila_auth.pending_items p
      WHERE p.installation_id=fresh.installation_id AND p.actor_id=fresh.owner_actor_id
        AND p.project_id=requested_project AND p.state<>'completed'
      ORDER BY p.request_id LIMIT policy.metadata_limit+1) b;
  IF coalesce(cardinality(selected_ids),0)>policy.metadata_limit THEN
    RAISE EXCEPTION 'pending metadata exceeds containment'
      USING ERRCODE='23514', CONSTRAINT='item_pending_capacity';
  END IF;
  -- Only this protected project. Select identity scalars, not large bytea.
  -- Clock sampling follows each potential pending-row wait.
  FOREACH selected_id IN ARRAY coalesce(selected_ids,ARRAY[]::bigint[]) LOOP
    SELECT p.state,p.expires_ms INTO STRICT pending_state,pending_expiry
      FROM kehila_auth.pending_items p WHERE p.request_id=selected_id FOR UPDATE;
    at_ms := kehila_auth.trusted_millis();
    IF pending_state='prepared' AND pending_expiry<=at_ms THEN
      UPDATE kehila_auth.pending_items SET state='expired',request_bytes=NULL
        WHERE request_id=selected_id;
      pending_state := 'expired';
    END IF;
    IF pending_state IN ('cancelled','expired') THEN
      DELETE FROM kehila_auth.pending_items WHERE request_id=selected_id;
    END IF;
  END LOOP;
  -- Owner fencing serializes this decision across projects. Elapsed rows in
  -- other projects still hold bytes and count until explicitly cleared there.
  IF (SELECT count(*) FROM (SELECT 1 FROM kehila_auth.pending_items p
      WHERE p.installation_id=fresh.installation_id AND p.actor_id=fresh.owner_actor_id
        AND p.state='prepared' LIMIT policy.prepared_limit) b)>=policy.prepared_limit THEN
    RAISE EXCEPTION 'prepared item capacity reached; clear pending work in its project'
      USING ERRCODE='54000';
  END IF;
  IF (SELECT count(*) FROM (SELECT 1 FROM kehila_auth.pending_items p
      WHERE p.installation_id=fresh.installation_id AND p.actor_id=fresh.owner_actor_id
        AND p.project_id=requested_project AND p.state<>'completed'
      LIMIT policy.metadata_limit) b)>=policy.metadata_limit THEN
    RAISE EXCEPTION 'pending item metadata capacity reached' USING ERRCODE='54000';
  END IF;
  at_ms := kehila_auth.trusted_millis();
  RETURN QUERY INSERT INTO kehila_auth.pending_items AS p
    (installation_id,actor_id,project_id,intent_handle,token,target_item_id,
     request_codec_version,request_sha256,request_bytes,registered_ms,expires_ms)
    VALUES(fresh.installation_id,fresh.owner_actor_id,requested_project,handle,request_token,target_item,
      codec,sha256(canonical_request),canonical_request,at_ms,
      (at_ms+policy.lifetime_seconds::numeric*1000)::kehila.u64)
    RETURNING p.request_id,p.intent_handle,p.token,p.target_item_id,p.request_codec_version,
      p.request_sha256,p.registered_ms,p.expires_ms,p.state,p.operation_row_id;
EXCEPTION WHEN NO_DATA_FOUND OR TOO_MANY_ROWS THEN
  RAISE EXCEPTION 'pending registration reference unavailable'
    USING ERRCODE='23514', CONSTRAINT='item_pending_reference';
END;
$$;

CREATE FUNCTION kehila_auth.cancel_item(
  expected kehila_auth.authority, requested_project kehila.ident,
  handle kehila_auth.intent_key
) RETURNS SETOF kehila_auth.pending_item_summary
LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE fresh kehila_auth.authority; selected_id bigint; pending_state text;
BEGIN
  fresh := kehila_auth.item_write_fence(expected,requested_project);
  SELECT p.request_id,p.state INTO selected_id,pending_state
    FROM kehila_auth.pending_items p
    WHERE p.installation_id=fresh.installation_id AND p.actor_id=fresh.owner_actor_id
      AND p.project_id=requested_project AND p.command_family='item_create'
      AND p.intent_handle=handle FOR UPDATE;
  IF NOT FOUND THEN RETURN; END IF;
  IF pending_state='prepared' THEN
    -- The transition guard owns the final clock sample and may classify this
    -- cancellation intent as expired. Return its persisted classification.
    UPDATE kehila_auth.pending_items SET state='cancelled',
      request_bytes=NULL WHERE request_id=selected_id;
  END IF;
  -- No full request materialization, compensation or recreation on absence.
  RETURN QUERY SELECT p.request_id,p.intent_handle,p.token,p.target_item_id,p.request_codec_version,
      p.request_sha256,p.registered_ms,p.expires_ms,p.state,p.operation_row_id
    FROM kehila_auth.pending_items p WHERE p.request_id=selected_id;
END;
$$;

REVOKE ALL ON FUNCTION kehila_auth.register_item(kehila_auth.authority,kehila.ident,
  kehila_auth.intent_key,kehila_auth.intent_key,kehila.ident,bigint,bytea),
  kehila_auth.cancel_item(kehila_auth.authority,kehila.ident,kehila_auth.intent_key) FROM PUBLIC;
