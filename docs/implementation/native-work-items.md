# Native WorkItem delivery

Status: Implementation; ADR-111 revision 3 explicitly accepted on 2026-11-04 at
`80db13c71d60fe9ea53784765c81644952774394`. Full capability and qualification pending.
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
| Current project can still equal its creation snapshot after item writes | Contradicted: native `read.rs` compares the whole Project; create advances sequence and recording estimates changes its monotonic flag | Preserve immutable creation snapshots and stable metadata checks. Accepted ADR-111 revision 3 carries an attributed operational-state chain in permanent item revisions, with indexed head verification. |
| ADR-107 authorizes recoverable item commands | Contradicted: it explicitly excludes generic drafts | Accepted ADR-111 separately supplies pending item creation and the selected direct-edit scope; no server-pending edit recovery is inferred. |
| Every generated item result can be prepared before any database access | Unsupported for the selected state-dependent approach | Accepted ADR-111 supplies narrowly bounded read preparation under ADR-104; project creation ingress is unchanged. |
| Existing owner profile defines item mutation grants | Current SQL profile is creation/read-specific; new mapping accepted in ADR-111 | Implement Read/CreateItem/EditItem with current-grant rechecks, retained scope and revocation fences in the reviewed schema/adapter packet. |
| This goal establishes supported real-data safety | Contradicted: Q-020 and existing qualification limits | Verify this synthetic slice; retain supported backup/restore/install gates. |

Implementation follows accepted
[ADR-111 revision 3](../architecture/ADR-111-native-work-items.md). Accepted ADR-103/105/107
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

### Authorized-read runtime checkpoint

At source `b77aabe`, production composition and startup still support projects
only. This focused checkpoint activates the fresh version-three schema,
updates the exact serving profile and existing project reader, and connects
`/items/read` and `/items/list` to authenticated production reads. Mutation routes
and the owner interface remain required subsequent work in this same delivery.
Owner-created synthetic fixture rows are test inputs, not a claim that product
item creation is implemented. No populated-database upgrade is offered.

Source-first review confirmed this boundary is within accepted ADR-111 revision
three. Its two implementation prerequisites are explicit nested-helper privileges
and bounded read materialization. The read authority and its grant-check helper
are invoker functions; both require enumerated execution privileges. Pending,
writer, reconciliation and save functions remain unavailable to serving until
their corresponding production capability is implemented.

The read plan retains the existing 4,096-byte ingress bound, shared sixteen
admitted lifetimes, single database mutex and original five-second work deadline.
Each operation uses one authorized read-only repeatable-read snapshot and confirms
rollback before releasing the database guard. A project is reconstructed and
checked against its historical configuration and stable creation metadata before
one indexed operational-head witness is materialized. Head zero requires the
creation core and initial counters; later heads require a fully decoded attributed
revision and its indexed predecessor link. Historical creation replay is unchanged.

An item read fetches bounded stable identity, its creation revision and its current
revision. Each history projection has a one-MiB SQL sentinel and a one-row bound.
Decode and verify the creation revision, retain its issued identity and fixed
lifecycle keys, then
drop its driver row and decoded result before fetching current history. Head
verification also finishes before item history is fetched. Compare the current
typed envelope with fixed relational fields and permanent operation attribution;
do not rerun mutable domain decisions on accepted historical content.

Listing fetches seventeen bounded identity/sequence rows to detect a next page,
then processes at most sixteen entries in sequence. It reuses the authorized
project/configuration snapshot and materializes one item's histories at a time;
it does not fetch seventeen complete histories together. Only bounded summaries
survive each iteration. The title is the configured text value with readable-ID
fallback, without changing stored text. Charge each complete encoded summary
against the outer wire budget before retaining it. Stop before the first row
that would exceed the budget, using the last delivered sequence as cursor; every
single admitted summary must fit, otherwise fail explicitly. Never truncate text
or advance past an undelivered row. Pages do not share a stable snapshot.

| Lifetime | Retained records and release boundary |
| --- | --- |
| Project proof | Current relational configuration plus the two bounded historical snapshots; release historical pairs and driver rows before fetching the single bounded head history. |
| Item proof | Current project/configuration, bounded scalar identity, at most one history driver row and its decoding temporaries; retain issued identity and fixed lifecycle keys between creation and current history. |
| List | At most seventeen scalar rows, the shared project/configuration, one item proof, accepted summary values and one temporary encoded candidate summary. |
| Delivery | Bounded encoded response and the existing session/reservation; database transaction and guard have been released. |

These are serialized-record and row bounds, not measured heap guarantees. JSON
trees, driver buffers, typed clones and encoding temporaries remain additional
costs under the existing shared capacity. Mutation preparation packet aggregate
and finite write/reprepare accounting remain prerequisites of the later save
adapter; no arbitrary values are accepted by this read checkpoint. Required
native evidence includes the real serving principal, head zero and multiple-item
heads, original project replay, pagination/byte boundaries, corrupt attribution,
unsupported startup profiles and denial of unexposed mutation capabilities.

Source preparation on 2026-11-05 connected the read/list production chain and
fresh-v3 read capabilities. Component review found a missing stable-Project
agreement check in decoded history; the correction binds operational-head,
creation and current-item history to the validated project metadata, permitting
only legitimate historical counter lag in this profile without metadata or
configuration writers. Such writers must introduce historical-snapshot comparison
before exposure. This is a source-reviewed restriction, not a mutable domain
decision on a historical accepted result.

The first container compiler/unit attempt at tree
`29767296f6d856485b0095da8c2db8de503bbbe0` compiled successfully: 157 library tests
passed, one failed and thirteen remained ignored. The failure was the page-budget
test fixture: its 64-KiB title exceeded the existing 16-KiB domain text bound and
never reached the page encoder. That failed result is retained. The corrected
fixture uses accepted 16-KiB text with six-byte JSON escape expansion; it does not
weaken the domain bound. The affected rerun at tree
`8ef037c5209f197e5fc96ab0432b0e0911309100` passed all seven selected item tests and
all twenty-five storage-controller tests. The other 157 passed observations remain
applicable only to unchanged behavior; this is not a new full-suite pass. No new
native database, HTTP/TLS or startup-profile qualification is claimed yet.

Both attempts used one compiler job in the pinned two-GiB/one-CPU runner and
confirmed resource removal and provider shutdown. The failed attempt took
134.610 seconds through cleanup, with sampled aggregate peak 3.437 GiB and minimum
available 2.398 GiB. The affected pass took 95.797 seconds through cleanup, with
peak 3.255 GiB and minimum available 2.825 GiB; neither reported a warning or stop
condition. These are sampled observations, not heap or performance guarantees.

The first native attempt at tree `516b74f35ba4dafe4a0edc13f1d462fe1b54c1b5`
compiled the release authority, installed fresh v3 and entered the new isolated
item-read journey. Individual creation and edited-current reads passed their
assertions; the first populated list returned 503. The attempt stopped there:
later pagination, corruption/startup controls, the independent observer and other
native journeys were not qualified. Cleanup removed the database and runner and
stopped the provider. Elapsed time through cleanup was 178.782 seconds, sampled
aggregate peak 2.901 GiB, minimum available 3.151 GiB, without warning/stop alerts.

Source diagnosis identified unqualified `ORDER BY` names matching text-projected
output labels, in both issued-sequence listing and the current-revision maximum.
PostgreSQL gives an output label precedence over an input name in this situation
([PostgreSQL 16 sorting rules](https://www.postgresql.org/docs/16/queries-order.html)).
The correction qualifies the numeric base columns, retaining the full sixteen-row
page. The native fixture checks every issued identity across twenty items and
extends the edited item through version eleven, covering both decimal digit
boundaries. The failed evidence is retained; the correction and remaining native
cases require a fresh affected attempt before any passing qualification claim.

The affected native campaign at tree `466c90e4483e71a7794fca76b120cb1bd31603ab`
passed all twelve provisioned modes, including the item-read journey and its
independent committed-state observer. It exercised complete twenty-item identity
pagination, current version eleven, zero then cleared estimate with monotonic
project state, unchanged historical project replay, selected head/non-head
corruption detection, six selected startup-profile rejections and restored normal
protections. The existing project/authentication/clock/restored journeys also
passed with the new project-head write ordinal included. The source correction
therefore has native repair evidence; the original failed attempt remains retained.

This pass took 212.391 seconds through cleanup; sampled aggregate peaked at
3.098 GiB and available memory reached a minimum of 2.721 GiB, without warning or
stop alerts. Owned database/runner removal and provider shutdown completed.
The mandatory full commit gate remains required. Its final fixture additionally
checks a native byte-budget page with accepted escaped field text and a lookup
under another authorized project. Those added cases are prepared, not yet passed.
Item mutation routes, client recovery and the owner interface remain unfinished.

The first mandatory full gate at tree
`5f4613ada5575327b966e29d46371955f854daea` failed its resource floor after
OpenSearch reported ready, during network/cleanup exit. The sampled available
memory reached 1.492 GiB below the retained 1.5-GiB stop floor; sampled aggregate
was 4.495 GiB below its six-GiB ceiling, and the Windows low-memory signal was
false. The controller terminated and joined its owned process tree. Interrupted
normal cleanup failures remain recorded; separate post-join recovery removed
the owned runner and OpenSearch resources and stopped the provider. No commit
was created. Total elapsed time through recovery was 535.969 seconds.

Before that stop, 475 Python tests passed with two skips, the authority library
passed 158 tests with fourteen ignored, and all twelve provisioned native modes
and their observers passed. This includes the final escaped-text byte page and
cross-project denial cases. NATS also completed; PostgreSQL/FerretDB task-path
verification was not reached. Those are retained partial observations, not a
successful full gate. The prior native repair and failed attempts are not replaced.

The next required-gate attempt lowers only the runner ceiling from 2 GiB to
1.5 GiB, with one compiler job and the same tests, CPU, deadlines, host floor and
aggregate limit. This is a provisional resource-containment experiment within
the previous ceiling, not an established performance improvement or a claim that
cache caused the entire measured footprint. A lower cgroup hard limit can force
reclaim or fail the workload if its memory cannot be reduced
([kernel memory controller](https://docs.kernel.org/admin-guide/cgroup-v2.html#memory-interface-files)).
The existing first-failure stop and exact owned recovery remain required. No
image build, VM/OS configuration or trust change is part of this adjustment.

The lower-ceiling mandatory full gate passed on exact tree
`a539347e11f0c82ec45c7b8c1255fbf2e4e17ae0`, producing read checkpoint commit
`c1c962f552e7ada50ff59a97217cd8a09b916a31` through normal hooks. Starting at
2026-11-05 23:01:29 UTC, it completed in 618.953 seconds through cleanup. It ran
the full selected matrix: 475 Python tests with two skips, fourteen project-page
tests, workspace/codec/oracle/Go checks, all twelve provisioned native authority
modes and their committed-state observers, NATS/OpenSearch readiness and the
PostgreSQL/FerretDB task path. The authority library passed 158 tests with fourteen
ignored; twelve provisioned native modes execute separately, while the external
password corpus and protected-startup asset case remain outside this fixture.
Task-path controls included forty-eight competing command pairs, stalled-body
and database deadlines, and Go-to-Rust save/replay/read with search unavailable.

The effective runner ceiling was 1.5 GiB with one CPU and compiler job. Sampled
aggregate working sets peaked at 4.193 GiB and available memory reached 1.641 GiB.
Reserve warnings occurred, without a stop alert. The controller joined normally,
all three service groups reached verified completion, anonymous volumes were
removed, and the provider stopped. The successful run does not replace the
earlier memory-stop evidence. Different launch headroom and concurrent host use
prevent attributing the outcome solely to the lower ceiling; this is not an
optimal-memory or safe-launch-minimum claim. No repository resource default was
changed.

The delivered read checkpoint installs fresh v3, exposes authorized item reads
and bounded lists, preserves project creation replay, and verifies current
operational state through indexed attributed history. It does not expose item
mutation functions or provide the owner interface. The remaining save adapter
must capture exact snapshot dependencies, bound aggregate preparation/retry work
under the original reservation, and own final replay eligibility before routes
and client recovery are qualified. Remote CI and final committed-head review
remain publication prerequisites; full-goal readiness remains incomplete.

#### Read checkpoint delivery review

The native twenty-item case found an ordering defect that source review and the
small pure fixtures missed: text-projected numeric labels were selected by
unqualified ordering. Keep table-qualified numeric ordering and actual decimal
digit-boundary cases in subsequent read/write review. The version-eleven and
complete-page identity controls detect both affected queries. A separate initial
fixture failure confused the codec string ceiling with the smaller domain field
ceiling; accepted-input fixtures now respect both limits.

The first full-gate resource stop demonstrated the owned post-join recovery path
for the runner and OpenSearch group, while also costing an incomplete full run.
The next run preserved checks and reduced the runner ceiling within the prior
budget. Continue measuring reserve and stop conditions; one successful rerun does
not establish repeatable savings or eliminate the narrow headroom risk. No new
general verification campaign is introduced by this observation.

Source preparation and independent review overlapped and were interrupted by
maintainer-requested pauses, so their active durations are unknown and are not
summed with campaign times. The separate compiler-fixture failure, native ordering
failure, corrected native pass, memory-stop gate and successful full gate retain
their own source identities, phases and validity limits above. This is a
proportional checkpoint review, not closure of the selected WorkItem goal.

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
| Lost response allocates a second item or repeats an edit | Full target is part of accepted scoped operation key; mechanism risk is high confidence, frequency unknown | High / before routes | Immutable creation request or retained exact direct-edit request, original scoped token, atomic successful binding, reconciliation and response-loss barriers | Author implements accepted ADR-111; review and revalidate on identity/lifecycle changes. |
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

### First codec component: implementation preparation

The pure `work_item_codec` candidate implements distinct version-one canonical
create/edit requests, saved results and permanent revision envelopes. It preserves
scope, ordered sparse actions, exact numeric strings and all typed field values.
Shared composition reuses the existing raw parser, bounded record writer and
version-one Project mapping. Canonical request keys are sorted explicitly,
independently of serde_json map features. The lockfile adds only the local crate;
no external package versions change.

SQL composition review then found that the candidate history JSON duplicated a
fresh database-assigned operation ID that is unavailable before write admission.
The unpublished representation is corrected: the relational revision row owns
that ID, while the pre-admitted payload keeps actor, configuration, predecessor,
operational version and accepted result. Reload must verify full relational
attribution. Write-entry dependency comparison includes operational head identity,
even when another item's edit leaves sequence/estimate values unchanged.
The earlier staged candidate is not eligible for execution; capture and review a
new exact tree after this source correction. No qualifying run had used it.

Component source review identified missing independent result/history fixtures;
those fixtures and a revision aggregate-boundary control were added and reviewed.
This is source evidence, not executed qualification. A verification-matrix
regression first failed because the new crate was omitted from the pure command,
then passed after selection was corrected; it uses command doubles only.

Initial Rust component diagnostics mistakenly ran on the host, outside the
required container lane. They are not compliant gate evidence. They retained
expected unimplemented request/history failures, a cache-resolution stop, a
compile error, a lint failure and later diagnostic passes separately. At that
point the final new fixture cases and required containerized gate were unexecuted.
No schema, serving route, native WorkItem transaction or browser behavior was
qualified by those diagnostics.

After RAM became available, the first container dependency-preparation attempt
failed during provider startup because a required resident-memory observation
was unavailable. It did not reach dependency preparation, compilation or tests.
The original automatic-cleanup failure is retained separately from the later
confirmed provider shutdown. No commit was created and the full gate did not run.
The startup-observation ordering defect was corrected and independently reviewed;
bounded regression controls failed against the original and passed against the
correction. Those controls establish no WorkItem or database qualification.

The corrected bounded dependency preparation passed on 2026-11-04, starting at
18:59:32 UTC and taking 517.359 seconds through cleanup. The mandatory full
container commit gate then passed from 19:09:25 UTC in 582.906 seconds through
cleanup, creating `b81b64c8ef6b25e176aaf8574bacccee19700051` with reviewed tree
`142d97ce0cf312c7b39519746fc684170da9947b`. Each phase confirmed provider shutdown;
the commit controller joined without termination. Source preparation and review
spans were not measured; no whole-task elapsed-time claim is made.

The gate passed 426 Python tooling tests with two platform skips, all 18 new
WorkItem codec tests, the shared workspace/oracle/Go checks, the existing native
authority fixture, NATS/OpenSearch readiness and PostgreSQL/FerretDB task-path
checks including 48 competing pairs. The external password-corpus test remains
explicitly ignored; actual browser execution is unqualified. This establishes
the codec checkpoint and regressions, not native WorkItem persistence or the
full selected capability. The SQL records/retention sources remain outside this
commit and its executed source snapshot.

Sampled preparation aggregate memory peaked at 1.611 GiB with minimum available
host RAM 4.000 GiB. The full gate peaked at 4.593 GiB aggregate and minimum
available RAM 1.677 GiB, producing a reserve warning but no stop-floor breach.
These sampled observations are not a continuous peak guarantee or calibration.
The narrow reserve during the full gate and dependency-cache preparation cost
inform later resource/caching work; they do not add another completion campaign.

The codec checkpoint was pushed to draft PR #96. Independent final committed-head
composition review found no substantive findings and inspected the retained gate
summary and codec test results. All three jobs passed on `b81b64c` in
[CI run 41896368550](https://github.com/MrQuality/kehila/actions/runs/41896368550):
`verify`, `native-authority` and `windows-owner-client`. This executable scope ran
their workload steps; it was not the earlier documentation-only skip. The PR
remains draft and assigned to `MrQuality`; full-goal completion and merge are not
claimed. The updated record and uninstalled SQL sources are local preparation
for the next coherent checkpoint, outside the published codec tree.

### Storage foundation checkpoint, 2026-11-04

The normal-hook full Podman gate subsequently passed on tree
`d067298285b6c456d53e8b1c2a9493de75338bad`, creating storage checkpoint
`0046be0a9b1966dc6e0c13fe7e0976b7bca8dde2` from `b81b64c`. It passed
474 tooling tests with two platform skips and the existing workspace, native
authority and service-backed task-path regressions. The run took 542.296 seconds
through cleanup; minimum sampled available RAM was 1.585 GiB and aggregate peak
4.647 GiB, with reserve warnings but no stop. All tracked service groups completed,
including normal anonymous-volume cleanup, and provider shutdown/controller join
were confirmed. See the [full checkpoint record](native-work-item-storage.md#passed-full-storage-checkpoint-gate-2026-11-05)
for exact scope and retained failures. The SQL remains uninstalled; this does not
complete Rust integration, the authenticated WorkItem journey or PR #96 readiness.

Owner-level atomic saves and a native predecessor-contention control passed on
2026-11-05 at tree `5a722f2c2e31bd13f18c3bb4ff45e17b3f4482e1`, with twenty-five
checker controls and a separate final observer. Source challenge and tests
exposed and corrected a late implicit-FK lock on the predecessor core; a
canonical malformed-history control also now rejects before that FK path.
The full focused campaign took 57.844 seconds through owned cleanup/provider
shutdown; minimum sampled available RAM was 3.669 GiB and aggregate peak
1.734 GiB. Earlier failures and an invalid initial malformed-history oracle
remain distinguished in the [save/ordering record](native-work-item-storage.md#atomic-saves-and-predecessor-ordering-checkpoint-2026-11-05).
This qualifies the uninstalled SQL slice under one command per transaction,
with full rollback before retry. Typed transaction ownership/reload, serving
adoption and authenticated end-to-end WorkItem behavior remain incomplete.

The required full gate on the subsequent documented storage tree stopped at
the 1.5 GiB available-memory floor during OpenSearch readiness. It created no
commit. Initial interrupted cleanup missed an owned service; separate verified
recovery removed it and stopped the provider. The [failed-gate record and
checkpoint retrospective](native-work-item-storage.md#required-gate-hold-and-checkpoint-retrospective-2026-11-05)
preserve both failures, the completed subphases and the remaining cleanup repair/
resource prerequisites. The storage packet remains local and PR #96 remains
draft; the published codec head's earlier gate does not qualify this new tree.

The subsequent write-ahead ownership repair passed its focused controls and one
real, bounded NATS interruption/recovery check. It preserves partial-start
identities before deletion and retains primary failures when cleanup refuses
ambiguous resources. The successful control confirms exact-ID recovery after a
joined lifecycle child, not general host-failure or anonymous-volume recovery.
See the [repair evidence and limits](native-work-item-storage.md#verification-ownership-repair-checkpoint-2026-11-05).
The full-gate hold was later closed by the passed checkpoint above; the original
failure and its qualification limits remain in the record.

Owner-level historical create/edit reconciliation passed on 2026-11-05 at tree
`8fb9bf7ba4bbeb17927088db67af6bbe35da0321`, with nineteen checker controls
and independent committed-state observation. It preserves recorded identity
before mutable eligibility, distinguishes conflict/expiry/scoped absence, and
performs no execution. Selected retention-corruption and constructed-time cases
fail closed. See the [reconciliation record](native-work-item-storage.md#historical-reconciliation-checkpoint-2026-11-05)
for evidence and limits. Internal replay availability does not qualify decoded
client success; typed Rust integration, serving adoption and the
full authenticated journey remain incomplete.

Owner-level read-only pending discovery/open subsequently passed on 2026-11-05
at tree `8152ad470cee19dcca9523b002bd89e9ac1a6223`, with seventeen checker
controls, scoped keyset pagination, complete expected authority, bounded content,
nonmutating effective expiry, corruption classification and a separate committed
observer. One corruption-harness failure is retained separately from the pass.
See the [discovery record](native-work-item-storage.md#read-only-pending-discovery-checkpoint-2026-11-05)
for scope and limits. Product installation/runtime privileges, typed application
integration and the full capability remain incomplete.

The later owner-level registration/cancellation campaign passed on 2026-11-05
at tree `94d43d8cd9060b5643fa0b619124bcb5965d8983`. It includes fifteen
checker controls, private item authority, current-grant negatives, fixed pending
registration/cancellation, cross-project owner capacity and independent retained
state observation. A cancellation expiry regression first failed and then passed
after the transition guard became the single clock owner. Both reached expected
failures and earlier resource failures remain retained separately. See the
[storage record](native-work-item-storage.md#fixed-registrationcancellation-checkpoint-2026-11-05)
for exact evidence and limits. These entries remain outside the installer with
no new serving grants; execution/reconciliation, read-only discovery, application
transactions, runtime profile and API/interface are still outstanding.

The subsequent history-agreement campaign passed at tree
`333b63deb3d0f9bb7040752260c764b022b22398`, including twelve checker controls,
selected attribution/representation/lifecycle/field-use rejections and separate
committed-state observation. Owned cleanup and provider shutdown completed;
independent evidence review found no material blocker to capability source work.
The earlier missing-history-guard failure remains failed evidence. See the
[storage record](native-work-item-storage.md#history-agreement-checkpoint-2026-11-04)
for exact scope, resource observations and limits. Product adoption, runtime
authorization, typed reload and full capability qualification remain incomplete.

The subsequent local [storage packet](native-work-item-storage.md) has passed
an owner-only PostgreSQL structural campaign at tree
`ee49b22a0f8a40143cfd57df1f800cd1f85aae10`: committed create/edit, 12 selected
named rejection controls, retirement/pruning and a separate retained-state
observer, with complete owned cleanup. Ten checker controls passed in-container.
The prior startup-lifetime and SQL-parser failures are retained separately. This
is not serving-login, concurrency, typed-history, API or complete goal
qualification. Product version-three adoption, fixed authorized write entry,
history/field-use agreement and runtime/interface integration remain unfinished.

### Implementation authority, 2026-11-04

The maintainer explicitly accepted ADR-111 revision 3 at `80db13c` and directed
implementation. This settles the pending item-creation, grants/capabilities,
operational-witness and ADR-104 amendment decision; do not repeat that approval.
It does not authorize merging or new runtime/trust changes. Exact representation,
SQL/capability and harness reviews and required verification remain.

The first implementation packet owns pure WorkItem create/edit request and saved
result/history representations. It reuses domain rules, frozen Project mapping,
original-byte JSON admission and existing locked dependencies. New representations
must preserve exact ordered command content, expected versions, scoped targets,
historical reconstruction and native byte/NUL/JSONB expansion bounds. No serving
route or schema capability is introduced by that packet. Later packets retain
the original full goal and regression/qualification matrix above.

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
Whitespace checks also passed.

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
