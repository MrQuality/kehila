-- Private write-fence contract controls as schema owner. This does not qualify
-- an authenticated runtime login, live session release or observed lock waits.
\set ON_ERROR_STOP on
SET statement_timeout = '15s';
SET lock_timeout = '3s';
SET idle_in_transaction_session_timeout = '15s';
SET ROLE kehila_owner;

SELECT pg_advisory_lock(1262831689,1);
SELECT * FROM kehila_auth.start_runtime('installation','item-fence-start');
CREATE TEMP TABLE expected_authority AS
  SELECT i.installation_id,i.owner_actor_id,i.runtime_generation,
    c.auth_generation,c.verifier_revision
  FROM kehila_auth.installation i JOIN kehila_auth.owner_credentials c USING(owner_actor_id);

CREATE FUNCTION pg_temp.check_fence(runtime_delta integer DEFAULT 0,
  auth_delta integer DEFAULT 0, actor text DEFAULT 'owner', project text DEFAULT 'p',
  revision_delta integer DEFAULT 0)
RETURNS void LANGUAGE plpgsql AS $$
DECLARE expected kehila_auth.authority; actual kehila_auth.authority;
BEGIN
  SELECT e.installation_id,actor,e.runtime_generation+runtime_delta,
    e.auth_generation+auth_delta,e.verifier_revision+revision_delta
    INTO STRICT expected FROM expected_authority e;
  actual := kehila_auth.item_write_fence(expected,project::kehila.ident);
  IF actual IS DISTINCT FROM expected THEN RAISE EXCEPTION 'wrong fresh item authority'; END IF;
END;
$$;
CREATE FUNCTION pg_temp.reject(statement text, expected_state text,
  expected_constraint text DEFAULT '') RETURNS void LANGUAGE plpgsql AS $$
DECLARE actual_state text; actual_constraint text;
BEGIN
  BEGIN
    EXECUTE statement;
  EXCEPTION WHEN OTHERS THEN
    GET STACKED DIAGNOSTICS actual_state=RETURNED_SQLSTATE, actual_constraint=CONSTRAINT_NAME;
    IF actual_state<>expected_state OR actual_constraint<>expected_constraint THEN
      RAISE EXCEPTION 'wrong authority rejection % / %, expected % / %',
        actual_state,actual_constraint,expected_state,expected_constraint;
    END IF;
    RETURN;
  END;
  RAISE EXCEPTION 'expected authority rejection absent: %',expected_state;
END;
$$;

SELECT pg_temp.check_fence();
SELECT pg_temp.reject('SELECT pg_temp.check_fence(runtime_delta=>1)','55000');
SELECT pg_temp.reject('SELECT pg_temp.check_fence(auth_delta=>1)','55000');
SELECT pg_temp.reject('SELECT pg_temp.check_fence(revision_delta=>1)','55000');
SELECT pg_temp.reject($q$SELECT pg_temp.check_fence(actor=>'other')$q$,'42501');
SELECT pg_temp.reject($q$SELECT pg_temp.check_fence(project=>'missing')$q$,'42501');
SELECT pg_temp.reject($q$UPDATE kehila.actors SET active=false WHERE actor_id='owner';
  SELECT pg_temp.check_fence()$q$,'55000');
SELECT pg_temp.reject($q$UPDATE kehila.project_grants SET active=false WHERE project_id='p';
  SELECT pg_temp.check_fence()$q$,'42501');
SELECT pg_temp.reject($q$UPDATE kehila.project_grants SET version=2 WHERE project_id='p';
  SELECT pg_temp.check_fence()$q$,'23514','item_grant_agreement');
SELECT pg_temp.reject($q$SELECT kehila_auth.item_write_fence(
  (installation_id,owner_actor_id,NULL,auth_generation,verifier_revision)::kehila_auth.authority,'p')
  FROM expected_authority$q$,'55000');
SELECT pg_temp.reject($q$SELECT kehila_auth.item_write_fence(
  (installation_id,owner_actor_id,runtime_generation,auth_generation,NULL)::kehila_auth.authority,'p')
  FROM expected_authority$q$,'55000');
SELECT pg_temp.reject($q$SELECT kehila_auth.item_write_fence(NULL::kehila_auth.authority,'p')$q$,'55000');
SELECT pg_temp.reject($q$SELECT kehila_auth.item_write_fence(
  (NULL,owner_actor_id,runtime_generation,auth_generation,verifier_revision)::kehila_auth.authority,'p')
  FROM expected_authority$q$,'55000');
SELECT pg_temp.reject($q$SELECT kehila_auth.item_write_fence(
  (installation_id,NULL,runtime_generation,auth_generation,verifier_revision)::kehila_auth.authority,'p')
  FROM expected_authority$q$,'55000');
SELECT pg_temp.reject($q$SELECT kehila_auth.item_write_fence(
  (installation_id,owner_actor_id,runtime_generation,NULL,verifier_revision)::kehila_auth.authority,'p')
  FROM expected_authority$q$,'55000');
SELECT pg_temp.reject($q$SELECT pg_temp.check_fence(project=>NULL)$q$,'42501');

-- The separate ability to create another project is not an existing item's
-- mutation grant. Mutable archive eligibility must follow old-success resolution.
BEGIN;
UPDATE kehila.actors SET can_create_project=false WHERE actor_id='owner';
SELECT pg_temp.check_fence();
UPDATE kehila.projects SET archived=true WHERE project_id='p';
SELECT pg_temp.check_fence();
ROLLBACK;
-- This helper stays private. Fixed SECURITY DEFINER command entries will own
-- its execution context; direct serving calls must fail before its body runs.
SELECT pg_temp.reject($q$SET LOCAL ROLE kehila_auth_runtime;
  SELECT kehila_auth.item_write_fence(NULL::kehila_auth.authority,'p')$q$,'42501');
SELECT pg_advisory_unlock(1262831689,1);
RESET ROLE;
SELECT 'PASS: private item write authority and coherent project grant' AS result;
