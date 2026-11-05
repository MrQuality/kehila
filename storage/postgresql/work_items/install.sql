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

-- This checkpoint serves authorized reads only. Invoker entry/helper execution
-- is explicit; pending, writer, replay and save entries remain owner-only.
GRANT SELECT ON kehila.items, kehila.item_revisions,
  kehila.project_operational_heads TO kehila_auth_runtime;
GRANT EXECUTE ON FUNCTION
  kehila_auth.item_read_authority(kehila_auth.authority,kehila.ident),
  kehila_auth.require_item_project_grant(kehila.ident,kehila.ident)
  TO kehila_auth_runtime;
