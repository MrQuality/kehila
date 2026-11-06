-- Fresh provisional acceptance only. This is not a historical replay loader.
-- The caller supplies the operation returned by this transaction's fixed save.
SELECT o.actor_id::text,o.command_family,o.target_kind,o.target_project_id::text,
 o.target_item_id::text,o.target_user_id::text,o.target_relationship_id::text,
 o.token::text,o.request_codec_version,o.request_sha256,o.required_grant,
 o.grant_policy_version::bigint,o.replay_origin_ms::text,o.replay_deadline_ms::text,
 o.payload_retired,
 CASE WHEN octet_length(p.request_bytes) BETWEEN 1 AND 1048576 THEN p.request_bytes END,
 p.result_codec_version,
 CASE WHEN octet_length(p.result::text) BETWEEN 1 AND 1048576 THEN p.result::text END,
 o.operation_row_id
FROM kehila.operations o
JOIN kehila.operation_payloads p ON p.operation_row_id=o.operation_row_id
WHERE o.operation_row_id=$1::bigint LIMIT 2
