CREATE FUNCTION kehila.protect_payload_identity()
RETURNS trigger LANGUAGE plpgsql SET search_path = pg_catalog, kehila, pg_temp AS $$
BEGIN
  IF NEW.operation_row_id IS DISTINCT FROM OLD.operation_row_id THEN
    RAISE EXCEPTION 'operation payload identity is immutable'
      USING ERRCODE = '23514', CONSTRAINT = 'operation_payload_identity_immutable',
            SCHEMA = 'kehila', TABLE = 'operation_payloads';
  END IF;
  RETURN NEW;
END;
$$;

CREATE TRIGGER operation_payload_identity_immutable
  BEFORE UPDATE ON kehila.operation_payloads FOR EACH ROW
  EXECUTE FUNCTION kehila.protect_payload_identity();
