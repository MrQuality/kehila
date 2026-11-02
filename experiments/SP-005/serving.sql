-- Actual foundation_serving connection. Existing structural creation fixture only.
\set ON_ERROR_STOP on
SET statement_timeout = '10s';
SET lock_timeout = '2s';
SET idle_in_transaction_session_timeout = '10s';
CREATE FUNCTION pg_temp.require(ok boolean, label text) RETURNS void
LANGUAGE plpgsql AS $$
BEGIN
  IF ok IS DISTINCT FROM true THEN RAISE EXCEPTION 'SP005: %', label; END IF;
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
      RAISE EXCEPTION 'SP005 wrong rejection: % / %, expected % / %',
        actual_state, actual_constraint, expected_state, expected_constraint;
    END IF;
    RETURN;
  END;
  RAISE EXCEPTION 'SP005 expected rejection absent: % / %',
    expected_state, expected_constraint;
END;
$$;

CREATE TEMP VIEW sp005_creation_fixture AS
SELECT o.* FROM kehila.projects p JOIN kehila.operations o
  ON o.operation_row_id=p.creation_operation_row_id WHERE p.project_id='created';
SELECT pg_temp.require((SELECT count(*) = 1 FROM pg_temp.sp005_creation_fixture), 'canonical creation core');
SELECT pg_temp.require((SELECT count(*) = 0 FROM sp005.creation_intents), 'empty overlay');
-- Keep the mapping empty during FK cases so uniqueness cannot mask attribution errors.
SELECT pg_temp.reject($q$INSERT INTO sp005.creation_intents
  SELECT 'other', 'project_create', repeat('a',32), 'project', 'created', operation_row_id
  FROM pg_temp.sp005_creation_fixture$q$,
  '23503', 'sp005_intent_attribution');
SELECT pg_temp.reject($q$INSERT INTO sp005.creation_intents
  SELECT actor_id, command_family, repeat('a',32), target_kind, 'wrong-target', operation_row_id
  FROM pg_temp.sp005_creation_fixture$q$,
  '23503', 'sp005_intent_attribution');
-- Attribute/core tuple is valid, but these cores did not create the project.
SELECT pg_temp.reject($q$INSERT INTO sp005.creation_intents
  SELECT actor_id,command_family,repeat('a',32),target_kind,target_project_id,operation_row_id
  FROM kehila.operations WHERE token='sp005-alternate'$q$,
  '23503', 'sp005_intent_creation');
SELECT pg_temp.reject($q$INSERT INTO sp005.creation_intents
  SELECT actor_id,command_family,repeat('a',32),target_kind,target_project_id,operation_row_id
  FROM kehila.operations WHERE token='sp005-other-creator'$q$,
  '23503', 'sp005_intent_creation');
-- Existing attributed operation has no project; isolate the creation FK oracle.
SELECT pg_temp.reject($q$INSERT INTO sp005.creation_intents
  SELECT actor_id,command_family,repeat('a',32),target_kind,target_project_id,operation_row_id
  FROM kehila.operations WHERE target_project_id='project'$q$,
  '23503', 'sp005_intent_creation');
SELECT pg_temp.reject($q$INSERT INTO sp005.creation_intents VALUES
  ('actor','project_create',repeat('a',32),'project','created',-1)$q$,
  '23503');
SELECT pg_temp.reject($q$INSERT INTO sp005.creation_intents
  SELECT actor_id, 'project_metadata', repeat('a',32), target_kind, target_project_id, operation_row_id
  FROM pg_temp.sp005_creation_fixture$q$,
  '23514', 'sp005_intent_family');
SELECT pg_temp.reject($q$INSERT INTO sp005.creation_intents
  SELECT actor_id, command_family, repeat('a',32), 'user', target_project_id, operation_row_id
  FROM pg_temp.sp005_creation_fixture$q$,
  '23514', 'sp005_intent_kind');
SELECT pg_temp.reject($q$INSERT INTO sp005.creation_intents
  SELECT actor_id, command_family, repeat('A',32), target_kind, target_project_id, operation_row_id
  FROM pg_temp.sp005_creation_fixture$q$,
  '23514', 'sp005_intent_handle');
SELECT pg_temp.reject($q$INSERT INTO sp005.creation_intents
  SELECT actor_id, command_family, repeat('a',31), target_kind, target_project_id, operation_row_id
  FROM pg_temp.sp005_creation_fixture$q$,
  '23514', 'sp005_intent_handle');
SELECT pg_temp.reject($q$INSERT INTO sp005.creation_intents
  SELECT actor_id, command_family, NULL, target_kind, target_project_id, operation_row_id
  FROM pg_temp.sp005_creation_fixture$q$, '23502');

BEGIN;
INSERT INTO sp005.creation_intents
SELECT actor_id, command_family, repeat('a',32), target_kind, target_project_id, operation_row_id
FROM pg_temp.sp005_creation_fixture;
SET CONSTRAINTS ALL IMMEDIATE;
ROLLBACK;
SELECT pg_temp.require((SELECT count(*) = 0 FROM sp005.creation_intents), 'mapping rollback');

INSERT INTO sp005.creation_intents
SELECT actor_id, command_family, repeat('a',32), target_kind, target_project_id, operation_row_id
FROM pg_temp.sp005_creation_fixture;
SELECT pg_temp.require((SELECT count(*) = 1 FROM sp005.creation_intents), 'mapping committed');
-- Token is read from the immutable core. This is a trusted-backend fixture query,
-- not an authenticated resolver or a complete replay implementation.
SELECT pg_temp.require((SELECT o.token = 'create' FROM sp005.creation_intents m
  JOIN kehila.operations o USING (operation_row_id)
  WHERE m.actor_id='actor' AND m.command_family='project_create'
    AND m.intent_handle=repeat('a',32)), 'derived token');
SELECT pg_temp.require(NOT EXISTS (SELECT FROM sp005.creation_intents m
  JOIN kehila.operations o USING (operation_row_id)
  WHERE m.actor_id='actor' AND m.command_family='project_create'
    AND m.intent_handle=repeat('a',32) AND o.token='different'), 'changed token does not match');
SELECT pg_temp.reject($q$INSERT INTO sp005.creation_intents
  SELECT actor_id, command_family, repeat('b',32), target_kind, target_project_id, operation_row_id
  FROM pg_temp.sp005_creation_fixture$q$,
  '23505', 'sp005_intent_target');
-- Reusing the primary identity also collides on target; require the SQLSTATE
-- rather than depending on which unique index PostgreSQL checks first.
SELECT pg_temp.reject($q$INSERT INTO sp005.creation_intents
  SELECT * FROM sp005.creation_intents$q$, '23505');
SELECT pg_temp.reject($q$UPDATE sp005.creation_intents SET intent_handle=repeat('c',32)$q$, '42501');
SELECT pg_temp.reject($q$DELETE FROM sp005.creation_intents$q$, '42501');
SELECT pg_temp.reject($q$TRUNCATE sp005.creation_intents$q$, '42501');
SELECT pg_temp.reject($q$CREATE TABLE sp005.untrusted(value text)$q$, '42501');
SELECT pg_temp.reject($q$ALTER TABLE sp005.creation_intents ADD COLUMN token text$q$, '42501');
SELECT pg_temp.reject($q$SET ROLE kehila_owner$q$, '42501');
SELECT pg_temp.require((SELECT count(*) = 1 FROM sp005.creation_intents), 'rejections left state unchanged');
SELECT 'SP005_SERVING_PASS';
