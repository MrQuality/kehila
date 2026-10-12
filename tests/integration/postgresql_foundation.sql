\set ON_ERROR_STOP on
SET search_path = pg_catalog, kehila;

CREATE FUNCTION pg_temp.require(ok boolean, label text) RETURNS void
LANGUAGE plpgsql AS $$
BEGIN
  IF ok IS DISTINCT FROM true THEN RAISE EXCEPTION 'FAILED: %', label; END IF;
END;
$$;
CREATE FUNCTION pg_temp.reject(statement text, expected_state text,
                               expected_constraint text DEFAULT NULL) RETURNS void
LANGUAGE plpgsql AS $$
DECLARE actual_state text; actual_constraint text;
BEGIN
  BEGIN
    EXECUTE statement;
  EXCEPTION WHEN OTHERS THEN
    GET STACKED DIAGNOSTICS actual_state = RETURNED_SQLSTATE,
                           actual_constraint = CONSTRAINT_NAME;
    IF actual_state <> expected_state OR
       (expected_constraint IS NOT NULL AND actual_constraint <> expected_constraint) THEN
      RAISE EXCEPTION 'wrong rejection: state %, constraint %, expected % / %',
        actual_state, actual_constraint, expected_state, expected_constraint;
    END IF;
    RETURN;
  END;
  RAISE EXCEPTION 'expected rejection % / %: %',
    expected_state, expected_constraint, statement;
END;
$$;

SELECT pg_temp.require((SELECT count(*) = 1 FROM kehila.schema_migrations
                        WHERE version = 1), 'installed version');
SELECT pg_temp.reject($q$SELECT ''::kehila.ident$q$, '23514');
SELECT pg_temp.reject($q$SELECT repeat('x',129)::kehila.ident$q$, '23514');
SELECT pg_temp.reject($q$SELECT 1.5::kehila.u64$q$, '23514');
SELECT pg_temp.reject($q$SELECT 18446744073709551616::kehila.u64$q$, '23514');
SELECT pg_temp.reject($q$SELECT 'a b'::kehila.operation_token$q$, '23514');
SELECT pg_temp.reject($q$SELECT repeat('a',129)::kehila.operation_token$q$, '23514');
SELECT pg_temp.require(' id '::kehila.ident = ' id ', 'no trimming');
SELECT pg_temp.require(U&'\00E9'::kehila.ident <> U&'e\0301'::kehila.ident,
                       'no Unicode normalization');
SELECT pg_temp.require(18446744073709551615::kehila.u64 = 18446744073709551615,
                       'u64 maximum exact');
SELECT pg_temp.require(octet_length(repeat(U&'\00E9',64)::kehila.ident) = 128,
                       'UTF8 identifier bound counts bytes');
SELECT pg_temp.reject($q$SELECT repeat(U&'\00E9',65)::kehila.ident$q$, '23514');
SELECT pg_temp.reject($q$SELECT convert_from(decode('00','hex'),'UTF8')$q$, '22021');

INSERT INTO kehila.actors VALUES ('actor', true, true);
BEGIN;
INSERT INTO kehila.operations
  (actor_id, command_family, target_kind, target_project_id, token,
   request_codec_version, request_sha256, required_grant, grant_policy_version,
   replay_origin_ms, replay_deadline_ms)
VALUES ('actor','project_create','project','project','token',1,
        decode(repeat('00',32),'hex'),'project_create',1,100,7776000100)
RETURNING operation_row_id \gset
INSERT INTO kehila.operation_payloads VALUES
  (:operation_row_id, decode('00','hex'), 1, '{}');
COMMIT;

SELECT pg_temp.reject($q$
  INSERT INTO kehila.operations
    (actor_id,command_family,target_kind,target_project_id,token,
     request_codec_version,request_sha256,required_grant,grant_policy_version,
     replay_origin_ms,replay_deadline_ms)
  SELECT actor_id,command_family,target_kind,target_project_id,token,
    request_codec_version,request_sha256,required_grant,grant_policy_version,
    replay_origin_ms,replay_deadline_ms FROM kehila.operations
$q$, '23505', 'operations_scoped_key');
SELECT pg_temp.reject($q$
  INSERT INTO kehila.operations
    (actor_id,command_family,target_kind,target_project_id,token,
     request_codec_version,request_sha256,required_grant,grant_policy_version,
     replay_origin_ms,replay_deadline_ms)
  SELECT actor_id,command_family,target_kind,'missing','missing',
    request_codec_version,request_sha256,required_grant,grant_policy_version,
    replay_origin_ms,replay_deadline_ms FROM kehila.operations;
  SET CONSTRAINTS ALL IMMEDIATE
$q$, '23514', 'operation_payload_shape');
SELECT pg_temp.reject($q$DELETE FROM kehila.operation_payloads;
  SET CONSTRAINTS ALL IMMEDIATE$q$, '23514', 'operation_payload_retirement_required');
SELECT pg_temp.reject($q$UPDATE kehila.operations SET token = 'changed'$q$,
                       '23514', 'operation_core_immutable');
SELECT pg_temp.reject($q$DELETE FROM kehila.operations$q$,
                       '23514', 'operation_core_immutable');
SELECT pg_temp.reject($q$TRUNCATE kehila.operations CASCADE$q$,
                       '23514', 'operation_storage_no_truncate');
SELECT pg_temp.reject($q$
  INSERT INTO kehila.operations
    (actor_id,command_family,target_kind,token,request_codec_version,
     request_sha256,required_grant,grant_policy_version,replay_origin_ms,replay_deadline_ms)
  SELECT actor_id,command_family,target_kind,'no-target',request_codec_version,
    request_sha256,required_grant,grant_policy_version,replay_origin_ms,replay_deadline_ms
  FROM kehila.operations
$q$, '23514', 'operations_target_shape');
SELECT pg_temp.reject($q$
  INSERT INTO kehila.operations
    (actor_id,command_family,target_kind,target_project_id,token,
     request_codec_version,request_sha256,required_grant,grant_policy_version,
     replay_origin_ms,replay_deadline_ms)
  SELECT actor_id,command_family,target_kind,'bad-window','bad-window',
    request_codec_version,request_sha256,required_grant,grant_policy_version,
    replay_origin_ms,replay_deadline_ms+1 FROM kehila.operations
$q$, '23514', 'operations_replay_window');
SELECT pg_temp.reject($q$
  INSERT INTO kehila.operations
    (actor_id,command_family,target_kind,target_project_id,token,
     request_codec_version,request_sha256,required_grant,grant_policy_version,
     replay_origin_ms,replay_deadline_ms,payload_retired)
  SELECT actor_id,command_family,target_kind,'born-retired','born-retired',
    request_codec_version,request_sha256,required_grant,grant_policy_version,
    replay_origin_ms,replay_deadline_ms,true FROM kehila.operations
$q$, '23514', 'operation_core_fresh');
SELECT pg_temp.reject($q$UPDATE kehila.operations SET payload_retired = true;
  SET CONSTRAINTS ALL IMMEDIATE$q$, '23514', 'operation_payload_shape');

-- Without the identity trigger this move satisfies only the NEW core's shape.
SELECT pg_temp.reject($q$
  INSERT INTO kehila.operations
    (actor_id,command_family,target_kind,target_project_id,token,
     request_codec_version,request_sha256,required_grant,grant_policy_version,
     replay_origin_ms,replay_deadline_ms)
  SELECT actor_id,command_family,target_kind,'moved','move',
    request_codec_version,request_sha256,required_grant,grant_policy_version,
    replay_origin_ms,replay_deadline_ms FROM kehila.operations;
  UPDATE kehila.operation_payloads SET operation_row_id =
    currval('kehila.operations_operation_row_id_seq');
  SET CONSTRAINTS ALL IMMEDIATE
$q$, '23514', 'operation_payload_identity_immutable');
SELECT pg_temp.require((SELECT count(*) = 1 FROM kehila.operations),
                       'rejected writes leave permanent core unchanged');
SELECT pg_temp.require((SELECT count(*) = 1 FROM kehila.operation_payloads),
                       'rejected writes leave payload unchanged');
SELECT pg_temp.require((SELECT count(*) = 2 FROM pg_roles
  WHERE rolname IN ('kehila_owner','kehila_creator') AND NOT rolcanlogin
    AND NOT rolsuper AND NOT rolcreatedb AND NOT rolcreaterole AND NOT rolbypassrls),
  'restricted capability roles');
SELECT pg_temp.require(NOT pg_has_role('kehila_creator','kehila_owner','MEMBER'),
                       'creator is not owner');

-- Exercise the supported creation key at its actual maximum byte widths.
BEGIN;
INSERT INTO kehila.actors
  SELECT string_agg(md5(n::text), ''), true, true FROM generate_series(1,4) n;
INSERT INTO kehila.operations
  (actor_id,command_family,target_kind,target_project_id,token,
   request_codec_version,request_sha256,required_grant,grant_policy_version,
   replay_origin_ms,replay_deadline_ms)
SELECT (SELECT string_agg(md5(n::text),'') FROM generate_series(1,4) n),
       'project_create','project',
       (SELECT string_agg(md5(n::text),'') FROM generate_series(5,8) n),
       (SELECT string_agg(md5(n::text),'') FROM generate_series(9,12) n),
       1,decode(repeat('00',32),'hex'),'project_create',1,0,7776000000
RETURNING operation_row_id \gset
INSERT INTO kehila.operation_payloads VALUES (:operation_row_id,'\x00',1,'{}');
SET CONSTRAINTS ALL IMMEDIATE;
ROLLBACK;

-- Aborted retirement must retain the original full operation and payload.
BEGIN;
UPDATE kehila.operations SET payload_retired = true;
DELETE FROM kehila.operation_payloads;
SET CONSTRAINTS ALL IMMEDIATE;
ROLLBACK;
SELECT pg_temp.require((SELECT NOT payload_retired FROM kehila.operations),
                       'retirement rollback preserves full core');
SELECT pg_temp.require((SELECT count(*) = 1 FROM kehila.operation_payloads),
                       'retirement rollback preserves payload');

BEGIN;
UPDATE kehila.operations SET payload_retired = true;
DELETE FROM kehila.operation_payloads;
COMMIT;
SELECT pg_temp.reject($q$
  INSERT INTO kehila.operation_payloads
    SELECT operation_row_id,'\xff',2,'{"changed":true}' FROM kehila.operations;
  SET CONSTRAINTS ALL IMMEDIATE
$q$, '23514', 'operation_payload_shape');
SELECT pg_temp.reject($q$UPDATE kehila.operations SET payload_retired = false$q$,
                       '23514', 'operation_core_immutable');
SELECT pg_temp.reject($q$TRUNCATE kehila.operation_payloads$q$,
                       '23514', 'operation_storage_no_truncate');
SELECT 'PASS: foundation integrity and identity regression' AS result;
