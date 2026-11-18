-- Closed SQL capability controls after selection_storage.sql/state observer.
-- Actual browser/session/HTTP and fault/restart evidence remain separate.
\set ON_ERROR_STOP on
SET statement_timeout='15s';
SET lock_timeout='3s';
SET ROLE kehila_owner;
SELECT pg_advisory_lock(1262831689,1);
SELECT * FROM kehila_auth.start_runtime('installation','selection-capability-start');
CREATE TEMP TABLE selection_authority AS
  SELECT ROW(i.installation_id,i.owner_actor_id,i.runtime_generation,c.auth_generation,c.verifier_revision)
    ::kehila_auth.authority AS proof
  FROM kehila_auth.installation i JOIN kehila_auth.owner_credentials c USING(owner_actor_id);
CREATE FUNCTION pg_temp.authority() RETURNS kehila_auth.authority LANGUAGE sql STABLE AS $$
  SELECT proof FROM selection_authority;
$$;
CREATE FUNCTION pg_temp.require(ok boolean,label text) RETURNS void LANGUAGE plpgsql AS $$
BEGIN IF ok IS DISTINCT FROM true THEN RAISE EXCEPTION 'selection authority: %',label; END IF; END;
$$;
CREATE FUNCTION pg_temp.reject(statement text,expected_state text) RETURNS void LANGUAGE plpgsql AS $$
DECLARE actual text;
BEGIN
  BEGIN EXECUTE statement;
  EXCEPTION WHEN OTHERS THEN
    GET STACKED DIAGNOSTICS actual=RETURNED_SQLSTATE;
    IF actual<>expected_state THEN RAISE EXCEPTION 'wrong authority error %, expected %',actual,expected_state; END IF;
    RETURN;
  END;
  RAISE EXCEPTION 'expected authority rejection absent';
END;
$$;

-- First expected-red baseline reaches the absent fixed capability, rather than
-- a fabricated authentication failure or malformed input.
SELECT pg_temp.require((SELECT outcome='absent' FROM kehila_auth.reconcile_selection(
  pg_temp.authority(),repeat('1',32)::kehila.operation_token,1::bigint,
  kehila.selection_request_bytes('owner',repeat('1',32),3,'p','saved-item'))),
  'unseen reconciliation is lookup-only');

CREATE FUNCTION pg_temp.proposal(token text,project text,item text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE head kehila.current_work_selections; version kehila.u64;
BEGIN
  SELECT * INTO STRICT head FROM kehila.current_work_selections WHERE user_id='owner';
  version:=head.selection_version+CASE WHEN (project,item)
    IS DISTINCT FROM (head.selected_project_id,head.selected_item_id) THEN 1 ELSE 0 END;
  RETURN jsonb_build_object('codec_version',1,'kind','selection_event','value',jsonb_build_object(
    'ordinal',(head.event_ordinal+1)::numeric(20,0)::text,'operation_id',token,
    'before',kehila.selection_state_value('owner',head.selection_version,head.selected_project_id,head.selected_item_id),
    'result',jsonb_build_object('codec_version',1,'kind','selection_result','value',
      kehila.selection_state_value('owner',version,project::kehila.ident,item::kehila.ident))));
END;
$$;
CREATE FUNCTION pg_temp.save(token text,expected numeric,project text,item text,event jsonb DEFAULT NULL)
RETURNS text LANGUAGE plpgsql AS $$
DECLARE outcome text;
BEGIN
  SELECT r.outcome INTO STRICT outcome FROM kehila_auth.save_selection(pg_temp.authority(),
    token::kehila.operation_token,expected::kehila.u64,
    kehila.selection_request_bytes('owner',token::kehila.operation_token,expected::kehila.u64,
      project::kehila.ident,item::kehila.ident),coalesce(event,pg_temp.proposal(token,project,item))) r;
  RETURN outcome;
END;
$$;

SELECT pg_temp.require(pg_temp.save(repeat('1',32),3,'p','saved-item')='accepted','set A accepted');
CREATE TEMP TABLE selection_stale_proposal AS SELECT pg_temp.proposal(repeat('5',32),'p','i') AS event;
SELECT pg_temp.require(pg_temp.save(repeat('2',32),4,'p','saved-item')='accepted','no-op A records success');
SELECT pg_temp.require(pg_temp.save(repeat('5',32),4,'p','i',(SELECT event FROM selection_stale_proposal))
  ='preparation_stale','event-only staleness is not a selection version conflict');
SELECT pg_temp.require(NOT EXISTS(SELECT FROM kehila.operations
  WHERE command_family='selection' AND token=repeat('5',32)),'stale preparation has no accepted operation');
SELECT pg_temp.require(pg_temp.save(repeat('3',32),4,'p','i')='accepted','set B accepted');
SELECT pg_temp.require((SELECT outcome='replay' FROM kehila_auth.reconcile_selection(pg_temp.authority(),
  repeat('1',32)::kehila.operation_token,1,kehila.selection_request_bytes('owner',repeat('1',32),3,'p','saved-item'))),
  'original A replays after later B');
SELECT pg_temp.require((SELECT p.result #>>'{value,requested,item_id}'='saved-item'
    AND p.result #>>'{value,version}'='4'
  FROM kehila.operations o JOIN kehila.operation_payloads p USING(operation_row_id)
  WHERE o.command_family='selection' AND o.token=repeat('1',32)),
  'original saved A result is retained');
SELECT pg_temp.require((SELECT selected_item_id='i' AND selection_version=5
  FROM kehila.current_work_selections WHERE user_id='owner'),'replay did not overwrite current B');
SELECT pg_temp.require((SELECT outcome='operation_id_reused' FROM kehila_auth.reconcile_selection(pg_temp.authority(),
  repeat('1',32)::kehila.operation_token,1,kehila.selection_request_bytes('owner',repeat('1',32),4,NULL,NULL))),
  'changed payload cannot reuse original operation');
SELECT pg_temp.require(pg_temp.save(repeat('6',32),4,'p','i')='version_conflict',
  'changed actual selection version rejects even when requested target is unchanged');

CREATE FUNCTION pg_temp.denied_original(token text,request bytea) RETURNS void LANGUAGE plpgsql AS $$
BEGIN
  UPDATE kehila.project_grants SET active=false WHERE project_id='p' AND actor_id='owner';
  PERFORM * FROM kehila_auth.reconcile_selection(pg_temp.authority(),token::kehila.operation_token,1,request);
END;
$$;
SELECT pg_temp.reject($q$SELECT pg_temp.denied_original(repeat('1',32),
  kehila.selection_request_bytes('owner',repeat('1',32),3,'p','saved-item'))$q$,'42501');
SELECT pg_temp.reject($q$SELECT pg_temp.denied_original(repeat('1',32),
  kehila.selection_request_bytes('owner',repeat('1',32),4,NULL,NULL))$q$,'42501');

-- Simulated revoked stored grant, not an implemented administration route.
-- Roll back both fixture revocation and clear to verify no residual acceptance.
BEGIN;
UPDATE kehila.project_grants SET active=false WHERE project_id='p' AND actor_id='owner';
SELECT pg_temp.require(pg_temp.save(repeat('4',32),5,NULL,NULL)='accepted',
  'self-clear does not require former target permission');
ROLLBACK;
SELECT pg_temp.require(NOT EXISTS(SELECT FROM kehila.operations
  WHERE command_family='selection' AND token=repeat('4',32)),'explicit rollback restores selection and success together');
SELECT pg_temp.require(pg_temp.save(repeat('4',32),5,NULL,NULL)='accepted','durable clear accepted');

-- Existing synthetic expired operation still reauthorizes ORIGINAL target.
SELECT pg_temp.require((SELECT outcome='replay_expired' FROM kehila_auth.reconcile_selection(pg_temp.authority(),
  repeat('d',32)::kehila.operation_token,1,kehila.selection_request_bytes('owner',repeat('d',32),0,'p','saved-item'))),
  'expired identity is not executable');
SELECT pg_temp.reject($q$SELECT pg_temp.denied_original(repeat('d',32),
  kehila.selection_request_bytes('owner',repeat('d',32),0,'p','saved-item'))$q$,'42501');

BEGIN;
UPDATE kehila.operations SET payload_retired=true WHERE command_family='selection' AND token=repeat('1',32);
DELETE FROM kehila.operation_payloads WHERE operation_row_id IN
  (SELECT operation_row_id FROM kehila.operations WHERE command_family='selection' AND token=repeat('1',32));
SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;
SELECT pg_temp.require((SELECT outcome='replay_expired' FROM kehila_auth.reconcile_selection(pg_temp.authority(),
  repeat('1',32)::kehila.operation_token,1,kehila.selection_request_bytes('owner',repeat('1',32),3,'p','saved-item'))),
  'retired identity keeps original authorization scope');
SELECT pg_temp.reject($q$SELECT pg_temp.denied_original(repeat('1',32),
  kehila.selection_request_bytes('owner',repeat('1',32),3,'p','saved-item'))$q$,'42501');

-- Isolate backward-clock handling with otherwise coherent live success state.
-- This synthetic future origin and its event/head are rolled back together.
SELECT pg_temp.require((SELECT outcome='replay' FROM kehila_auth.reconcile_selection(pg_temp.authority(),
  repeat('4',32)::kehila.operation_token,1,kehila.selection_request_bytes('owner',repeat('4',32),5,NULL,NULL))),
  'ordinary live clear remains replayable before the clock control');
BEGIN;
DO $$
DECLARE operation bigint; origin kehila.u64:=kehila_auth.trusted_millis()+86400000;
  event jsonb:=pg_temp.proposal(repeat('8',32),NULL,NULL);
  request bytea:=kehila.selection_request_bytes('owner',repeat('8',32),6,NULL,NULL);
BEGIN
  INSERT INTO kehila.operations(actor_id,command_family,target_kind,target_user_id,token,
    request_codec_version,request_sha256,required_grant,grant_policy_version,replay_origin_ms,replay_deadline_ms)
    VALUES('owner','selection','user','owner',repeat('8',32),1,sha256(request),'selection',1,
      origin,origin+7776000000) RETURNING operation_row_id INTO operation;
  INSERT INTO kehila.operation_payloads(operation_row_id,request_bytes,result_codec_version,result)
    VALUES(operation,request,1,event #>'{value,result}');
  INSERT INTO kehila.selection_events(user_id,event_ordinal,previous_event_ordinal,actor_id,command_family,
    target_kind,token,operation_row_id,before_version,before_project_id,before_item_id,selection_version,
    selected_project_id,selected_item_id,history_codec_version,history)
    VALUES('owner',11,10,'owner','selection','user',repeat('8',32),operation,6,NULL,NULL,6,NULL,NULL,1,event);
  UPDATE kehila.current_work_selections SET event_ordinal=11 WHERE user_id='owner';
END;
$$;
SET CONSTRAINTS ALL IMMEDIATE;
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.reconcile_selection(pg_temp.authority(),
  repeat('8',32)::kehila.operation_token,1,
  kehila.selection_request_bytes('owner',repeat('8',32),6,NULL,NULL))$q$,'55000');
ROLLBACK;

CREATE TEMP TABLE selection_runtime_proposal AS SELECT repeat('7',32)::kehila.operation_token AS token,
  6::kehila.u64 AS expected,kehila.selection_request_bytes('owner',repeat('7',32),6,NULL,NULL) AS request,
  pg_temp.proposal(repeat('7',32),NULL,NULL) AS event;
GRANT SELECT ON selection_authority,selection_runtime_proposal TO kehila_auth_runtime;
GRANT EXECUTE ON FUNCTION pg_temp.authority(),pg_temp.require(boolean,text),pg_temp.reject(text,text)
  TO kehila_auth_runtime;
SET ROLE kehila_auth_runtime;
SELECT pg_temp.require((SELECT outcome='accepted' FROM selection_runtime_proposal p,
  LATERAL kehila_auth.save_selection(pg_temp.authority(),p.token,p.expected,p.request,p.event)),
  'actual runtime role can record an unchanged clear through the fixed capability');
SELECT pg_temp.reject($q$INSERT INTO kehila.current_work_selections
  SELECT * FROM kehila.current_work_selections$q$,'42501');
SELECT pg_temp.reject($q$UPDATE kehila.current_work_selections SET selection_version=selection_version$q$,'42501');
RESET ROLE;
