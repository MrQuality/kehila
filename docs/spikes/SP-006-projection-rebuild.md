# SP-006: projection rebuild snapshot-to-live handoff

| Field | Value |
| --- | --- |
| Status / outcome | Four bounded live cases passed in R01 and reviewed R02; final staged/hosted verification tracked in PR |
| Owner / assessor | Project maintainer; separate-context source/relevance review |
| Created / last updated | 2026-10-24 |
| Related work | #49 W3/W5/W7, B-005/B-023, D-016/D-017, ADR-101/102; #40/#41 prerequisites remain |
| Run limit / stopping condition | 600 seconds work plus bounded owned cleanup; fail on incorrect state, unmet resources or cleanup failure |
| Prior evidence | Existing task-path projection/version rules; no native outbox/indexer implementation |

## Outcome and framing

The maintainer directed design and implementation of a corrected rebuild revision
on 2026-10-24 and separately allowed a bounded standalone PostgreSQL/OpenSearch
proof and mandatory commit gates. The native application adapter remains absent;
this prototype does not adopt production schema or settle ADR-103. Baseline: merged
PR #81 at `18391aa`.

A per-project maximum of independent per-document revisions cannot establish
completeness. PostgreSQL identity allocation is not a commit frontier. An
application-maintained transaction fence can instead register the building target
and establish its snapshot with complete writer coverage, then use individual
durable delivery acknowledgments before cutover. See the
[implemented protocol and limits](../../experiments/SP-006/README.md).

| Consequential premise | Evidence / disposition |
| --- | --- |
| A current native indexer can be patched | Contradicted by implementation status; this is a standalone proof, with native integration still gated |
| Maximum ID or maximum document revision proves catch-up | Rejected; actual counterexample cases are required before claiming a replacement |
| Every writer participates in the gate | Required prototype invariant in the fixture put function; production must enforce it through capabilities |
| Registering a target means the SQL snapshot exists | False until a statement establishes the repeatable-read snapshot while fenced |
| SQL metadata and an external alias change atomically together | False; durable switching intent, dual delivery and actual alias reconciliation are required |
| A doc/test result establishes architectural acceptance | False; source review and real-service evidence remain bounded and adoption remains separate |

Separate-context Phase A checked public requirements before the candidate. Phase B
accepted this bounded layer subject to exact artifact review before live execution,
including lock order, acknowledgment semantics, snapshot loss, alias uncertainty
and explicit refresh. Later source/evidence dispositions belong in this record.

## Cases

| Case | Required actual observation | Claim supported |
| --- | --- | --- |
| C01 | Lower identity allocated by an uncommitted transaction is absent from the snapshot but commits below its visible maximum | Maximum sequence is unsafe as a generic commit frontier |
| C02 | Observe registration blocked by the writer through pg_blocking_pids; exercise newer event before older snapshot, ACK loss boundary, same-version corruption and hidden stale document; corrected cutover reaches exact fixture state and searchable alias | Gate/snapshot/individual-ACK handoff works for these schedules |
| C03 | Interrupt coordinator after alias application before metadata completion; later writes target both indexes; a new coordinator reconciles; the read alias exposes the expected post-interruption value/revision and promoted-target pending deliveries are zero | Alias/metadata split does not silently drop these tested updates or leave the tested value unsearchable |
| C04 | Terminate the source backend, reject promotion, preserve old alias, rebuild a fresh epoch | A lost snapshot is not resumed with a fresh transaction into the same epoch |

No database/search I/O mock substitutes for these assertions. Synthetic protocol
cases do not establish production auth, grants, deletion semantics or performance.

## Environment and results

### SP-006-R01, 2026-10-24

All four planned cases passed on Windows using the configured PostgreSQL 16.13
and OpenSearch 2.19.1 images. The [result/source manifest](../../experiments/SP-006/results.observed.json)
records exact LF-normalized executed/publication hashes and sanitized observations.
C01 demonstrated the rejected identity frontier. C02 observed the actual SQL
blocker and the hidden-stale-document counterexample before corrected cutover;
duplicate/out-of-order and same-version corruption assertions passed. C03 retained
both targets across the injected alias/metadata interruption and recovered under
a new coordinator. C04 used actual source-backend termination, refused promotion,
kept the old alias and completed a fresh epoch.

Preparation included an entry-point setup correction, a failed cgroup-limit
startup and a RAM-preflight stop; none was reported as a passed protocol case.
The maintainer separately approved the revised fixed-small-workload profile with
no per-container caps. Available RAM was checked between phases; resources and the
owned VM were cleaned up after every startup attempt. Source/runtime inspection
must not be confused with tested all-provider crash/deletion/privilege behavior.

No production schema, adapter, authentication or new runtime dependency is adopted.
Two targeted mutations were detected at the intended assertions: omitting the
writer shared gate failed the expected-wait check; bypassing same-version payload
equality failed corruption rejection. Source was restored and all four cases
passed again. Executed hashes retain the tested implementation. The final Rebuild docstring
was corrected to disclose the read-only SQL snapshot spanning bulk-copy HTTP and
its MVCC-retention cost; AST comparison with docstrings removed found no executable
change. Separate publication hashes bind that correction to final source.
Separate-context final review corrected a case-description reversal and reported
no additional actionable findings within this bounded scope; it did not execute
the services. Both focused executable commits passed mandatory staged verification, including
275 tooling tests (272 passed, three documented Windows skips), Rust/Go/static
and owned live-service checks. Publication review reported no disclosure findings.
Hosted CI is tracked in the PR. Raw logs and local metadata remain outside the
checkout.

## Coding-standard scope

The changed shared operation is `service_group`: an optional fixture path is
resolved and checked before startup, and the same Compose command/project owns
startup, diagnostics and cleanup. Existing callers retain the default fixture;
the regression verifies custom-file and project ownership. At R01, all 36 lifecycle
unit tests passed. Ignoring the custom file failed the new ownership assertion;
restoring source returned all 36 tests to passing.

SP-006 is an opt-in I/O experiment with synthetic trusted SQL, not pure domain
code or a production API. Applicable ownership/bounds/error/persistence rules are
covered by owned participants/services, fixed byte/work/fixture limits, explicit
failed assertions, individual delivery state and the actual failure schedules.
SQL identifiers/values come only from the bounded fixture and checked literal
helper; this does not qualify production parameterization or privileges. The
standard-library HTTP adapter disables proxies/redirects and bounds responses.
Production authority, schema evolution, deletion and supported operating limits
remain #49 W3/W5/W7 obligations; authentication and allocation remain #40/#41.

## PR #82 ownership and regression follow-up

Review identified a conditional ownership race in global PostgreSQL discovery:
if the owned container exited while an unrelated new one appeared, the runner
could select the unrelated identity. The service owner now yields exactly the
IDs it verified for its project; SP-006 retains that PostgreSQL ID and fails if
it disappears. Exclusive machine mode is required before startup.

All 37 service-lifecycle tests and four cheap SP-006 import/admission/ownership
tests passed. The new boundary regressions failed before implementation at
premature startup and forbidden global discovery; the initial import-harness
search-path correction is setup evidence, not a behavioral regression result.
These tests run in ordinary discovered CI without live services. The live recovery
assertions and result are recorded below.

### SP-006-R02, 2026-10-24

All four real-service cases passed after the ownership and independent recovery
assertion changes. C03 verified the known `aftercrash` revision-two payload through
the recovered read alias, exact fixture IDs, and zero pending promoted-target
deliveries. Fixture automatic refresh is disabled so the protocol's explicit
refresh is observable; no production refresh policy is adopted.

Three targeted regressions were detected: bypassed exclusive admission, unrelated
container-ID substitution, and recovery-only omission of explicit refresh. The
last failed the new expected search-value assertion. Source was restored before
the complete successful live run. Eight LF-normalized source/test hashes match
the exercised files in `review_followup_R02` of the result manifest. Cleanup
removed the owned services and stopped the owned VM. R01 retains its earlier
source hashes; it does not purport to qualify this changed implementation.

The first focused follow-up commit passed mandatory staged verification: 280
tooling tests, 277 passed and three documented Windows skips, plus Rust/Go/static
and owned live-service checks. Final follow-up staged verification and exact-head
hosted CI are recorded in PR #82. Separate-context source/relevance review checked
ownership, admission, assertion independence and deterministic refresh sensitivity;
it did not execute services. The earlier review missed the ownership race; passing
shared gates did not supply SP-006 regression coverage. No broader production,
crash, deletion, resource-limit or authentication qualification follows.

## Handoff

Review the PR and exact-source hosted CI, then decide the next bounded slice.
Keep #49 open. Online native pipeline integration remains separately reviewed work.
