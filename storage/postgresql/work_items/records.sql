-- Source preparation for accepted ADR-111. Not included by an installer yet.
-- Include as kehila_owner only with the reviewed transition/capability packet.
-- These keys describe references; transition guards are separately required.

-- Identity generation does not constrain explicit OVERRIDING SYSTEM VALUE
-- inserts. All core references use the positive signed-bigint vocabulary.
ALTER TABLE kehila.operations ADD CONSTRAINT operations_positive_id
  CHECK (operation_row_id > 0);

ALTER TABLE kehila.operations ADD CONSTRAINT operations_item_attribution UNIQUE
  (operation_row_id, actor_id, command_family, target_kind,
   target_project_id, target_item_id, token);

CREATE TABLE kehila_auth.pending_items (
  request_id bigint GENERATED ALWAYS AS IDENTITY (MINVALUE 1 NO CYCLE) PRIMARY KEY,
  installation_id kehila.ident NOT NULL,
  actor_id kehila.ident NOT NULL,
  project_id kehila.ident NOT NULL REFERENCES kehila.projects(project_id),
  command_family text COLLATE "C" NOT NULL DEFAULT 'item_create'
    CHECK (command_family = 'item_create'),
  target_kind text COLLATE "C" NOT NULL DEFAULT 'work_item'
    CHECK (target_kind = 'work_item'),
  intent_handle kehila_auth.intent_key NOT NULL,
  token kehila.operation_token NOT NULL,
  target_item_id kehila.ident NOT NULL UNIQUE,
  request_codec_version bigint NOT NULL
    CHECK (request_codec_version BETWEEN 1 AND 4294967295),
  request_sha256 bytea NOT NULL CHECK (octet_length(request_sha256) = 32),
  request_bytes bytea,
  registered_ms kehila.u64 NOT NULL,
  expires_ms kehila.u64 NOT NULL,
  state text COLLATE "C" NOT NULL DEFAULT 'prepared'
    CHECK (state IN ('prepared', 'completed', 'cancelled', 'expired')),
  operation_row_id bigint UNIQUE,
  CONSTRAINT pending_item_positive_id CHECK (request_id > 0),
  CONSTRAINT pending_item_lifetime CHECK
    (expires_ms > registered_ms
     AND expires_ms - registered_ms BETWEEN 60000 AND 604800000),
  CONSTRAINT pending_item_request_bound CHECK
    (request_bytes IS NULL OR octet_length(request_bytes) BETWEEN 1 AND 1048576),
  CONSTRAINT pending_item_fingerprint CHECK
    (request_bytes IS NULL OR sha256(request_bytes) = request_sha256),
  CONSTRAINT pending_item_owner FOREIGN KEY (installation_id, actor_id)
    REFERENCES kehila_auth.installation(installation_id, owner_actor_id),
  CONSTRAINT pending_item_handle UNIQUE
    (installation_id, actor_id, project_id, command_family, intent_handle),
  CONSTRAINT pending_item_operation_scope UNIQUE
    (actor_id, command_family, project_id, target_item_id, token),
  CONSTRAINT pending_item_target_binding UNIQUE
    (request_id, project_id, target_item_id),
  CONSTRAINT pending_item_completion_binding UNIQUE
    (request_id, project_id, target_item_id, operation_row_id),
  CONSTRAINT pending_item_state_shape CHECK
    ((state = 'prepared' AND request_bytes IS NOT NULL AND operation_row_id IS NULL)
     OR (state IN ('cancelled', 'expired')
         AND request_bytes IS NULL AND operation_row_id IS NULL)
     OR (state = 'completed' AND operation_row_id IS NOT NULL)),
  CONSTRAINT pending_item_core FOREIGN KEY
    (operation_row_id, actor_id, command_family, target_kind,
     project_id, target_item_id, token)
    REFERENCES kehila.operations
      (operation_row_id, actor_id, command_family, target_kind,
       target_project_id, target_item_id, token)
);

CREATE INDEX pending_item_discovery ON kehila_auth.pending_items
  (installation_id, actor_id, project_id, request_id DESC);
CREATE INDEX pending_item_never_created ON kehila_auth.pending_items
  (installation_id, actor_id, request_id) WHERE state <> 'completed';

CREATE TABLE kehila.items (
  item_id kehila.ident PRIMARY KEY,
  project_id kehila.ident NOT NULL REFERENCES kehila.projects(project_id),
  issued_prefix kehila.project_prefix NOT NULL,
  issued_sequence kehila.u64 NOT NULL CHECK (issued_sequence >= 1),
  creation_request_id bigint NOT NULL UNIQUE,
  creation_operation_row_id bigint NOT NULL UNIQUE,
  current_version kehila.u64 NOT NULL CHECK (current_version >= 1),
  CONSTRAINT item_project_identity UNIQUE (project_id, item_id),
  CONSTRAINT item_issued_sequence UNIQUE (project_id, issued_sequence),
  CONSTRAINT item_creation_reservation FOREIGN KEY
    (creation_request_id, project_id, item_id)
    REFERENCES kehila_auth.pending_items(request_id, project_id, target_item_id),
  CONSTRAINT item_creation_completion FOREIGN KEY
    (creation_request_id, project_id, item_id, creation_operation_row_id)
    REFERENCES kehila_auth.pending_items
      (request_id, project_id, target_item_id, operation_row_id)
    DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE kehila.item_revisions (
  project_id kehila.ident NOT NULL,
  item_id kehila.ident NOT NULL,
  item_version kehila.u64 NOT NULL CHECK (item_version >= 1),
  previous_item_version kehila.u64,
  configuration_revision kehila.u64 NOT NULL CHECK (configuration_revision >= 1),
  actor_id kehila.ident NOT NULL,
  command_family text COLLATE "C" NOT NULL
    CHECK (command_family IN ('item_create', 'item_edit')),
  target_kind text COLLATE "C" NOT NULL DEFAULT 'work_item'
    CHECK (target_kind = 'work_item'),
  token kehila.operation_token NOT NULL,
  operation_row_id bigint NOT NULL UNIQUE,
  type_id kehila.ident NOT NULL,
  workflow_id kehila.ident NOT NULL,
  status_id kehila.ident NOT NULL,
  phase kehila.phase NOT NULL,
  archived boolean NOT NULL CHECK (NOT archived),
  estimate_thousandths integer CHECK
    (estimate_thousandths BETWEEN 0 AND 999999999),
  estimate_unit text COLLATE "C" NOT NULL CHECK (estimate_unit IN ('hours', 'points')),
  history_codec_version bigint NOT NULL
    CHECK (history_codec_version BETWEEN 1 AND 4294967295),
  history jsonb NOT NULL CHECK
    (jsonb_typeof(history) = 'object'
     AND octet_length(history::text) <= 1048576),
  project_state_version kehila.u64 NOT NULL CHECK (project_state_version >= 1),
  previous_project_operation bigint NOT NULL REFERENCES kehila.operations(operation_row_id),
  resulting_next_sequence kehila.u64 NOT NULL CHECK (resulting_next_sequence >= 2),
  resulting_ever_estimated boolean NOT NULL,
  CONSTRAINT item_revision_identity PRIMARY KEY (project_id, item_id, item_version),
  CONSTRAINT item_revision_operation_binding UNIQUE
    (project_id, item_id, item_version, operation_row_id),
  CONSTRAINT item_project_state_identity UNIQUE (project_id, project_state_version),
  CONSTRAINT item_project_state_binding UNIQUE
    (project_id, project_state_version, operation_row_id),
  CONSTRAINT item_revision_item FOREIGN KEY (project_id, item_id)
    REFERENCES kehila.items(project_id, item_id),
  CONSTRAINT item_revision_previous FOREIGN KEY (project_id, item_id, previous_item_version)
    REFERENCES kehila.item_revisions(project_id, item_id, item_version),
  CONSTRAINT item_revision_step CHECK
    ((command_family = 'item_create' AND item_version = 1 AND previous_item_version IS NULL)
     OR (command_family = 'item_edit' AND item_version > 1
         AND previous_item_version IS NOT NULL AND item_version = previous_item_version + 1)),
  CONSTRAINT item_revision_core FOREIGN KEY
    (operation_row_id, actor_id, command_family, target_kind, project_id, item_id, token)
    REFERENCES kehila.operations
      (operation_row_id, actor_id, command_family, target_kind,
       target_project_id, target_item_id, token),
  CONSTRAINT item_revision_configuration FOREIGN KEY (project_id, configuration_revision)
    REFERENCES kehila.configuration_revisions(project_id, revision),
  CONSTRAINT item_revision_type_workflow FOREIGN KEY (project_id, type_id, workflow_id)
    REFERENCES kehila.type_workflows(project_id, type_id, workflow_id),
  CONSTRAINT item_revision_workflow_status FOREIGN KEY (project_id, workflow_id, status_id)
    REFERENCES kehila.workflow_statuses(project_id, workflow_id, status_id),
  CONSTRAINT item_revision_status_phase FOREIGN KEY (project_id, status_id, phase)
    REFERENCES kehila.statuses(project_id, status_id, phase),
  CONSTRAINT item_revision_estimate_witness CHECK
    (estimate_thousandths IS NULL OR resulting_ever_estimated)
);

ALTER TABLE kehila.items ADD CONSTRAINT item_current_revision FOREIGN KEY
  (project_id, item_id, current_version)
  REFERENCES kehila.item_revisions(project_id, item_id, item_version)
  DEFERRABLE INITIALLY DEFERRED;

CREATE TABLE kehila.item_field_uses (
  project_id kehila.ident NOT NULL,
  owner_type_id kehila.ident NOT NULL,
  field_id kehila.ident NOT NULL,
  first_item_id kehila.ident NOT NULL,
  first_item_version kehila.u64 NOT NULL,
  first_operation_row_id bigint NOT NULL,
  PRIMARY KEY (project_id, owner_type_id, field_id),
  FOREIGN KEY (project_id, owner_type_id, field_id)
    REFERENCES kehila.fields(project_id, owner_type_id, field_id),
  FOREIGN KEY (project_id, first_item_id, first_item_version, first_operation_row_id)
    REFERENCES kehila.item_revisions(project_id, item_id, item_version, operation_row_id)
);

-- Transition/closure guards must additionally prove: each admitted history
-- envelope agrees with relational actor/configuration/item/project facts and
-- result data; the project predecessor is the exact previous state in THIS
-- project; a first field-use revision has this owner type and actually contains
-- a value for this field. The reference keys alone do not establish those facts.

-- A single head row also represents version zero, whose operation is the
-- immutable project creation. The transition guard checks family and attribution;
-- a bare core FK cannot prove either. Later heads resolve by the unique indexed
-- (project_id, project_state_version, operation_row_id) revision key above.
CREATE TABLE kehila.project_operational_heads (
  project_id kehila.ident PRIMARY KEY REFERENCES kehila.projects(project_id),
  state_version kehila.u64 NOT NULL,
  operation_row_id bigint NOT NULL UNIQUE REFERENCES kehila.operations(operation_row_id)
);

-- Deliberately no runtime grants, installer inclusion, family-guard replacement
-- or removal of operation_success_mapping in this records-only source packet.
