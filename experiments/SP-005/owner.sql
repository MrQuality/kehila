-- Guard checks as schema owner; an administrator can still disable/replace guards.
\set ON_ERROR_STOP on
SET statement_timeout = '10s';
SET lock_timeout = '2s';
SET idle_in_transaction_session_timeout = '10s';
SET ROLE kehila_owner;
CREATE FUNCTION pg_temp.reject(statement text) RETURNS void
LANGUAGE plpgsql AS $$
DECLARE actual_state text; actual_constraint text;
BEGIN
  BEGIN
    EXECUTE statement;
  EXCEPTION WHEN OTHERS THEN
    GET STACKED DIAGNOSTICS actual_state = RETURNED_SQLSTATE,
                           actual_constraint = CONSTRAINT_NAME;
    IF actual_state = '23514' AND actual_constraint = 'sp005_intent_immutable' THEN RETURN; END IF;
    RAISE EXCEPTION 'SP005 wrong owner rejection: % / %', actual_state, actual_constraint;
  END;
  RAISE EXCEPTION 'SP005 owner mutation accepted';
END;
$$;
SELECT pg_temp.reject($q$UPDATE sp005.creation_intents SET intent_handle=intent_handle$q$);
SELECT pg_temp.reject($q$DELETE FROM sp005.creation_intents$q$);
SELECT pg_temp.reject($q$TRUNCATE sp005.creation_intents$q$);
DO $$
BEGIN
  IF (SELECT count(*) FROM sp005.creation_intents) <> 1 THEN
    RAISE EXCEPTION 'SP005 owner probes changed state';
  END IF;
  IF (SELECT array_agg(attname::text ORDER BY attnum) FROM pg_attribute
      WHERE attrelid='sp005.creation_intents'::regclass AND attnum>0 AND NOT attisdropped)
      IS DISTINCT FROM ARRAY['actor_id','command_family','intent_handle','target_kind',
                             'target_project_id','operation_row_id'] THEN
    RAISE EXCEPTION 'SP005 unexpected mapping representation';
  END IF;
  IF NOT (SELECT bool_and(attnotnull) FROM pg_attribute
          WHERE attrelid='sp005.creation_intents'::regclass AND attnum>0 AND NOT attisdropped)
     OR NOT EXISTS (SELECT FROM pg_constraint
       WHERE conrelid='sp005.creation_intents'::regclass AND contype='p'
         AND conkey=ARRAY[1,2,3]::smallint[])
     OR NOT EXISTS (SELECT FROM pg_constraint
       WHERE conrelid='sp005.creation_intents'::regclass AND contype='u'
         AND conkey=ARRAY[5]::smallint[]) THEN
    RAISE EXCEPTION 'SP005 missing non-null or identity/target uniqueness contract';
  END IF;
  IF has_function_privilege('foundation_serving','sp005.protect_intent()','EXECUTE')
     OR has_function_privilege('foundation_untrusted','sp005.protect_intent()','EXECUTE') THEN
    RAISE EXCEPTION 'SP005 public trigger function execution';
  END IF;
END;
$$;
SELECT 'SP005_OWNER_PASS';
