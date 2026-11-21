SELECT count(*)=1 AND coalesce(bool_and(
  o.actor_id=$2::text::kehila.ident AND o.command_family='selection' AND o.target_kind='user'
  AND o.target_user_id=$2::text::kehila.ident AND o.target_project_id IS NULL
  AND o.target_item_id IS NULL AND o.target_relationship_id IS NULL
  AND o.token=$3::text::kehila.operation_token AND o.request_codec_version=1
  AND o.request_sha256=sha256($4::bytea) AND o.required_grant='selection' AND o.grant_policy_version=1
  AND NOT o.payload_retired AND o.replay_deadline_ms-o.replay_origin_ms=7776000000
  AND p.request_bytes=$4::bytea AND p.result_codec_version=1 AND p.result=$5::text::jsonb
  AND e.user_id=$2::text::kehila.ident AND e.actor_id=o.actor_id AND e.token=o.token
  AND e.command_family=o.command_family AND e.target_kind=o.target_kind
  AND e.history_codec_version=1 AND e.history=$6::text::jsonb
  AND e.event_ordinal=(e.history#>>'{value,ordinal}')::kehila.u64
  AND e.previous_event_ordinal IS NOT DISTINCT FROM CASE WHEN e.event_ordinal=1 THEN NULL ELSE e.event_ordinal-1 END
  AND e.before_version=(e.history#>>'{value,before,version}')::kehila.u64
  AND e.before_project_id IS NOT DISTINCT FROM e.history#>>'{value,before,requested,project_id}'
  AND e.before_item_id IS NOT DISTINCT FROM e.history#>>'{value,before,requested,item_id}'
  AND e.selection_version=(p.result#>>'{value,version}')::kehila.u64
  AND e.selected_project_id IS NOT DISTINCT FROM p.result#>>'{value,requested,project_id}'
  AND e.selected_item_id IS NOT DISTINCT FROM p.result#>>'{value,requested,item_id}'
  AND h.selection_version=e.selection_version AND h.event_ordinal=e.event_ordinal
  AND h.selected_project_id IS NOT DISTINCT FROM e.selected_project_id
  AND h.selected_item_id IS NOT DISTINCT FROM e.selected_item_id
  AND e.event_ordinal=(SELECT max(v.event_ordinal) FROM kehila.selection_events v WHERE v.user_id=e.user_id)
),false)
FROM kehila.operations o JOIN kehila.operation_payloads p USING(operation_row_id)
JOIN kehila.selection_events e USING(operation_row_id)
JOIN kehila.current_work_selections h ON h.user_id=e.user_id
WHERE o.operation_row_id=$1::bigint
