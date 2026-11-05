-- Actual runtime login after a fresh product installation and synthetic enrollment.
-- These are structural SQL fixtures, not typed-codec or password qualification.
\set ON_ERROR_STOP on
SET statement_timeout = '15s';
SET lock_timeout = '3s';
SET idle_in_transaction_session_timeout = '15s';
SET ROLE kehila_auth_runtime;

CREATE FUNCTION pg_temp.require(ok boolean, label text) RETURNS void
LANGUAGE plpgsql AS $$
BEGIN
  IF ok IS DISTINCT FROM true THEN RAISE EXCEPTION 'pending check: %', label; END IF;
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
      RAISE EXCEPTION 'wrong pending rejection: % / %, expected % / %',
        actual_state, actual_constraint, expected_state, expected_constraint;
    END IF;
    RETURN;
  END;
  RAISE EXCEPTION 'expected pending rejection absent: % / %', expected_state, expected_constraint;
END;
$$;
CREATE FUNCTION pg_temp.authority() RETURNS kehila_auth.authority LANGUAGE sql AS $$
  SELECT ROW('installation','owner',1,1,1)::kehila_auth.authority;
$$;

-- Hold the real dedicated lock for this structural session; this does not prove
-- that a Rust owner keeps its separate acquisition alive through shutdown.
SELECT pg_advisory_lock(1262831689,1);
SELECT pg_temp.require((kehila_auth.start_runtime('installation','structural-start')).runtime_generation = 1,
  'fresh runtime generation');
SELECT pg_temp.require((SELECT array_agg(version ORDER BY version) = ARRAY[1,2,3]
  FROM kehila.schema_migrations), 'exact product version');

SELECT pg_temp.reject('SET ROLE kehila_owner', '42501');
SELECT pg_temp.reject('SET ROLE kehila_auth_admin', '42501');
SELECT pg_temp.reject('UPDATE kehila_auth.pending_policy SET prepared_limit = 64', '42501');
SELECT pg_temp.reject('DELETE FROM kehila_auth.pending_projects', '42501');
SELECT pg_temp.reject('TRUNCATE kehila_auth.pending_projects RESTART IDENTITY', '42501');
SELECT pg_temp.reject('ALTER TABLE kehila_auth.pending_projects ADD COLUMN unsafe text', '42501');
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.enroll('x','y','z','v')$q$, '42501');
SELECT pg_temp.reject($q$SELECT kehila_auth.retire_project_payload('installation',1)$q$, '42501');
SELECT pg_temp.reject($q$SELECT setval('kehila_auth.pending_projects_request_id_seq',1)$q$, '42501');

SELECT request_id AS first_id FROM kehila_auth.register_project(pg_temp.authority(),
  repeat('a',32),repeat('f',32),'First','FI','hours') \gset
SELECT request_id AS second_id FROM kehila_auth.register_project(pg_temp.authority(),
  repeat('b',32),repeat('f',32),'Second','SE','points') \gset
SELECT pg_temp.require(:second_id > :first_id, 'server identity increases');
SELECT pg_temp.require((SELECT count(*) = 2 FROM kehila_auth.pending_projects
  WHERE token = repeat('f',32)), 'same token allowed for distinct intents');
SELECT pg_temp.require((SELECT name = 'First' FROM kehila_auth.open_project(pg_temp.authority(),:first_id)),
  'open exact saved content');

CREATE FUNCTION pg_temp.structural_create(request_number bigint, target text,
  wrong_core_token text DEFAULT NULL, omit_completion boolean DEFAULT false) RETURNS void
LANGUAGE plpgsql AS $$
DECLARE p kehila_auth.pending_projects; operation_id bigint; origin kehila.u64;
BEGIN
  SELECT * INTO STRICT p FROM kehila_auth.open_project(pg_temp.authority(),request_number);
  origin := kehila_auth.trusted_millis();
  INSERT INTO kehila.operations
    (actor_id,command_family,target_kind,target_project_id,token,request_codec_version,
     request_sha256,required_grant,grant_policy_version,replay_origin_ms,replay_deadline_ms)
    VALUES ('owner','project_create','project',target,coalesce(wrong_core_token,p.token),1,
      decode(repeat('01',32),'hex'),'project_create',1,origin,origin+7776000000)
    RETURNING operation_row_id INTO operation_id;
  INSERT INTO kehila.operation_payloads VALUES (operation_id,'\x01',1,'{}');
  INSERT INTO kehila.projects
    (project_id,name,current_prefix,estimate_unit,configuration_revision,next_sequence,
     created_by,creation_operation_row_id,seed_profile)
    VALUES (target,p.name,p.prefix,p.estimate_unit,1,1,'owner',operation_id,'m1_v1');
  INSERT INTO kehila.configuration_revisions VALUES
    (target,1,1,'{}','{}','owner',operation_id,'project_create','project');
  INSERT INTO kehila.project_grants VALUES (target,'owner','initial_owner',true,1);
  INSERT INTO kehila.project_grant_events
    (project_id,actor_id,version,access_profile,active,granted_by,granting_operation_row_id,command_family)
    VALUES (target,'owner',1,'initial_owner',true,'owner',operation_id,'project_create');
  IF NOT omit_completion THEN
    PERFORM kehila_auth.complete_project(pg_temp.authority(),request_number,p.token,
      p.name,p.prefix,p.estimate_unit,target,operation_id);
  END IF;
END;
$$;

BEGIN;
SELECT pg_temp.reject($q$
  INSERT INTO kehila.operations
    (actor_id,command_family,target_kind,target_project_id,token,request_codec_version,
     request_sha256,required_grant,grant_policy_version,replay_origin_ms,replay_deadline_ms)
    SELECT 'owner','project_create','project','orphan-core','orphan',1,
      decode(repeat('01',32),'hex'),'project_create',1,origin,origin+7776000000
      FROM (SELECT kehila_auth.trusted_millis() AS origin) sampled;
  INSERT INTO kehila.operation_payloads
    SELECT operation_row_id,'\x01',1,'{}' FROM kehila.operations WHERE target_project_id = 'orphan-core';
  SET CONSTRAINTS ALL IMMEDIATE
$q$, '23503', 'operation_success_mapping');
SELECT pg_temp.reject(format('SELECT pg_temp.structural_create(%s,%L,NULL,true); SET CONSTRAINTS ALL IMMEDIATE',
  :first_id,'missing-completion'), '23503');
SELECT pg_temp.require((SELECT count(*) = 0 FROM kehila.projects), 'missing completion rolls back all project rows');
SELECT pg_temp.require((SELECT count(*) = 0 FROM kehila.operations), 'missing completion rolls back core');
SELECT pg_temp.reject(format('SELECT pg_temp.structural_create(%s,%L,%L); SET CONSTRAINTS ALL IMMEDIATE',
  :first_id,'wrong-token',repeat('e',32)), '23503', 'pending_core_token');
SELECT pg_temp.require((SELECT count(*) = 0 FROM kehila.operation_payloads), 'token rejection rolls back payload');
ROLLBACK;

BEGIN;
SELECT pg_temp.structural_create(:first_id,'first-project');
SET CONSTRAINTS ALL IMMEDIATE;
ROLLBACK;
SELECT pg_temp.require((SELECT state = 'prepared' FROM kehila_auth.pending_projects WHERE request_id = :first_id),
  'rollback preserves executable pending state');
SELECT pg_temp.require((SELECT count(*) = 0 FROM kehila.operations), 'rollback preserves empty permanent history');

BEGIN;
SELECT pg_temp.structural_create(:first_id,'first-project');
SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;
BEGIN;
SELECT pg_temp.structural_create(:second_id,'second-project');
SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;
SELECT pg_temp.require((SELECT count(*) = 2 FROM kehila_auth.creation_intents), 'distinct targets share token safely');
SELECT pg_temp.require((SELECT state = 'completed' AND target_project_id = 'first-project'
  FROM kehila_auth.cancel_project(pg_temp.authority(),:first_id)), 'cancel cannot undo completed project');
SELECT pg_temp.require((SELECT count(*) = 2 FROM kehila.projects), 'completed cancellation keeps projects');
SELECT pg_temp.reject('UPDATE kehila_auth.creation_intents SET target_project_id = target_project_id', '42501');

DO $$
DECLARE request_number bigint; i integer;
BEGIN
  FOR i IN 1..32 LOOP
    SELECT request_id INTO STRICT request_number FROM kehila_auth.register_project(pg_temp.authority(),
      lpad(to_hex(i),32,'0'),lpad(to_hex(i),32,'0'),'Quota','QU','hours');
  END LOOP;
END;
$$;
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.register_project(pg_temp.authority(),
  repeat('c',32),repeat('c',32),'Overflow','OV','hours')$q$, '54000');
SELECT pg_temp.require((SELECT count(*) = 32 FROM kehila_auth.pending_projects WHERE state = 'prepared'),
  'quota does not allocate overflow entry');
SELECT pg_temp.require((SELECT count(*) = 17 FROM kehila_auth.list_projects(pg_temp.authority(),NULL)),
  'page size plus continuation sentinel');
SELECT pg_temp.require((SELECT bool_and(request_id < :second_id)
  FROM kehila_auth.list_projects(pg_temp.authority(),:second_id)), 'strict cursor bound');

SELECT pg_advisory_unlock(1262831689,1);
SELECT 'PASS: product pending structural capabilities and atomic success';
