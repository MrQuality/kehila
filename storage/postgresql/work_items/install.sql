-- Included as kehila_owner inside the fresh product install transaction.
\ir records.sql
\ir retention.sql
\ir heads.sql
\ir history.sql
\ir closure.sql
\ir authority.sql
\ir pending.sql
\ir discovery.sql
\ir reconciliation.sql
\ir save.sql
\ir release.sql

-- Authorized reads and pending creation lifecycle are serving capabilities.
-- Mutation and historical release use the closed owner-defined entries below.
GRANT SELECT ON kehila.items, kehila.item_revisions,
  kehila.project_operational_heads TO kehila_auth_runtime;
GRANT EXECUTE ON FUNCTION
  kehila_auth.item_read_authority(kehila_auth.authority,kehila.ident),
  kehila_auth.require_item_project_grant(kehila.ident,kehila.ident)
  TO kehila_auth_runtime;

-- Policy metadata determines the actual bounded discovery page size.
GRANT SELECT ON kehila_auth.item_pending_policy TO kehila_auth_runtime;
GRANT EXECUTE ON FUNCTION
  kehila_auth.register_item(kehila_auth.authority,kehila.ident,kehila_auth.intent_key,kehila_auth.intent_key,kehila.ident,bigint,bytea),
  kehila_auth.cancel_item(kehila_auth.authority,kehila.ident,kehila_auth.intent_key),
  kehila_auth.list_pending_items(kehila_auth.authority,kehila.ident,bigint),
  kehila_auth.open_pending_item(kehila_auth.authority,kehila.ident,kehila_auth.intent_key)
  TO kehila_auth_runtime;

-- Execution, reconciliation and final replay release are one serving capability.
-- Private transition helpers and pending table access remain owner-only.
GRANT EXECUTE ON FUNCTION
  kehila_auth.reconcile_item_create(kehila_auth.authority,kehila.ident,kehila_auth.intent_key),
  kehila_auth.reconcile_item_edit(kehila_auth.authority,kehila.ident,kehila.ident,kehila.operation_token,bigint,bytea),
  kehila_auth.save_item_create(kehila_auth.authority,kehila.ident,kehila_auth.intent_key,bytea,kehila_auth.item_preparation,jsonb),
  kehila_auth.save_item_edit(kehila_auth.authority,kehila.ident,kehila.ident,kehila.operation_token,bytea,kehila_auth.item_preparation,jsonb),
  kehila_auth.authorize_item_replay(kehila_auth.authority,kehila.ident,text,kehila.ident,kehila.operation_token,bigint,bigint,bytea,kehila.u64,kehila.u64,bigint,kehila.u64,kehila_auth.intent_key)
  TO kehila_auth_runtime;
