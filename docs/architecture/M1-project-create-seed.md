# M1 project creation: seed and history tables

Status: Proposed, 2026-10-08. Continue the SQL in the [storage specification](M1-project-create-storage.md). This is proposed review SQL, not a migration. The frozen SQL and complete seed were executed in [SP-002](../spikes/SP-002-postgresql-project-create.md); its findings and limitations remain applicable. It covers creation only, not arbitrary configuration, WorkItem, or grant-administration routes.

The [command and verification proposal](M1-project-create-protocol.md) describes
how these rows become one action and which invariants need database evidence.

The trusted seed follows [D-029](../product/decisions.md#d-029) and [project_create.rs](../../src/pure/task_contract/src/project_create.rs): Task/Milestone, optional application Title/Description text fields per type, one New/Active/Done workflow, configuration revision one, next sequence one, and no recorded estimate.

## Project, history, current grants, and grant events

```sql
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
```

The name byte bound does not replace Rust's Unicode whitespace/control checks. Prefixes have no global uniqueness constraint. The allocator must honor its checked u64::MAX failure, not overflow or wrap.

The agreed access direction replaces immutable-only initial assignments with a
current project_grants row and append-only project_grant_events. Creation inserts
one active version-one initial_owner grant plus its attributed event in the same
transaction. Reload and compare both; the FK alone does not prove a current row
has a matching event. recorded_at is event-recording time, not authoritative
commit time. The physical layout is proposed; profile-to-permission mapping
remains P-03/#11/#32 work before any route is exposed.

Future revocation updates the current row/version and appends an event atomically
through a restricted audited path. Define its lock ownership with protected
commands and the current permission evaluator. Creation serving has no direct
grant UPDATE/DELETE. A revoked grant remains as an inactive current row; history
is not deleted and cannot authorize a request by itself. Provisioning, sessions,
profile semantics, and stable target allocation remain prerequisites.

## Relational trusted configuration

The first loader supplies empty status-group, choice-option, and relationship-type collections because M1V1 seeds none. Text/application/optional field checks deliberately restrict this slice. Supporting later configurable kinds and permissions needs separate specifications.

```sql
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
```

The initial_phase discriminator enforces New-phase eligibility without a literal in a foreign key. The title FK enforces type ownership; this slice's field-kind CHECK guarantees text. If later kinds are admitted, that title-kind guarantee needs explicit replacement.

```sql
-- First-loader restrictions, not the complete field vocabulary.
ALTER TABLE kehila.fields ADD CONSTRAINT slice1_field_kind CHECK (value_kind = 'text');
ALTER TABLE kehila.fields ADD CONSTRAINT slice1_field_origin CHECK (origin = 'application');
ALTER TABLE kehila.fields ADD CONSTRAINT slice1_field_usage CHECK (usage = 'optional');
ALTER TABLE kehila.fields ADD CONSTRAINT slice1_field_archival CHECK (NOT archived);
```

In executable migrations, separate temporary guards from lasting rules. Prefix
syntax, New initial phase, allowed phase edges, and ownership/membership are
domain rules. Cross-check supported SQL values and family/target mappings against
the pure contract; unsupported slices remain explicitly rejected. Removing a
guard must replace any invariant relying on it. NOT VALID avoids the initial
validation scan, not all DDL locks.

The SQL protects structure but cannot alone prove the exact trusted seed. Insert every definition from the single ProjectCreateResult, not a separately maintained seed. Positions preserve typed vector order and are not definition identities. Deferred constraints allow cyclic seed dependencies, not partially accepted state.

## Historical reconstruction

configuration_snapshot retains complete Configuration including empty collections; project_snapshot retains the recorded Project. Document the storage codec and keep decoders/golden fixtures for old versions. JSONB object-key order is immaterial to these structs; vector order and exact scalar values are not. Never derive historical state from current definitions.

Relational rows own current configuration; snapshots own recorded history. The
trusted pure result produces both. Snapshot/response copies are not independently
editable current sources. Keep this hybrid direction for upcoming WorkItem
references; do not copy a complete seed on each item transition. Measure creation
before attributing a throughput limit to its row count. Retain useful deferred
membership constraints; remove the generic operation-to-Project back-reference,
not all cycles by rule.

At creation, reload ordered current rows and compare typed equality against the trusted result and snapshots in the same transaction. Later drift audits require one database snapshot. Compare only revision-owned metadata: next_sequence and ever_estimated can change through item commands without a configuration-revision increment, so historical Project snapshots will legitimately differ on those fields.

## Index ownership and later scope

Primary/unique keys already supply indexes. The actor-first grant index serves immediate lookup. Before later mutation/deletion or audited joins, assess referencing-side indexes on workflow_statuses(project_id,status_id), workflows(project_id,initial_status_id), type_workflows(project_id,workflow_id), fields(project_id,owner_type_id), work_item_types(project_id,default_workflow_id), work_item_types(project_id,type_id,title_field_id), and each history/grant attribution key. Include the exact inventory in executable migrations; do not create a duplicate index for a primary/unique prefix without a query need.

This slice grants no definition deletion/update route. Historical identity registries, group/option scopes, and historical-use aggregates belong to the command slices that specify their ownership and retention. Immutable snapshots and attributed grant events preserve creation history. Later grants need their own permission vocabulary, command families and management protocol before expanding the temporary guards.
