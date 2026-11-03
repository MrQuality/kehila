-- Fresh dedicated product database only; this is not an upgrade script.
\set ON_ERROR_STOP on
BEGIN;
\ir foundation.sql
\ir auth/enrollment.sql
\ir auth/authority.sql
\ir auth/audit.sql
\ir auth/management.sql
RESET ROLE;
GRANT kehila_creator TO kehila_auth_runtime;
SET LOCAL ROLE kehila_owner;
\ir pending.sql
INSERT INTO kehila.schema_migrations(version, description)
  VALUES (2, 'Authenticated project creation and server-owned pending requests');
COMMIT;
