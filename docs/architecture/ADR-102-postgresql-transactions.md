# ADR-102 — Native PostgreSQL and bounded transactions

Status: Accepted. Owner: project maintainer. Recorded: 2026-10-08.
Acceptance: [issue #26 architecture update](https://github.com/MrQuality/kehila/issues/26#issuecomment-6001442395).
Related: [B-005](../product/backlog.md#b-005), M1-01, M1-02, M1-07,
[Q-019](../product/open-questions.md#q-019),
[D-032](../product/decisions.md#d-032) through D-034.

## Context

The accepted configurable model coordinates project state, configuration,
permissions, history, selections, and successful-operation records. The earlier
reference requires single-document mutations through FerretDB and asynchronous
sagas for all multi-document work. That restriction makes atomic acceptance of
related M1 changes an application responsibility.

The maintainer accepted native PostgreSQL access, JSONB for configurable values,
and bounded transactions for changes that must succeed together. MongoDB
compatibility is not a product requirement. The existing worker still uses
FerretDB; this record approves a direction, not its implementation.

## Decision

Use native PostgreSQL for new authoritative M1 persistence. Use relational keys
and constraints for stable structure and JSONB where flexible content needs it.
The physical schema, driver, identity representation, and migration remain to be
specified and reviewed.

Replace the blanket prohibition on multi-record transactions with bounded
transactions for related changes requiring coherent acceptance. Project creation
must commit the project, trusted defaults, initial access, history, and successful
request result together. Keep transactions independent of synchronous broker,
search, or other external-service responses.

Large operations remain background jobs with progress and recovery, using bounded
transactions. Determine their visibility and atomicity individually: dividing
work into chunks must not weaken an accepted all-or-nothing product requirement.
This record does not select a saga, queue-order, or graph-lock protocol.

Preserve authoritative save/read independence from search, expected-version
conflicts, current authorization, actor/family/target-scoped request identity,
exact typed comparison, 90-day replay, permanent tombstones, and product history.
Transactions do not replace these rules.

ADR-101 continues to describe the existing bounded task path. This record
supersedes its FerretDB storage direction for new M1 implementation and the
reference's blanket transaction ban, without invalidating historical evidence or
changing ADR-101's save/replay/projection semantics.

## Alternatives

- Retain FerretDB and implement cross-record acceptance/recovery in the
  application. Not selected: MongoDB compatibility is unnecessary and the model
  requires coherent changes to related records.
- Put all project state in one document. Not selected: project items and history
  grow independently; one large mutable document introduces shared contention
  and does not establish a practical retention boundary.
- Use one unbounded transaction for every bulk job. Not selected: job size,
  lock duration, retry cost, and visibility requirements differ by operation.

## Consequences and impacts

- Security: trusted identity, current grants, revocation, and scoped replay still
  need an integrated access design. Database roles must separate provisioning
  from serving and restrict history mutation.
- Performance: specify isolation, lock order, transaction limits, retry budgets,
  and workload measurements. No throughput or capacity improvement is claimed.
- Operations: rework provisioning and diagnostics for native access. Decide CDC
  and/or outbox delivery from committed state; a database commit does not make a
  separate broker publication atomic.
- Migration: retain existing experimental records until an explicit compatibility
  or import decision. No destructive conversion or supported upgrade is approved.
- Reversibility: keep the existing adapter until replacement verification is
  complete. Once native records exist, rollback requires explicit data, history,
  replay, and codec compatibility; switching drivers alone is insufficient.

## Verification and follow-up

Specify first-slice SQL and an invariant/enforcement/test matrix before executable
migrations. Review exact-value encodings, identifier/key budgets, grant scopes,
immutable history, and replay timestamp semantics without silently changing
product contracts.

The [project-creation storage specification](M1-project-create-storage.md) is a
proposal for that first slice, with its own acceptance and execution limits.

Implementation requires real PostgreSQL constraint, concurrency, rollback,
uncertain-response retry, compaction, and clean-instance restore tests. Existing
FerretDB/SP-001 results apply only to their recorded stack and scope. There is no
native adapter, schema execution, or maturity advancement established here.
