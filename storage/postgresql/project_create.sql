-- Native PostgreSQL 16 baseline. Installed only through install.sql.
CREATE DOMAIN kehila.project_prefix AS text COLLATE "C"
  CHECK (VALUE ~ '^[A-Z][A-Z0-9]{1,11}$');
CREATE DOMAIN kehila.phase AS text COLLATE "C"
  CHECK (VALUE IN ('new', 'active', 'done'));

CREATE TABLE kehila.projects (
  project_id kehila.ident PRIMARY KEY,
  name text NOT NULL CHECK (octet_length(name) BETWEEN 1 AND 256),
  current_prefix kehila.project_prefix NOT NULL,
  estimate_unit text COLLATE "C" NOT NULL
    CHECK (estimate_unit IN ('hours', 'points')),
  configuration_revision kehila.u64 NOT NULL CHECK (configuration_revision >= 1),
  next_sequence kehila.u64 NOT NULL CHECK (next_sequence >= 1),
  ever_estimated boolean NOT NULL DEFAULT false,
  archived boolean NOT NULL DEFAULT false,
  created_by kehila.ident NOT NULL,
  creation_operation_row_id bigint NOT NULL UNIQUE,
  creation_command_family text COLLATE "C" NOT NULL DEFAULT 'project_create'
    CHECK (creation_command_family = 'project_create'),
  creation_target_kind text COLLATE "C" NOT NULL DEFAULT 'project'
    CHECK (creation_target_kind = 'project'),
  seed_profile text COLLATE "C" NOT NULL,
  FOREIGN KEY (creation_operation_row_id, creation_command_family,
               creation_target_kind, project_id, created_by)
    REFERENCES kehila.operations
      (operation_row_id, command_family, target_kind, target_project_id, actor_id)
    ON DELETE RESTRICT
);

CREATE TABLE kehila.configuration_revisions (
  project_id kehila.ident NOT NULL REFERENCES kehila.projects(project_id)
    ON DELETE RESTRICT,
  revision kehila.u64 NOT NULL CHECK (revision >= 1),
  snapshot_codec_version integer NOT NULL CHECK (snapshot_codec_version > 0),
  configuration_snapshot jsonb NOT NULL
    CHECK (jsonb_typeof(configuration_snapshot) = 'object'),
  project_snapshot jsonb NOT NULL
    CHECK (jsonb_typeof(project_snapshot) = 'object'),
  actor_id kehila.ident NOT NULL,
  operation_row_id bigint NOT NULL,
  command_family text COLLATE "C" NOT NULL,
  target_kind text COLLATE "C" NOT NULL DEFAULT 'project' CHECK (target_kind = 'project'),
  PRIMARY KEY (project_id, revision),
  FOREIGN KEY (operation_row_id, command_family, target_kind, project_id, actor_id)
    REFERENCES kehila.operations
      (operation_row_id, command_family, target_kind, target_project_id, actor_id)
    ON DELETE RESTRICT
);
ALTER TABLE kehila.projects ADD CONSTRAINT projects_current_revision
  FOREIGN KEY (project_id, configuration_revision)
  REFERENCES kehila.configuration_revisions(project_id, revision)
  ON DELETE NO ACTION DEFERRABLE INITIALLY DEFERRED;
CREATE TABLE kehila.project_grants (
  project_id kehila.ident NOT NULL REFERENCES kehila.projects(project_id)
    ON DELETE RESTRICT,
  actor_id kehila.ident NOT NULL REFERENCES kehila.actors(actor_id)
    ON DELETE RESTRICT,
  access_profile text COLLATE "C" NOT NULL,
  active boolean NOT NULL,
  version kehila.u64 NOT NULL CHECK (version >= 1),
  PRIMARY KEY (project_id, actor_id)
);
CREATE INDEX project_grants_by_actor ON kehila.project_grants(actor_id, project_id);

CREATE TABLE kehila.project_grant_events (
  project_id kehila.ident NOT NULL,
  actor_id kehila.ident NOT NULL,
  version kehila.u64 NOT NULL CHECK (version >= 1),
  access_profile text COLLATE "C" NOT NULL,
  active boolean NOT NULL,
  granted_by kehila.ident NOT NULL,
  granting_operation_row_id bigint NOT NULL,
  command_family text COLLATE "C" NOT NULL,
  target_kind text COLLATE "C" NOT NULL DEFAULT 'project' CHECK (target_kind = 'project'),
  recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (project_id, actor_id, version),
  FOREIGN KEY (project_id, actor_id) REFERENCES kehila.project_grants(project_id, actor_id)
    ON DELETE RESTRICT,
  FOREIGN KEY (granting_operation_row_id, command_family, target_kind,
               project_id, granted_by)
    REFERENCES kehila.operations
      (operation_row_id, command_family, target_kind, target_project_id, actor_id)
    ON DELETE RESTRICT
);

-- Temporary slice guards, separately named for deliberate evolution.
ALTER TABLE kehila.projects ADD CONSTRAINT slice1_seed_profile CHECK (seed_profile = 'm1_v1');
ALTER TABLE kehila.configuration_revisions ADD CONSTRAINT slice1_revision_family
  CHECK (command_family = 'project_create');
ALTER TABLE kehila.project_grants ADD CONSTRAINT slice1_grant_profile
  CHECK (access_profile = 'initial_owner');
ALTER TABLE kehila.project_grant_events ADD CONSTRAINT slice1_grant_event
  CHECK (command_family = 'project_create' AND access_profile = 'initial_owner'
         AND active AND version = 1 AND actor_id = granted_by);

CREATE TABLE kehila.statuses (
  project_id kehila.ident NOT NULL REFERENCES kehila.projects(project_id),
  status_id kehila.ident NOT NULL,
  name text NOT NULL CHECK (octet_length(name) BETWEEN 1 AND 256),
  phase kehila.phase NOT NULL,
  archived boolean NOT NULL,
  position smallint NOT NULL CHECK (position >= 0),
  PRIMARY KEY (project_id, status_id),
  UNIQUE (project_id, status_id, phase),
  UNIQUE (project_id, position) DEFERRABLE INITIALLY DEFERRED
);
CREATE TABLE kehila.workflows (
  project_id kehila.ident NOT NULL REFERENCES kehila.projects(project_id),
  workflow_id kehila.ident NOT NULL,
  initial_status_id kehila.ident NOT NULL,
  initial_phase kehila.phase NOT NULL DEFAULT 'new' CHECK (initial_phase = 'new'),
  archived boolean NOT NULL,
  position smallint NOT NULL CHECK (position >= 0),
  PRIMARY KEY (project_id, workflow_id),
  UNIQUE (project_id, position) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY (project_id, initial_status_id, initial_phase)
    REFERENCES kehila.statuses(project_id, status_id, phase)
);
CREATE TABLE kehila.workflow_statuses (
  project_id kehila.ident NOT NULL,
  workflow_id kehila.ident NOT NULL,
  status_id kehila.ident NOT NULL,
  position smallint NOT NULL CHECK (position >= 0),
  PRIMARY KEY (project_id, workflow_id, status_id),
  UNIQUE (project_id, workflow_id, position) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY (project_id, workflow_id)
    REFERENCES kehila.workflows(project_id, workflow_id),
  FOREIGN KEY (project_id, status_id)
    REFERENCES kehila.statuses(project_id, status_id)
);
ALTER TABLE kehila.workflows ADD CONSTRAINT workflow_initial_membership
  FOREIGN KEY (project_id, workflow_id, initial_status_id)
  REFERENCES kehila.workflow_statuses(project_id, workflow_id, status_id)
  ON DELETE NO ACTION DEFERRABLE INITIALLY DEFERRED;

CREATE TABLE kehila.workflow_phase_changes (
  project_id kehila.ident NOT NULL,
  workflow_id kehila.ident NOT NULL,
  from_phase kehila.phase NOT NULL,
  to_phase kehila.phase NOT NULL,
  position smallint NOT NULL CHECK (position >= 0),
  PRIMARY KEY (project_id, workflow_id, from_phase, to_phase),
  UNIQUE (project_id, workflow_id, position) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY (project_id, workflow_id)
    REFERENCES kehila.workflows(project_id, workflow_id),
  CHECK ((from_phase, to_phase) IN
    (('new','active'), ('new','done'), ('active','new'),
     ('active','done'), ('done','active')))
);
CREATE TABLE kehila.work_item_types (
  project_id kehila.ident NOT NULL REFERENCES kehila.projects(project_id),
  type_id kehila.ident NOT NULL,
  default_workflow_id kehila.ident NOT NULL,
  title_field_id kehila.ident,
  archived boolean NOT NULL,
  position smallint NOT NULL CHECK (position >= 0),
  PRIMARY KEY (project_id, type_id),
  UNIQUE (project_id, position) DEFERRABLE INITIALLY DEFERRED
);
CREATE TABLE kehila.type_workflows (
  project_id kehila.ident NOT NULL,
  type_id kehila.ident NOT NULL,
  workflow_id kehila.ident NOT NULL,
  position smallint NOT NULL CHECK (position >= 0),
  PRIMARY KEY (project_id, type_id, workflow_id),
  UNIQUE (project_id, type_id, position) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY (project_id, type_id)
    REFERENCES kehila.work_item_types(project_id, type_id),
  FOREIGN KEY (project_id, workflow_id)
    REFERENCES kehila.workflows(project_id, workflow_id)
);
ALTER TABLE kehila.work_item_types ADD CONSTRAINT type_default_membership
  FOREIGN KEY (project_id, type_id, default_workflow_id)
  REFERENCES kehila.type_workflows(project_id, type_id, workflow_id)
  ON DELETE NO ACTION DEFERRABLE INITIALLY DEFERRED;

CREATE TABLE kehila.fields (
  project_id kehila.ident NOT NULL,
  field_id kehila.ident NOT NULL,
  owner_type_id kehila.ident NOT NULL,
  name text NOT NULL CHECK (octet_length(name) BETWEEN 1 AND 256),
  value_kind text COLLATE "C" NOT NULL,
  origin text COLLATE "C" NOT NULL,
  usage text COLLATE "C" NOT NULL,
  archived boolean NOT NULL,
  position smallint NOT NULL CHECK (position >= 0),
  PRIMARY KEY (project_id, field_id),
  UNIQUE (project_id, owner_type_id, field_id),
  UNIQUE (project_id, position) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY (project_id, owner_type_id)
    REFERENCES kehila.work_item_types(project_id, type_id)
);
ALTER TABLE kehila.work_item_types ADD CONSTRAINT type_title_field
  FOREIGN KEY (project_id, type_id, title_field_id)
  REFERENCES kehila.fields(project_id, owner_type_id, field_id)
  ON DELETE NO ACTION DEFERRABLE INITIALLY DEFERRED;

-- First-loader restrictions, not the complete field vocabulary.
ALTER TABLE kehila.fields ADD CONSTRAINT slice1_field_kind CHECK (value_kind = 'text');
ALTER TABLE kehila.fields ADD CONSTRAINT slice1_field_origin CHECK (origin = 'application');
ALTER TABLE kehila.fields ADD CONSTRAINT slice1_field_usage CHECK (usage = 'optional');
ALTER TABLE kehila.fields ADD CONSTRAINT slice1_field_archival CHECK (NOT archived);
