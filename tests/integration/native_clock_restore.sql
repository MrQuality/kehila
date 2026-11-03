SET ROLE kehila_owner;
DO $$ DECLARE original text; BEGIN
  SELECT definition INTO STRICT original FROM kehila_native_fixture.original_clock;
  EXECUTE original;
  ALTER FUNCTION kehila_auth.trusted_millis() SECURITY INVOKER;
  IF NOT EXISTS (SELECT FROM kehila_native_fixture.original_clock o JOIN pg_proc p ON p.oid=o.oid)
    OR EXISTS (SELECT FROM kehila_native_fixture.original_clock o
    JOIN pg_proc p ON p.oid = o.oid
    WHERE p.proowner <> o.proowner OR p.proacl IS DISTINCT FROM o.proacl
      OR p.prosecdef OR pg_get_functiondef(p.oid) <> o.definition) THEN
    RAISE EXCEPTION 'clock baseline was not restored';
  END IF;
END $$;
DROP TABLE kehila_native_fixture.clock, kehila_native_fixture.original_clock;
RESET ROLE;
