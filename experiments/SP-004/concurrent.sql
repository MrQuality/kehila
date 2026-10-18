-- Disposable authority fixtures only; no authentication or production adapter.
BEGIN;
SET LOCAL ROLE kehila_owner;

CREATE TABLE sp004.creation_permission_changes (
  token kehila.operation_token PRIMARY KEY, manager kehila.ident NOT NULL, subject kehila.ident NOT NULL,
  previous boolean NOT NULL, active boolean NOT NULL
);
CREATE FUNCTION sp004.change_creation(
  p_manager kehila.ident, p_subject kehila.ident, p_expected boolean, p_active boolean, p_token kehila.operation_token
) RETURNS boolean LANGUAGE plpgsql SECURITY DEFINER
SET search_path = pg_catalog, kehila, sp004, pg_temp AS $$
DECLARE current_permission boolean;
BEGIN
  IF p_manager IS NULL OR p_subject IS NULL OR p_expected IS NULL OR p_active IS NULL
     OR p_token IS NULL THEN
    RAISE EXCEPTION 'creation management inputs required' USING ERRCODE='22004';
  END IF;
  PERFORM a.actor_id FROM kehila.actors a WHERE a.actor_id=ANY(ARRAY[p_manager,p_subject])
    ORDER BY a.actor_id COLLATE "C" FOR NO KEY UPDATE;
  IF NOT EXISTS (SELECT FROM kehila.actors a JOIN sp004.management_actors m USING(actor_id)
                 WHERE a.actor_id=p_manager AND a.active) THEN
    RAISE EXCEPTION 'creation management authority denied' USING ERRCODE='42501';
  END IF;
  SELECT can_create_project INTO current_permission FROM kehila.actors WHERE actor_id=p_subject;
  IF NOT FOUND THEN RAISE EXCEPTION 'creation subject missing' USING ERRCODE='23503'; END IF;
  IF current_permission <> p_expected THEN
    RAISE EXCEPTION 'stale creation permission' USING ERRCODE='40001';
  END IF;
  IF current_permission=p_active THEN
    RAISE EXCEPTION 'creation permission no-op' USING ERRCODE='22023';
  END IF;
  INSERT INTO sp004.creation_permission_changes VALUES
    (p_token,p_manager,p_subject,current_permission,p_active);
  UPDATE kehila.actors SET can_create_project=p_active WHERE actor_id=p_subject;
  RETURN p_active;
END;
$$;

CREATE FUNCTION sp004.fixture_create(p_actor kehila.ident,p_project kehila.ident,p_token kehila.operation_token)
RETURNS text LANGUAGE plpgsql SET search_path = pg_catalog, kehila, sp004, pg_temp AS $$
DECLARE a kehila.actors%ROWTYPE; o kehila.operations%ROWTYPE; op bigint; request bytea;
BEGIN
  SELECT * INTO a FROM kehila.lock_actor(p_actor);
  IF NOT FOUND OR NOT a.active OR NOT a.can_create_project THEN
    RAISE EXCEPTION 'creation authority denied' USING ERRCODE='42501';
  END IF;
  request := convert_to(jsonb_build_array('sp004_creation_fixture',p_actor,p_project,p_token)::text,'UTF8');
  SELECT * INTO o FROM kehila.operations WHERE actor_id=p_actor AND command_family='project_create'
    AND target_kind='project' AND target_project_id=p_project AND token=p_token;
  IF FOUND THEN
    IF o.required_grant <> 'project_create' OR o.grant_policy_version <> 1
       OR o.payload_retired OR o.request_codec_version <> 1
       OR o.request_sha256 <> sha256(request)
       OR NOT EXISTS (SELECT FROM kehila.operation_payloads p WHERE p.operation_row_id=o.operation_row_id
         AND p.request_bytes=request AND p.result_codec_version=1
         AND p.result=jsonb_build_object('kind','sp004_creation_fixture','project',p_project)) THEN
      RAISE EXCEPTION 'creation replay fixture mismatch' USING ERRCODE='XX001';
    END IF;
    RETURN 'replayed';
  END IF;
  INSERT INTO kehila.operations (
    actor_id,command_family,target_kind,target_project_id,token,request_codec_version,
    request_sha256,required_grant,grant_policy_version,replay_origin_ms,replay_deadline_ms
  ) VALUES (p_actor,'project_create','project',p_project,p_token,1,sha256(request),
            'project_create',1,0,7776000000) RETURNING operation_row_id INTO op;
  INSERT INTO kehila.operation_payloads VALUES
    (op,request,1,jsonb_build_object('kind','sp004_creation_fixture','project',p_project));
  INSERT INTO kehila.projects (
    project_id,name,current_prefix,estimate_unit,configuration_revision,next_sequence,
    created_by,creation_operation_row_id,seed_profile
  ) VALUES (p_project,'Concurrent structural fixture','CF','hours',1,1,p_actor,op,'m1_v1');
  INSERT INTO kehila.configuration_revisions VALUES
    (p_project,1,1,'{}','{}',p_actor,op,'project_create','project');
  INSERT INTO kehila.project_grants VALUES (p_project,p_actor,'initial_owner',true,1);
  INSERT INTO kehila.project_grant_events (
    project_id,actor_id,version,access_profile,active,granted_by,granting_operation_row_id,command_family
  ) VALUES (p_project,p_actor,1,'initial_owner',true,p_actor,op,'project_create');
  INSERT INTO kehila.statuses VALUES
    (p_project,'new','New','new',false,0),(p_project,'done','Done','done',false,1);
  INSERT INTO kehila.workflows VALUES (p_project,'flow','new','new',false,0);
  INSERT INTO kehila.workflow_statuses VALUES (p_project,'flow','new',0),(p_project,'flow','done',1);
  INSERT INTO kehila.workflow_phase_changes VALUES (p_project,'flow','new','done',0);
  INSERT INTO kehila.work_item_types VALUES (p_project,'task','flow','title',false,0);
  INSERT INTO kehila.type_workflows VALUES (p_project,'task','flow',0);
  INSERT INTO kehila.fields VALUES (p_project,'title','task','Title','text','application','optional',false,0);
  RETURN 'created';
END;
$$;

-- The following table models only a project-scoped command/replay outcome.
-- It never represents persisted WorkItem/configuration changes.
CREATE TABLE sp004.project_fixture_operations (
  actor_id kehila.ident NOT NULL, project_id kehila.ident NOT NULL, token kehila.operation_token NOT NULL,
  PRIMARY KEY(actor_id,project_id,token),
  FOREIGN KEY(project_id,actor_id) REFERENCES kehila.project_grants(project_id,actor_id)
);
CREATE FUNCTION sp004.fixture_save(p_actor kehila.ident,p_project kehila.ident,p_token kehila.operation_token)
RETURNS text LANGUAGE plpgsql SET search_path = pg_catalog, kehila, sp004, pg_temp AS $$
DECLARE a kehila.actors%ROWTYPE; g kehila.project_grants%ROWTYPE; e kehila.project_grant_events%ROWTYPE;
BEGIN
  SELECT * INTO a FROM kehila.lock_actor(p_actor);
  IF NOT FOUND OR NOT a.active THEN RAISE EXCEPTION 'project authority denied' USING ERRCODE='42501'; END IF;
  SELECT * INTO g FROM kehila.project_grants WHERE actor_id=p_actor AND project_id=p_project;
  IF NOT FOUND THEN RAISE EXCEPTION 'project authority denied' USING ERRCODE='42501'; END IF;
  SELECT * INTO e FROM kehila.project_grant_events WHERE actor_id=p_actor AND project_id=p_project
    ORDER BY version DESC LIMIT 1;
  IF NOT FOUND OR (g.version,g.active,g.access_profile) IS DISTINCT FROM (e.version,e.active,e.access_profile)
     OR g.access_profile <> 'initial_owner' THEN
    RAISE EXCEPTION 'project authority incoherent' USING ERRCODE='XX001';
  END IF;
  IF NOT g.active THEN RAISE EXCEPTION 'project authority denied' USING ERRCODE='42501'; END IF;
  IF EXISTS (SELECT FROM sp004.project_fixture_operations
             WHERE actor_id=p_actor AND project_id=p_project AND token=p_token) THEN RETURN 'replayed'; END IF;
  INSERT INTO sp004.project_fixture_operations VALUES (p_actor,p_project,p_token);
  RETURN 'saved';
END;
$$;

REVOKE ALL ON FUNCTION sp004.change_creation(kehila.ident,kehila.ident,boolean,boolean,kehila.operation_token),
  sp004.fixture_create(kehila.ident,kehila.ident,kehila.operation_token),
  sp004.fixture_save(kehila.ident,kehila.ident,kehila.operation_token) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION sp004.change_creation(kehila.ident,kehila.ident,boolean,boolean,kehila.operation_token) TO sp004_manager;
GRANT EXECUTE ON FUNCTION sp004.fixture_create(kehila.ident,kehila.ident,kehila.operation_token),
  sp004.fixture_save(kehila.ident,kehila.ident,kehila.operation_token) TO kehila_creator;
GRANT SELECT,INSERT ON sp004.project_fixture_operations TO kehila_creator;
COMMIT;
