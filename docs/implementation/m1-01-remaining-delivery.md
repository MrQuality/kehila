# M1-01 remaining delivery: successor plan

Status: Planning for maintainer review, 2026-10-13. Execution issue:
[#49](https://github.com/MrQuality/kehila/issues/49). Implementation has not begun
and requires explicit maintainer approval of the first slice.

## Tracking decision and baseline

The maintainer directed that closed [#26](https://github.com/MrQuality/kehila/issues/26)
remain closed and that unfinished M1-01 work receive a new issue and backlog
record. #49 is that successor execution issue under [B-005](../product/backlog.md#b-005)
([#9](https://github.com/MrQuality/kehila/issues/9)). M1-01's ID, product scope,
dependencies and acceptance criteria are preserved. Closure of #26 does not
establish completion; merging a planning or snapshot PR must not close #49.

[PR #46](https://github.com/MrQuality/kehila/pull/46), merged at
59b374f7ddd4bb57c4577ce39115a5c54e8ac3b9, supplied the
[native SQL foundation](../../storage/postgresql/README.md).
[PR #48](https://github.com/MrQuality/kehila/pull/48), merged at
8b9a7e4ceea71ba66330194db335216a9eded2bd, supplied the
[pure production codecs](../../src/pure/project_create_codec/README.md) tracked
by [#47](https://github.com/MrQuality/kehila/issues/47).
The [foundation handoff](issue-26-postgresql-foundation.md) and
[codec handoff](project-create-production-codecs.md) retain their evidence and
limitations. [SP-002](../spikes/SP-002-postgresql-project-create.md) and
[SP-003/#39](../spikes/SP-003-replay-codecs.md) remain bounded experimental evidence.

[ADR-102](../architecture/ADR-102-postgresql-transactions.md) accepts the native
transaction direction. [ADR-104](../architecture/ADR-104-project-create-codecs.md)
was explicitly Accepted on 2026-10-13, with its
[approval record](https://github.com/MrQuality/kehila/pull/48#issuecomment-6151468708).
[ADR-103](../architecture/ADR-103-native-creation-foundation.md) remains Proposed
for production adoption. This plan accepts no additional architecture or product
decision. The running worker remains on FerretDB; no native application adapter
or supported configurable-model persistence path is implemented.

## First-slice pre-execution plan

1. **Goal and context:** Implement the separate version-one
   `configuration_revisions.project_snapshot` codec specified by ADR-104 Contract 3
   and verify actual PostgreSQL JSONB reconstruction before implementing the
   internal persistence adapter. This is the proposed first PR under #49.
2. **Definition of Done:** Exact envelope keys `codec_version`, `kind`, `value`,
   integer version 1, kind `project_snapshot`, and complete project fields
   `id`, `name`, `prefix`, `configuration_revision`, `next_sequence`, `archived`,
   `estimate_unit`, `ever_estimated`. Preserve exact strings, booleans and unit
   tags; both u64 fields use canonical range-checked decimal strings. Reject
   missing/unknown fields, wrong types/kinds/versions, malformed/trailing input
   and duplicate decoded keys, including escaped equivalents, before parsing the
   next value or overwriting a map entry. Apply all existing resource ceilings.
   Historical decode must not consult mutable current domain acceptance.
3. **Dependencies and prerequisites:** Reuse the pure codec's existing project
   mapping, bounded parser and locked dependencies. Put real PostgreSQL 16 JSONB
   checks in the integration/fixture boundary, with no database driver in the
   pure crate. Implementation waits for maintainer review and approval of #49.
   Ask for memory readiness before memory-heavy builds or service execution.
4. **Risks and mitigations:** A second project mapping could drift from saved
   results; reuse the canonical mapping. JSONB loses object-key order and
   duplicates; verify typed equality after storage and original-byte duplicate
   rejection separately. Derivation from a single accepted result remains a
   mandatory future adapter obligation, not evidence supplied by a pure codec.
5. **Spikes and open questions:** No new engine spike or representation choice is
   proposed. ADR-104 already fixes this envelope. ADR-103 production adoption and
   #40/#41 actor/allocation contracts remain separate gates for later delivery.
6. **Architecture and file scope:** Pure encode/decode APIs, focused codec tests
   and frozen fixtures, native JSONB round-trip checks in the existing test
   boundary, shared verification integration where needed, and updated handoffs.
   No public route, legacy import, compactor, schema upgrade or allocation policy
   belongs to this PR. Keep commits focused; review new dependencies explicitly
   if implementation demonstrates a need rather than assuming an upgrade.
7. **Verification:** Exercise zero, values above 2^53, u64::MAX, overflow and
   noncanonical decimal forms; exact reconstruction and envelope/version errors;
   applicable exact/over-limit resource cases; raw escaped duplicates and
   rejection before their value. Preserve binary request and existing JSON
   fixtures. Test actual JSONB round trips and associated configuration order and
   empty collections. Retain meaningful failing regression/mutation evidence;
   run applicable pure/native and required shared checks, then record actual
   platforms, service versions and limitations in #49 and the PR summary.

The [issue acceptance checklist](https://github.com/MrQuality/kehila/issues/49)
tracks execution. No check above claims an implementation or successful test.

## Remaining delivery after the first slice

#49 retains M1-01's full remaining scope: native startup/schema compatibility,
transaction ownership and coherent persistence/attribution from one trusted
result; stored-version dispatch and corruption handling; trusted replay clocks,
retries, 90-day replay and gap-free permanent-tombstone compaction/restore;
configuration history/reference protection and configuration/item contention;
archive/current-selection crash and recovery consistency; supported integration
and B-005 durability/restore evidence. Subsequent slices need bounded plans and
review. Completing the snapshot codec alone does not complete M1-01.

All four ADR-104 contracts remain mandatory. Future ingress must prove bounded
admission before pools/workers; authentication and tenant/project authorization
must be independent of decode/fingerprint success; snapshots must satisfy the
fixed structural contract; unmigrated legacy state stays isolated until a
dedicated approved migration RFC defines translation and recovery gates.

[SP-004/#40](../spikes/SP-004-actor-authority.md), under B-007/#11 and related #32,
and [SP-005/#41](../spikes/SP-005-project-target-allocation.md), under M1-02/#27,
remain prerequisites before native routes. See the [B-003 contract](../product/B-003-contract.md),
[requirements](../product/requirements.md), [product decisions](../product/decisions.md),
[implementation status](../IMPLEMENTATION.md), [testing guide](../TESTING.md) and
[Definition of Done](../engineering/DEFINITION-OF-DONE.md) for governing scope and
evidence. Action stacks #44 remains postponed.

## Historical issue reconciliation

#26 remains closed. #47's stated codec scope landed in PR #48; #39 retains its
experimental history. Successor links clarify where unfinished implementation
is tracked without rewriting frozen observations or silently accepting historical
proposals. This planning change does not change their issue states. Historical
pending-assessment text must be read alongside ADR-104's explicit acceptance;
it does not override ADR-103's separately Proposed status.
