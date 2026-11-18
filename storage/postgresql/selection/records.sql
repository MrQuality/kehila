-- Fresh-v4 selection identity and permanent success evidence. No serving DDL.
ALTER TABLE kehila.operations ADD CONSTRAINT selection_operation_attribution UNIQUE
  (operation_row_id,command_family,target_kind,target_user_id,actor_id,token);

CREATE TABLE kehila.selection_events (
  user_id kehila.ident NOT NULL REFERENCES kehila.actors(actor_id),
  event_ordinal kehila.u64 NOT NULL CHECK (event_ordinal>=1),
  previous_event_ordinal kehila.u64,
  actor_id kehila.ident NOT NULL,
  command_family text COLLATE "C" NOT NULL CHECK(command_family='selection'),
  target_kind text COLLATE "C" NOT NULL CHECK(target_kind='user'),
  token kehila.operation_token NOT NULL,
  operation_row_id bigint NOT NULL UNIQUE,
  before_version kehila.u64 NOT NULL,
  before_project_id kehila.ident,
  before_item_id kehila.ident,
  selection_version kehila.u64 NOT NULL,
  selected_project_id kehila.ident,
  selected_item_id kehila.ident,
  history_codec_version bigint NOT NULL CHECK(history_codec_version=1),
  history jsonb NOT NULL CHECK(jsonb_typeof(history)='object' AND octet_length(history::text)<=16384),
  PRIMARY KEY(user_id,event_ordinal),
  CONSTRAINT selection_subject_actor CHECK(actor_id=user_id),
  CONSTRAINT selection_token_shape CHECK(token ~ '^[0-9a-f]{32}$'),
  CONSTRAINT selection_target_pair CHECK
    ((before_project_id IS NULL)=(before_item_id IS NULL)
     AND (selected_project_id IS NULL)=(selected_item_id IS NULL)),
  CONSTRAINT selection_initial_shape CHECK
    ((before_version<>0 OR before_project_id IS NULL)
     AND (selection_version<>0 OR selected_project_id IS NULL)),
  CONSTRAINT selection_ordinal_step CHECK
    ((event_ordinal=1 AND previous_event_ordinal IS NULL)
     OR (event_ordinal>1 AND previous_event_ordinal IS NOT NULL
       AND previous_event_ordinal=event_ordinal-1)),
  CONSTRAINT selection_version_step CHECK
    (((selected_project_id,selected_item_id) IS NOT DISTINCT FROM (before_project_id,before_item_id)
       AND selection_version=before_version)
     OR ((selected_project_id,selected_item_id) IS DISTINCT FROM (before_project_id,before_item_id)
       AND selection_version=before_version+1)),
  CONSTRAINT selection_event_operation FOREIGN KEY
    (operation_row_id,command_family,target_kind,user_id,actor_id,token)
    REFERENCES kehila.operations
      (operation_row_id,command_family,target_kind,target_user_id,actor_id,token),
  CONSTRAINT selection_event_predecessor FOREIGN KEY(user_id,previous_event_ordinal)
    REFERENCES kehila.selection_events(user_id,event_ordinal),
  CONSTRAINT selection_event_target FOREIGN KEY(selected_project_id,selected_item_id)
    REFERENCES kehila.items(project_id,item_id)
);

CREATE TABLE kehila.current_work_selections (
  user_id kehila.ident PRIMARY KEY REFERENCES kehila.actors(actor_id),
  selection_version kehila.u64 NOT NULL,
  selected_project_id kehila.ident,
  selected_item_id kehila.ident,
  event_ordinal kehila.u64 NOT NULL CHECK(event_ordinal>=1),
  CONSTRAINT selection_current_pair CHECK
    ((selected_project_id IS NULL)=(selected_item_id IS NULL)),
  CONSTRAINT selection_current_event FOREIGN KEY(user_id,event_ordinal)
    REFERENCES kehila.selection_events(user_id,event_ordinal)
);

-- Before-target identity is proved by the immutable predecessor, and current
-- target identity by the exact referenced event. Avoid redundant item FKs that
-- would acquire implicit earlier item locks after the selection lock class.
REVOKE ALL ON kehila.selection_events,kehila.current_work_selections FROM PUBLIC;
