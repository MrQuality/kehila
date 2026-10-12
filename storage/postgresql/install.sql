\set ON_ERROR_STOP on
BEGIN;
SET LOCAL lock_timeout = '5s';
SET LOCAL statement_timeout = '15s';
DO $$
BEGIN
  IF current_setting('server_version_num')::integer / 10000 <> 16
     OR current_setting('server_encoding') <> 'UTF8' THEN
    RAISE EXCEPTION 'requires PostgreSQL 16 and UTF8';
  END IF;
  IF EXISTS (SELECT FROM pg_namespace WHERE nspname = 'kehila')
     OR EXISTS (SELECT FROM pg_roles WHERE rolname IN ('kehila_owner','kehila_creator')) THEN
    RAISE EXCEPTION 'fresh dedicated database and unused capability role names required';
  END IF;
END;
$$;
CREATE ROLE kehila_owner NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE
  NOINHERIT NOREPLICATION NOBYPASSRLS;
CREATE ROLE kehila_creator NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE
  NOINHERIT NOREPLICATION NOBYPASSRLS;
CREATE SCHEMA kehila AUTHORIZATION kehila_owner;
SET LOCAL ROLE kehila_owner;
REVOKE ALL ON SCHEMA kehila FROM PUBLIC;
ALTER DEFAULT PRIVILEGES REVOKE EXECUTE ON FUNCTIONS FROM PUBLIC;
CREATE TABLE kehila.schema_migrations (
  version integer PRIMARY KEY CHECK (version > 0),
  description text NOT NULL,
  installed_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
\ir operations.sql
\ir project_create.sql
\ir access.sql
INSERT INTO kehila.schema_migrations(version, description)
  VALUES (1, 'Project creation storage foundation');
COMMIT;
