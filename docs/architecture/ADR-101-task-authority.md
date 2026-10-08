# ADR-101 — Task authority, replay and projection

Status: Accepted behavior restatement. Recorded: 2026-10-02.
Owner: project maintainer. Acceptance source:
[D-016](../product/decisions.md#d-016) and
[D-017](../product/decisions.md#d-017); this record makes no new architecture choice.

[ADR-102](ADR-102-postgresql-transactions.md) supersedes the FerretDB storage
direction for new M1 implementation. This record retains the current task-path
evidence and accepted save/replay/projection behavior.

## Context

The bounded task path separates database acknowledgment from asynchronous search
visibility. [SP-001](../spikes/SP-001-task-path.md) used Python adapters;
[the current worker](../../src/io/task_worker/src/lib.rs) has separate Go/Rust
regression evidence described in [implementation status](../IMPLEMENTATION.md).
Neither establishes a supported application or general cross-document consistency.

## Decision

PostgreSQL through the selected FerretDB development mapping is authoritative
for this path. Search is a rebuildable, non-authoritative projection. A successful
mutation acknowledges database persistence with its version; it does not promise
search visibility. Missing search hits or search outages are not authoritative
task loss. Authoritative reads remain independent of OpenSearch.

Mutations use an expected version and operation ID. Identical recorded successes
replay before stale-version rejection, returning the original result even after
later updates. Changed content produces `operation_id_reused`; a stale new intent
produces `version_conflict`. Both are distinct 409 outcomes. Uncertain transport
outcomes reuse the ID; changed intent uses a new ID. Mutation, version and success
record must be accepted atomically for the promised scope. Current code uses
immutable operation records with task/version identity and operation uniqueness.
This candidate layout does not establish cross-record atomicity.

Projection versions and authoritative client state must not regress. The existing
Go search route enforces a caller-supplied minimum version using an actual search
query. A future client must also retain its latest authoritative version. CDC
captures acknowledged persistence independently of the worker; consumer replay
and ordering must be version-aware. SP-001 supports this bounded proposal; main
does not contain a production CDC/indexer. The 30-second spike bound is not an SLO.

The configurable model's later scoped opaque operation IDs, original-grant checks
and permanent tombstones are governed by
[D-032](../product/decisions.md#d-032) through
[D-034](../product/decisions.md#d-034). They are pure contracts, not worker features.

## Alternatives

Treating search as authority would conflict with accepted independent-save/read
semantics. Waiting for visibility before declaring a save would conflate two
different outcomes. A mutable task plus separate success record needs an atomic
acceptance/recovery protocol not established here. Multi-document transactions
or sagas for configuration, reservations and costs remain unresolved in Q-019;
this record neither rejects nor accepts a new physical protocol.

## Consequences and impacts

- Security: loopback/origin controls are transport restrictions, not identity.
  Integrated authentication and current authorization must precede replay.
- Performance: retained operation history grows; compaction, WAL cost, task lists
  and index rebuild during new writes need measurement and qualification.
- Operations: report saved-but-search-pending separately; detect CDC/projection
  lag and retained WAL/backlog growth, and provide rebuild/poison recovery.
- Migration: preserve success/replay state and product history; storage mapping,
  replica identity, publications and indexes require upgrade revalidation.
- Reversibility: search can be reconstructed; replacing authoritative layout
  requires a verified data and replay migration, not merely a new index.

## Verification and follow-up

[Live worker tests](../../tests/integration/live_task_worker.py) provide finite
cross-process races, replay and failure-boundary evidence. Finite races are not a
linearizability proof. Production projection/client monotonicity needs integrated
tests. [B-005](../product/backlog.md#b-005),
[B-022](../product/backlog.md#b-022), [B-023](../product/backlog.md#b-023) and
[B-024](../product/backlog.md#b-024) cover contract, consistency and recovery work.
Q-019 remains open for cross-record command acceptance and resource/cost effects.
