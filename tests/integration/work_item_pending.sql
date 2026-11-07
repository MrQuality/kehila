-- Tests-first fixed registration/cancellation contract. Prepared for the fresh
-- owner-authored overlay; runtime login/session/concurrency are separate gates.
-- Constructed past timestamps exercise expiry branches, not elapsed-time proof.
\set ON_ERROR_STOP on
SET statement_timeout = '15s';
SET lock_timeout = '3s';
SET idle_in_transaction_session_timeout = '15s';
SET ROLE kehila_owner;
SELECT pg_advisory_lock(1262831689,1);
SELECT * FROM kehila_auth.start_runtime('installation','item-pending-start');

CREATE FUNCTION pg_temp.authority() RETURNS kehila_auth.authority LANGUAGE sql AS $$
  SELECT ROW(i.installation_id,i.owner_actor_id,i.runtime_generation,
    c.auth_generation,c.verifier_revision)::kehila_auth.authority
  FROM kehila_auth.installation i JOIN kehila_auth.owner_credentials c USING(owner_actor_id);
$$;
CREATE FUNCTION pg_temp.require(ok boolean,label text) RETURNS void LANGUAGE plpgsql AS $$
BEGIN
  IF ok IS DISTINCT FROM true THEN RAISE EXCEPTION 'pending contract: %',label; END IF;
END;
$$;
CREATE FUNCTION pg_temp.reject(statement text,expected_state text) RETURNS void LANGUAGE plpgsql AS $$
DECLARE actual_state text;
BEGIN
  BEGIN
    EXECUTE statement;
  EXCEPTION WHEN OTHERS THEN
    GET STACKED DIAGNOSTICS actual_state=RETURNED_SQLSTATE;
    IF actual_state<>expected_state THEN
      RAISE EXCEPTION 'wrong pending rejection %, expected %',actual_state,expected_state;
    END IF;
    RETURN;
  END;
  RAISE EXCEPTION 'expected pending rejection absent: %',expected_state;
END;
$$;
CREATE FUNCTION pg_temp.request_bytes(seed integer,project text DEFAULT 'p') RETURNS bytea LANGUAGE sql AS $$
  SELECT convert_to('{"codec_version":1,"kind":"item_create_request","value":{"estimate":null,'
    '"expected_configuration_revision":"1","item_id":"pending-'||seed||'","operation_id":"'
    ||lpad(to_hex(seed),32,'0')||'","project_id":"'||project||'","type_id":"task","values":[],"workflow_id":null}}','UTF8');
$$;
CREATE FUNCTION pg_temp.register(seed integer,codec bigint DEFAULT 1,bytes bytea DEFAULT NULL,
  project text DEFAULT 'p',runtime_delta integer DEFAULT 0)
RETURNS bigint LANGUAGE plpgsql AS $$
DECLARE created_id bigint; expected kehila_auth.authority;
BEGIN
  expected := pg_temp.authority();
  expected.runtime_generation := expected.runtime_generation+runtime_delta;
  SELECT r.request_id INTO STRICT created_id FROM kehila_auth.register_item(
    expected,project::kehila.ident,lpad(to_hex(seed),32,'0')::kehila_auth.intent_key,
    lpad(to_hex(seed),32,'0')::kehila_auth.intent_key,('pending-'||seed)::kehila.ident,
    codec,coalesce(bytes,pg_temp.request_bytes(seed,project))) r;
  RETURN created_id;
END;
$$;
CREATE FUNCTION pg_temp.stale_authority() RETURNS kehila_auth.authority LANGUAGE sql AS $$
  SELECT ROW(i.installation_id,i.owner_actor_id,i.runtime_generation+1,
    c.auth_generation,c.verifier_revision)::kehila_auth.authority
  FROM kehila_auth.installation i JOIN kehila_auth.owner_credentials c USING(owner_actor_id);
$$;

\ir /opt/kehila-work-item-project-fixture.sql

BEGIN;
SELECT pg_temp.register(1);
SELECT pg_temp.register(2);
SELECT pg_temp.require((SELECT count(*)=2 AND bool_and(state='prepared'
    AND operation_row_id IS NULL AND request_codec_version=1
    AND request_sha256=sha256(request_bytes) AND expires_ms-registered_ms=86400000)
  FROM kehila_auth.pending_items WHERE target_item_id IN ('pending-1','pending-2')),
  'independent registrations store exact immutable intent and bounded lifetime');
SELECT pg_temp.require((SELECT request_bytes=pg_temp.request_bytes(1)
  FROM kehila_auth.pending_items WHERE target_item_id='pending-1'),'retained request bytes');
SELECT pg_temp.reject('SELECT pg_temp.register(1)','23505');
SELECT pg_temp.reject('SELECT pg_temp.register(3,runtime_delta=>1)','55000');
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.cancel_item(pg_temp.stale_authority(),'p',repeat('0',31)||'1')$q$,'55000');
SELECT pg_temp.reject($q$UPDATE kehila.project_grants SET active=false WHERE project_id='p';
  SELECT pg_temp.register(3)$q$,'42501');
SELECT pg_temp.reject($q$UPDATE kehila.project_grants SET active=false WHERE project_id='p';
  SELECT * FROM kehila_auth.cancel_item(pg_temp.authority(),'p',repeat('0',31)||'1')$q$,'42501');
SELECT pg_temp.reject('SELECT pg_temp.register(3,codec=>2)','22023');
SELECT pg_temp.reject($q$SELECT pg_temp.register(3,bytes=>''::bytea)$q$,'22023');
SELECT pg_temp.reject($q$SELECT pg_temp.register(3,bytes=>decode(repeat('61',1048577),'hex'))$q$,'22023');
SELECT pg_temp.require((SELECT state='cancelled' AND operation_row_id IS NULL
    FROM kehila_auth.cancel_item(pg_temp.authority(),'p',repeat('0',31)||'1')),
  'cancel prepared request');
SELECT pg_temp.require((SELECT state='cancelled'
    FROM kehila_auth.cancel_item(pg_temp.authority(),'p',repeat('0',31)||'1')),
  'repeat cancellation does not recreate or execute');
SELECT pg_temp.require((SELECT count(*)=0
    FROM kehila_auth.cancel_item(pg_temp.authority(),'p',repeat('f',32))),
  'missing identity stays missing');
SELECT pg_temp.require((SELECT state='completed' AND target_item_id='i' AND operation_row_id IS NOT NULL
    FROM kehila_auth.cancel_item(pg_temp.authority(),'p',repeat('b',32))),
  'completed identity never compensated');
SELECT pg_temp.require((SELECT request_bytes IS NULL FROM kehila_auth.pending_items
  WHERE target_item_id='pending-1'),'cancel clears request content');
SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;

-- Owner-authored old registration isolates physical expiry without changing an
-- immutable live row or asking the application to accept a caller clock.
-- The guard owns classification at its final clock sample. Cancellation intent
-- cannot become an integrity error merely by crossing expiry before that sample.
BEGIN;
INSERT INTO kehila_auth.pending_items(installation_id,actor_id,project_id,intent_handle,
  token,target_item_id,request_codec_version,request_sha256,request_bytes,registered_ms,expires_ms)
VALUES ('installation','owner','p',lpad(to_hex(501),32,'0'),lpad(to_hex(501),32,'0'),
  'pending-501',1,sha256(pg_temp.request_bytes(501)),pg_temp.request_bytes(501),0,60000);
UPDATE kehila_auth.pending_items SET state='cancelled',request_bytes=NULL WHERE target_item_id='pending-501';
SELECT pg_temp.require((SELECT state='expired' AND request_bytes IS NULL
  FROM kehila_auth.pending_items WHERE target_item_id='pending-501'),
  'cancellation intent is classified by the storage guard clock');
ROLLBACK;

BEGIN;
INSERT INTO kehila_auth.pending_items(installation_id,actor_id,project_id,intent_handle,
  token,target_item_id,request_codec_version,request_sha256,request_bytes,registered_ms,expires_ms)
VALUES ('installation','owner','p',lpad(to_hex(500),32,'0'),lpad(to_hex(500),32,'0'),
  'pending-500',1,sha256(pg_temp.request_bytes(500)),pg_temp.request_bytes(500),0,60000);
SELECT pg_temp.require((SELECT state='expired'
  FROM kehila_auth.cancel_item(pg_temp.authority(),'p',lpad(to_hex(500),32,'0'))),
  'elapsed prepared cancellation reports expired');
SELECT pg_temp.require((SELECT request_bytes IS NULL FROM kehila_auth.pending_items
  WHERE target_item_id='pending-500'),'expiry clears request content');
ROLLBACK;

BEGIN;
-- The already committed pending-2 plus 31 new requests fill the owner budget.
SELECT pg_temp.register(n) FROM generate_series(10,40) n;
SELECT pg_temp.require((SELECT count(*)=32 FROM kehila_auth.pending_items
  WHERE state='prepared'),'prepared count exactly at containment limit');
SELECT pg_temp.reject('SELECT pg_temp.register(41)','54000');
ROLLBACK;

BEGIN;
SELECT pg_temp.second_project();
SET CONSTRAINTS ALL IMMEDIATE;
SELECT pg_temp.require((SELECT count(*)=0 FROM kehila_auth.cancel_item(
    pg_temp.authority(),'p2',repeat('0',31)||'2')),
  'wrong project cannot find or cancel another project request');
INSERT INTO kehila_auth.pending_items(installation_id,actor_id,project_id,intent_handle,
  token,target_item_id,request_codec_version,request_sha256,request_bytes,registered_ms,expires_ms)
VALUES ('installation','owner','p2',lpad(to_hex(500),32,'0'),lpad(to_hex(500),32,'0'),
  'pending-500',1,sha256(pg_temp.request_bytes(500,'p2')),pg_temp.request_bytes(500,'p2'),0,60000);
-- One live p request plus one elapsed p2 request and 30 new p requests = 32.
SELECT pg_temp.register(n) FROM generate_series(10,39) n;
SELECT pg_temp.reject('SELECT pg_temp.register(40)','54000');
SELECT pg_temp.require((SELECT state='prepared' AND request_bytes IS NOT NULL
  FROM kehila_auth.pending_items WHERE project_id='p2' AND target_item_id='pending-500'),
  'registration does not maintain another project and elapsed bytes consume quota');
SELECT pg_temp.require((SELECT state='expired' FROM kehila_auth.cancel_item(
    pg_temp.authority(),'p2',lpad(to_hex(500),32,'0'))),
  'explicit action in the other project clears elapsed prepared content');
SELECT pg_temp.register(40);
SELECT pg_temp.require((SELECT count(*)=32 FROM kehila_auth.pending_items WHERE state='prepared'),
  'owner capacity becomes available after other-project expiry');
ROLLBACK;

SELECT pg_temp.require((SELECT count(*)=1 FROM kehila.items),'registration never inserts an item');
SELECT pg_temp.require((SELECT count(*)=5 FROM kehila.operations),'registration never accepts an operation');
SELECT pg_temp.require((SELECT next_sequence=2 FROM kehila.projects WHERE project_id='p'),
  'registration never consumes a readable sequence');
SELECT pg_advisory_unlock(1262831689,1);
RESET ROLE;
SELECT 'PASS: bounded item registration, cancellation and prepared capacity' AS result;
