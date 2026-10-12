-- Run inside a transaction after the creation fixture; the caller rolls back.
-- Exercise normal owner DML, without disabling any schema guard.
SET LOCAL ROLE kehila_owner;
DO $$
DECLARE original kehila.operation_payloads%ROWTYPE;
        statement text; label text; actual_constraint text;
BEGIN
  SELECT p.* INTO STRICT original FROM kehila.operation_payloads p
    JOIN kehila.operations o USING (operation_row_id)
    WHERE o.target_project_id = 'created';
  FOR statement, label IN SELECT * FROM (VALUES
    ('UPDATE kehila.operation_payloads SET request_bytes = decode(''ff'',''hex'') WHERE operation_row_id = $1', 'request bytes'),
    ('UPDATE kehila.operation_payloads SET result_codec_version = 2 WHERE operation_row_id = $1', 'result version'),
    ('UPDATE kehila.operation_payloads SET result = ''{"changed":true}'' WHERE operation_row_id = $1', 'result content'),
    ('UPDATE kehila.operation_payloads SET result = result WHERE operation_row_id = $1', 'no-op update')
  ) cases(statement, label) LOOP
    BEGIN
      EXECUTE statement USING original.operation_row_id;
      SET CONSTRAINTS ALL IMMEDIATE;
      RAISE EXCEPTION 'payload mutation accepted: %', label;
    EXCEPTION WHEN check_violation THEN
      GET STACKED DIAGNOSTICS actual_constraint = CONSTRAINT_NAME;
      IF actual_constraint <> 'operation_payload_immutable' THEN RAISE; END IF;
    END;
  END LOOP;
  BEGIN
    -- A final-state shape check alone permits replacing the deleted payload.
    DELETE FROM kehila.operation_payloads
      WHERE operation_row_id = original.operation_row_id;
    INSERT INTO kehila.operation_payloads VALUES
      (original.operation_row_id, '\xff', 2, '{"changed":true}');
    SET CONSTRAINTS ALL IMMEDIATE;
    RAISE EXCEPTION 'payload mutation accepted: payload replacement';
  EXCEPTION WHEN check_violation THEN
    GET STACKED DIAGNOSTICS actual_constraint = CONSTRAINT_NAME;
    IF actual_constraint <> 'operation_payload_retirement_required' THEN RAISE; END IF;
  END;
  IF NOT EXISTS (SELECT FROM kehila.operation_payloads p WHERE p = original) THEN
    RAISE EXCEPTION 'rejected mutations changed the original payload';
  END IF;
END;
$$;
