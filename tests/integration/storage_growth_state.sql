-- Independent backend observation after native runtime ownership has ended.
-- Raw synthetic history stays in external evidence. No temporary expectation
-- tables or production mutations are needed to compare with the client oracle.
\set ON_ERROR_STOP on
\pset tuples_only on
\pset format unaligned
BEGIN READ ONLY;
SET LOCAL statement_timeout='15s';
SET LOCAL lock_timeout='3s';
SELECT jsonb_build_object('type','meta',
  'projects',(SELECT count(*) FROM kehila.projects),
  'items',(SELECT count(*) FROM kehila.items),
  'revisions',(SELECT count(*) FROM kehila.item_revisions),
  'operations',(SELECT count(*) FROM kehila.operations),
  'payloads',(SELECT count(*) FROM kehila.operation_payloads),
  'field_uses',(SELECT count(*) FROM kehila.item_field_uses),
  'runtime_backends',(SELECT count(*) FROM pg_stat_activity
    WHERE datname=current_database() AND usename='kehila_auth_runtime_login'),
  'prepared_transactions',(SELECT count(*) FROM pg_prepared_xacts WHERE database=current_database()),
  'authority_locks',(SELECT count(*) FROM pg_locks
    WHERE locktype='advisory' AND database=(SELECT oid FROM pg_database WHERE datname=current_database())
      AND classid=1262831689::oid AND objid=1::oid AND objsubid=2));
SELECT jsonb_build_object('type','identity',
  'item_id',i.item_id,'project_id',i.project_id,'current_version',i.current_version::text,
  'creation_operation_row_id',i.creation_operation_row_id,'issued_prefix',i.issued_prefix,
  'issued_sequence',i.issued_sequence::text,'state_version',h.state_version::text,
  'head_operation_row_id',h.operation_row_id,'pending_state',p.state,
  'pending_operation_row_id',p.operation_row_id,'pending_target_item_id',p.target_item_id,
  'pending_request_sha256',encode(p.request_sha256,'hex'),
  'pending_payload_sha256',encode(sha256(p.request_bytes),'hex'),
  'project_creation_operation_row_id',j.creation_operation_row_id,
  'next_sequence',j.next_sequence::text,'ever_estimated',j.ever_estimated)
  FROM kehila.items i JOIN kehila.project_operational_heads h USING(project_id)
  JOIN kehila.projects j USING(project_id)
  JOIN kehila_auth.pending_items p ON p.request_id=i.creation_request_id;
SELECT jsonb_build_object('type','revision','version',r.item_version::text,
  'operation_row_id',r.operation_row_id,'previous_item_version',r.previous_item_version::text,
  'project_state_version',r.project_state_version::text,'previous_project_operation',r.previous_project_operation,
  'actor_id',o.actor_id,'family',o.command_family,'project_id',o.target_project_id,'item_id',o.target_item_id,
  'token',o.token,'history_token',r.token,'request_sha256',encode(o.request_sha256,'hex'),
  'payload_sha256',encode(sha256(p.request_bytes),'hex'),'payload_result',p.result,
  'request_codec_version',o.request_codec_version,'history_codec_version',r.history_codec_version,
  'payload_retired',o.payload_retired,'replay_origin_ms',o.replay_origin_ms::text,
  'replay_deadline_ms',o.replay_deadline_ms::text,'history',r.history)
  FROM kehila.item_revisions r JOIN kehila.operations o USING(operation_row_id)
  LEFT JOIN kehila.operation_payloads p USING(operation_row_id)
  ORDER BY r.item_version;
SELECT jsonb_build_object('type','field_use','field_id',f.field_id,'owner_type_id',f.owner_type_id,
  'first_item_id',f.first_item_id,'first_item_version',f.first_item_version::text,
  'first_operation_row_id',f.first_operation_row_id)
  FROM kehila.item_field_uses f ORDER BY f.field_id COLLATE "C";
COMMIT;
