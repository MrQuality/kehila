-- Confidential synthetic fixture state. Capture externally; never print/upload
-- this output: it includes the copied password verifier and audit records.
-- Compare before admitting an authority to the restored database.
-- PostgreSQL can flatten associative Boolean grouping when reparsing its own
-- CHECK deparse output. Reparse only checks on empty temporary objects; retain
-- original validation/inheritance metadata and raw definitions separately.
CREATE FUNCTION pg_temp.native_canonical_check(relation_id oid, type_id oid, definition text)
RETURNS text LANGUAGE plpgsql AS $$
DECLARE canonical text; base text; collation_name text; scratch_oid oid;
BEGIN
  IF relation_id <> 0 AND type_id = 0 THEN
    EXECUTE format('CREATE TEMP TABLE native_normalize_table (LIKE %s)',relation_id::regclass);
    EXECUTE 'ALTER TABLE pg_temp.native_normalize_table ADD CONSTRAINT native_check '||definition;
    scratch_oid := to_regclass('pg_temp.native_normalize_table')::oid;
    IF scratch_oid IS NULL THEN RAISE EXCEPTION 'scratch CHECK table missing'; END IF;
    SELECT pg_get_constraintdef(oid) INTO STRICT canonical FROM pg_constraint
      WHERE conrelid=scratch_oid AND conname='native_check';
    DROP TABLE pg_temp.native_normalize_table;
  ELSIF type_id <> 0 AND relation_id = 0 THEN
    SELECT format_type(typbasetype,typtypmod),
      CASE WHEN typcollation=0 THEN '' ELSE ' COLLATE '||typcollation::regcollation::text END
      INTO STRICT base,collation_name FROM pg_type WHERE oid=type_id AND typtype='d';
    EXECUTE 'CREATE DOMAIN pg_temp.native_normalize_domain AS '||base||collation_name||
      ' CONSTRAINT native_check '||definition;
    -- A literal regtype cast can retain an earlier OID in a cached PL/pgSQL
    -- statement after DROP/recreate. Resolve this owned generation explicitly.
    scratch_oid := to_regtype('pg_temp.native_normalize_domain')::oid;
    IF scratch_oid IS NULL THEN RAISE EXCEPTION 'scratch CHECK domain missing'; END IF;
    SELECT pg_get_constraintdef(oid) INTO STRICT canonical FROM pg_constraint
      WHERE contypid=scratch_oid AND conname='native_check';
    DROP DOMAIN pg_temp.native_normalize_domain;
  ELSE
    RAISE EXCEPTION 'unsupported CHECK normalization target';
  END IF;
  RETURN canonical;
END $$;
CREATE TEMP TABLE native_phase_control (LIKE kehila.workflow_phase_changes);
DO $$ DECLARE nested text; flat text; changed text; BEGIN
  nested := pg_temp.native_canonical_check(0,'kehila.operation_token'::regtype,
    $c$CHECK (((octet_length(VALUE)>=1 AND octet_length(VALUE)<=128) AND VALUE ~ '^[!-~]+$'))$c$);
  flat := pg_temp.native_canonical_check(0,'kehila.operation_token'::regtype,
    $c$CHECK (octet_length(VALUE)>=1 AND octet_length(VALUE)<=128 AND VALUE ~ '^[!-~]+$')$c$);
  changed := pg_temp.native_canonical_check(0,'kehila.operation_token'::regtype,
    $c$CHECK (octet_length(VALUE)>=1 AND octet_length(VALUE)<=127 AND VALUE ~ '^[!-~]+$')$c$);
  IF nested <> flat OR nested = changed THEN RAISE EXCEPTION 'scalar CHECK normalization control failed'; END IF;
  nested := pg_temp.native_canonical_check('pg_temp.native_phase_control'::regclass,0,
    $c$CHECK (((((from_phase='new' AND to_phase='active') OR (from_phase='new' AND to_phase='done'))
      OR (from_phase='active' AND to_phase='new')) OR (from_phase='active' AND to_phase='done'))
      OR (from_phase='done' AND to_phase='active'))$c$);
  flat := pg_temp.native_canonical_check('pg_temp.native_phase_control'::regclass,0,
    $c$CHECK ((from_phase='new' AND to_phase='active') OR (from_phase='new' AND to_phase='done')
      OR (from_phase='active' AND to_phase='new') OR (from_phase='active' AND to_phase='done')
      OR (from_phase='done' AND to_phase='active'))$c$);
  changed := pg_temp.native_canonical_check('pg_temp.native_phase_control'::regclass,0,
    $c$CHECK ((from_phase='new' AND to_phase='active') OR (from_phase='new' AND to_phase='done')
      OR (from_phase='active' AND to_phase='new') OR (from_phase='active' AND to_phase='done'))$c$);
  IF nested <> flat OR nested = changed THEN RAISE EXCEPTION 'phase CHECK normalization control failed'; END IF;
END $$;
DROP TABLE pg_temp.native_phase_control;
CREATE FUNCTION pg_temp.native_copy_snapshot() RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE state jsonb := jsonb_build_object('tables','{}'::jsonb,'sequences','{}'::jsonb);
  relation record; rows jsonb; metadata jsonb;
BEGIN
  FOR relation IN SELECT n.nspname,c.relname,c.relkind,c.oid
    FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
    WHERE n.nspname IN ('kehila','kehila_auth','kehila_native_fixture') AND c.relkind IN ('r','S')
    ORDER BY n.nspname,c.relname LOOP
    IF relation.relkind='r' THEN
      EXECUTE format('SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text),''[]''::jsonb) FROM %I.%I t',
        relation.nspname,relation.relname) INTO rows;
      state := jsonb_set(state,ARRAY['tables',relation.nspname||'.'||relation.relname],rows);
    ELSE
      EXECUTE format('SELECT jsonb_build_object(''last_value'',last_value::text,''is_called'',is_called) FROM %I.%I',
        relation.nspname,relation.relname) INTO rows;
      SELECT rows || to_jsonb(s) - 'seqrelid' INTO rows FROM pg_sequence s WHERE seqrelid=relation.oid;
      state := jsonb_set(state,ARRAY['sequences',relation.nspname||'.'||relation.relname],rows);
    END IF;
  END LOOP;
  SELECT jsonb_agg(jsonb_build_object('schema',n.nspname,'owner',n.nspowner::regrole::text,
    'acl',(SELECT jsonb_agg(a::text ORDER BY a::text) FROM unnest(coalesce(n.nspacl,acldefault('n',n.nspowner))) a)) ORDER BY n.nspname)
    INTO metadata FROM pg_namespace n WHERE n.nspname IN ('kehila','kehila_auth','kehila_native_fixture');
  state := state || jsonb_build_object('schemas',metadata);
  SELECT jsonb_agg(jsonb_build_object('name',n.nspname||'.'||c.relname,'kind',c.relkind,
    'owner',c.relowner::regrole::text,'acl',(SELECT jsonb_agg(a::text ORDER BY a::text)
      FROM unnest(coalesce(c.relacl,acldefault((CASE WHEN c.relkind='S' THEN 'S' ELSE 'r' END)::"char",c.relowner))) a))
    ORDER BY n.nspname,c.relname) INTO metadata FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
    WHERE n.nspname IN ('kehila','kehila_auth','kehila_native_fixture') AND c.relkind IN ('r','S','i','c');
  state := state || jsonb_build_object('relations',metadata);
  SELECT jsonb_agg(jsonb_build_object('name',n.nspname||'.'||t.typname,'kind',t.typtype,
    'owner',t.typowner::regrole::text,'acl',(SELECT jsonb_agg(a::text ORDER BY a::text) FROM unnest(coalesce(t.typacl,acldefault('T',t.typowner))) a),
    'base',CASE WHEN t.typbasetype=0 THEN NULL ELSE format_type(t.typbasetype,t.typtypmod) END,
    'notnull',t.typnotnull,'default',t.typdefault,'collation',t.typcollation::regcollation::text)
    ORDER BY n.nspname,t.typname) INTO metadata FROM pg_type t JOIN pg_namespace n ON n.oid=t.typnamespace
    WHERE n.nspname IN ('kehila','kehila_auth','kehila_native_fixture');
  state := state || jsonb_build_object('types',metadata);
  SELECT jsonb_agg(pg_get_indexdef(c.oid) ORDER BY n.nspname,c.relname)
    INTO metadata FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
    WHERE c.relkind='i' AND n.nspname IN ('kehila','kehila_auth','kehila_native_fixture');
  state := state || jsonb_build_object('indexes',metadata);
  SELECT jsonb_agg(jsonb_build_object('relation',n.nspname||'.'||c.relname,'position',a.attnum,
    'name',a.attname,'type',format_type(a.atttypid,a.atttypmod),'notnull',a.attnotnull,
    'identity',a.attidentity,'generated',a.attgenerated,'default',pg_get_expr(d.adbin,d.adrelid),
    'collation',a.attcollation::regcollation::text)
    ORDER BY n.nspname,c.relname,a.attnum) INTO metadata
    FROM pg_attribute a JOIN pg_class c ON c.oid=a.attrelid JOIN pg_namespace n ON n.oid=c.relnamespace
    LEFT JOIN pg_attrdef d ON d.adrelid=c.oid AND d.adnum=a.attnum
    WHERE n.nspname IN ('kehila','kehila_auth','kehila_native_fixture') AND c.relkind IN ('r','c')
      AND a.attnum>0 AND NOT a.attisdropped;
  state := state || jsonb_build_object('columns',metadata);
  SELECT jsonb_agg(jsonb_build_object('schema',n.nspname,'name',p.proname,
    'arguments',pg_get_function_identity_arguments(p.oid),'owner',p.proowner::regrole::text,
    'acl',(SELECT jsonb_agg(a::text ORDER BY a::text) FROM unnest(coalesce(p.proacl,acldefault('f',p.proowner))) a),
    'definition',pg_get_functiondef(p.oid)) ORDER BY n.nspname,p.proname,pg_get_function_identity_arguments(p.oid))
    INTO metadata FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace
    WHERE n.nspname IN ('kehila','kehila_auth','kehila_native_fixture');
  state := state || jsonb_build_object('functions',metadata);
  SELECT jsonb_agg(jsonb_build_object('schema',n.nspname,'name',c.conname,
    'definition',pg_get_constraintdef(c.oid)) ORDER BY n.nspname,c.conname,c.conrelid::regclass::text)
    INTO metadata FROM pg_constraint c JOIN pg_namespace n ON n.oid=c.connamespace
    WHERE c.contype='c' AND n.nspname IN ('kehila','kehila_auth','kehila_native_fixture');
  state := state || jsonb_build_object('raw_checks',metadata);
  SELECT jsonb_agg(jsonb_build_object('schema',n.nspname,'name',c.conname,
    'relation',CASE WHEN c.conrelid=0 THEN NULL ELSE c.conrelid::regclass::text END,
    'type',CASE WHEN c.contypid=0 THEN NULL ELSE c.contypid::regtype::text END,
    'definition',CASE WHEN c.contype='c' THEN pg_temp.native_canonical_check(c.conrelid,c.contypid,pg_get_constraintdef(c.oid))
      ELSE pg_get_constraintdef(c.oid) END,'validated',c.convalidated,
    'deferrable',c.condeferrable,'deferred',c.condeferred,'noinherit',c.connoinherit,
    'islocal',c.conislocal,'inherit_count',c.coninhcount) ORDER BY n.nspname,c.conname,c.conrelid::regclass::text)
    INTO metadata FROM pg_constraint c JOIN pg_namespace n ON n.oid=c.connamespace
    WHERE n.nspname IN ('kehila','kehila_auth','kehila_native_fixture');
  state := state || jsonb_build_object('constraints',metadata);
  SELECT jsonb_agg(jsonb_build_object('relation',n.nspname||'.'||c.relname,'name',t.tgname,
    'enabled',t.tgenabled,'definition',pg_get_triggerdef(t.oid)) ORDER BY n.nspname,c.relname,t.tgname)
    INTO metadata FROM pg_trigger t JOIN pg_class c ON c.oid=t.tgrelid JOIN pg_namespace n ON n.oid=c.relnamespace
    WHERE NOT t.tgisinternal AND n.nspname IN ('kehila','kehila_auth','kehila_native_fixture');
  RETURN state || jsonb_build_object('triggers',metadata);
END $$;
SELECT pg_temp.native_copy_snapshot();
