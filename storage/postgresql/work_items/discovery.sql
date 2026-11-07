-- Nonmutating snapshot reads. No installer/runtime grants until v3 adoption.
CREATE TYPE kehila_auth.opened_pending_item AS (
  request_id bigint,
  intent_handle kehila_auth.intent_key,
  token kehila.operation_token,
  target_item_id kehila.ident,
  request_codec_version bigint,
  request_sha256 bytea,
  registered_ms kehila.u64,
  expires_ms kehila.u64,
  state text,
  operation_row_id bigint,
  request_bytes bytea
);

CREATE FUNCTION kehila_auth.list_pending_items(
  expected kehila_auth.authority, requested_project kehila.ident, before_id bigint
) RETURNS SETOF kehila_auth.pending_item_summary
LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE fresh kehila_auth.authority; selected_limit integer; at_ms kehila.u64; upper_id bigint;
BEGIN
  fresh := kehila_auth.item_read_authority(expected,requested_project);
  IF before_id IS NOT NULL AND before_id<=0 THEN
    RAISE EXCEPTION 'invalid pending cursor' USING ERRCODE='22023';
  END IF;
  -- Positive bigint identity permits an inclusive bound without nullable OR.
  -- Subtraction is safe after validation; the first page includes bigint max.
  upper_id := CASE WHEN before_id IS NULL THEN 9223372036854775807 ELSE before_id-1 END;
  SELECT p.page_limit INTO STRICT selected_limit FROM kehila_auth.item_pending_policy p WHERE p.singleton;
  -- VOLATILE reflects this single per-call clock sample, not write behavior.
  -- Every table read is in the caller's required read-only RR snapshot.
  at_ms := kehila_auth.trusted_millis();
  RETURN QUERY SELECT p.request_id,p.intent_handle,p.token,p.target_item_id,p.request_codec_version,
      p.request_sha256,p.registered_ms,p.expires_ms,
      CASE WHEN p.state='prepared' AND p.expires_ms<=at_ms THEN 'expired' ELSE p.state END,
      p.operation_row_id
    FROM kehila_auth.pending_items p
    WHERE p.installation_id=fresh.installation_id AND p.actor_id=fresh.owner_actor_id
      AND p.project_id=requested_project AND p.command_family='item_create'
      AND p.request_id<=upper_id
    ORDER BY p.request_id DESC LIMIT selected_limit+1;
EXCEPTION WHEN NO_DATA_FOUND OR TOO_MANY_ROWS THEN
  RAISE EXCEPTION 'pending discovery policy unavailable'
    USING ERRCODE='23514', CONSTRAINT='item_pending_reference';
END;
$$;

CREATE FUNCTION kehila_auth.open_pending_item(
  expected kehila_auth.authority, requested_project kehila.ident, handle kehila_auth.intent_key
) RETURNS SETOF kehila_auth.opened_pending_item
LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE fresh kehila_auth.authority; opened record; at_ms kehila.u64;
BEGIN
  fresh := kehila_auth.item_read_authority(expected,requested_project);
  at_ms := kehila_auth.trusted_millis();
  SELECT p.request_id,p.intent_handle,p.token,p.target_item_id,p.request_codec_version,
      p.request_sha256,p.registered_ms,p.expires_ms,p.state,p.operation_row_id,
      octet_length(p.request_bytes) AS byte_count,
      CASE WHEN octet_length(p.request_bytes) BETWEEN 1 AND 1048576
        AND (p.state='completed' OR (p.state='prepared' AND p.expires_ms>at_ms))
        THEN p.request_bytes END AS request_bytes
    INTO opened FROM kehila_auth.pending_items p
    WHERE p.installation_id=fresh.installation_id AND p.actor_id=fresh.owner_actor_id
      AND p.project_id=requested_project AND p.command_family='item_create'
      AND p.intent_handle=handle;
  IF NOT FOUND THEN RETURN; END IF;
  -- Sentinels are read before bounded bytes are returned. Effective expiry does
  -- not conceal missing mandatory physical prepared content or invalid length.
  IF opened.state IS NULL OR opened.state NOT IN ('prepared','completed','cancelled','expired')
     OR (opened.state='prepared' AND opened.byte_count IS NULL)
     OR (opened.byte_count IS NOT NULL AND opened.byte_count NOT BETWEEN 1 AND 1048576)
     OR (opened.state IN ('cancelled','expired') AND opened.byte_count IS NOT NULL)
     OR (opened.request_bytes IS NOT NULL AND sha256(opened.request_bytes) IS DISTINCT FROM opened.request_sha256) THEN
    RAISE EXCEPTION 'invalid pending content representation'
      USING ERRCODE='23514', CONSTRAINT='item_pending_content';
  END IF;
  RETURN QUERY SELECT opened.request_id,opened.intent_handle,opened.token,opened.target_item_id,
    opened.request_codec_version,opened.request_sha256,opened.registered_ms,opened.expires_ms,
    CASE WHEN opened.state='prepared' AND opened.expires_ms<=at_ms THEN 'expired' ELSE opened.state END,
    opened.operation_row_id,opened.request_bytes;
END;
$$;

REVOKE ALL ON FUNCTION kehila_auth.list_pending_items(kehila_auth.authority,kehila.ident,bigint),
  kehila_auth.open_pending_item(kehila_auth.authority,kehila.ident,kehila_auth.intent_key) FROM PUBLIC;
