# ADR-108 - Native schema and runtime compatibility

Status: Proposed, initial draft, 2026-11-02. Owner: project maintainer.
Acceptance: pending; scope remains open for maintainer questions.
Related: ADR-102/103/104/105; #49, B-024/#60, B-032/#65;
[integration questions](../implementation/authenticated-project-integration.md#native-adoption-questions-in-progress).

## Context

The native installer is fresh-database-only. Serving can read schema version
metadata but no production compatibility gate or populated upgrade is delivered.
A worker's compiled domain rules can become incompatible with new constraints
or meanings even if it previously passed a startup version check. Constraint
rejection alone cannot prove its remaining writes and reads are semantically safe.
The anonymous Go/FerretDB prototype must remain isolated from native state.

## Proposed decision

Recommend maintenance-only transitions for the initial local profile. Do not
claim rolling upgrades or permit incompatible DDL while old writers can serve.
A separately privileged migration owns schema changes; serving cannot migrate
or repair its own schema/capabilities.

The transition contract must establish this sequence before a populated upgrade
can be supported:

1. Close admission and account for owned inflight work, including uncertain
   commits. Draining a request does not prove its transaction rolled back.
2. Exclude old runtimes from new writes and stale authoritative reads; prevent
   restart/reconnection from bypassing maintenance. A process stop or a version
   check without an enforceable exclusion mechanism is not a complete fence.
3. Under migration ownership and bounded database lock/work deadlines, check the
   exact predecessor, capabilities and data compatibility, then apply the reviewed
   migration. Do not infer that all future migrations can run in one transaction.
4. Record completion only after required postconditions hold. Start a compatible
   authority and verify readiness before reopening admission. Unsupported, partial
   or incoherent state stays blocked; do not fall back to the anonymous prototype.

**Unresolved before implementation:** exact maintenance/migration exclusion and
writer-version protocol, integration with ADR-105's installation-before-actor
order, compatibility descriptor, and recovery from migration-owner failure.
The current singleton advisory lock alone is not evidence of a migration fence.
No function, privilege, schema epoch or migration command is selected by this draft.

Preserve stored-version request/result/snapshot decoders independently of mutable
current domain rules. New-write compatibility, historical decode compatibility,
browser/API compatibility and deployment compatibility are distinct obligations;
one matching integer cannot be assumed to establish them all.

## Alternatives

The additional question about ingestion during migration does not establish a
lock-free upgrade requirement or replace this maintenance-only recommendation.
PostgreSQL DDL takes locks. Where an eventual online profile is required, evaluate
expand/contract changes, bounded batched backfills, applicable `NOT VALID` followed
by validation, and concurrent index construction. These reduce some blocking;
they do not eliminate table locks, incompatible-writer risks or resource costs.
Concurrent index creation also has transaction restrictions and failure cleanup
that a generic all-in-one migration transaction cannot hide. Define lock-acquisition
deadlines, workload controls, compatibility and recovery per operation before
claiming online support. An ingestion queue cannot make unlimited upstream writes
safe during an arbitrarily long maintenance window.

References: [PostgreSQL 16 ALTER TABLE](https://www.postgresql.org/docs/16/sql-altertable.html)
and [CREATE INDEX](https://www.postgresql.org/docs/16/sql-createindex.html).

- Startup-only version check: useful readiness input but cannot exclude old
  processes already running when migration begins.
- Let changed constraints reject old workers: catches some invalid writes but
  does not establish compatible interpretation or safe service availability.
- Expand/contract with mixed writer versions: possible future choice requiring
  an explicit supported-version matrix and enforceable deployment protocol;
  not initial rolling-upgrade support.

## Consequences and impacts

Maintenance trades availability for a smaller compatibility contract. It does
not excuse unbounded locks, lost acknowledged state or an unreviewed rollback.
Migration privilege remains separate from serving. Define backup prerequisites,
forward repair and rollback per migration; a down migration or schema drop is
not automatically safe once native data exists. ADR-105 restore invalidation and
stable actor/operation/tombstone identities remain mandatory.

Canonical identifiers, stored tags and error codes remain locale independent.
Future agents receive no migration privilege by being authenticated callers.
Replicated rolling transitions remain under #65; broader recovery remains #60.

## Verification and follow-up

Before adoption of an executable transition, define actual-role tests for an old
writer already connected, reconnecting and starting during maintenance; concurrent
request/DDL ownership; lock timeout; crash at each migration boundary; incompatible
new runtime startup; retained historical decoding; and verified recovery to a
declared supported state. Report observed outcomes, never infer exclusion from a
startup check. This draft supplies no migration implementation or execution evidence.
