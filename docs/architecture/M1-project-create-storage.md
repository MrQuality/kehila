# M1 project creation: storage specification

Status: Proposed, 2026-10-08. Scope: #26/B-005 with #27 and #11/#32.
This is illustrative PostgreSQL 16 SQL for review, not an installed migration.
No SQL in this document has been executed. [ADR-102](ADR-102-postgresql-transactions.md)
accepts the storage direction; it does not approve the physical choices below.

## Outcome and boundary

One accepted creation saves the Project, trusted M1V1 Configuration, immutable
revision-one history, initial access, and successful request result in one
transaction. The existing experimental worker remains unchanged.

The [seed/history tables](M1-project-create-seed.md) continue this SQL. Read both
documents as one proposed schema; neither fragment is a complete migration.

The schema is split into the permanent request core and an expiring payload.
Deleting payload must never remove the request's unique identity. Product history
references that permanent core, not the replay payload. Full replay retains exact
typed request comparison; a digest is not a substitute for it.

## Choices requiring review

| ID | Proposal | Acceptance boundary |
| --- | --- | --- |
| P-01 | Opaque IDs use UTF8 text with C collation and a 128-byte limit. | A new bound on several String-backed IDs; must be explicitly approved and aligned with contract validation. Existing records require inspection before import. |
| P-02 | Reject U+0000 in persisted identity/text input. | PostgreSQL text/JSONB cannot represent it. Do not silently normalize, truncate, or change pure acceptance. The project-name rule already rejects controls. |
| P-03 | Creation permission is a capability on a stable actor row, protected by that row's lock. | Proposed #11 access mechanism; no global authority singleton and no new counter merely for locking. Login/session provisioning remains open. |
| P-04 | Permanent operation core plus optional replay payload, with an explicit irreversible retirement flag. | Requires atomic shape enforcement and compaction tests. Missing payload alone is not proof of intentional expiry. |
| P-05 | Current relational seed plus complete immutable JSONB configuration/project snapshots. | One accepted typed result produces both; reconstruction/drift checks are required. Historical codec support must survive upgrades. |
| P-06 | READ COMMITTED, defined row-lock order, bounded requests and full-transaction retry. | Proposed command protocol, not a database-wide default for all later operations. |
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

-- Only the ProjectCreate family/Project target is admitted in this slice.
-- Later families must extend the typed key deliberately, not overload a string.
CREATE TABLE kehila.operations (
  operation_row_id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  actor_id kehila.ident NOT NULL REFERENCES kehila.actors(actor_id),
  command_family text COLLATE "C" NOT NULL
    CHECK (command_family = 'project_create'),
  target_project_id kehila.ident NOT NULL,
  token kehila.operation_token NOT NULL,
  request_codec_version bigint NOT NULL
    CHECK (request_codec_version BETWEEN 1 AND 4294967295),
  request_sha256 bytea NOT NULL CHECK (octet_length(request_sha256) = 32),
  required_grant text COLLATE "C" NOT NULL
    CHECK (required_grant = 'project_create'),
  grant_policy_version integer NOT NULL CHECK (grant_policy_version > 0),
  replay_origin_ms kehila.u64 NOT NULL,
  replay_deadline_ms kehila.u64 NOT NULL,
  payload_retired boolean NOT NULL DEFAULT false,
  CONSTRAINT operations_replay_window CHECK (
    replay_deadline_ms = replay_origin_ms + 7776000000
  ),
  CONSTRAINT operations_scoped_key UNIQUE
    (actor_id, command_family, target_project_id, token),
  -- Supports checked attribution from Project/revision/access rows.
  CONSTRAINT operations_attribution UNIQUE
    (operation_row_id, target_project_id, actor_id)
);

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

The scoped-key index has at most three 128-byte strings plus the fixed family
label; attribution has two such strings and a bigint. Validate actual maximum
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
RETURNS trigger LANGUAGE plpgsql SET search_path = pg_catalog, kehila AS $$
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
    RAISE EXCEPTION 'incoherent operation payload' USING ERRCODE = '23514';
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
RETURNS trigger LANGUAGE plpgsql SET search_path = pg_catalog, kehila AS $$
BEGIN
  IF TG_OP = 'DELETE' THEN
    RAISE EXCEPTION 'operation identity is permanent' USING ERRCODE = '23514';
  END IF;
  IF (to_jsonb(NEW) - 'payload_retired') IS DISTINCT FROM
     (to_jsonb(OLD) - 'payload_retired')
     OR OLD.payload_retired OR NOT NEW.payload_retired THEN
    RAISE EXCEPTION 'invalid operation core update' USING ERRCODE = '23514';
  END IF;
  RETURN NEW;
END;
$$;

CREATE TRIGGER operation_core_immutable
  BEFORE UPDATE OR DELETE ON kehila.operations FOR EACH ROW
  EXECUTE FUNCTION kehila.protect_operation_core();
```

These shape checks do not protect the replay deadline from premature compaction.
The restricted compactor must lock the core, validate expiry against the reviewed
clock policy, set payload_retired, and delete payload in the same transaction.
Retirement is irreversible even if the clock later moves backward. Replay must
read core/payload coherently; the command protocol must define the locks.

## Review and implementation gates

Approve the proposed scalar/access/codec choices before installing this SQL.
Review SQL together with the seed, attribution, role, and command specifications.
Test direct constraint violations, multi-process retries, premature compaction,
missing payload, clock rollback, restore, and core mutation attempts. Preserve the
explicit trusted function search_path in executable migrations; serving roles
must not own tables or be able to replace functions/triggers.

This document establishes no native PostgreSQL behavior or successful I/O test.
