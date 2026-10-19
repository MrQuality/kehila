# M1-01 remaining delivery: successor plan

Status: W1 implementation approved by explicit maintainer instruction on
2026-10-13 and merged in PR #51 on 2026-10-14, as tracked by [#49](https://github.com/MrQuality/kehila/issues/49#issuecomment-6154165059).
Later work packages and ADR-103 production adoption retain separate review gates.

Planning [PR #50](https://github.com/MrQuality/kehila/pull/50) merged into main on
2026-10-14 at 0c89bfbfd0c20b8556894e4c91c7814d6eacc96c. Implementation
[PR #51](https://github.com/MrQuality/kehila/pull/51) merged on 2026-10-14 at
2efd444e38a4d60eaeb195a00359813174aa3d23. W1 is delivered. W2-W7 remain outstanding;
#49 is the continuing delivery issue and #26 retains its historical closed state.

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

The [W1 implementation report](m1-01-project-snapshots.md) records the delivered
project snapshot codec, pair verification and bounded native JSONB admission,
with Windows/Podman PostgreSQL 16.15 evidence from 2026-10-14. The plan below
retains the acceptance requirements; fixture success does not complete the
application adapter or later work packages.

## First-slice pre-execution plan

1. **Goal and context:** Implement the separate version-one
   `configuration_revisions.project_snapshot` codec specified by ADR-104 Contract 3
   and verify actual PostgreSQL JSONB reconstruction before implementing the
   internal persistence adapter. This first slice was delivered in PR #51 under #49.
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
   mapping, shared project preflight/decoder and locked dependencies. Put real PostgreSQL 16 JSONB
   checks in the integration/fixture boundary, with no database driver in the
   pure crate. W1 execution is now approved; later packages remain pending review.
   Check available memory before heavy builds/service execution and ask the
   maintainer to free more when insufficient; use one compiler job.
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
tracks execution. The [snapshot verification plan](m1-01-snapshot-verification.md)
defines the production-codec/native runner bridge, fixture classes, row/pair
coherence, required native command and JSONB compatibility gates. Native checks
remain separately required PR evidence; current shared/hosted checks do not run
them. Their execution results are recorded in the W1 implementation report;
this retained plan alone is not execution evidence.

## Remaining delivery after the first slice

These work packages preserve the existing acceptance scope. They establish
ordering and review gates, not a commitment to one PR per package. Each stateful
package needs its own bounded plan, negative/failure evidence and maintainer
review before execution. W1 is merged and W2's #40 investigation is in progress;
#41 and the remaining W2-W7 work retain their separate plans and review gates.

| Package | Prerequisites and boundary | Required exit evidence |
| --- | --- | --- |
| W1: Project snapshot codec and native verification | Explicit approval of the first-slice plan; Accepted ADR-104. No stateful application adapter. | Shared project mapping/preflight, production Rust-to-JSONB-to-Rust checks, relational versus representation fixtures, coherence rejection and resolved JSONB compatibility gates in the linked verification plan. |
| W2: Actor/scope and stable allocation contracts | Review #40 under B-007/#11 and #41 under M1-02/#27, related #32. Can be planned alongside W1; no route exposure. | Trusted actor/scope, revocation, operation/replay lookup and stable retry target ownership are defined. Use these contracts before finalizing runtime adapter interfaces; do not infer them from payload shape or fingerprints. |
| W3: Internal creation persistence adapter | W1 and W2 contracts before runtime interface finalization; separate dependency review and bounded transaction plan. ADR-103 disposition before production adoption. | Schema/startup compatibility, capability/connection/transaction ownership, coherent persistence and reload from one trusted result, request/core/result/snapshot attribution, checked JSONB compatibility for every generated envelope (saved result and both snapshots), stored-version corruption rejection, rollback and uncertain-response/contention evidence. |
| W4: Replay clocks and payload retirement/compaction | W3 operation persistence; trusted replay clock/retry contract and reviewed compactor capability. Keep compactor delivery separately reviewable from initial adapter persistence. | Actor/family/target-scoped lookup, 90-day replay, bounded retries, gap-free irreversible retirement to permanent tombstones and tombstone backup/restore. Serving privileges must not silently expand to compactor privileges. |
| W5: Multi-command configuration and history | W3/W2 ownership contracts, applicable W4 replay semantics, and command-specific B-003 rules. Explicit versioned schema/codec evolution beyond creation-only guards. | Historical status, status-group assignment/grant, workflow, type, field and relationship-type references remain protected after item migration; configuration/item contention and replay after configuration changes. Coordinate work-item persistence with M1-02/#27. |
| W6: Archive and current-selection coordination | W5 applicable state/history representation, M1-02 identity/selection persistence, authority and ADR-102 bounded transaction design. | Explicit archive/restore/selection-clear protocol with crash barriers, competing selections, retry and restore checks before archival routes. Readers observe neither accepted archives with stale selections nor selection clears from uncommitted archives. |
| W7: Supported integration and recovery qualification | Applicable W1-W6 evidence, ADR-103 production-adoption disposition and all ADR-104 ingress/authority/legacy gates. | Native command/read integration, zero pool/worker admission for rejected edge input, independent authorization, supported-platform evidence and applicable B-005 restart/clean-restore/durability/operating-limit checks. Keep authoritative state separate from rebuildable projections. |

W2 contracts and edge admission design must inform earlier work; placing final
integration in W7 does not defer those architectural constraints. Native
Linux-host evidence must be explicit, not inferred from Linux-hosted pure CI or
a Linux container on Windows. Legacy translation is excluded until a dedicated
approved migration RFC exists; isolation is required throughout these packages.
Completion of W1 alone leaves #49 open and does not establish M1-01 completion.

## Progress against the product backlog

The current product backlog defines M1-01 under B-005; the historical v0.2
technical milestone labels describe a different plan and are not completion
labels for today's product backlog. The native SQL foundation (PR #46), accepted
version-one codecs (PR #48), and W1 snapshot/JSONB verification (PR #51) are
usable engineering prerequisites. The running worker still uses FerretDB. There
is no supported native application command path, so the B-005 configurable
project/task and restart/recovery acceptance criteria have not been met.

W2's [#40/SP-004 contract](../spikes/SP-004-actor-authority.md) was approved and
merged in PR #53 on 2026-10-14 at `988631a353b3086769e302943fa0d7b7fbebffd1`.
The serial A05 management subset merged in [PR #54](https://github.com/MrQuality/kehila/pull/54)
on 2026-10-16 at `7b7e78825e1b639e2a222bb1403d24b74e71665a`.
Its declared native overlay passed R02 after correcting a deferred-trigger read
privilege failure; R03 added distinct-manager attribution and authority/input
guards. R04/R05 in merged PR #59 cover history audits and selected
database concurrency cases; actual credential/session/channel choices are not
supplied by the verified-identity fixture. W2 is not complete. See the canonical
SP-004 record for exact evidence and pending cases. Its
[bounded history/concurrency plan](../spikes/SP-004-actor-authority.md#next-bounded-slice-history-and-concurrent-authority)
covers history audits and observed concurrent authority/revocation schedules;
the plan was approved through merged PR #58 and implementation merged in
[PR #59](https://github.com/MrQuality/kehila/pull/59) on 2026-10-18. Its
[R05 evidence](../../experiments/SP-004/results.authority-review.json) passed at committed
source `9919460`; authentication, read/composite-scope and production integration
remain separate obligations. Heavy execution requires separate permission.

The [next #40 design review](../spikes/SP-004-actor-authority.md#next-design-slice-owner-authentication-and-trusted-caller-identity)
addresses real owner credentials and trusted caller identity. Under
[D-037](../product/decisions.md#d-037), MVP/runtime designs must account for
recovery dependencies, locale-independent identities/canonical data and future
machine-caller/attribution needs. B-024/#60 broader DR, B-030/#61 languages and
B-031/#62 agent execution remain post-MVP delivery; current real-data gates apply.

[ADR-105](../architecture/ADR-105-local-owner-authentication.md) is now the Proposed
authentication/boundary resolution, with a separate
[unexecuted verification plan](local-owner-authentication-verification.md).
Explicit ADR-105 approval and its dependency/transport/schema gates precede
implementation; ADR-103 production adoption is still a separate decision.
W2, stable allocation #41 and W3-W7 remain outstanding.

Plan #41/SP-005's stable target allocation and
response-loss reconciliation against the same actor/family/target/token scope.
Use both contracts before finalizing W3 adapter interfaces. W3 still needs a
separate transaction/dependency plan and ADR-103 production-adoption disposition;
W4-W7 retain their own gates. The bounded SP-004 evidence exposes no native route
and does not qualify authentication or accept ADR-103 production adoption.

### W3 creation-envelope admission obligation

Before database admission, W3 must check every generated persistence envelope
from the single accepted result: `operation_payloads.result`,
`configuration_revisions.project_snapshot` and
`configuration_revisions.configuration_snapshot`. Each must reject NUL and
remain within the existing codec ceilings after PostgreSQL 16 JSONB text output.
A successful `NativeRevisionSnapshots` admits only the snapshot pair; it is not
whole-transaction admission. The adapter must require a checked aggregate or an
explicit composition of checked boundaries and must not start persistence if
any envelope fails. Its native tests must persist and reload the actual saved
result alongside both snapshots, without placeholder result objects.

The W1 representation-stress fixture demonstrates the distinction: its
configuration text is exactly 1,048,576 bytes, while the corresponding saved
result would be 1,048,824 bytes. The latter cannot be admitted for that read path.
This is a W3 acceptance obligation under ADR-104's existing generated-envelope
ceilings, not authorization to add an application adapter or raise a ceiling.
A whole-creation admission API remains part of the separately reviewed W3 plan.

### ADR-103 disposition gate

Request separate maintainer review of ADR-103 before W3's production adoption,
using the foundation, role/integrity evidence, known platform/upgrade limitations
and proposed application use. The outcome may be acceptance, amendment or
rejection; approval of #49 does not supply it. ADR-103 already authorizes local
implementation/testing for review, so its Proposed status is not a blanket ban
on isolated verification. W1 implements Accepted ADR-104 and does not depend on
accepting ADR-103. Any driver dependency is reviewed in the stateful adapter
plan under the [dependency policy](../engineering/DEPENDENCIES.md).

### Planning and verification dispositions

Severity describes potential impact in the affected future capability; priority
sets the applicable gate. These are requirements to verify, not observed native
adapter defects. W1 implementation/fixture evidence is recorded in the report above; adapter
adoption and the later gates remain pending.

| Item | Severity / priority | Gate and disposition |
| --- | --- | --- |
| Production codec/native execution bridge | Medium / P1 | W1 plan defines the runner/IPC path and separately required native PR evidence; shared CI is not a substitute. |
| Representation versus relational fixtures | Medium / P1 | W1 separates u64 representation extremes from valid attributed revision rows and negative relational cases. |
| Row/pair coherence | Medium / P1 | W1 tests explicit relationships; W3 must enforce them on real transactional persistence/read verification. Keep historical envelope decoding separate. |
| Shared mapping and project preflight | Medium / P1 | W1 shares project encoding/decoding and validates strings before tree construction. |
| Dependency ordering | Medium / P2 | W1-W7 expose prerequisites and exit evidence; later plans must split work into focused, reviewable PRs. |
| ADR-103 production-adoption disposition | Low / P2 | Separate maintainer gate before W3 production adoption; status remains Proposed. |
| JSONB text size expansion | Medium / P1 | W1 must measure near-limit associated configuration round trips and resolve bounded admission/read compatibility. W1 measured the snapshot boundary; whole-creation saved-result compatibility remains an explicit W3 gate. No ceiling change is authorized. |
| Codec-valid versus SQL-storable strings | Medium / P2 | W1 records codec preservation versus JSONB NUL rejection; W3 admission checks generated content before database admission without narrowing historical decode. |

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

#26 remains closed. #47's stated codec scope landed in PR #48 and is complete;
PR #51 adds W1. #39 is complete: Accepted ADR-104 supersedes its codec
recommendation and PR #48 delivers the production codec. Historical experiment
observations remain frozen; global identifier narrowing is not authorized.
Remaining application delivery belongs to #49, with authority/allocation
contracts in #40/#41. There is no separate pending historical-assessment task.
ADR-103 retains its separately Proposed status.
