-- Selected critical capability checks for the fresh version-three item profile.
-- This is not a general database integrity or PostgreSQL security audit.
WITH principals AS (
  SELECT * FROM pg_catalog.pg_roles WHERE rolname IN (current_user,session_user,'kehila_creator')
), forbidden(table_name, privileges) AS (VALUES
  ('kehila.actors','INSERT,UPDATE,DELETE,TRUNCATE'),
  ('kehila.schema_migrations','INSERT,UPDATE,DELETE,TRUNCATE'),
  ('kehila.operations','UPDATE,DELETE,TRUNCATE'),
  ('kehila.operation_payloads','UPDATE,DELETE,TRUNCATE'),
  ('kehila.configuration_revisions','UPDATE,DELETE,TRUNCATE'),
  ('kehila.project_grants','UPDATE,DELETE,TRUNCATE'),
  ('kehila.project_grant_events','UPDATE,DELETE,TRUNCATE'),
  ('kehila.items','INSERT,UPDATE,DELETE,TRUNCATE'),
  ('kehila.item_revisions','INSERT,UPDATE,DELETE,TRUNCATE'),
  ('kehila.item_field_uses','INSERT,UPDATE,DELETE,TRUNCATE'),
  ('kehila.project_operational_heads','INSERT,UPDATE,DELETE,TRUNCATE'),
  ('kehila_auth.pending_items','SELECT,INSERT,UPDATE,DELETE,TRUNCATE'),
  ('kehila_auth.item_pending_policy','INSERT,UPDATE,DELETE,TRUNCATE'),
  ('kehila_auth.installation','SELECT,INSERT,UPDATE,DELETE,TRUNCATE'),
  ('kehila_auth.owner_credentials','SELECT,INSERT,UPDATE,DELETE,TRUNCATE'),
  ('kehila_auth.management_events','INSERT,UPDATE,DELETE,TRUNCATE'),
  ('kehila_auth.pending_policy','INSERT,UPDATE,DELETE,TRUNCATE'),
  ('kehila_auth.pending_projects','INSERT,UPDATE,DELETE,TRUNCATE'),
  ('kehila_auth.creation_intents','INSERT,UPDATE,DELETE,TRUNCATE')
), native_reads(function_id) AS (VALUES
  ('kehila_auth.item_read_authority(kehila_auth.authority,kehila.ident)'::regprocedure),
  ('kehila_auth.require_item_project_grant(kehila.ident,kehila.ident)'::regprocedure)
), native_pending(function_id) AS (VALUES
  ('kehila_auth.register_item(kehila_auth.authority,kehila.ident,kehila_auth.intent_key,kehila_auth.intent_key,kehila.ident,bigint,bytea)'::regprocedure),
  ('kehila_auth.cancel_item(kehila_auth.authority,kehila.ident,kehila_auth.intent_key)'::regprocedure),
  ('kehila_auth.list_pending_items(kehila_auth.authority,kehila.ident,bigint)'::regprocedure),
  ('kehila_auth.open_pending_item(kehila_auth.authority,kehila.ident,kehila_auth.intent_key)'::regprocedure)
), native_mutations(function_id) AS (VALUES
  ('kehila_auth.reconcile_item_create(kehila_auth.authority,kehila.ident,kehila_auth.intent_key)'::regprocedure),
  ('kehila_auth.reconcile_item_edit(kehila_auth.authority,kehila.ident,kehila.ident,kehila.operation_token,bigint,bytea)'::regprocedure),
  ('kehila_auth.save_item_create(kehila_auth.authority,kehila.ident,kehila_auth.intent_key,bytea,kehila_auth.item_preparation,jsonb)'::regprocedure),
  ('kehila_auth.save_item_edit(kehila_auth.authority,kehila.ident,kehila.ident,kehila.operation_token,bytea,kehila_auth.item_preparation,jsonb)'::regprocedure),
  ('kehila_auth.authorize_item_replay(kehila_auth.authority,kehila.ident,text,kehila.ident,kehila.operation_token,bigint,bigint,bytea,kehila.u64,kehila.u64,bigint,kehila.u64,kehila_auth.intent_key)'::regprocedure)
), permitted_functions(function_id) AS (VALUES
  ('kehila.lock_actor(kehila.ident)'::regprocedure),
  ('kehila.lock_operation(bigint)'::regprocedure),
  ('kehila_auth.fence(kehila.ident,kehila.u64,kehila.u64,kehila.u64)'::regprocedure),
  ('kehila_auth.start_runtime(kehila.ident,kehila_auth.public_id)'::regprocedure),
  ('kehila_auth.read_authority(kehila.ident,kehila.u64,kehila.u64)'::regprocedure),
  ('kehila_auth.credentials(kehila.ident)'::regprocedure),
  ('kehila_auth.audit_issue(kehila.ident,kehila.u64,kehila.u64,kehila.u64,kehila_auth.public_id,text)'::regprocedure),
  ('kehila_auth.audit_logout(kehila.ident,kehila.u64,kehila.u64,kehila_auth.public_id,text)'::regprocedure),
  ('kehila_auth.audit_revoke(kehila.ident,kehila.u64,kehila.u64,kehila.u64,kehila_auth.public_id,text,kehila_auth.public_id)'::regprocedure),
  ('kehila_auth.change_password(kehila.ident,kehila.u64,kehila.u64,kehila.u64,kehila_auth.verifier,kehila_auth.public_id,kehila_auth.public_id)'::regprocedure),
  ('kehila_auth.revoke_all(kehila.ident,kehila.u64,kehila.u64,kehila.u64,kehila_auth.public_id,kehila_auth.public_id)'::regprocedure),
  ('kehila_auth.trusted_millis()'::regprocedure),
  ('kehila_auth.register_project(kehila_auth.authority,kehila_auth.intent_key,kehila_auth.intent_key,text,kehila.project_prefix,text)'::regprocedure),
  ('kehila_auth.list_projects(kehila_auth.authority,bigint)'::regprocedure),
  ('kehila_auth.open_project(kehila_auth.authority,bigint)'::regprocedure),
  ('kehila_auth.cancel_project(kehila_auth.authority,bigint)'::regprocedure),
  ('kehila_auth.complete_project(kehila_auth.authority,bigint,kehila_auth.intent_key,text,kehila.project_prefix,text,kehila.ident,bigint)'::regprocedure)
  UNION ALL SELECT function_id FROM native_reads
  UNION ALL SELECT function_id FROM native_pending
  UNION ALL SELECT function_id FROM native_mutations
), namespaces AS (
  SELECT oid FROM pg_catalog.pg_namespace WHERE nspname IN ('kehila','kehila_auth')
)
SELECT current_setting('server_version_num')::integer / 10000 = 16
 AND current_setting('server_encoding') = 'UTF8'
 AND current_user = 'kehila_auth_runtime' AND session_user = 'kehila_auth_runtime_login'
 AND NOT EXISTS (SELECT FROM pg_catalog.pg_roles
   WHERE rolname IN (current_user,session_user,'kehila_creator')
   AND (rolsuper OR rolcreatedb OR rolcreaterole OR rolreplication OR rolbypassrls
     OR (rolname<>session_user AND rolcanlogin)))
 AND NOT (SELECT rolinherit FROM pg_catalog.pg_roles WHERE rolname=session_user)
 AND NOT pg_has_role(session_user,'kehila_owner','MEMBER')
 AND NOT pg_has_role(session_user,'kehila_auth_admin','MEMBER')
 AND pg_has_role(current_user,'kehila_creator','USAGE')
 AND has_table_privilege(current_user,'kehila_auth.item_pending_policy','SELECT')
 AND has_table_privilege(current_user,'kehila.items','SELECT')
 AND has_table_privilege(current_user,'kehila.item_revisions','SELECT')
 AND has_table_privilege(current_user,'kehila.project_operational_heads','SELECT')
 AND NOT EXISTS (SELECT FROM pg_catalog.pg_proc f JOIN native_reads n ON n.function_id=f.oid
   WHERE f.proowner<>'kehila_owner'::regrole OR f.prosecdef OR f.provolatile<>'s'
     OR f.proconfig IS DISTINCT FROM ARRAY['search_path=pg_catalog, kehila_auth, kehila, pg_temp'])
 AND NOT EXISTS (SELECT FROM pg_catalog.pg_proc f JOIN native_pending n ON n.function_id=f.oid
   WHERE f.proowner<>'kehila_owner'::regrole OR NOT f.prosecdef OR f.provolatile<>'v'
     OR f.proconfig IS DISTINCT FROM ARRAY['search_path=pg_catalog, kehila_auth, kehila, pg_temp'])
 AND NOT EXISTS (SELECT FROM pg_catalog.pg_proc f JOIN native_mutations n ON n.function_id=f.oid
   WHERE f.proowner<>'kehila_owner'::regrole OR NOT f.prosecdef OR f.provolatile<>'v'
     OR f.proconfig IS DISTINCT FROM ARRAY['search_path=pg_catalog, kehila_auth, kehila, pg_temp'])
 AND EXISTS (SELECT FROM pg_catalog.pg_proc f
   JOIN pg_catalog.pg_trigger t ON t.tgfoid=f.oid
   WHERE f.oid='kehila.check_item_revision_uses()'::regprocedure
     AND f.proowner='kehila_owner'::regrole AND f.prosecdef
     AND f.proconfig=ARRAY['search_path=pg_catalog, kehila, pg_temp']
     AND t.tgrelid='kehila.item_revisions'::regclass
     AND t.tgname='item_revision_field_uses' AND t.tgenabled='O'
     AND t.tgtype=5 AND t.tgconstraint<>0
     AND t.tgdeferrable AND t.tginitdeferred)
 AND NOT EXISTS (SELECT FROM pg_catalog.pg_class c
   WHERE c.oid IN ('kehila.items'::regclass,'kehila.item_revisions'::regclass,
                  'kehila.project_operational_heads'::regclass,'kehila_auth.item_pending_policy'::regclass)
     AND (c.relowner<>'kehila_owner'::regrole OR c.relkind<>'r'))
 AND NOT EXISTS (
   SELECT FROM pg_catalog.pg_auth_members m
   JOIN pg_catalog.pg_roles member ON member.oid=m.member
   JOIN pg_catalog.pg_roles parent ON parent.oid=m.roleid
   WHERE (member.rolname=session_user AND (parent.rolname<>'kehila_auth_runtime'
       OR m.admin_option OR m.inherit_option OR NOT m.set_option))
     OR (member.rolname=current_user AND (parent.rolname<>'kehila_creator'
       OR m.admin_option OR NOT m.inherit_option))
     OR member.rolname='kehila_creator')
 AND NOT EXISTS (SELECT FROM principals p CROSS JOIN forbidden f
   WHERE has_table_privilege(p.rolname,f.table_name,f.privileges))
 AND NOT EXISTS (SELECT FROM pg_catalog.pg_class c, principals p
   WHERE c.relnamespace IN (SELECT oid FROM namespaces) AND c.relkind IN ('r','p')
     AND has_table_privilege(p.oid,c.oid,'UPDATE,DELETE,TRUNCATE,TRIGGER'))
 AND NOT EXISTS (SELECT FROM pg_catalog.pg_class c, principals p
   WHERE c.relnamespace IN (SELECT oid FROM namespaces) AND c.relkind='S'
     AND has_sequence_privilege(p.oid,c.oid,'UPDATE'))
 AND NOT EXISTS (SELECT FROM principals p CROSS JOIN namespaces n
   WHERE has_schema_privilege(p.oid,n.oid,'CREATE'))
 AND NOT EXISTS (SELECT FROM pg_catalog.pg_database d, principals p
   WHERE d.datname=current_database() AND d.datdba=p.oid)
 AND NOT EXISTS (SELECT FROM pg_catalog.pg_class c, principals p
   WHERE c.relnamespace IN (SELECT oid FROM namespaces) AND c.relowner=p.oid)
 AND NOT EXISTS (SELECT FROM pg_catalog.pg_type t, principals p
   WHERE t.typnamespace IN (SELECT oid FROM namespaces) AND t.typowner=p.oid)
 AND NOT EXISTS (SELECT FROM pg_catalog.pg_proc f, principals p
   WHERE f.pronamespace IN (SELECT oid FROM namespaces) AND f.proowner=p.oid)
 AND NOT EXISTS (SELECT FROM pg_catalog.pg_proc f, principals p
   WHERE f.pronamespace IN (SELECT oid FROM namespaces)
     AND has_function_privilege(p.oid,f.oid,'EXECUTE')
     AND (p.rolname=session_user OR f.oid NOT IN (SELECT function_id FROM permitted_functions)))
 AND NOT EXISTS (SELECT FROM permitted_functions
   WHERE NOT has_function_privilege(current_user,function_id,'EXECUTE'));
