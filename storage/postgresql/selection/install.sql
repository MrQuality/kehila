-- Included as kehila_owner by the fresh product installer, never by serving.
\ir records.sql
\ir integrity.sql
\ir closure.sql
\ir authority.sql
\ir save.sql
-- Trusted-backend preparation reads; these are not end-user SQL accounts.
-- Remote self/target authorization remains an independent serving boundary.
GRANT SELECT ON kehila.selection_events,kehila.current_work_selections TO kehila_auth_runtime;
GRANT EXECUTE ON FUNCTION
  kehila_auth.reconcile_selection(kehila_auth.authority,kehila.operation_token,bigint,bytea),
  kehila_auth.save_selection(kehila_auth.authority,kehila.operation_token,kehila.u64,bytea,jsonb)
  TO kehila_auth_runtime;
