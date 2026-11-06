-- Disposable fixture only: two pending-TTL clock samples in one RR snapshot.
-- No host clock change or SavedResult delivery authorization is supplied.
DO $$ BEGIN
  IF session_user <> 'postgres' OR current_database() <> 'native_item_read'
    OR NOT EXISTS(SELECT FROM kehila_native_fixture.marker WHERE value='disposable native fixture')
    OR (SELECT proowner <> 'kehila_owner'::regrole OR prosecdef OR provolatile <> 'v'
        FROM pg_proc WHERE oid='kehila_auth.trusted_millis()'::regprocedure) THEN
    RAISE EXCEPTION 'expected unchanged disposable factory clock baseline';
  END IF;
END $$;
CREATE OR REPLACE FUNCTION kehila_auth.trusted_millis() RETURNS kehila.u64
LANGUAGE plpgsql VOLATILE SECURITY INVOKER SET search_path = pg_catalog, kehila, pg_temp AS $$
DECLARE value numeric; first_text text; second_text text; step_text text; first_ms numeric; second_ms numeric;
BEGIN
  first_text:=nullif(current_setting('kehila_native_fixture.first_ms',true),'');
  second_text:=nullif(current_setting('kehila_native_fixture.second_ms',true),'');
  step_text:=nullif(current_setting('kehila_native_fixture.clock_step',true),'');
  IF session_user='postgres' AND (first_text IS NOT NULL OR second_text IS NOT NULL OR step_text IS NOT NULL) THEN
    IF first_text IS NULL OR second_text IS NULL OR step_text IS NULL
      OR first_text !~ '^(0|[1-9][0-9]{0,19})$' OR second_text !~ '^(0|[1-9][0-9]{0,19})$'
      OR step_text NOT IN ('0','1','2') THEN
      RAISE EXCEPTION 'invalid disposable clock configuration' USING ERRCODE='22023';
    END IF;
    first_ms:=first_text::numeric; second_ms:=second_text::numeric;
    IF first_ms>=second_ms OR second_ms>18446744073709551615 THEN
      RAISE EXCEPTION 'invalid disposable clock samples' USING ERRCODE='22023';
    END IF;
    value:=CASE WHEN step_text='0' THEN first_ms ELSE second_ms END;
    PERFORM set_config('kehila_native_fixture.clock_step',CASE WHEN step_text='0' THEN '1' ELSE '2' END,true);
  ELSE
    value:=floor(extract(epoch FROM clock_timestamp())*1000);
  END IF;
  IF value<0 OR value>18446744073709551615 OR value::text IN ('NaN','Infinity','-Infinity') THEN
    RAISE EXCEPTION 'trusted clock out of range' USING ERRCODE='22003';
  END IF;
  RETURN value::kehila.u64;
END;
$$;
