# M1 project creation: storage specification

Status: Proposed, 2026-10-08. Scope: #26/B-005 with #27 and #11/#32.
This is illustrative PostgreSQL 16 SQL for review, not an installed migration.
No SQL in this document has been executed. [ADR-102](ADR-102-postgresql-transactions.md)
accepts the storage direction; it does not approve the physical choices below.
The typed-target layout below replaces the project-only key sketch. Its physical
columns are a proposed realization of the agreed target-scope correction.

## Outcome and boundary

One accepted creation saves the Project, trusted M1V1 Configuration, immutable
revision-one history, initial access, and successful request result in one
transaction. The existing experimental worker remains unchanged.

The [seed/history tables](M1-project-create-seed.md) continue this SQL. Read both
documents as one proposed schema; neither fragment is a complete migration.
The [command and verification proposal](M1-project-create-protocol.md) defines
the locking, retry, privilege, and evidence requirements for this slice.

The schema is split into the permanent request core and an expiring payload.
Deleting payload must never remove the request's unique identity. Product history
references that permanent core, not the replay payload. Full replay retains exact
typed request comparison; a digest is not a substitute for it.

## Choices requiring review

| ID | Proposal | Acceptance boundary |
| --- | --- | --- |
| P-01 | Opaque IDs use UTF8 text with C collation and a 128-byte limit. | A new bound on several String-backed IDs; must be explicitly approved and aligned with contract validation. Existing records require inspection before import. |
| P-02 | Reject U+0000 in persisted identity/text input. | PostgreSQL text/JSONB cannot represent it. Do not silently normalize, truncate, or change pure acceptance. The project-name rule already rejects controls. |
| P-03 | Creation capability uses an exclusive actor lock through a restricted function; current grants are separate from immutable grant history. | Agreed correction; effective owner rights, provisioning, and grant-management locks remain #11/#32 work. No global authority singleton. |
| P-04 | Permanent typed operation core plus optional replay payload, with irreversible retirement and fresh-insert enforcement. | Preserve permanent retry identity and exact comparison. Proposed typed columns and atomic shape enforcement require real database tests. |
| P-05 | Relational current configuration plus complete immutable JSONB historical snapshots. | Agreed hybrid direction with explicit ownership. One typed result produces both; reconstruction/drift and historical-codec checks remain required. |
| P-06 | READ COMMITTED with exclusive per-actor creation coordination, restricted locking functions, no Project lock on creation replay, and bounded retry. | Agreed creation direction, not a database-wide default. Function privileges and retry limits require real database tests. |
| P-07 | Replay origin remains a separate reviewed time policy. | Preserve D-034's 90-days-after-commit semantics. A pre-commit clock sample must not silently replace the accepted origin. No route can promise the boundary until this is resolved. |

Only project creation is represented here. WorkItem, relationship, knowledge,
status-group grant, and archival routes need their own scoped schema changes.
The [B-003 contract](../product/B-003-contract.md) remains the behavioral authority.

## Scalars and permanent operations

Deployment prerequisites are PostgreSQL 16 and database encoding UTF8. Roles and
schema ownership must be provisioned separately. The integer domain deliberately
uses unconstrained numeric plus an integral-value check: numeric(20,0) can round
fractional input before a CHECK sees it. The adapter must still decode to u64
exactly and reject overflow. Internal generated row IDs may have gaps; they are
not user-visible readable numbers or operation tokens.

```sql
CREATE SCHEMA kehila;

CREATE DOMAIN kehila.ident AS text COLLATE "C"
  CHECK (octet_length(VALUE) BETWEEN 1 AND 128);

CREATE DOMAIN kehila.u64 AS numeric
  CHECK (VALUE = trunc(VALUE)
         AND VALUE BETWEEN 0 AND 18446744073709551615);

CREATE DOMAIN kehila.operation_token AS text COLLATE "C"
  CHECK (octet_length(VALUE) BETWEEN 1 AND 128
         AND VALUE ~ '^[!-~]+$');

CREATE TABLE kehila.actors (
  actor_id kehila.ident PRIMARY KEY,
  active boolean NOT NULL,
  can_create_project boolean NOT NULL DEFAULT false
);

-- Represent known typed targets now; only ProjectCreate is served in this slice.
CREATE TABLE kehila.operations (
  operation_row_id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  actor_id kehila.ident NOT NULL REFERENCES kehila.actors(actor_id),
  command_family text COLLATE "C" NOT NULL,
  target_kind text COLLATE "C" NOT NULL,
  target_project_id kehila.ident,
  target_item_id kehila.ident,
  target_user_id kehila.ident,
  target_relationship_id kehila.ident,
  token kehila.operation_token NOT NULL,
  request_codec_version bigint NOT NULL
    CHECK (request_codec_version BETWEEN 1 AND 4294967295),
  request_sha256 bytea NOT NULL CHECK (octet_length(request_sha256) = 32),
  required_grant text COLLATE "C" NOT NULL,
  grant_policy_version integer NOT NULL CHECK (grant_policy_version > 0),
  replay_origin_ms kehila.u64 NOT NULL,
  replay_deadline_ms kehila.u64 NOT NULL,
  payload_retired boolean NOT NULL DEFAULT false,
  CONSTRAINT operations_replay_window CHECK (
    replay_deadline_ms = replay_origin_ms + 7776000000
  ),
  CONSTRAINT operations_target_shape CHECK (
    (target_kind = 'project' AND target_project_id IS NOT NULL
      AND target_item_id IS NULL AND target_user_id IS NULL
      AND target_relationship_id IS NULL)
    OR (target_kind = 'work_item' AND target_project_id IS NOT NULL
      AND target_item_id IS NOT NULL AND target_user_id IS NULL
      AND target_relationship_id IS NULL)
    OR (target_kind = 'user' AND target_project_id IS NULL
      AND target_item_id IS NULL AND target_user_id IS NOT NULL
      AND target_relationship_id IS NULL)
    OR (target_kind = 'relationship' AND target_project_id IS NOT NULL
      AND target_item_id IS NULL AND target_user_id IS NULL
      AND target_relationship_id IS NOT NULL)
  ),
  CONSTRAINT operations_family_target CHECK (
    (target_kind = 'project' AND command_family IN
      ('project_create', 'project_metadata', 'project_archive',
       'configuration_change', 'choice_option_admin'))
    OR (target_kind = 'work_item' AND command_family IN
      ('item_create', 'item_edit', 'item_archive', 'item_transition',
       'item_conversion', 'knowledge_create', 'knowledge_edit', 'follow_up_create'))
    OR (target_kind = 'user' AND command_family = 'selection')
    OR (target_kind = 'relationship' AND command_family = 'relationship_create')
  ),
  CONSTRAINT operations_scoped_key UNIQUE NULLS NOT DISTINCT
    (actor_id, command_family, target_kind, target_project_id, target_item_id,
     target_user_id, target_relationship_id, token),
  -- Supports checked attribution from Project/revision/access rows.
  CONSTRAINT operations_attribution UNIQUE
    (operation_row_id, command_family, target_kind, target_project_id, actor_id)
);

-- Temporary serving-slice guards, not permanent domain vocabulary.
ALTER TABLE kehila.operations ADD CONSTRAINT slice1_operation_family
  CHECK (command_family = 'project_create');
ALTER TABLE kehila.operations ADD CONSTRAINT slice1_operation_grant
  CHECK (required_grant = 'project_create');

CREATE TABLE kehila.operation_payloads (
  operation_row_id bigint PRIMARY KEY
    REFERENCES kehila.operations(operation_row_id) ON DELETE RESTRICT,
  request_bytes bytea NOT NULL,
  result_codec_version bigint NOT NULL
    CHECK (result_codec_version BETWEEN 1 AND 4294967295),
  result jsonb NOT NULL CHECK (jsonb_typeof(result) = 'object')
);

CREATE INDEX operations_full_expiry
  ON kehila.operations(replay_deadline_ms)
  WHERE NOT payload_retired;
```

The origin/deadline domains reject overflow; they do not resolve when a timestamp
is authoritative. The permanent original grant is creation permission on actor_id
for this family, checked on every retry. Other grant scopes are not encoded here.
Request and result codecs must be specified and versioned before implementation.

Target fields mirror OperationTarget in the pure contract: a Relationship's
project component is its owner_project_id. NULLS NOT DISTINCT prevents absent
components from admitting duplicate keys. Test all valid shape/family pairs and
rejected extra/missing components. Stored family labels are a proposed mapping;
freeze it with the codec. Expand temporary guards only with the corresponding
command/grant protocol; listing a future family does not implement its route.

The permanent core does not reference a mutable target table. Domain/history
tables reference attributed operations; creation's transaction and reconstruction
prove the created Project exists. Operation retention must not require every
target kind to have a Project FK. This does not approve target erasure or weaken
historical attribution.

Valid scoped keys have at most four 128-byte strings plus family/kind labels;
attribution has two such strings, labels, and a bigint. Validate actual maximum
index tuples on the supported database before enabling this proposed ID limit.
PostgreSQL limits a B-tree entry to approximately one-third of a page after any
applicable compression; do not rely on compressible user input. See
[B-tree limits](https://www.postgresql.org/docs/16/btree-intro.html).

## Atomic payload shape

A committed non-retired operation must have exactly one payload; a retired one
must have none. Enforce this in addition to the payload FK with deferred checks
on both tables. Immediate payload deletion alone would leave ambiguous state.

```sql
CREATE FUNCTION kehila.check_operation_payload()
RETURNS trigger LANGUAGE plpgsql SET search_path = pg_catalog, kehila, pg_temp AS $$
DECLARE
  checked_id bigint;
  retired boolean;
  has_payload boolean;
BEGIN
  IF TG_OP = 'DELETE' THEN
    checked_id := OLD.operation_row_id;
  ELSE
    checked_id := NEW.operation_row_id;
  END IF;
  SELECT o.payload_retired,
         EXISTS (SELECT 1 FROM kehila.operation_payloads p
                 WHERE p.operation_row_id = o.operation_row_id)
    INTO retired, has_payload
    FROM kehila.operations o
    WHERE o.operation_row_id = checked_id;
  IF NOT FOUND OR retired = has_payload THEN
    RAISE EXCEPTION 'incoherent operation payload'
      USING ERRCODE = '23514', CONSTRAINT = 'operation_payload_shape',
            SCHEMA = 'kehila', TABLE = 'operations';
  END IF;
  RETURN NULL;
END;
$$;

CREATE CONSTRAINT TRIGGER operation_payload_shape
  AFTER INSERT OR UPDATE OR DELETE ON kehila.operations
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW
  EXECUTE FUNCTION kehila.check_operation_payload();

CREATE CONSTRAINT TRIGGER replay_payload_shape
  AFTER INSERT OR UPDATE OR DELETE ON kehila.operation_payloads
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW
  EXECUTE FUNCTION kehila.check_operation_payload();

CREATE FUNCTION kehila.protect_operation_core()
RETURNS trigger LANGUAGE plpgsql SET search_path = pg_catalog, kehila, pg_temp AS $$
BEGIN
  IF TG_OP = 'INSERT' THEN
    IF NEW.payload_retired THEN
      RAISE EXCEPTION 'new operation requires a replay payload'
        USING ERRCODE = '23514', CONSTRAINT = 'operation_core_fresh',
              SCHEMA = 'kehila', TABLE = 'operations';
    END IF;
    RETURN NEW;
  END IF;
  IF TG_OP = 'DELETE' THEN
    RAISE EXCEPTION 'operation identity is permanent'
      USING ERRCODE = '23514', CONSTRAINT = 'operation_core_immutable',
            SCHEMA = 'kehila', TABLE = 'operations';
  END IF;
  IF (to_jsonb(NEW) - 'payload_retired') IS DISTINCT FROM
     (to_jsonb(OLD) - 'payload_retired')
     OR OLD.payload_retired OR NOT NEW.payload_retired THEN
    RAISE EXCEPTION 'invalid operation core update'
      USING ERRCODE = '23514', CONSTRAINT = 'operation_core_immutable',
            SCHEMA = 'kehila', TABLE = 'operations';
  END IF;
  RETURN NEW;
END;
$$;

CREATE TRIGGER operation_core_immutable
  BEFORE INSERT OR UPDATE OR DELETE ON kehila.operations FOR EACH ROW
  EXECUTE FUNCTION kehila.protect_operation_core();

CREATE FUNCTION kehila.reject_operation_truncate()
RETURNS trigger LANGUAGE plpgsql SET search_path = pg_catalog, kehila, pg_temp AS $$
BEGIN
  RAISE EXCEPTION 'operation storage cannot be truncated'
    USING ERRCODE = '23514', CONSTRAINT = 'operation_storage_no_truncate',
          SCHEMA = TG_TABLE_SCHEMA, TABLE = TG_TABLE_NAME;
END;
$$;
CREATE TRIGGER operation_core_no_truncate
  BEFORE TRUNCATE ON kehila.operations FOR EACH STATEMENT
  EXECUTE FUNCTION kehila.reject_operation_truncate();
CREATE TRIGGER operation_payload_no_truncate
  BEFORE TRUNCATE ON kehila.operation_payloads FOR EACH STATEMENT
  EXECUTE FUNCTION kehila.reject_operation_truncate();
```

These shape checks do not protect the replay deadline from premature compaction.
The restricted compactor must lock the core, validate expiry against the reviewed
clock policy, set payload_retired, and delete payload in the same transaction.
Retirement is irreversible even if the clock later moves backward. Replay must
read core/payload coherently; the command protocol must define the locks.

The fresh-insert guard applies to normal serving. Restoration of tombstones
needs a reviewed owner-controlled load procedure and final validation; do not
weaken normal INSERT to accommodate it. TRUNCATE guards protect against mistakes,
not an owner able to disable them. The serving role has no TRUNCATE privilege.
If a future privileged path moves payload identity, forbid it or validate both
old and new cores: the current shape function checks one operation ID. Serving
payload UPDATE is forbidden here.

Retirement changes a partial-index column and prevents HOT updates. Measure
compaction batch size, lock waits, WAL and vacuum behavior. The attribution index
is additional storage for composite integrity, not a replacement for the primary
key. No performance bottleneck is established.

## Review and implementation gates

Approve the proposed scalar/access/codec choices before installing this SQL.
Review SQL together with the seed, attribution, role, and command specifications.
Test direct constraint violations, multi-process retries, premature compaction,
missing payload, clock rollback, restore, and core mutation attempts. Preserve the
explicit trusted function search_path in executable migrations; serving roles
must not own tables or be able to replace functions/triggers.

This document establishes no native PostgreSQL behavior or successful I/O test.

Owner-controlled repair/import migrations must preserve operation identity and
audit corrections. Define backup, validation, rollback and retention/erasure
rules before real data. Immutability restricts serving writes, not reviewed schema
evolution. Never edit an old successful result to hide a faulty current-state write.
