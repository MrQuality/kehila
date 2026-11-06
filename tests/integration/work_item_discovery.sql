-- Owner-level read contracts; no application session or typed request claim.
\set ON_ERROR_STOP on
SET statement_timeout='15s';
SET lock_timeout='3s';
SET ROLE kehila_owner;
\ir /opt/kehila-work-item-project-fixture.sql
CREATE FUNCTION pg_temp.require(ok boolean,label text) RETURNS void LANGUAGE plpgsql AS $$
BEGIN IF ok IS DISTINCT FROM true THEN RAISE EXCEPTION 'discovery: %',label; END IF; END;
$$;
CREATE FUNCTION pg_temp.reject(statement text,expected_state text) RETURNS void LANGUAGE plpgsql AS $$
DECLARE actual text;
BEGIN
  BEGIN EXECUTE statement;
  EXCEPTION WHEN OTHERS THEN
    GET STACKED DIAGNOSTICS actual=RETURNED_SQLSTATE;
    IF actual<>expected_state THEN RAISE EXCEPTION 'wrong discovery rejection %, expected %',actual,expected_state; END IF;
    RETURN;
  END;
  RAISE EXCEPTION 'expected discovery rejection absent: %',expected_state;
END;
$$;
CREATE TEMP TABLE expected_read AS SELECT i.installation_id,i.owner_actor_id,i.runtime_generation,
  c.auth_generation,c.verifier_revision FROM kehila_auth.installation i
  JOIN kehila_auth.owner_credentials c USING(owner_actor_id);
CREATE FUNCTION pg_temp.expected() RETURNS kehila_auth.authority LANGUAGE sql STABLE AS $$
  SELECT ROW(e.*)::kehila_auth.authority FROM expected_read e;
$$;
CREATE FUNCTION pg_temp.bad_expected(field text,value jsonb)
RETURNS kehila_auth.authority LANGUAGE sql STABLE AS $$
  SELECT jsonb_populate_record(NULL::kehila_auth.authority,to_jsonb(e)||jsonb_build_object(field,value))
    FROM expected_read e;
$$;

BEGIN;
SELECT pg_temp.second_project();
INSERT INTO kehila_auth.pending_items(installation_id,actor_id,project_id,intent_handle,token,
  target_item_id,request_codec_version,request_sha256,request_bytes,registered_ms,expires_ms)
  SELECT 'installation','owner','p',lpad(to_hex(n),32,'0'),lpad(to_hex(n),32,'0'),
    'listed-'||n,1,sha256(convert_to('{}','UTF8')),convert_to('{}','UTF8'),
    kehila_auth.trusted_millis(),kehila_auth.trusted_millis()+86400000
  FROM generate_series(6001,6020)n;
INSERT INTO kehila_auth.pending_items(installation_id,actor_id,project_id,intent_handle,token,
  target_item_id,request_codec_version,request_sha256,request_bytes,registered_ms,expires_ms)
VALUES ('installation','owner','p',repeat('e',32),repeat('e',32),'read-expired',1,
    sha256(convert_to('{}','UTF8')),convert_to('{}','UTF8'),0,60000),
  ('installation','owner','p',repeat('f',32),repeat('f',32),'read-big',1,
    sha256(convert_to(repeat('x',1048576),'UTF8')),convert_to(repeat('x',1048576),'UTF8'),
    kehila_auth.trusted_millis(),kehila_auth.trusted_millis()+86400000),
  ('installation','owner','p2',repeat('d',32),repeat('d',32),'read-other',1,
    sha256(convert_to('{}','UTF8')),convert_to('{}','UTF8'),
    kehila_auth.trusted_millis(),kehila_auth.trusted_millis()+86400000);
SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;
-- Scalar row-version witness avoids copying large bytea into JSON just to
-- establish that successful read calls never rewrote the pending collection.
CREATE TEMP TABLE before_discovery AS SELECT request_id,xmin::text AS row_version,
  state,request_sha256,request_bytes IS NULL AS content_absent FROM kehila_auth.pending_items;

BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SELECT pg_temp.require((SELECT count(*)=17 FROM kehila_auth.list_pending_items(pg_temp.expected(),'p',NULL)),
  'page plus one sentinel');
DO $$
DECLARE first_ids bigint[]; second_ids bigint[]; field text;
BEGIN
  SELECT array_agg(request_id ORDER BY request_id DESC) INTO first_ids
    FROM kehila_auth.list_pending_items(pg_temp.expected(),'p',NULL);
  SELECT array_agg(request_id ORDER BY request_id DESC) INTO second_ids
    FROM kehila_auth.list_pending_items(pg_temp.expected(),'p',first_ids[16]);
  PERFORM pg_temp.require(cardinality(second_ids)=9 AND second_ids[1]=first_ids[17],
    'cursor uses last displayed row, preserving the sentinel for the next page');
  PERFORM pg_temp.require((SELECT count(*)=0 FROM kehila_auth.list_pending_items(pg_temp.expected(),'p',1)),
    'exclusive cursor boundary');
  PERFORM pg_temp.require((SELECT count(*)=1 FROM kehila_auth.list_pending_items(pg_temp.expected(),'p2',NULL)),
    'authorized project list remains scoped');
  FOREACH field IN ARRAY ARRAY['installation_id','owner_actor_id','runtime_generation','auth_generation','verifier_revision'] LOOP
    PERFORM pg_temp.reject(format('SELECT * FROM kehila_auth.list_pending_items(pg_temp.bad_expected(%L,''null''::jsonb),''p'',NULL)',field),'55000');
    PERFORM pg_temp.reject(format('SELECT * FROM kehila_auth.open_pending_item(pg_temp.bad_expected(%L,''null''::jsonb),''p'',repeat(''f'',32))',field),'55000');
  END LOOP;
END;
$$;
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.list_pending_items(pg_temp.expected(),'p',0)$q$,'22023');
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.list_pending_items(pg_temp.bad_expected('owner_actor_id','"other"'),'p',NULL)$q$,'42501');
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.list_pending_items(pg_temp.bad_expected('runtime_generation',to_jsonb((pg_temp.expected()).runtime_generation+1)),'p',NULL)$q$,'55000');
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.list_pending_items(pg_temp.bad_expected('auth_generation',to_jsonb((pg_temp.expected()).auth_generation+1)),'p',NULL)$q$,'55000');
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.list_pending_items(pg_temp.bad_expected('verifier_revision',to_jsonb((pg_temp.expected()).verifier_revision+1)),'p',NULL)$q$,'55000');
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.open_pending_item(pg_temp.bad_expected('owner_actor_id','"other"'),'p',repeat('f',32))$q$,'42501');
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.open_pending_item(pg_temp.bad_expected('runtime_generation',to_jsonb((pg_temp.expected()).runtime_generation+1)),'p',repeat('f',32))$q$,'55000');
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.open_pending_item(pg_temp.bad_expected('auth_generation',to_jsonb((pg_temp.expected()).auth_generation+1)),'p',repeat('f',32))$q$,'55000');
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.open_pending_item(pg_temp.bad_expected('verifier_revision',to_jsonb((pg_temp.expected()).verifier_revision+1)),'p',repeat('f',32))$q$,'55000');
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.list_pending_items(pg_temp.bad_expected('installation_id','"missing"'),'p',NULL)$q$,'55000');
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.open_pending_item(pg_temp.bad_expected('installation_id','"missing"'),'p',repeat('f',32))$q$,'55000');
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.list_pending_items(pg_temp.expected(),'missing',NULL)$q$,'42501');
SELECT pg_temp.require((SELECT count(*)=0 FROM kehila_auth.open_pending_item(pg_temp.expected(),'p',repeat('d',32))),
  'another project locator is not authority');
SELECT pg_temp.require((SELECT count(*)=0 FROM kehila_auth.open_pending_item(pg_temp.expected(),'p',repeat('a',32))),
  'missing locator remains missing');
SELECT pg_temp.require((SELECT state='expired' AND request_bytes IS NULL
  FROM kehila_auth.open_pending_item(pg_temp.expected(),'p',repeat('e',32))),
  'effective expiry omits content');
SELECT pg_temp.require((SELECT state='prepared' AND octet_length(request_bytes)=1048576
  FROM kehila_auth.open_pending_item(pg_temp.expected(),'p',repeat('f',32))),
  'individual SQL byte ceiling is inclusive; typed decoding is separate');
SELECT pg_temp.require((SELECT state='cancelled' AND request_bytes IS NULL
  FROM kehila_auth.open_pending_item(pg_temp.expected(),'p',repeat('0',31)||'1')),
  'cancelled identity stays terminal');
SELECT pg_temp.require((SELECT state='completed' AND operation_row_id IS NOT NULL AND request_bytes IS NULL
  FROM kehila_auth.open_pending_item(pg_temp.expected(),'p',repeat('b',32))),
  'completed pruned identity remains discoverable');
SELECT pg_temp.require((SELECT request_bytes=convert_to('{}','UTF8') FROM kehila_auth.open_pending_item(
  pg_temp.expected(),'p2',repeat('d',32))),'authorized individual open');
SELECT pg_temp.require(NOT EXISTS(
  (SELECT request_id,xmin::text,state,request_sha256,request_bytes IS NULL FROM kehila_auth.pending_items
    EXCEPT SELECT * FROM before_discovery)
  UNION ALL (SELECT * FROM before_discovery EXCEPT
    SELECT request_id,xmin::text,state,request_sha256,request_bytes IS NULL FROM kehila_auth.pending_items)),
  'successful discovery leaves every pending row version unchanged');
SELECT pg_temp.require((SELECT state='prepared' AND request_bytes IS NOT NULL FROM kehila_auth.pending_items
  WHERE target_item_id='read-expired'),'effective expiry does not perform maintenance');
COMMIT;

SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.list_pending_items(pg_temp.expected(),'p',NULL)$q$,'25000');
BEGIN ISOLATION LEVEL REPEATABLE READ;
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.open_pending_item(pg_temp.expected(),'p',repeat('f',32))$q$,'25000');
ROLLBACK;

BEGIN;
UPDATE kehila.project_grants SET active=false WHERE project_id='p';
COMMIT;
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.list_pending_items(pg_temp.expected(),'p',NULL)$q$,'42501');
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.open_pending_item(pg_temp.expected(),'p',repeat('f',32))$q$,'42501');
COMMIT;
UPDATE kehila.project_grants SET active=true WHERE project_id='p';
UPDATE kehila.project_grants SET version=2 WHERE project_id='p';
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.list_pending_items(pg_temp.expected(),'p',NULL)$q$,'23514');
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.open_pending_item(pg_temp.expected(),'p',repeat('f',32))$q$,'23514');
COMMIT;
UPDATE kehila.project_grants SET version=1 WHERE project_id='p';

-- Deliberate owner corruption in a disposable fixture qualifies reader
-- classification/bounded projection, not protection against privileged DDL.
BEGIN;
SET CONSTRAINTS ALL IMMEDIATE;
ALTER TABLE kehila_auth.pending_items DROP CONSTRAINT pending_item_request_bound;
ALTER TABLE kehila_auth.pending_items DISABLE TRIGGER pending_item_transition;
UPDATE kehila_auth.pending_items SET request_bytes=convert_to(repeat('x',1048577),'UTF8'),
  request_sha256=sha256(convert_to(repeat('x',1048577),'UTF8')) WHERE target_item_id='read-big';
ALTER TABLE kehila_auth.pending_items ENABLE TRIGGER pending_item_transition;
COMMIT;
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SELECT pg_temp.require((SELECT count(*)=17 FROM kehila_auth.list_pending_items(pg_temp.expected(),'p',NULL)),
  'summary list remains available with oversized stored content');
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.open_pending_item(pg_temp.expected(),'p',repeat('f',32))$q$,'23514');
COMMIT;
BEGIN;
SET CONSTRAINTS ALL IMMEDIATE;
ALTER TABLE kehila_auth.pending_items DISABLE TRIGGER pending_item_transition;
UPDATE kehila_auth.pending_items SET request_bytes=convert_to(repeat('x',1048576),'UTF8'),
  request_sha256=sha256(convert_to(repeat('x',1048576),'UTF8')) WHERE target_item_id='read-big';
ALTER TABLE kehila_auth.pending_items ENABLE TRIGGER pending_item_transition;
ALTER TABLE kehila_auth.pending_items ADD CONSTRAINT pending_item_request_bound
  CHECK(request_bytes IS NULL OR octet_length(request_bytes) BETWEEN 1 AND 1048576);
COMMIT;

CREATE TEMP TABLE original_small_request AS SELECT request_bytes FROM kehila_auth.pending_items
  WHERE target_item_id='pending-2';
BEGIN;
SET CONSTRAINTS ALL IMMEDIATE;
ALTER TABLE kehila_auth.pending_items DROP CONSTRAINT pending_item_state_shape;
ALTER TABLE kehila_auth.pending_items DISABLE TRIGGER pending_item_transition;
UPDATE kehila_auth.pending_items SET request_bytes=NULL WHERE target_item_id='pending-2';
ALTER TABLE kehila_auth.pending_items ENABLE TRIGGER pending_item_transition;
COMMIT;
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.open_pending_item(
  pg_temp.expected(),'p','00000000000000000000000000000002')$q$,'23514');
COMMIT;
BEGIN;
SET CONSTRAINTS ALL IMMEDIATE;
ALTER TABLE kehila_auth.pending_items DISABLE TRIGGER pending_item_transition;
UPDATE kehila_auth.pending_items SET request_bytes=(SELECT request_bytes FROM original_small_request)
  WHERE target_item_id='pending-2';
ALTER TABLE kehila_auth.pending_items ENABLE TRIGGER pending_item_transition;
ALTER TABLE kehila_auth.pending_items ADD CONSTRAINT pending_item_state_shape CHECK(
  (state='prepared' AND request_bytes IS NOT NULL AND operation_row_id IS NULL)
  OR (state IN ('cancelled','expired') AND request_bytes IS NULL AND operation_row_id IS NULL)
  OR (state='completed' AND operation_row_id IS NOT NULL));
COMMIT;

-- Reconstruct the legitimate interim retired-core/retained-content state only
-- in this owner fixture, then prune it using the normal irreversible guard.
BEGIN;
SET CONSTRAINTS ALL IMMEDIATE;
ALTER TABLE kehila_auth.pending_items DISABLE TRIGGER pending_item_transition;
UPDATE kehila_auth.pending_items SET request_bytes=convert_to('{"codec_version":1,"kind":"item_create_request","value":{"estimate":"0","expected_configuration_revision":"1","item_id":"i","operation_id":"op","project_id":"p","type_id":"task","values":[],"workflow_id":null}}','UTF8')
  WHERE target_item_id='i';
ALTER TABLE kehila_auth.pending_items ENABLE TRIGGER pending_item_transition;
COMMIT;
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SELECT pg_temp.require((SELECT state='completed' AND request_bytes IS NOT NULL
  FROM kehila_auth.open_pending_item(pg_temp.expected(),'p',repeat('b',32))),
  'retired completed identity permits temporarily retained content');
COMMIT;
UPDATE kehila_auth.pending_items SET request_bytes=NULL WHERE target_item_id='i';
RESET ROLE;
SELECT 'PASS: read-only scoped pending discovery and bounded individual open' AS result;
