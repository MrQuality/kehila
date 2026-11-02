-- Deliberately invalid *candidate bindings*, with otherwise valid operation cores.
-- No claim of complete typed application creation or authenticated actors.
\set ON_ERROR_STOP on
BEGIN;
SET LOCAL statement_timeout = '10s';
SET LOCAL lock_timeout = '2s';
INSERT INTO kehila.actors VALUES ('sp005-other', true, true);
INSERT INTO kehila.operations
  (actor_id,command_family,target_kind,target_project_id,token,
   request_codec_version,request_sha256,required_grant,grant_policy_version,
   replay_origin_ms,replay_deadline_ms)
SELECT v.actor_id, o.command_family, o.target_kind, o.target_project_id, v.token,
       o.request_codec_version,o.request_sha256,o.required_grant,o.grant_policy_version,
       o.replay_origin_ms,o.replay_deadline_ms
FROM kehila.projects p JOIN kehila.operations o
  ON o.operation_row_id=p.creation_operation_row_id
CROSS JOIN (VALUES ('actor','sp005-alternate'),
                   ('sp005-other','sp005-other-creator')) AS v(actor_id,token)
WHERE p.project_id='created';
INSERT INTO kehila.operation_payloads
SELECT alternate.operation_row_id, payload.request_bytes, payload.result_codec_version,
       payload.result
FROM kehila.operations alternate
JOIN kehila.projects project ON project.project_id=alternate.target_project_id
JOIN kehila.operation_payloads payload ON payload.operation_row_id=project.creation_operation_row_id
WHERE alternate.token IN ('sp005-alternate','sp005-other-creator');
SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;
SELECT 'SP005_FIXTURE_PASS';
