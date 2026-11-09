-- Read-only physical census for a fresh, isolated synthetic product database.
-- The caller records stage/source/profile and keeps raw observations externally.
-- WAL insert position is cluster-wide; its delta includes background activity.
-- Allocation is not live data size or a promise of shrinkage after deletion.
-- Caller: the isolated fixture administrator, never the runtime role. No grants
-- are added for observation. Consumers must reconcile nonnegative component
-- sums to each relation total, with all writers settled at census checkpoints.
WITH relations AS (
  SELECT c.oid, n.nspname || '.' || c.relname AS name, c.reltoastrelid AS toast,
    c.reloptions
  FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
  WHERE n.nspname IN ('kehila', 'kehila_auth') AND c.relkind = 'r'
), census AS (
  SELECT name, reloptions,
    pg_relation_size(oid, 'main') AS heap_main_bytes,
    pg_relation_size(oid, 'fsm') + pg_relation_size(oid, 'vm')
      + pg_relation_size(oid, 'init') AS heap_auxiliary_bytes,
    pg_indexes_size(oid) AS index_bytes,
    CASE WHEN toast = 0 THEN 0 ELSE pg_relation_size(toast, 'main') END AS toast_main_bytes,
    CASE WHEN toast = 0 THEN 0 ELSE pg_relation_size(toast, 'fsm')
      + pg_relation_size(toast, 'vm') + pg_relation_size(toast, 'init') END AS toast_auxiliary_bytes,
    CASE WHEN toast = 0 THEN 0 ELSE pg_indexes_size(toast) END AS toast_index_bytes,
    pg_total_relation_size(oid) AS total_bytes
  FROM relations
), history AS (
  SELECT count(*) AS revisions,
    coalesce(sum(octet_length(history::text)), 0) AS logical_history_text_bytes,
    coalesce(min(octet_length(history::text)), 0) AS minimum_history_text_bytes,
    coalesce(max(octet_length(history::text)), 0) AS maximum_history_text_bytes
  FROM kehila.item_revisions
), payloads AS (
  SELECT count(*) AS replay_payloads,
    coalesce(sum(octet_length(request_bytes)), 0) AS logical_request_bytes,
    coalesce(sum(octet_length(result::text)), 0) AS logical_result_text_bytes
  FROM kehila.operation_payloads
), pending AS (
  SELECT count(*) AS pending_creation_identities,
    count(request_bytes) AS pending_creation_payloads,
    coalesce(sum(octet_length(request_bytes)), 0) AS logical_pending_request_bytes
  FROM kehila_auth.pending_items
)
SELECT jsonb_build_object(
  'schema', 1,
  'cluster_system_identifier', (SELECT system_identifier::text FROM pg_control_system()),
  'database_oid', (SELECT oid::bigint FROM pg_database WHERE datname = current_database()),
  'postgres', jsonb_build_object(
    'version', current_setting('server_version'),
    'block_size', current_setting('block_size'),
    'default_toast_compression', current_setting('default_toast_compression'),
    'wal_compression', current_setting('wal_compression'),
    'full_page_writes', current_setting('full_page_writes'),
    'autovacuum', current_setting('autovacuum'),
    'checkpoint_timeout', current_setting('checkpoint_timeout'),
    'max_wal_size', current_setting('max_wal_size')),
  'wal_insert_lsn', pg_current_wal_insert_lsn()::text,
  'wal_directory_allocated_bytes', (SELECT coalesce(sum(size), 0) FROM pg_ls_waldir()),
  'database_allocated_bytes', pg_database_size(current_database()),
  'relations', (SELECT jsonb_agg(to_jsonb(census) ORDER BY name) FROM census),
  'column_storage', (SELECT jsonb_agg(jsonb_build_object(
      'relation', n.nspname || '.' || c.relname, 'column', a.attname,
      'storage', a.attstorage, 'compression', a.attcompression)
      ORDER BY n.nspname, c.relname, a.attnum)
    FROM pg_attribute a JOIN pg_class c ON c.oid = a.attrelid
      JOIN pg_namespace n ON n.oid = c.relnamespace
    WHERE n.nspname IN ('kehila', 'kehila_auth') AND c.relkind = 'r'
      AND a.attnum > 0 AND NOT a.attisdropped AND a.attlen = -1),
  'logical', (SELECT to_jsonb(history) || to_jsonb(payloads) || to_jsonb(pending)
              FROM history CROSS JOIN payloads CROSS JOIN pending),
  'items', (SELECT count(*) FROM kehila.items),
  'operations', (SELECT count(*) FROM kehila.operations),
  'retired_operations', (SELECT count(*) FROM kehila.operations WHERE payload_retired),
  'field_uses', (SELECT count(*) FROM kehila.item_field_uses));
