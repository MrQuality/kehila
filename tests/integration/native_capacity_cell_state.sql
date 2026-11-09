-- Read-only product-state witness. JSON Lines are parsed one record at a time.
-- JSON escapes CSV control delimiters, preserving JSON quotes and backslashes.
CREATE TEMP TABLE capacity_input (record jsonb NOT NULL);
\copy capacity_input FROM '/tmp/native-capacity-expectations.jsonl' WITH (FORMAT csv, DELIMITER E'\x1f', QUOTE E'\x01', ESCAPE E'\x01');
CREATE TEMP VIEW cap_meta AS SELECT record->'plan' plan,(record->>'calibration')::boolean calibration FROM capacity_input WHERE record->>'type'='meta';
CREATE TEMP TABLE cap_seeds AS SELECT (record->>'slot')::int slot,record->>'project_id' project,
  record->>'item_id' item,record->'registration' registration,decode(record->>'request_sha256','hex') digest,
  decode(record->>'request_base64','base64') request_bytes,
  record->'result' result,record->'field_ids' fields FROM capacity_input WHERE record->>'type'='seed';
CREATE UNIQUE INDEX ON cap_seeds(slot);
CREATE UNIQUE INDEX ON cap_seeds(item);
CREATE TEMP TABLE cap_calls AS SELECT record->>'phase' phase,(record->>'ordinal')::int ordinal,
  (record->>'slot')::int slot,(record->>'edit')::boolean edit,record->>'project_id' project,
  record->>'item_id' item,(record->>'expected_version')::bigint expected_version,
  record->>'operation_id' token,decode(record->>'request_sha256','hex') digest,
  decode(record->>'request_base64','base64') request_bytes,
  (record->>'accepted')::boolean accepted,(record->>'success')::boolean success,
  nullif(record->'result','null'::jsonb) result FROM capacity_input WHERE record->>'type'='attempt';
CREATE UNIQUE INDEX ON cap_calls(phase,ordinal);
CREATE UNIQUE INDEX ON cap_calls(token) WHERE edit;
CREATE TEMP TABLE cap_slots AS SELECT (record->>'slot')::int slot,record->>'project_id' project,
  record->>'item_id' item,(record->>'expected_version')::bigint version,record->'result' result
  FROM capacity_input WHERE record->>'type'='slot';
CREATE UNIQUE INDEX ON cap_slots(slot);
CREATE TEMP VIEW cap_expected AS
  SELECT project,item,1::bigint version,registration->>'token' token,digest,result,'item_create'::text family FROM cap_seeds
  UNION ALL SELECT project,item,expected_version+1,token,digest,result,'item_edit' FROM cap_calls WHERE edit AND accepted;

DO $$ DECLARE projects int; fields int; edits bigint; calibration_calls int; BEGIN
  IF (SELECT count(*) FROM cap_meta)<>1 THEN RAISE EXCEPTION 'capacity expectation metadata mismatch'; END IF;
  SELECT (plan->>'projects')::int,(plan->>'fields_per_item')::int INTO projects,fields FROM cap_meta;
  SELECT count(*) INTO edits FROM cap_calls WHERE edit AND accepted;
  SELECT CASE WHEN calibration THEN 2 ELSE 0 END INTO calibration_calls FROM cap_meta;
  IF projects NOT IN (1,4) OR fields NOT IN (4,128)
    OR (SELECT count(*) FROM capacity_input)<>144+calibration_calls
    OR (SELECT count(*) FROM cap_seeds)<>16 OR (SELECT count(*) FROM cap_slots)<>16
    OR EXISTS(SELECT FROM cap_seeds WHERE slot NOT BETWEEN 0 AND 15)
    OR EXISTS(SELECT FROM cap_slots WHERE slot NOT BETWEEN 0 AND 15)
    OR (SELECT count(*) FROM cap_calls WHERE phase='warmup')<>10
    OR (SELECT count(*) FROM cap_calls WHERE phase='measured')<>100
    OR (SELECT count(*) FROM cap_calls WHERE phase='control' AND edit AND accepted)<>1
    OR (SELECT count(*) FROM cap_calls WHERE phase='calibration' AND edit AND accepted)<>calibration_calls
    OR (SELECT count(*) FROM cap_calls)<>111+calibration_calls THEN
    RAISE EXCEPTION 'capacity expectation population mismatch';
  END IF;
  IF (SELECT count(*) FROM kehila.projects)<>projects
    OR (SELECT count(*) FROM kehila.fields)<>projects*fields
    OR (SELECT count(*) FROM kehila.work_item_types)<>projects
    OR (SELECT count(*) FROM kehila.configuration_revisions)<>projects
    OR (SELECT count(*) FROM kehila.project_grants)<>projects
    OR (SELECT count(*) FROM kehila.items)<>16
    OR (SELECT count(*) FROM kehila.item_revisions)<>16+edits
    OR (SELECT count(*) FROM kehila.operations)<>projects+16+edits
    OR (SELECT count(*) FROM kehila.operation_payloads)<>projects+16+edits
    OR EXISTS(SELECT FROM kehila.operations WHERE payload_retired)
    OR (SELECT count(*) FROM kehila.operations WHERE command_family='project_create')<>projects
    OR (SELECT count(*) FROM kehila.operations WHERE command_family='item_create')<>16
    OR (SELECT count(*) FROM kehila.operations WHERE command_family='item_edit')<>edits
    OR (SELECT count(*) FROM kehila_auth.pending_items)<>16
    OR EXISTS(SELECT FROM kehila_auth.pending_items WHERE state<>'completed')
    OR EXISTS(SELECT FROM kehila.projects WHERE archived OR ever_estimated OR configuration_revision<>1)
    OR EXISTS(SELECT FROM kehila.item_revisions WHERE estimate_thousandths IS NOT NULL OR archived OR resulting_ever_estimated)
    OR (SELECT count(*) FROM kehila.item_field_uses)<>projects*fields THEN
    RAISE EXCEPTION 'capacity committed cardinality mismatch';
  END IF;

  -- This independent leaf/state comparison is also exercised with one altered
  -- expected text value. The negative control must reach this exact rejection.
  IF EXISTS(SELECT FROM cap_slots s LEFT JOIN kehila.items i ON i.item_id=s.item AND i.project_id=s.project
    LEFT JOIN kehila.item_revisions r ON r.project_id=i.project_id AND r.item_id=i.item_id AND r.item_version=i.current_version
    WHERE i.item_id IS NULL OR i.current_version<>s.version OR r.operation_row_id IS NULL
      OR r.history#>'{value,result}' IS DISTINCT FROM s.result) THEN
    RAISE EXCEPTION 'capacity final state mismatch';
  END IF;

  IF EXISTS(SELECT FROM cap_seeds s LEFT JOIN kehila.items i ON i.item_id=s.item AND i.project_id=s.project
    LEFT JOIN kehila_auth.pending_items p ON p.request_id=(s.registration->>'request_id')::bigint
    LEFT JOIN kehila.operations o ON o.operation_row_id=i.creation_operation_row_id
    LEFT JOIN kehila.operation_payloads b ON b.operation_row_id=o.operation_row_id
    LEFT JOIN kehila.item_revisions r ON r.project_id=i.project_id AND r.item_id=i.item_id AND r.item_version=1
    WHERE i.item_id IS NULL OR p.request_id IS NULL OR o.operation_row_id IS NULL OR r.operation_row_id IS NULL
      OR i.creation_request_id<>p.request_id OR p.target_item_id<>s.item OR p.project_id<>s.project
      OR p.actor_id<>'owner' OR p.token<>s.registration->>'token' OR p.intent_handle<>s.registration->>'intent_handle'
      OR p.operation_row_id<>o.operation_row_id OR r.operation_row_id<>o.operation_row_id
      OR o.actor_id<>'owner' OR o.command_family<>'item_create' OR o.target_item_id<>s.item OR o.target_project_id<>s.project
      OR o.token<>s.registration->>'token' OR o.request_codec_version<>1 OR o.request_sha256<>s.digest
      OR p.request_codec_version<>1 OR p.request_sha256<>s.digest
      OR p.request_bytes IS DISTINCT FROM s.request_bytes OR sha256(p.request_bytes)<>s.digest
      OR r.history#>'{value,result}' IS DISTINCT FROM s.result
      OR i.issued_sequence<>s.slot/projects+1 OR i.issued_prefix<>('CP'||(s.slot%projects)::text)
      OR b.operation_row_id IS NULL OR b.request_bytes IS DISTINCT FROM s.request_bytes
      OR sha256(b.request_bytes)<>s.digest OR b.result IS DISTINCT FROM s.result) THEN
    RAISE EXCEPTION 'capacity seed identity mismatch';
  END IF;

  IF EXISTS(SELECT FROM cap_calls c LEFT JOIN kehila.operations o
      ON o.actor_id='owner' AND o.command_family='item_edit' AND o.token=c.token
    LEFT JOIN kehila.operation_payloads b ON b.operation_row_id=o.operation_row_id
    LEFT JOIN kehila.item_revisions r ON r.operation_row_id=o.operation_row_id
    WHERE c.edit AND (
      (NOT c.accepted AND (o.operation_row_id IS NOT NULL OR r.operation_row_id IS NOT NULL))
      OR (c.accepted AND (o.operation_row_id IS NULL OR r.operation_row_id IS NULL
        OR o.target_project_id<>c.project OR o.target_item_id<>c.item OR o.request_codec_version<>1 OR o.request_sha256<>c.digest
        OR r.project_id<>c.project OR r.item_id<>c.item OR r.item_version<>c.expected_version+1
        OR r.history#>'{value,result}' IS DISTINCT FROM c.result
        OR b.operation_row_id IS NULL OR b.request_bytes IS DISTINCT FROM c.request_bytes
        OR sha256(b.request_bytes)<>c.digest OR b.result IS DISTINCT FROM c.result)))) THEN
    RAISE EXCEPTION 'capacity attempt identity or acceptance mismatch';
  END IF;
  IF EXISTS(SELECT FROM cap_calls c LEFT JOIN kehila.item_revisions r
    ON r.project_id=c.project AND r.item_id=c.item AND r.item_version=c.expected_version
    WHERE NOT c.edit AND c.success AND (r.operation_row_id IS NULL OR r.history#>'{value,result}' IS DISTINCT FROM c.result)) THEN
    RAISE EXCEPTION 'capacity read snapshot mismatch';
  END IF;

  IF EXISTS(SELECT FROM kehila.item_revisions r
    LEFT JOIN cap_expected e ON e.project=r.project_id AND e.item=r.item_id AND e.version=r.item_version
    JOIN kehila.operations o ON o.operation_row_id=r.operation_row_id
    WHERE e.result IS NULL OR r.actor_id<>'owner' OR r.command_family<>e.family OR r.token<>e.token
      OR o.actor_id<>r.actor_id OR o.command_family<>r.command_family OR o.token<>r.token
      OR o.target_project_id<>r.project_id OR o.target_item_id<>r.item_id OR o.request_sha256<>e.digest
      OR r.history_codec_version<>1 OR r.history->>'kind'<>'item_revision' OR r.history->>'codec_version'<>'1'
      OR r.history#>>'{value,actor_id}'<>'owner' OR r.history#>>'{value,configuration_revision}'<>'1'
      OR r.history#>>'{value,project_state_version}'<>r.project_state_version::numeric(20,0)::text
      OR r.history#>>'{value,previous_project_operation}'<>r.previous_project_operation::text
      OR r.history#>'{value,result}' IS DISTINCT FROM e.result
      OR r.type_id<>e.result#>>'{value,item,type_id}' OR r.workflow_id<>e.result#>>'{value,item,workflow_id}'
      OR r.status_id<>e.result#>>'{value,item,status_id}' OR r.phase::text<>e.result#>>'{value,phase}'
      OR r.resulting_next_sequence::numeric(20,0)::text<>e.result#>>'{value,project,next_sequence}'
      OR jsonb_array_length(e.result#>'{value,data,values}')<>fields) THEN
    RAISE EXCEPTION 'capacity immutable revision mismatch';
  END IF;
  IF EXISTS(SELECT FROM kehila.projects p JOIN kehila.project_operational_heads h USING(project_id)
    LEFT JOIN kehila.item_revisions latest ON latest.project_id=p.project_id AND latest.project_state_version=h.state_version
    WHERE p.next_sequence<>16/projects+1 OR h.state_version<>16/projects+(SELECT count(*) FROM cap_calls c WHERE c.project=p.project_id AND c.edit AND c.accepted)
      OR latest.operation_row_id IS NULL OR h.operation_row_id<>latest.operation_row_id
      OR h.state_version<>(SELECT count(*) FROM kehila.item_revisions r WHERE r.project_id=p.project_id))
    OR EXISTS(SELECT FROM kehila.item_revisions r JOIN kehila.projects p USING(project_id)
      LEFT JOIN kehila.item_revisions previous ON previous.project_id=r.project_id AND previous.project_state_version=r.project_state_version-1
      WHERE r.previous_project_operation IS DISTINCT FROM CASE WHEN r.project_state_version=1 THEN p.creation_operation_row_id ELSE previous.operation_row_id END) THEN
    RAISE EXCEPTION 'capacity project operational chain mismatch';
  END IF;
  IF EXISTS(SELECT FROM pg_stat_activity WHERE datname=current_database() AND usename='kehila_auth_runtime_login')
    OR EXISTS(SELECT FROM pg_prepared_xacts WHERE database=current_database())
    OR EXISTS(SELECT FROM pg_locks WHERE locktype='advisory' AND database=(SELECT oid FROM pg_database WHERE datname=current_database())
      AND classid=1262831689::oid AND objid=1::oid AND objsubid=2)
    OR has_table_privilege('kehila_auth_runtime','kehila_auth.pending_items','SELECT,INSERT,UPDATE,DELETE,TRUNCATE') THEN
    RAISE EXCEPTION 'capacity authority release or privilege mismatch';
  END IF;
END $$;
SELECT 'NATIVE_CAPACITY_CELL_STATE_PASS';
