SELECT e.user_id::text,e.actor_id::text,e.token::text,e.operation_row_id,
  e.event_ordinal::numeric(20,0)::text,e.previous_event_ordinal::numeric(20,0)::text,
  e.before_version::numeric(20,0)::text,e.before_project_id::text,e.before_item_id::text,
  e.selection_version::numeric(20,0)::text,e.selected_project_id::text,e.selected_item_id::text,
  e.history_codec_version,
  CASE WHEN octet_length(e.history::text)<=16384 THEN e.history::text END,
  o.actor_id::text,o.command_family::text,o.target_kind::text,o.target_user_id::text,
  o.token::text,o.request_codec_version,
  CASE WHEN octet_length(o.request_sha256)=32 THEN o.request_sha256 END,
  o.required_grant::text,o.grant_policy_version,
  o.target_project_id IS NULL AND o.target_item_id IS NULL AND o.target_relationship_id IS NULL,
  o.replay_origin_ms::numeric(20,0)::text,o.replay_deadline_ms::numeric(20,0)::text
FROM kehila.selection_events e JOIN kehila.operations o USING(operation_row_id)
WHERE e.operation_row_id=$1::bigint LIMIT 2
