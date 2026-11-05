-- One indexed permanent revision. Never fetch expired replay payloads to read
-- current state. The CASE sentinel bounds driver materialization before decode.
SELECT r.project_id::text,r.item_id::text,r.item_version::numeric(20,0)::text,
 r.configuration_revision::numeric(20,0)::text,r.actor_id::text,r.command_family,
 r.token::text,r.operation_row_id,r.type_id::text,r.workflow_id::text,r.status_id::text,
 r.phase::text,r.archived,r.estimate_thousandths,r.estimate_unit,r.history_codec_version,
 CASE WHEN octet_length(r.history::text)<=1048576 THEN r.history::text END,
 r.project_state_version::numeric(20,0)::text,r.previous_project_operation,
 r.resulting_next_sequence::numeric(20,0)::text,r.resulting_ever_estimated,
 COALESCE(o.actor_id=r.actor_id AND o.command_family=r.command_family
   AND o.target_kind='work_item' AND o.target_project_id=r.project_id
   AND o.target_item_id=r.item_id AND o.target_user_id IS NULL
   AND o.target_relationship_id IS NULL AND o.token=r.token
   AND o.required_grant=r.command_family AND o.grant_policy_version=1
   AND r.target_kind='work_item',false),
 COALESCE(po.target_project_id=r.project_id AND po.target_user_id IS NULL
   AND po.target_relationship_id IS NULL AND po.required_grant=po.command_family
   AND po.grant_policy_version=1 AND
   ((r.project_state_version=1 AND po.operation_row_id=p.creation_operation_row_id
      AND po.actor_id=p.created_by AND po.command_family='project_create'
      AND po.target_kind='project' AND po.target_item_id IS NULL)
    OR (r.project_state_version>1 AND pr.operation_row_id=po.operation_row_id
      AND po.actor_id=pr.actor_id AND po.command_family=pr.command_family
      AND po.command_family IN ('item_create','item_edit') AND po.target_kind='work_item'
      AND po.target_item_id=pr.item_id AND po.token=pr.token)),false)
FROM kehila.item_revisions r
JOIN kehila.projects p ON p.project_id=r.project_id
LEFT JOIN kehila.operations o ON o.operation_row_id=r.operation_row_id
LEFT JOIN kehila.operations po ON po.operation_row_id=r.previous_project_operation
LEFT JOIN kehila.item_revisions pr ON pr.project_id=r.project_id
 AND pr.project_state_version=r.project_state_version-1
 AND pr.operation_row_id=r.previous_project_operation
WHERE r.project_id=$1::text::kehila.ident AND r.item_id=$2::text::kehila.ident
 AND r.item_version=$3::text::kehila.u64 LIMIT 1
