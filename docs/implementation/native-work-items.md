# Native WorkItem delivery

Status: Assessment; refinements and direct-edit recovery choice accepted;
remaining capability/read-preparation architecture proposed in ADR-111 revision 3.
Baseline: `d6f8d3f563dbc8f9a242ec5abf779ab8e7142500`, 2026-11-04.
Owners: M1-02/#27, operations #8, interface #10, authority #40 and storage #49.

## Outcome and scope

The maintainer selected this outcome on 2026-11-04: in a fresh development
database, an authenticated owner can create a project, create Task and Milestone
WorkItems in it, retrieve them through authorized reads and a bounded project
list, and edit their configured text fields and estimates. Retries, concurrent
changes and interrupted responses preserve identity and never silently duplicate
accepted work. This is implementation direction; architectural acceptance,
execution/resource and trust permissions remain distinct.

The list means a bounded, project-scoped WorkItem list. Include the existing
authenticated project journey, first-party interface, versioned representations,
native transactions and failure evidence. This advances partial M1; it does not
complete #27, #40, #49 or M1. Use synthetic data in a fresh database. No upgrade
of a populated version-two database is offered.

Excluded: configuration-administration routes, transitions, conversion, archival,
current-work selection, relationships/knowledge, search/events, supported
installation/real-data use, general compactor, replicated authority and hosted
testbench work. Their accepted integrity requirements remain relevant. Actual
browser execution remains deferred; no browser/trust or automation-framework
permission is inferred from the goal.

## Evidence and premises

| Premise | Status and decisive source | Consequence / next action |
| --- | --- | --- |
| Project creation already has product authentication and native persistence | Established: merged PR #92; `src/io/kehila_authority/`; `project-authority-verification.md` | Extend the product authority; do not reimplement the anonymous worker or repeat completed creation work. |
| Item creation and sparse editing have accepted domain rules | Established: B-003 seventh slice; `task_contract::item_mutation`, `project`, `operation`; D-021/023/032/033/034 | Reuse those rules; physical codecs/storage are missing. |
| Current SQL/profile permits item writes | Contradicted: `operations.sql` creation-only slice guards; `access.sql`; authority `postgres/profile.sql` | Review explicit new capabilities and exact startup profile before implementation. |
| Current project can still equal its creation snapshot after item writes | Contradicted: native `read.rs` compares the whole Project; create advances sequence and recording estimates changes its monotonic flag | Preserve immutable creation snapshots and stable metadata checks. Proposed ADR-111 revision 3 carries an attributed operational-state chain in permanent item revisions, with indexed head verification. |
| ADR-107 authorizes recoverable item commands | Contradicted: it explicitly excludes generic drafts | ADR-111 proposes pending creation only. On 2026-11-04 the maintainer explicitly selected direct edits with retained client retry identity; no server-pending edit recovery is inferred. |
| Every generated item result can be prepared before any database access | Unsupported: result depends on current item/configuration/project state | ADR-111 proposes a narrowly bounded read-preparation exception to ADR-104; no change to creation ingress. |
| Existing owner profile defines item mutation grants | Unresolved: current serving mapping is creation/read-specific | Propose explicit Read/CreateItem/EditItem mapping with current-grant rechecks, retained scope and revocation fences. |
| This goal establishes supported real-data safety | Contradicted: Q-020 and existing qualification limits | Verify this synthetic slice; retain supported backup/restore/install gates. |

The first implementation depends on disposition of
[ADR-111](../architecture/ADR-111-native-work-items.md). Accepted ADR-103/105/107
are reused within scope; Proposed ADR-108/109 are not silently adopted.

## Completion criteria

1. The existing authenticated project journey still works. Its version-one
   request/result/configuration/project snapshots and historical replay remain
   unchanged after item creation and edits.
2. Task/Milestone creation uses the configured default workflow and initial New
   status, version one and stable internal plus issued prefix/sequence identity.
   Optional configured text fields preserve exact UTF-8 and readable-ID fallback
   for absent/blank titles. No mandatory-title or fixed-column substitution.
3. Estimates use exact thousandths through 999999.999. Zero differs from absent;
   clearing does not clear `ever_estimated`. No floating-point round trip.
4. Sparse Keep/Set/Clear edits preserve omissions and validate complete resulting
   data with expected item version and configuration revision. Ordinary editing
   preserves type/workflow/status/phase. Existing hidden/archived-write and
   required-field rules are not weakened to fit the seeded interface.
5. Item/project/history/success commit together, with pending completion for item
   creation only. Issued numbers
   never recycle; historical type/workflow/status/field use survives replay
   retirement. Reads establish current state without rewriting history.
6. Fresh current mutation grants authorize execution and original-scope replay.
   Exact request comparison includes collection order and expected versions.
   Logical 90-day expiry and permanent tombstones prevent reexecution; changed
   content under an existing identity conflicts. Reconciliation never executes.
7. Individually authorized item reads and a bounded keyset project list use fresh
   coherent snapshots and final live-session checks. Pagination does not claim a
   stable snapshot spanning requests. No unbounded item-field materialization.
8. Creation supports server-owned discovery, explicit execution/cancellation and
   reconciliation after response loss/restart. Direct edits retain the same client
   token and exact request through response loss/retries. Browser/client-record
   loss recovery for edits is excluded by the explicit selected scope. Conflicts
   do not silently rebase requests or resolve earlier uncertainty. Login, reload
   and discovery never execute commands.
9. Source/component, native database/HTTP/TLS, interface, contention, failure and
   restart evidence cover the matrix below. Required hooks/CI and exact-head
   independent review pass; PR is assigned to MrQuality. Actual browser and
   supported recovery limits remain explicit. Architectural acceptance and merge
   remain maintainer decisions.

## Delivery and review map

One coherent capability may use focused commits for contract/codecs, SQL/profile,
adapter/coordinator, HTTP/interface and native evidence. Reassess the delivery
boundary if investigation introduces another subsystem. Component approval does
not substitute for final exact-head composition review.

| Layer | Required review / evidence |
| --- | --- |
| Intent/risk | Source-first Phase A, then ADR-111 challenge and dispositions before dependent implementation. |
| Pure representations | Frozen create/edit request fingerprints and historical result/item envelopes; raw duplicate/depth/size boundaries; exact values/order and no mutable historical validation. |
| SQL/capabilities | Exact fresh schema and function signatures, owners/search paths/PUBLIC revocation, serving effective privileges, named guards and profile rejection. |
| Composition | Preparation/admission, authorization and lock rechecks, current-project/history agreement, transaction rollback/reentry and response eligibility. |
| Interface | Configured fields, creation-pending discovery, bound direct-edit retry identity, sparse edit intent, bounded decoding and conflict/unknown presentation. Simulated DOM is not browser evidence. |
| Native harness | Target/profile/version/role/deadline/resource admission, controlled barriers, independent state and cleanup oracles before runtime campaign. |
| Final delivery | Exact proposed head, applicable local/CI checks, independent composition review, source/evidence validity and proportional retrospective. |

## Risk record

| Failure mechanism | Evidence / confidence / likelihood | Impact / priority | Control and next observation | Residual owner / invalidation |
| --- | --- | --- | --- | --- |
| Lost response allocates a second item or repeats an edit | Full target is part of accepted scoped operation key; mechanism risk is high confidence, frequency unknown | High / before routes | Immutable creation request or retained exact direct-edit request, original scoped token, atomic successful binding, reconciliation and response-loss barriers | Maintainer accepts remaining ADR-111 scope; revisit on identity/lifecycle changes. |
| Serving gains arbitrary project/item/auth mutation | Current profile explicitly forbids updates; widening is a confirmed boundary change | High / before SQL adoption | Narrow fixed functions, guarded transitions and exact effective-privilege negatives; no broad UPDATE grant | Maintainer; function owners/signatures or role changes invalidate review. |
| New item breaks project read or rewrites creation history | Existing full Project equality is inspected source evidence | High / before adapter | Separate current mutation evidence from immutable creation/configuration snapshots; read-after-create/edit and old replay controls | Author/reviewer; current/history reconstruction changes. |
| Concurrent create/edit/configuration state loses allocation or history | Pure rules require coherent project/configuration/item commit; actual new protocol untested | High / before native routes | Explicit fence order, lock waits and post-wait reads; independent SQL cardinality/history checks | Author; future config writers must join protocol before exposure. |
| Preparation becomes a database-work denial-of-service bypass | Generated envelopes depend on stored state; risk of proposed exception, likelihood unknown | High / architectural decision | Bounded raw ingress first; authenticated bounded read-only preparation, shared capacity/deadline; no write transaction before generated admission | Maintainer accepts narrow exception; new queues/limits invalidate. |
| Retired replay destroys product history or permits reuse | D-034 requires permanent identity; existing creation retirement is separate | High / before replay exposure | Separate revision history from replay payload; actual owner retirement and rewind/retry checks | Author; retention/restore changes. |
| Verification overuses memory/user attention | Prior delivery records measured multi-GiB gates; new cost unknown | Medium / before execution | Podman-only serial service phases, fresh resource preflight and approved limits; review harness before any assisted cases | Maintainer resource authority; environment/profile changes. |

## Verification matrix

| Group | Required independent observations |
| --- | --- |
| Nominal and boundary | Both types; absent/blank/non-ASCII text, exact and over-limit values, estimates absent/zero/max/clear, multiple pages/projects, issued IDs and retained unit lock. Compare truthful stored title values with display-only readable-ID fallback in the same native fixture. |
| Authorization | Anonymous/expired/revoked caller, cross-project/request/target mismatch, missing current grant, replay requiring original mutation grants, final-release session check. |
| Concurrency | Observed waits for same command and distinct creates; unique sequence allocation; equal/changed retries; two direct edits at one version; creation execute/cancel ordering; config fence/control with no admin route. |
| Uncertainty | Lost creation registration, rollback before commit, committed create/direct edit before lost acknowledgement, authority restart/new login and original identity reconciliation; discovery never executes. Direct-edit client-record loss is an explicit recovery limit, never proof of rollback. |
| Integrity/history | Corrupt references/codec/version/value/attribution fail closed; stable Project metadata and operational-head read after mutations; creation replay remains original; zero then clear-last-estimate; false unlock/regressed head/duplicate sequence; field/type/workflow/status history and independent relational totals. |
| Replay lifetime | Constructed clock cases distinguish eligibility from elapsed-time evidence; current grants, expiry, permanent tombstone, changed reuse and gap-free owner retirement; successful creation locator and operational estimate witness survive pruning/retirement; serving cannot retire. |
| Admission/lifecycle | Original-byte failures before queues/database, oversized generated envelope before writes, bounded preparation cancellation/deadline and joined jobs/permits; shutdown retains genuine failure. |
| Regression/delivery | Existing creation/auth modes and relevant shared checks; required staged/CI gates, exact-head review and retained failed attempts. |

## Assessment record

### Revision 3 direction, 2026-11-04

The maintainer accepted the proposal-refinement recommendations and explicitly
selected direct edit saves with retained client retry identity. Revision 3 removes
server-pending edit registration/cancellation and does not claim edit recovery
after browser/client-record loss. Pending creation recovery remains proposed.
The original full item create/read/list/edit outcome is unchanged.

The revision states owner-wide mutation and current process-wide serving database
serialization, preserves stable Project verification, proposes an attributed
operational-state witness carried by permanent item revisions, and requires exact
finite retry-work accounting within the original shared capacity/deadline.
The native matrix adds display-only title fallback, zero then clear-last-estimate,
false unlock/regressed head and successful locator survival after pruning.
No fixed retry/backoff/latency numbers or constant-time claims are accepted.

The remaining architectural disposition covers pending ItemCreate, item grants/
capabilities, the operational witness and the narrow ADR-104 read-preparation
amendment. Refinement approval and
the direct-edit product choice are not recorded as acceptance of those unresolved
mechanisms. Revision 3 source/relevance review and implementation gates remain.
Independent revision-three source/relevance review found no substantive blocker
to presenting this proposed bundle. Its clarification requests are incorporated:
bound unresolved edit retention, fail visibly before transmission if retention
fails, and reject operational state-version exhaustion atomically. This is source
review, not SQL/runtime qualification or architectural acceptance.

### Existing coverage and required integration delta

Source inspection at `454d960` mapped the following reusable controls. This is
coverage planning, not a new test execution or proof of native WorkItem behavior.

| Accepted behavior | Existing source control | Missing evidence for this goal |
| --- | --- | --- |
| Default workflow/initial status, optional title and sparse edits | `src/pure/task_contract/tests/item_mutation.rs`: `create_uses_default_initial_status_and_allows_an_untitled_item`, `edit_preserves_hidden_values_and_does_not_change_status_or_usage`, configured-title and hidden/duplicate-field cases | Persist/reload both seeded types through the authenticated native authority; verify issued display fallback and unchanged lifecycle from independent database state. |
| Exact estimates and permanent unit lock | `src/pure/task_contract/tests/project_rules.rs`: exact/zero/absence, unit-lock and sequence-exhaustion cases | Native create/edit/clear persists exact values and monotonic `ever_estimated`, including a competing prepared command with an older project snapshot. |
| Expected revisions and replay before changed validation | `item_mutation.rs` tests: `stale_create_conflicts_but_recorded_success_replays`, `edit_checks_versions_and_replays_recorded_success`, `recorded_success_replays_before_current_payload_limit` | Same scoped identity replays after later item/project changes; fresh stale commands conflict; neither Execute preparation nor generated-envelope failure can mask completed replay. |
| Scoped identity, retained original grants and expiry/tombstones | `src/pure/task_contract/tests/operation.rs`: exact ordered content, retained-scope authorization, expiry boundary and no-grace origin cases | Actual item-family cores, exact versioned codecs, role denial, expiry/retirement and preserved locator/target binding with independent SQL observers. |
| Recovery interface and bounded response decoding | `tests/integration/project_page.test.cjs`: project registration/execution/cancellation uncertainty, binding change, expiry identity and actual response-byte-limit cases | Item-specific interface cases using its eventual accepted protocol. Existing project tests are regression controls only; they do not establish item behavior or actual browser execution. |
| Native HTTP/TLS, faults, contention and final state | `src/io/kehila_authority/src/coordinator/jobs/tests/project/`: `admission`, `authorization`, `clock`, `competing`, `faults`, `restored`, `write_faults`, `evidence` | Extend a separately reviewed synthetic native fixture for items; retain actual held-lock/commit barriers, complete typed reconstruction and connection/lock cleanup observations. |
| Exact startup capability rejection | `src/io/kehila_authority/src/postgres/profile.sql` checks current version-two effective privileges and approved functions | Version-three positive and negative profiles, including unexpected function access, broad UPDATE and modified ownership; no whitelist widening without exact review. |

The current `native-authority` CI job skips its workload steps for documentation
scope (`.github/workflows/ci.yml`). Its green status on this proposal therefore
does not mean native fixtures executed. Later executable changes must select the
full applicable gate, with explicit new WorkItem test selection in the native
fixture and observer; an unchanged project-only mode cannot qualify this goal.

The preceding advisory review was not an implementation acceptance. A separate
context received the selected user outcome and inspected B-003, domain rules,
operation retention, native SQL/profile, reads and accepted architecture before
receiving the candidate. Phase A identified the creation-only privilege boundary,
missing item-target recovery/grant contracts and the mutable-Project versus
creation-snapshot conflict. No runtime work was executed for this assessment.
Phase B reviewed ADR-111 revision one against that framing. Two pre-implementation
P1 ambiguities were corrected in revision two: Execute must resolve completed
historical replay before mutable preparation; successful public-locator recovery
mapping must remain permanent independently of pruned pending/replay content.
The revision also specifies registration collision exclusion, complete mutable
project dependency comparison, bounded/indexed current-state evidence, final
response eligibility and separate generated-envelope expansion limits.

The independent verdict requires maintainer architectural acceptance before
dependent implementation. The smallest decision covers ADR-111's limited
recoverable item-command lifecycle, explicit grants/capabilities and narrow
ADR-104 read-preparation exception together. No dependent schema, codec or route
implementation is authorized by a reviewer verdict. Exact SQL/codecs/harness
remain subsequent reviewed implementation artifacts, not new product goals.

An initial documentation check ran 425 tooling tests with one failure, 37 errors
and three platform skips. The reported Windows account-directory resolution
failure occurred under restricted execution. This is a failed check; it is not
runtime evidence, and an unrestricted rerun must be recorded separately rather
than replacing the failure. No service campaign was started.

The separate normal-host documentation check passed all 425 tooling tests with
three platform skips in 35.504 seconds, followed by naming checks and synchronized
42-requirement engineering metadata. This checks documentation/tooling only;
WorkItem runtime, schema, codecs and interface are not implemented or qualified.
Whitespace and working-tree publication checks also passed.

The independent source/relevance reviewer checked revision two and the associated
status-document corrections, confirmed both P1 dispositions and found no
remaining substantive relevance blocker to publishing the proposal. That verdict
does not accept architecture or replace final exact-head review. The draft PR
remains a contract checkpoint toward the full selected outcome, not goal completion.

Timing: initial source/review preparation was not instrumented; duration unknown.
At 2026-11-04 13:39:49 UTC assessment remained active. Source-reading and
independent-review spans overlap; no summed elapsed-time claim is made.
At 2026-11-04 13:48:33 UTC proposal relevance review and normal-host documentation
checks had completed; exact-head review/publication remained pending.
