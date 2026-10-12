-- Run after postgresql_foundation.sql in the same administrator session.
-- Deliberately small structural fixture, not evidence of the complete M1 seed.
CREATE ROLE foundation_serving LOGIN IN ROLE kehila_creator;
SET SESSION AUTHORIZATION foundation_serving;
SELECT pg_temp.require((SELECT count(*) = 1 FROM kehila.schema_migrations WHERE version = 1),
                       'serving can discover the installed schema version');
SELECT pg_temp.reject($q$INSERT INTO kehila.schema_migrations(version,description)
  VALUES (2,'forbidden')$q$, '42501');
SELECT pg_temp.reject($q$UPDATE kehila.schema_migrations SET description = 'forbidden'$q$, '42501');
SELECT pg_temp.reject($q$DELETE FROM kehila.schema_migrations$q$, '42501');
SELECT pg_temp.reject($q$TRUNCATE kehila.schema_migrations$q$, '42501');
SELECT pg_temp.require((SELECT actor_id = 'actor' FROM kehila.lock_actor('actor')),
                       'serving actor lock');
BEGIN;
INSERT INTO kehila.operations
  (actor_id,command_family,target_kind,target_project_id,token,
   request_codec_version,request_sha256,required_grant,grant_policy_version,
   replay_origin_ms,replay_deadline_ms)
VALUES ('actor','project_create','project','created','create',1,
        decode(repeat('01',32),'hex'),'project_create',1,200,7776000200)
RETURNING operation_row_id \gset
INSERT INTO kehila.operation_payloads VALUES (:operation_row_id,'\x01',1,'{}');
INSERT INTO kehila.projects
  (project_id,name,current_prefix,estimate_unit,configuration_revision,
   next_sequence,created_by,creation_operation_row_id,seed_profile)
VALUES ('created','Structure fixture','SF','hours',1,1,'actor',:operation_row_id,'m1_v1');
INSERT INTO kehila.configuration_revisions VALUES
  ('created',1,1,'{}','{}','actor',:operation_row_id,'project_create','project');
INSERT INTO kehila.project_grants VALUES ('created','actor','initial_owner',true,1);
INSERT INTO kehila.project_grant_events
  (project_id,actor_id,version,access_profile,active,granted_by,
   granting_operation_row_id,command_family)
VALUES ('created','actor',1,'initial_owner',true,'actor',:operation_row_id,'project_create');
INSERT INTO kehila.statuses VALUES
  ('created','new','New','new',false,0), ('created','done','Done','done',false,1);
INSERT INTO kehila.workflows VALUES ('created','flow','new','new',false,0);
INSERT INTO kehila.workflow_statuses VALUES
  ('created','flow','new',0), ('created','flow','done',1);
INSERT INTO kehila.workflow_phase_changes VALUES ('created','flow','new','done',0);
INSERT INTO kehila.work_item_types VALUES ('created','task','flow','title',false,0);
INSERT INTO kehila.type_workflows VALUES ('created','task','flow',0);
INSERT INTO kehila.fields VALUES
  ('created','title','task','Title','text','application','optional',false,0);
SET CONSTRAINTS ALL IMMEDIATE;
SELECT pg_temp.require((SELECT operation_row_id = :operation_row_id
  FROM kehila.lock_operation(:operation_row_id)), 'serving replay lock');
COMMIT;

SELECT pg_temp.reject($q$SET ROLE kehila_owner$q$, '42501');
SELECT pg_temp.reject($q$CREATE TABLE kehila.untrusted(id integer)$q$, '42501');
SELECT pg_temp.reject($q$UPDATE kehila.actors SET active = false$q$, '42501');
SELECT pg_temp.reject($q$INSERT INTO kehila.actors VALUES ('other',true,true)$q$, '42501');
SELECT pg_temp.reject($q$UPDATE kehila.operation_payloads SET result = '{}'$q$, '42501');
SELECT pg_temp.reject($q$DELETE FROM kehila.operation_payloads$q$, '42501');
SELECT pg_temp.reject($q$UPDATE kehila.operations SET payload_retired = true$q$, '42501');
SELECT pg_temp.reject($q$TRUNCATE kehila.operations CASCADE$q$, '42501');
SELECT pg_temp.reject($q$UPDATE kehila.project_grants SET active = false$q$, '42501');
SELECT pg_temp.reject($q$DELETE FROM kehila.configuration_revisions$q$, '42501');
RESET SESSION AUTHORIZATION;

SELECT pg_temp.reject($q$UPDATE kehila.configuration_revisions SET revision = 2$q$,
                       '23514', 'creation_history_immutable');
SELECT pg_temp.reject($q$TRUNCATE kehila.project_grant_events$q$,
                       '23514', 'creation_history_immutable');
SELECT pg_temp.reject($q$UPDATE kehila.projects SET configuration_revision = 2;
  SET CONSTRAINTS ALL IMMEDIATE$q$, '23503', 'projects_current_revision');
SELECT pg_temp.reject($q$DELETE FROM kehila.workflow_statuses WHERE status_id = 'new';
  SET CONSTRAINTS ALL IMMEDIATE$q$, '23503', 'workflow_initial_membership');
SELECT pg_temp.reject($q$DELETE FROM kehila.type_workflows;
  SET CONSTRAINTS ALL IMMEDIATE$q$, '23503', 'type_default_membership');
SELECT pg_temp.reject($q$UPDATE kehila.fields SET owner_type_id = 'missing'$q$, '23503');
SELECT pg_temp.reject($q$
  INSERT INTO kehila.work_item_types VALUES ('created','other','flow',NULL,false,1);
  INSERT INTO kehila.type_workflows VALUES ('created','other','flow',0);
  UPDATE kehila.fields SET owner_type_id = 'other';
  SET CONSTRAINTS ALL IMMEDIATE
$q$, '23503', 'type_title_field');
SELECT pg_temp.reject($q$UPDATE kehila.projects SET creation_operation_row_id =
  (SELECT operation_row_id FROM kehila.operations WHERE target_project_id = 'project')
$q$, '23503');
SELECT pg_temp.reject($q$UPDATE kehila.fields SET value_kind = 'number'$q$,
                       '23514', 'slice1_field_kind');
SELECT pg_temp.require((SELECT count(*) = 1 FROM kehila.projects),
                       'rejected writes preserve project');
SELECT pg_temp.require((SELECT bool_and(proconfig @> ARRAY[
  'search_path=pg_catalog, kehila, pg_temp']) FROM pg_proc
  WHERE pronamespace = 'kehila'::regnamespace AND prosecdef), 'definer search path');
CREATE ROLE foundation_untrusted LOGIN;
SET SESSION AUTHORIZATION foundation_untrusted;
SELECT pg_temp.reject($q$SELECT * FROM kehila.schema_migrations$q$, '42501');
SELECT pg_temp.reject($q$SELECT * FROM kehila.lock_actor('actor')$q$, '42501');
RESET SESSION AUTHORIZATION;
SELECT 'PASS: creation structure, history and serving permissions' AS result;
