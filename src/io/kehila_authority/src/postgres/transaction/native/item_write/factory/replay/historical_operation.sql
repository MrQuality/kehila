-- Historical-only, fully scoped identity obtained by the preceding RR lookup.
-- Every record is bounded before driver materialization; no current head,
-- current configuration or mutable-domain acceptance participates here.
-- Creation pending facts and exact request equality were verified by the closed
-- reconcile_item_create entry in this SAME RR snapshot. The runtime role cannot
-- SELECT the private pending table; immutable item keys bind its resolved ID.
SELECT o.actor_id::text,o.command_family,o.target_kind,o.target_project_id::text,
 o.target_item_id::text,o.target_user_id::text,o.target_relationship_id::text,
 o.token::text,o.request_codec_version,o.request_sha256,o.required_grant,
 o.grant_policy_version::bigint,o.replay_origin_ms::text,o.replay_deadline_ms::text,
 o.payload_retired,
 CASE WHEN octet_length(p.request_bytes) BETWEEN 1 AND 1048576 THEN p.request_bytes END,
 p.result_codec_version,
 CASE WHEN octet_length(p.result::text) BETWEEN 1 AND 1048576 THEN p.result::text END,
 r.item_version::numeric(20,0)::text,
 COALESCE(p.result=r.history#>'{value,result}',false),
 i.issued_prefix::text,i.issued_sequence::numeric(20,0)::text,i.creation_operation_row_id,
 CASE WHEN o.command_family='item_edit' THEN true ELSE COALESCE(
   i.creation_request_id=$7::bigint AND i.creation_operation_row_id=o.operation_row_id,false) END
FROM kehila.operations o
JOIN kehila.operation_payloads p ON p.operation_row_id=o.operation_row_id
JOIN kehila.item_revisions r ON r.operation_row_id=o.operation_row_id
 AND r.project_id=o.target_project_id AND r.item_id=o.target_item_id
JOIN kehila.items i ON i.project_id=r.project_id AND i.item_id=r.item_id
WHERE o.operation_row_id=$1::bigint AND o.actor_id=$2::text::kehila.ident
 AND o.command_family=$3::text AND o.target_kind='work_item'
 AND o.target_project_id=$4::text::kehila.ident AND o.target_item_id=$5::text::kehila.ident
 AND o.target_user_id IS NULL AND o.target_relationship_id IS NULL
 AND o.token=$6::text::kehila.operation_token LIMIT 2
