-- Uninstalled source preparation; requires records.sql and the complete
-- transition/capability packet before fresh product adoption. No runtime grants.

CREATE FUNCTION kehila.protect_item_history() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, kehila, pg_temp AS $$
BEGIN
  RAISE EXCEPTION 'item history is permanent'
    USING ERRCODE = '23514', CONSTRAINT = 'item_history_immutable',
          SCHEMA = TG_TABLE_SCHEMA, TABLE = TG_TABLE_NAME;
END;
$$;

CREATE TRIGGER item_revision_immutable
  BEFORE UPDATE OR DELETE OR TRUNCATE ON kehila.item_revisions
  FOR EACH STATEMENT EXECUTE FUNCTION kehila.protect_item_history();
CREATE TRIGGER item_field_use_immutable
  BEFORE UPDATE OR DELETE OR TRUNCATE ON kehila.item_field_uses
  FOR EACH STATEMENT EXECUTE FUNCTION kehila.protect_item_history();
CREATE TRIGGER item_identity_no_truncate
  BEFORE TRUNCATE ON kehila.items
  FOR EACH STATEMENT EXECUTE FUNCTION kehila.protect_item_history();
CREATE TRIGGER item_pending_no_truncate
  BEFORE TRUNCATE ON kehila_auth.pending_items
  FOR EACH STATEMENT EXECUTE FUNCTION kehila.protect_item_history();

CREATE FUNCTION kehila.protect_item_identity() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, kehila, pg_temp AS $$
BEGIN
  IF TG_OP = 'INSERT' THEN
    IF NEW.current_version = 1 THEN RETURN NEW; END IF;
  ELSIF TG_OP = 'UPDATE' THEN
    IF (to_jsonb(NEW) - 'current_version') IS NOT DISTINCT FROM
       (to_jsonb(OLD) - 'current_version')
       AND NEW.current_version = OLD.current_version + 1 THEN
      RETURN NEW;
    END IF;
  END IF;
  RAISE EXCEPTION 'invalid item identity or head transition'
    USING ERRCODE = '23514', CONSTRAINT = 'item_identity_transition',
          SCHEMA = TG_TABLE_SCHEMA, TABLE = TG_TABLE_NAME;
END;
$$;
CREATE TRIGGER item_identity_transition
  BEFORE INSERT OR UPDATE OR DELETE ON kehila.items
  FOR EACH ROW EXECUTE FUNCTION kehila.protect_item_identity();

CREATE FUNCTION kehila_auth.protect_pending_item() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE at_ms kehila.u64;
BEGIN
  IF TG_OP = 'INSERT' THEN
    IF NEW.state = 'prepared' AND NEW.request_bytes IS NOT NULL
       AND NEW.operation_row_id IS NULL THEN RETURN NEW; END IF;
  ELSIF TG_OP = 'DELETE' THEN
    -- Successful identity is permanent; only never-created terminal work may
    -- be pruned. Missing execution must not reconstruct a deleted request.
    IF OLD.state IN ('cancelled', 'expired') THEN RETURN OLD; END IF;
  -- Compare bounded identity scalars directly: serializing the entire row to
  -- JSON would first copy/hex-encode the potentially full-size request bytea.
  ELSIF ROW(NEW.request_id, NEW.installation_id, NEW.actor_id, NEW.project_id,
            NEW.command_family, NEW.target_kind, NEW.intent_handle, NEW.token,
            NEW.target_item_id, NEW.request_codec_version, NEW.request_sha256,
            NEW.registered_ms, NEW.expires_ms)
      IS NOT DISTINCT FROM
        ROW(OLD.request_id, OLD.installation_id, OLD.actor_id, OLD.project_id,
            OLD.command_family, OLD.target_kind, OLD.intent_handle, OLD.token,
            OLD.target_item_id, OLD.request_codec_version, OLD.request_sha256,
            OLD.registered_ms, OLD.expires_ms) THEN
    at_ms := kehila_auth.trusted_millis();
    IF OLD.state = 'prepared' THEN
      -- Cancellation expresses intent, not an earlier caller's clock decision.
      -- Classify expiry once here, after the protecting pending-row wait, so a
      -- deadline crossed before this sample cannot cause a false integrity error.
      IF NEW.state = 'cancelled' AND OLD.expires_ms <= at_ms THEN
        NEW.state := 'expired';
      END IF;
      IF NEW.state = 'completed' AND NEW.operation_row_id IS NOT NULL
         AND NEW.request_bytes IS NOT DISTINCT FROM OLD.request_bytes THEN
        IF OLD.expires_ms <= at_ms THEN
          -- Ordinary completion expiry is a narrow rollback signal, not a
          -- generic integrity failure. Deferred attribution is still pending.
          RAISE EXCEPTION 'pending item expired before completion'
            USING ERRCODE='P0002', CONSTRAINT='pending_item_expired';
        END IF;
        RETURN NEW;
      ELSIF NEW.state IN ('cancelled', 'expired')
         AND NEW.operation_row_id IS NULL AND NEW.request_bytes IS NULL
         AND ((NEW.state = 'expired' AND OLD.expires_ms <= at_ms)
           OR (NEW.state = 'cancelled' AND OLD.expires_ms > at_ms)) THEN
        RETURN NEW;
      END IF;
    ELSIF OLD.state = 'completed' AND NEW.state = 'completed'
       AND NEW.operation_row_id IS NOT DISTINCT FROM OLD.operation_row_id
       AND OLD.request_bytes IS NOT NULL AND NEW.request_bytes IS NULL THEN
      -- Nonlocking read of an irreversible fact. The maintenance entry must run
      -- pruning separately from retirement; this trigger cannot prove that
      -- transaction boundary. Do not add a core lock after pending here.
      IF EXISTS (SELECT FROM kehila.operations o
                 WHERE o.operation_row_id = OLD.operation_row_id
                   AND o.payload_retired AND o.replay_deadline_ms <= at_ms) THEN
        RETURN NEW;
      END IF;
    END IF;
  END IF;
  RAISE EXCEPTION 'invalid pending item transition'
    USING ERRCODE = '23514', CONSTRAINT = 'pending_item_transition',
          SCHEMA = TG_TABLE_SCHEMA, TABLE = TG_TABLE_NAME;
END;
$$;
CREATE TRIGGER pending_item_transition
  BEFORE INSERT OR UPDATE OR DELETE ON kehila_auth.pending_items
  FOR EACH ROW EXECUTE FUNCTION kehila_auth.protect_pending_item();

REVOKE ALL ON FUNCTION kehila.protect_item_history(),
  kehila.protect_item_identity(), kehila_auth.protect_pending_item() FROM PUBLIC;

-- These immediate guards do not replace deferred reverse family/head closure,
-- historical JSON agreement, current authorization or fixed-order write entry.
