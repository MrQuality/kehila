-- Fresh v3 preparation: immutable representation agreement, not a second
-- prospective business-rule engine. These checks run on new records only.
CREATE FUNCTION kehila.check_item_history() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, kehila, pg_temp AS $$
DECLARE
  project record; item kehila.items; snapshot jsonb; snapshot_value jsonb; snapshot_codec integer;
  saved jsonb; data_values jsonb; expected_item jsonb; expected_project jsonb;
  expected_stable jsonb; expected_value jsonb; expected_kind text;
  previous record; entry jsonb; entry_id text; field_owner kehila.ident; field_kind text;
BEGIN
  IF NEW.history_codec_version IS DISTINCT FROM 1 OR jsonb_typeof(NEW.history) IS DISTINCT FROM 'object'
     OR octet_length(NEW.history::text)>1048576 THEN
    RAISE EXCEPTION 'invalid item history representation' USING ERRCODE='23514', CONSTRAINT='item_history_agreement';
  END IF;
  saved := NEW.history #> '{value,result}';
  -- JSONB considers numeric 1.0 equal to 1, but its rendered form is not an
  -- integer token accepted by the frozen Rust envelope decoder.
  IF jsonb_typeof(NEW.history->'codec_version') IS DISTINCT FROM 'number'
     OR NEW.history->>'codec_version' IS DISTINCT FROM '1'
     OR jsonb_typeof(saved->'codec_version') IS DISTINCT FROM 'number'
     OR saved->>'codec_version' IS DISTINCT FROM '1' THEN
    RAISE EXCEPTION 'noncanonical history codec version' USING ERRCODE='23514', CONSTRAINT='item_history_agreement';
  END IF;
  data_values := saved #> '{value,data,values}';
  IF jsonb_typeof(data_values) IS DISTINCT FROM 'array' THEN
    RAISE EXCEPTION 'invalid retained field collection' USING ERRCODE='23514', CONSTRAINT='item_history_fields';
  END IF;
  -- 128 bounds a supplied sparse change set, not the resulting retained values.
  -- Use the complete storage-record codec ceiling before expanding this array.
  IF jsonb_array_length(data_values)>1024 THEN
    RAISE EXCEPTION 'retained field collection exceeds record bound' USING ERRCODE='23514', CONSTRAINT='item_history_fields';
  END IF;
  SELECT p.name,p.current_prefix,p.estimate_unit,p.configuration_revision,p.archived
    INTO STRICT project FROM kehila.projects p WHERE p.project_id=NEW.project_id;
  SELECT i.* INTO STRICT item FROM kehila.items i
    WHERE i.project_id=NEW.project_id AND i.item_id=NEW.item_id;
  SELECT c.snapshot_codec_version,
      CASE WHEN octet_length(c.project_snapshot::text)<=1048576 THEN c.project_snapshot END
    INTO STRICT snapshot_codec,snapshot FROM kehila.configuration_revisions c
    WHERE c.project_id=NEW.project_id AND c.revision=NEW.configuration_revision;
  snapshot_value := snapshot -> 'value';
  IF snapshot_codec<>1 OR jsonb_typeof(snapshot_value) IS DISTINCT FROM 'object'
     OR jsonb_typeof(snapshot->'codec_version') IS DISTINCT FROM 'number'
     OR snapshot->>'codec_version' IS DISTINCT FROM '1'
     OR jsonb_typeof(snapshot_value->'next_sequence') IS DISTINCT FROM 'string'
     OR jsonb_typeof(snapshot_value->'ever_estimated') IS DISTINCT FROM 'boolean' THEN
    RAISE EXCEPTION 'invalid retained project representation' USING ERRCODE='23514', CONSTRAINT='item_history_agreement';
  END IF;
  -- SQL numeric retains scale; normalize an integral u64 to its canonical
  -- decimal string rather than requiring a particular physical numeric scale.
  expected_stable := jsonb_build_object('id',NEW.project_id,'name',project.name,
    'prefix',project.current_prefix,'estimate_unit',project.estimate_unit,
    'configuration_revision',NEW.configuration_revision::numeric(20,0)::text,'archived',project.archived);
  IF snapshot IS DISTINCT FROM jsonb_build_object('codec_version',1,'kind','project_snapshot','value',snapshot_value)
     OR (snapshot_value - ARRAY['next_sequence','ever_estimated']) IS DISTINCT FROM expected_stable
     OR project.configuration_revision<>NEW.configuration_revision OR project.estimate_unit<>NEW.estimate_unit THEN
    RAISE EXCEPTION 'item history project snapshot disagreement' USING ERRCODE='23514', CONSTRAINT='item_history_agreement';
  END IF;
  expected_project := expected_stable || jsonb_build_object(
    'next_sequence',NEW.resulting_next_sequence::numeric(20,0)::text,'ever_estimated',NEW.resulting_ever_estimated);
  expected_item := jsonb_build_object('id',NEW.item_id,'project_id',NEW.project_id,'type_id',NEW.type_id,
    'workflow_id',NEW.workflow_id,'status_id',NEW.status_id,'version',NEW.item_version::numeric(20,0)::text,
    'archived',NEW.archived);
  expected_value := jsonb_build_object('project',expected_project,'item',expected_item,
    'data',jsonb_build_object('estimate',coalesce(to_jsonb(NEW.estimate_thousandths::text),'null'::jsonb),
      'values',data_values),'phase',NEW.phase);
  IF NEW.command_family='item_create' THEN
    expected_kind := 'item_create_result';
    IF item.creation_operation_row_id<>NEW.operation_row_id OR item.issued_prefix<>project.current_prefix
       OR NEW.resulting_next_sequence<>item.issued_sequence+1 THEN
      RAISE EXCEPTION 'item creation readable identity disagreement' USING ERRCODE='23514', CONSTRAINT='item_history_agreement';
    END IF;
    expected_value := expected_value || jsonb_build_object('readable_id',jsonb_build_object(
      'project_id',NEW.project_id,'item_id',NEW.item_id,'prefix',item.issued_prefix,
      'sequence',item.issued_sequence::numeric(20,0)::text));
  ELSE
    expected_kind := 'item_edit_result';
  END IF;
  IF saved IS DISTINCT FROM jsonb_build_object('codec_version',1,'kind',expected_kind,'value',expected_value)
     OR NEW.history IS DISTINCT FROM jsonb_build_object('codec_version',1,'kind','item_revision','value',
       jsonb_build_object('actor_id',NEW.actor_id,'configuration_revision',NEW.configuration_revision::numeric(20,0)::text,
         'project_state_version',NEW.project_state_version::numeric(20,0)::text,
         'previous_project_operation',NEW.previous_project_operation::text,'result',saved)) THEN
    RAISE EXCEPTION 'item history scalar disagreement' USING ERRCODE='23514', CONSTRAINT='item_history_agreement';
  END IF;
  IF NEW.command_family='item_edit' THEN
    SELECT r.type_id,r.workflow_id,r.status_id,r.phase,r.archived INTO STRICT previous
      FROM kehila.item_revisions r WHERE r.project_id=NEW.project_id AND r.item_id=NEW.item_id
        AND r.item_version=NEW.previous_item_version;
    IF ROW(NEW.type_id,NEW.workflow_id,NEW.status_id,NEW.phase,NEW.archived)
        IS DISTINCT FROM ROW(previous.type_id,previous.workflow_id,previous.status_id,previous.phase,previous.archived) THEN
      RAISE EXCEPTION 'ordinary edit changes item lifecycle' USING ERRCODE='23514', CONSTRAINT='item_edit_lifecycle';
    END IF;
  END IF;
  FOR entry IN SELECT value FROM jsonb_array_elements(data_values) LOOP
    IF jsonb_typeof(entry->'id') IS DISTINCT FROM 'string'
       OR entry IS DISTINCT FROM jsonb_build_object('id',entry->'id','value',entry->'value')
       OR jsonb_typeof(entry #> '{value,value}') IS DISTINCT FROM 'string'
       OR entry->'value' IS DISTINCT FROM jsonb_build_object('kind','text','value',entry #> '{value,value}') THEN
      RAISE EXCEPTION 'invalid native text field representation' USING ERRCODE='23514', CONSTRAINT='item_history_fields';
    END IF;
    entry_id := entry->>'id';
    IF octet_length(entry_id)>128 OR octet_length(entry #>> '{value,value}')>65536 THEN
      RAISE EXCEPTION 'field representation exceeds record bound' USING ERRCODE='23514', CONSTRAINT='item_history_fields';
    END IF;
    SELECT f.owner_type_id,f.value_kind INTO field_owner,field_kind FROM kehila.fields f
      WHERE f.project_id=NEW.project_id AND f.field_id=entry_id;
    IF NOT FOUND OR field_owner<>NEW.type_id OR field_kind<>'text' THEN
      RAISE EXCEPTION 'field definition ownership disagreement' USING ERRCODE='23514', CONSTRAINT='item_history_field_reference';
    END IF;
  END LOOP;
  IF (SELECT count(DISTINCT (value->>'id') COLLATE "C") FROM jsonb_array_elements(data_values))
       <>jsonb_array_length(data_values) THEN
    RAISE EXCEPTION 'duplicate retained field identity' USING ERRCODE='23514', CONSTRAINT='item_history_fields';
  END IF;
  RETURN NEW;
EXCEPTION WHEN NO_DATA_FOUND OR TOO_MANY_ROWS THEN
  RAISE EXCEPTION 'item history reference unavailable' USING ERRCODE='23514', CONSTRAINT='item_history_agreement';
END;
$$;
CREATE TRIGGER item_history_agreement BEFORE INSERT ON kehila.item_revisions
  FOR EACH ROW EXECUTE FUNCTION kehila.check_item_history();

CREATE FUNCTION kehila.check_item_field_use() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, kehila, pg_temp AS $$
DECLARE referenced_type kehila.ident; data_values jsonb;
BEGIN
  SELECT r.type_id,CASE WHEN octet_length(r.history::text)<=1048576
      THEN r.history #> '{value,result,value,data,values}' END
    INTO referenced_type,data_values FROM kehila.item_revisions r
    WHERE r.project_id=NEW.project_id AND r.item_id=NEW.first_item_id
      AND r.item_version=NEW.first_item_version AND r.operation_row_id=NEW.first_operation_row_id;
  IF NOT FOUND OR referenced_type<>NEW.owner_type_id OR jsonb_typeof(data_values) IS DISTINCT FROM 'array' THEN
    RAISE EXCEPTION 'invalid first valued-field use reference' USING ERRCODE='23514', CONSTRAINT='item_field_use_reference';
  END IF;
  IF jsonb_array_length(data_values)>1024 THEN
    RAISE EXCEPTION 'first-use history exceeds collection bound' USING ERRCODE='23514', CONSTRAINT='item_field_use_reference';
  END IF;
  IF NOT EXISTS (SELECT FROM jsonb_array_elements(data_values) e
      WHERE e.value->'id'=to_jsonb(NEW.field_id::text)
        AND jsonb_typeof(e.value #> '{value,value}')='string') THEN
    RAISE EXCEPTION 'first use has no accepted value' USING ERRCODE='23514', CONSTRAINT='item_field_use_reference';
  END IF;
  RETURN NEW;
END;
$$;
CREATE TRIGGER item_field_use_reference BEFORE INSERT ON kehila.item_field_uses
  FOR EACH ROW EXECUTE FUNCTION kehila.check_item_field_use();

-- Deferred checks can run after the save entry has returned to the serving
-- role. Validate private first-use evidence with the schema owner's authority.
CREATE FUNCTION kehila.check_item_revision_uses() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, kehila, pg_temp AS $$
DECLARE data_values jsonb; entry jsonb;
BEGIN
  data_values := NEW.history #> '{value,result,value,data,values}';
  IF jsonb_typeof(data_values) IS DISTINCT FROM 'array' THEN
    RAISE EXCEPTION 'invalid retained field collection' USING ERRCODE='23514', CONSTRAINT='item_field_use_missing';
  END IF;
  IF jsonb_array_length(data_values)>1024 THEN
    RAISE EXCEPTION 'retained fields exceed collection bound' USING ERRCODE='23514', CONSTRAINT='item_field_use_missing';
  END IF;
  FOR entry IN SELECT value FROM jsonb_array_elements(data_values) LOOP
    IF NOT EXISTS (SELECT FROM kehila.item_field_uses u JOIN kehila.item_revisions first_use
        ON first_use.project_id=u.project_id AND first_use.item_id=u.first_item_id
          AND first_use.item_version=u.first_item_version AND first_use.operation_row_id=u.first_operation_row_id
        WHERE u.project_id=NEW.project_id AND u.owner_type_id=NEW.type_id AND u.field_id=entry->>'id'
          AND first_use.project_state_version<=NEW.project_state_version) THEN
      RAISE EXCEPTION 'valued field lacks retained first use' USING ERRCODE='23514', CONSTRAINT='item_field_use_missing';
    END IF;
  END LOOP;
  RETURN NULL;
END;
$$;
CREATE CONSTRAINT TRIGGER item_revision_field_uses AFTER INSERT ON kehila.item_revisions
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION kehila.check_item_revision_uses();

REVOKE ALL ON FUNCTION kehila.check_item_history(), kehila.check_item_field_use(),
  kehila.check_item_revision_uses() FROM PUBLIC;

-- Every valued revision requires immutable use evidence no later than itself,
-- and every use points to an actual value in that exact immutable revision.
-- Together these enforce the first-use boundary inductively without scanning
-- all historical revisions. No current hidden/required/archive business rule is
-- applied to old history during replay or retirement; pure Rust owns those rules.
