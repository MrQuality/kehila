-- Test-only replacement, exclusively for the disposable native fixture.
-- This does not qualify the production clock implementation or backup/restore.
SET ROLE kehila_owner;
DO $$ BEGIN
  IF (SELECT value FROM kehila_native_fixture.marker) <> 'disposable native fixture'
    OR (SELECT prosecdef OR proowner <> 'kehila_owner'::regrole
        FROM pg_proc WHERE oid = 'kehila_auth.trusted_millis()'::regprocedure) THEN
    RAISE EXCEPTION 'expected unchanged disposable clock baseline';
  END IF;
END $$;
CREATE TABLE kehila_native_fixture.original_clock AS
  SELECT oid, proowner, proacl, pg_get_functiondef(oid) AS definition
  FROM pg_proc WHERE oid = 'kehila_auth.trusted_millis()'::regprocedure;
CREATE TABLE kehila_native_fixture.clock (
  singleton boolean PRIMARY KEY CHECK (singleton), value numeric NOT NULL);
INSERT INTO kehila_native_fixture.clock VALUES
  (true, floor(extract(epoch FROM clock_timestamp()) * 1000));
REVOKE ALL ON ALL TABLES IN SCHEMA kehila_native_fixture FROM PUBLIC;
CREATE OR REPLACE FUNCTION kehila_auth.trusted_millis() RETURNS kehila.u64
LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE value numeric;
BEGIN
  SELECT c.value INTO STRICT value FROM kehila_native_fixture.clock c WHERE singleton;
  IF value < 0 OR value > 18446744073709551615 OR value <> trunc(value)
    OR value::text IN ('NaN','Infinity','-Infinity') THEN
    RAISE EXCEPTION 'disposable clock out of range' USING ERRCODE = '22003';
  END IF;
  RETURN value::kehila.u64;
END $$;
DO $$ BEGIN
  IF EXISTS (SELECT FROM kehila_native_fixture.original_clock o
    JOIN pg_proc p ON p.oid = o.oid
    WHERE p.proowner <> o.proowner OR p.proacl IS DISTINCT FROM o.proacl)
    OR NOT EXISTS (SELECT FROM kehila_native_fixture.original_clock o
      JOIN pg_proc p ON p.oid = o.oid) THEN
    RAISE EXCEPTION 'clock replacement changed identity or authority';
  END IF;
END $$;
RESET ROLE;
