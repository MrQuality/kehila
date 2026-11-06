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

-- Authorized reads and pending creation lifecycle are serving capabilities.
-- Domain mutation, replay and save entries remain owner-only.
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
