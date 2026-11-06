-- Separate post-COMMIT backend oracle for the owner-only structural fixture.
-- Every positive join predicate must hold; missing rows/NULL facts cannot pass.
\set ON_ERROR_STOP on
SET statement_timeout = '15s';
SET ROLE kehila_owner;
DO $$
BEGIN
  IF (SELECT count(*) FROM kehila.projects) <> 1
     OR (SELECT count(*) FROM kehila.items) <> 1
     OR (SELECT count(*) FROM kehila.item_revisions) <> 2
     OR (SELECT count(*) FROM kehila.operations) <> 3
     OR (SELECT count(*) FROM kehila.operation_payloads) <> 1
     OR (SELECT count(*) FROM kehila_auth.pending_items) <> 1
     OR NOT EXISTS (
       SELECT FROM kehila.items i JOIN kehila.projects p USING (project_id)
         JOIN kehila.project_operational_heads h USING (project_id)
         JOIN kehila.item_revisions r1 ON r1.project_id=i.project_id
           AND r1.item_id=i.item_id AND r1.item_version=1
         JOIN kehila.item_revisions r2 ON r2.project_id=i.project_id
           AND r2.item_id=i.item_id AND r2.item_version=i.current_version
         JOIN kehila.operations o1 ON o1.operation_row_id=r1.operation_row_id
         JOIN kehila.operations o2 ON o2.operation_row_id=r2.operation_row_id
         JOIN kehila_auth.pending_items pending ON pending.request_id=i.creation_request_id
       WHERE i.project_id='p' AND i.item_id='i' AND i.issued_prefix='PR' AND i.issued_sequence=1
         AND i.creation_operation_row_id=o1.operation_row_id AND i.current_version=2
         AND h.state_version=2 AND h.operation_row_id=o2.operation_row_id
         AND p.next_sequence=2 AND p.ever_estimated
         AND r1.project_state_version=1 AND r1.previous_project_operation=p.creation_operation_row_id
         AND r1.estimate_thousandths=0 AND r1.resulting_next_sequence=2 AND r1.resulting_ever_estimated
         AND r2.item_version=2 AND r2.previous_item_version=1
         AND r2.project_state_version=2 AND r2.previous_project_operation=o1.operation_row_id
         AND r2.estimate_thousandths IS NULL AND r2.resulting_next_sequence=2 AND r2.resulting_ever_estimated
         AND r2.type_id=r1.type_id AND r2.workflow_id=r1.workflow_id
         AND r2.status_id=r1.status_id AND r2.phase=r1.phase AND NOT r2.archived
         AND r1.history #> '{value,result,value,data,estimate}' = '"0"'::jsonb
         AND r2.history #> '{value,result,value,data,estimate}' = 'null'::jsonb
         AND r1.history #> '{value,result,value,project,ever_estimated}' = 'true'::jsonb
         AND r2.history #> '{value,result,value,project,ever_estimated}' = 'true'::jsonb
         AND r2.history #>> '{value,previous_project_operation}' = o1.operation_row_id::text
         AND o1.actor_id='owner' AND o1.command_family='item_create' AND o1.required_grant='item_create'
         AND o1.grant_policy_version=1 AND o1.target_kind='work_item'
         AND o1.target_project_id='p' AND o1.target_item_id='i' AND o1.token='op'
         AND o1.request_codec_version=1
         AND o1.request_sha256=decode('85d4fb8af0baa1f670abc88958fe277d7da62628d5c661ff55a8032b499a3cd1','hex')
         AND o2.actor_id='owner' AND o2.command_family='item_edit' AND o2.required_grant='item_edit'
         AND o2.grant_policy_version=1 AND o2.target_kind='work_item'
         AND o2.target_project_id='p' AND o2.target_item_id='i' AND o2.token='edit-op'
         AND o2.request_codec_version=1
         AND o2.request_sha256=decode('6d81ad47bfae157e33aac5262eca50f9da00ea168305d503717da33bebfcc3f5','hex')
         AND o1.payload_retired AND o2.payload_retired
         AND NOT EXISTS (SELECT FROM kehila.operation_payloads replay
                          WHERE replay.operation_row_id IN (o1.operation_row_id,o2.operation_row_id))
         AND pending.installation_id='installation' AND pending.actor_id='owner'
         AND pending.project_id='p' AND pending.command_family='item_create'
         AND pending.intent_handle=repeat('b',32) AND pending.token=o1.token
         AND pending.target_item_id='i' AND pending.operation_row_id=o1.operation_row_id
         AND pending.request_codec_version=o1.request_codec_version
         AND pending.request_sha256=o1.request_sha256
         AND pending.state='completed' AND pending.request_bytes IS NULL
     ) THEN
    RAISE EXCEPTION 'committed WorkItem structure mismatch';
  END IF;
END;
$$;
RESET ROLE;
SELECT 'PASS: independent committed WorkItem cores, heads, history and pending binding' AS result;
