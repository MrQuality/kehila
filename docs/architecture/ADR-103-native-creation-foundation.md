# ADR-103 - Native creation storage foundation

Status: Proposed. Owner: project maintainer. Recorded: 2026-10-10.
Acceptance: Production adoption pending maintainer review. Local implementation
and testing were authorized on 2026-10-10; that does not establish route readiness.
Related: [ADR-102](ADR-102-postgresql-transactions.md), #26/B-005, #39,
[implementation plan](../implementation/issue-26-postgresql-foundation.md),
[storage specification](M1-project-create-storage.md).

## Context

ADR-102 accepts native PostgreSQL and bounded transactions but leaves physical
storage and provisioning open. SP-002 exposed a privileged payload reassignment
gap; SP-003 established bounded versioned-codec evidence. Neither experiment is
a production installer or an authenticated native command implementation.

## Decision

Implement a fresh PostgreSQL 16 UTF8 baseline locally for review. Own executable
SQL under storage/postgresql/; keep frozen experiment sources unchanged. Install
scalar domains, permanent operations and replay payloads, project/configuration
rows, attributed append-only history and explicit capability roles in one
transaction. Record schema version one only after all fragments succeed.

Separate NOLOGIN schema ownership from NOLOGIN creation capability. The trusted
backend receives INSERT/SELECT and restricted actor/core locking functions, with
no actor administration, replay UPDATE/DELETE, history mutation or DDL in kehila.
Read-only schema-migration metadata is available to the creation capability;
migration writes remain restricted to the deployment/ownership boundary.
Revoke PUBLIC access to the kehila schema/functions and pin SECURITY DEFINER search paths.

Use exact C-collated UTF8 identifiers bounded to 128 bytes and checked integral
numeric for u64. Apply those bounds to new native storage, with domain/interface
alignment required before route exposure. Preserve permanent scoped operation
identity, immediate payload-identity protection, deferred atomic payload shape
and irreversible retirement. This foundation grants no compactor capability.

All payload updates are rejected. Deletion requires setting the irreversible
core retirement flag first within the same transaction; deferred shape checks
reject payload reinsertion after retirement. This also prevents privileged
DELETE/INSERT replacement without disabling a guard.

Relational rows own current configuration; version-tagged JSONB snapshots own
history. SQL enforces attribution/membership, while the future typed adapter
must prove exact trusted-result equality and encode/reconstruct saved content.
Actor authority (#40), target allocation (#41), codecs and replay clock/retry
handling remain prerequisites for a native route.

## Alternatives

- Execute Markdown SQL at application startup: rejected for this implementation;
  reviewed explanatory documents are not a stable deployment interface.
- Reuse experiment installation helpers: rejected; frozen experiments must retain
  their original evidence boundaries and cannot own production schema changes.
- Silently adopt existing schemas/roles or supply destructive reset: rejected;
  this baseline has no reviewed populated-database upgrade/repair contract.
- Use the owner account for serving: rejected; it defeats capability separation.

## Consequences and impacts

- Security: backend capability remains trusted, with application authorization
  still required. Owners can disable guards; no protection against administrators
  or production authentication configuration is claimed.
- Performance: creation-only indexes and bounded fixture tests establish no
  throughput capacity. Query/mutation slices must justify additional indexes.
- Operations: deployment requires a fresh database and unused cluster-wide role
  names. Provision backend login and actors separately. No broker/search I/O is
  part of installation or authoritative acceptance.
- Migration: legacy FerretDB data and public String-backed wrappers are unchanged.
  Multi-database role provisioning, native upgrades/import and backup/restore
  qualification remain separate work before real data.
- Reversibility: installation failure rolls back roles and schema. After native
  data is committed, dropping the schema is not a supported rollback strategy.

## Verification and follow-up

The [native guide](../../storage/postgresql/README.md) maps invariants to executable
checks. PostgreSQL 16.15 verification on Windows with a Linux container passed
installation rollback, role separation, structural constraints and a payload
identity mutation regression. Shared staged checks also passed. Native Linux-host
execution, hosted native CI, integrated retries, compaction, restore and load
qualification remain unverified. Review this record with the local foundation
before accepting production adoption; do not equate schema execution with M1 readiness.
