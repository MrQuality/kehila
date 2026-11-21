# Architecture verification follow-up

Status: selected implementation and component evidence are recorded below. Final
delivery status belongs to [PR #107](https://github.com/MrQuality/kehila/pull/107);
component passes alone do not establish completion or wider qualification.

This delivery strengthens regression protection around the current native path.
It addresses verification gaps rather than asserting a demonstrated production
integrity failure. Preserve earlier test results with their source and environment
identities; preparation and mocked orchestration are not real-service evidence.

## Outcome and review map

| Boundary | Required outcome | Selected verification |
| --- | --- | --- |
| SQL integrity | Integrate currently uncovered foundation and product assertions without duplicating existing native controls | Compare standalone assertions against native fixtures; test orchestration admission, failure propagation and owned cleanup; execute unique real-role SQL controls in fresh isolated clusters |
| Pure dependencies and APIs | Add a bounded regression guard for forbidden dependency directions and external observations | Accepted/rejected fixtures, alias handling, deterministic API controls and current module inventory; document unsupported syntactic forms and semantic limits |
| Codec and replay exploration | Add reproducible bounded generation for named parser and operation invariants | Fixed seeds, finite Unicode/JSON and sequence budgets, independent observable properties and retained counterexamples; keep valid unrelated inputs to reach the intended rejection |
| Minimum Rust | Establish evidence for the affected declared minimum compiler/dependency combinations | Exact compiler versions with locked inputs and explicit crate selection; investigate incompatibility before changing declarations or dependencies |
| Browser journal | Extend evidence for actual journal contention and selected storage failures | Review harness and controller checks first; bounded actual two-tab transaction cases and observed abort/failure controls; distinguish injected errors from actual quota exhaustion |
| Current architecture records | Correct stale current descriptions and retain future capability gates | Register/document synchronization, changed links and source review; preserve dated inventories and failed evidence |
| Qualification reporting | Keep scope and actual execution visible | Check exact source, selected scope and executed steps; a successful job whose native steps were skipped is not native qualification |

## SQL coverage and isolation constraints

`check_selection_storage.py` already composes `check_work_item_storage.py`.
Running both independently repeats the product structure assertions. Native
qualification also tests actual permissions, altered function properties and
disabled Selection triggers. Those controls remain useful and must not be
described as absent.

The integrated assertions include foundation installation rollback and
reinstallation rejection, deliberate payload-trigger mutation controls, and
Selection readiness rejection after replacing a trigger with an inert
`WHEN (false)` trigger. Final coverage review also moved SQL-only zero-version,
zero-sequence, revision-attribution and JSONB NUL rejection controls from the
standalone snapshot-bridge coverage into the structural foundation fixture.
The valid attributed-revision and Unicode-escape controls keep unrelated inputs
valid before their paired rejections.

The remaining standalone snapshot bridge is a separate representation contract:
Rust fixture/codec reconstruction through JSONB, exact read expansion/overflow and
stored/decoded identity/version/coherence checks. Its compiler/process bridge is
not invoked by the structural profiles. Native typed routes and pure codec tests
retain their own evidence; this delivery does not claim automatic qualification
of the entire standalone bridge or broader storage-format compatibility.

The standalone checkers require a fresh cluster as well as a fresh database:
their admission predicates reject existing cluster-global product roles. Native
fixtures currently copy databases inside an initialized cluster. Do not weaken
freshness checks, drop roles, or run standalone installers against those copies.
Use verified owned isolation and bounded cleanup for any added cluster lifecycle.

## Test selection and delivery boundaries

Run focused checks as the corresponding boundary changes. SQL controller tests
include `test_container_native`, the PostgreSQL checker tests and the WorkItem
checker tests where their contracts change. Add meaningful Selection controller
coverage if introducing a new invocation path. Controller passes do not execute
the SQL assertions. Scope-selection tests apply when scope behavior changes.

Do not repeat unrelated service, Windows client or complete browser campaigns
solely because tests were added elsewhere. Existing mandatory commit and CI gates
remain applicable; identify any broader execution imposed by a workflow before
triggering it. Heavy execution needs its actual resource window and owned cleanup.
Complete cheap source/controller checks and harness review before real browser
participation. Retain affected reruns separately from earlier failed attempts.

This delivery does not implement populated upgrades, event delivery, disaster
recovery, production maintenance, release scanners or a universal browser matrix.
Their applicable acceptance and release gates remain. Missing TypeScript consumer
scaffolding is not treated as demonstrated native-wire drift. General API scanning
cannot establish purity or replace authoritative domain and architectural review.

Completion requires implemented outcomes, actual selected evidence, independent
review of the proposed head, related issue/documentation updates and a final PR
summary. Unfinished outcomes remain explicit. No merge or PR status transition is
part of this delivery.

## Component evidence and remaining limits

The documentation checkpoint corrected current boundary references and the two
stale no-UI gap statements. Its 22 engineering-controller tests and required
pure staged gate passed. That gate is not native SQL or browser evidence.

The structural SQL implementation adds two serial, owned networkless clusters to
full native qualification. The foundation profile executes installation rollback,
reinstallation, real-role and payload-trigger mutation controls without compiling
the standalone codec bridge. The Selection profile composes WorkItem checks once,
then exercises Selection storage/authority, independent committed-state observers
and the runtime-profile mutation controls. Focused native-item selectors do not
repeat these profiles.

Final-server readiness uses container-local TCP: the image entrypoint's temporary
initialization server cannot satisfy that check. The container has no attached
network or published ports. Startup calls share the remaining readiness deadline.
Failures retain bounded server diagnostics outside source before owned release;
diagnostic failures do not replace the original failure. Removal requires the
original immutable identity/owner/name and a subsequent absence observation.

The selected real SQL profiles passed on PostgreSQL 16.15 after the startup and
diagnostic corrections. Controller evidence includes seven structural lifetime
cases, existing foundation checks and native orchestration controls. This is
structural SQL evidence, not a rerun of authenticated HTTP/browser qualification.

The pure regression guard uses Cargo/Go metadata and compiler-resolved Clippy
checks rather than a custom Rust parser. It was delivered in a separate focused
checkpoint after the structural SQL changes. The dependency allowlists require review
for additions. Renamed imports do not change the actual dependency name. Library
API checks cover the configured filesystem, environment, clock, process, network
and output paths while allowing in-memory writer traits and formatting. Compiler
controls require both deterministic positive cases and specific lint rejections.
Those controls and the required staged development gate passed after fixing a
test import that had depended on an ambient `PYTHONPATH`. The failed attempt
remains separate from the later pass.

Its limits remain explicit: Rust checks library targets under the active target
and configuration; Go import metadata covers files selected by that toolchain.
Excluded build-tag files, transitive library internals, new unlisted standard
APIs and Go built-in `print`/`println` require review. This bounded guard does not
establish whole-program determinism or replace dependency/semantic review.

Three generated-property tests passed: 768 Unicode identity/result cases with
accepted version mutations and explicit length/UTF-8/discriminant rejection
oracles, 1,024 arbitrary-byte/unknown-JSON smoke cases, and 24,576 generated steps
over one retained operation. These are bounded properties, not a general command
model or full fuzz campaign. The initial arbitrary-byte generator never reached
the binary decoder beyond its header; review corrected that weak coverage with
valid-envelope perturbations. Earlier smoke results are not reclassified as deep
decoder exploration.

The declared compiler groups passed locked, offline, all-target checks: Rust
1.75.0 for the seven non-authority workspace crates and Rust 1.88.0 for
`kehila_authority`. The checks use explicit compiler identities without changing
dependencies, declarations or the lockfile. The first 1.75 attempt failed during
offline dependency resolution; matching-Cargo dependency preparation preceded the
later offline pass. That initial failure was not a source incompatibility result.
These observations bind the selected source/dependency/compiler inputs; changed
inputs require affected qualification rather than a stable-only CI assumption.

## Selected real browser storage component

Chrome 157.0.8103.80 on Windows passed the new file-origin IndexedDB component
fixture using the shipped journal scripts and two test target contexts:

- The first strict readwrite transaction remained active with its application
  read observed; the second was created but its read stayed queued. Explicit
  release occurred before the safety timer expired. The first compare had not
  settled before transaction completion; it then won and the queued compare lost.
- Throwing an injected `QuotaExceededError` at the actual admission callback
  produced a native transaction `abort` event, no `complete` event, and unchanged
  state visible through the other context. This does not establish actual quota
  exhaustion.
- A valid retained handle admitted its original wire before another context
  cleared the journal. The old handle then rejected as stale, and a fresh session
  restored no entry.
- Both synthetic databases and known targets were released. The caller observed
  normal whole-browser Job release, verified the original profile identity, then
  deleted the profile and checked absence. Ordinary deletion is not secure erasure.

The first component pass was retained separately. A stronger assertion requiring
the held first writer specifically to win was added, and the affected fresh-profile
run passed. Four protocol-controller cases cover session/response identity,
out-of-order replies, close, timeout and malformed-response failure behavior.

The fixture, instrumentation, production scripts and archived source identity are
bound in the retained run inputs. The test uses a file origin and synthetic data;
it does not replace prior HTTPS/native/UI evidence, qualify profile eviction,
power loss or actual quota exhaustion, resolve aggregate qualification memory
pressure, or establish a supported browser matrix. See the
[component invocation and limits](../../tests/browser/README.md).

## Delivery review and evidence validity

The original outcome was stronger verification of the current architecture,
without treating future capabilities or absent general automation as demonstrated
P0 defects. The delivered components preserve that scope: selected SQL controls,
bounded dependency/API regression checks, reproducible properties, explicit
compiler qualification, real storage-component cases and corrected current
documentation. Populated upgrades, events, disaster recovery, release scanners,
general browser support and semantic architecture acceptance remain separate.

Source-first review found consequential harness assumptions: cluster-global roles
require fresh clusters; whole-byte generation did not reach the binary decoder;
unchanged storage alone did not prove abort; concurrent requests alone did not
prove contention; and a stale-handle rejection needed a valid admission control.
The corresponding admission, reachability, event and control oracles were corrected
and exercised. This shows those particular corrections, not a measured general
review effectiveness or detection rate.

Initial gate configuration/image admission, Go package selection, test-import and
offline dependency-resolution failures retain their original stages and outcomes.
Later passes do not convert them into successful first attempts or establish
unobserved causes. No product integrity failure was inferred from those harness
and environment failures. The stronger first-writer browser assertion has its own
affected rerun rather than being attributed to the earlier pass.

Evidence reuse is conditional on relevant byte identities: SQL source/checkers
and isolation policy; pure manifests, configured APIs and compiler controls;
codec/replay code and seeded inputs; compiler, locked dependencies and selected
crate sets; browser fixture/instrumentation/client and production journal scripts.
Changes to those contracts need affected revalidation. Prose/register closeout
does not reattribute the original runtime observations to a later executed head.
The final PR identifies the composed head and its required checks/review.

Build, test, cleanup and controller spans are retained separately where observed.
Preparation, review and waiting overlapped or were not measured as distinct labor;
there is no defensible total engineering duration or claimed time/memory saving.
Observed owned cluster, Job and profile release is bounded cleanup evidence,
not secure erasure or disaster recovery. Wider capacity and operational claims
retain their existing decisions, owners and qualification gates.

The final review must assess the exact proposed head and these evidence limits.
Related issues remain open for their broader obligations; completion of this
verification increment does not close those parents or authorize merging or a
PR status transition.

## Review corrections and selected revalidation

The review follow-up addresses bounded weaknesses in the verification mechanisms,
not a demonstrated production integrity failure. The original observations above
remain historical evidence for their original source inputs. Changed mechanisms
require the selected checks below before a new completion claim.

| Correction | Selected checks | Evidence affected |
| --- | --- | --- |
| Mixed-output seeded generator | Fixed SplitMix64 vectors and bounded low-byte/category controls; Selection codec properties and retained-operation properties | Earlier generated-input runs do not qualify the changed generator |
| Replay precedence | Valid full-record and tombstone cases combining revoked authorization with changed requests, paired with authorized rejection controls | Existing timestamp, fingerprint and retirement tests remain relevant |
| Recursive pure inventory and target admission | Nested workspace/missing-member controls; reject build-script, binary and proc-macro targets before compilation; allow ordinary library/test targets | Run actual Cargo inventory and compiler rejection controls after source checks |
| Known external APIs and implicit hash seeding | DirBuilder, Unix-socket and randomized-collection compiler controls; relevant task-contract behavioral tests | Membership sets use deterministic storage; output vectors retain their traversal order; no general semantic purity proof |
| Compiler wrappers and input admission | Reject invalid homes/ambient overrides before launching qualification; assert wrapper-neutralized commands; exact declared-compiler qualification | Earlier compiler observations retain their original environment; they do not exercise the revised checker |
| SQL harness rejection branches | Controller cases for in-source diagnostics and each readiness-deadline location, preserving original failure and owned release | SQL and schema sources are unchanged; no unrelated full native campaign selected |
| Browser protocol and cleanup | Oversized/non-string/error-response controls; surviving-context fallback and exhaustion controls; affected real-browser component run after harness review | Original storage observations remain source-bound; revised cleanup needs its own evidence |
| Current standard wording | Regenerate the standard and run register synchronization/controller checks | Experimental local-owner authentication/UI remains distinct from wider release qualification |

The focused Python/Node controllers and source checks precede compiler and browser
qualification. Required commit/CI gates remain applicable and may execute broader
mechanical/unit checks; their scope is reported separately. No new automatic MSRV
or browser CI service, blanket mutation campaign, full Cartesian replay model,
SQL schema change, release gate or PR status transition is part of this follow-up.

### Correction evidence and retained limits

The focused container run passed the updated pure inventory/API guard and its
12 actual compiler controls, 159 task-contract tests, four Selection property
tests, formatting and strict all-target Clippy for the affected crates. The shared
generator controls execute in both property consumers. These finite checks do not
establish statistical randomness, complete parser exploration or whole-program
purity. The deterministic membership collections do not enumerate into domain
outputs; existing source traversal still determines those vectors.

Twenty focused Python controllers and 22 engineering-document controllers passed.
The new rejection controls were also challenged in isolated copies: removing
nested inventory, executable-target admission, diagnostic/source separation,
readiness deadlines, toolchain/source separation or wrapper command overrides
caused the selected tests to fail. Initial readiness/home experiments failed from
inadequate mocks; their corrected affected reruns use explicit boundary assertions.
The original failed observations are retained, not counted as clean first attempts
or as a general mutation score.

Rust 1.75.0 and 1.88.0 passed the revised locked, offline, all-target declaration
groups. A configured rejecting wrapper was first reached by an intentional
unneutralized Cargo command: it failed with the expected wrapper exit and marker.
Both subsequent neutralized groups passed without invoking either configured
wrapper. Compiler installation and matching-Cargo dependency preparation occurred
separately in the disposable environment; no image, lockfile or declaration changed.
A preparation failure occurred before compiler execution and was retained
separately from compiler results.

Thirteen Node protocol/cleanup controllers passed, including the actual fixture's
cleanup before initialization. Chrome 157.0.8103.80 on Windows passed the normal
storage component and the selected context-loss case: the first target was
explicitly closed, an uninitialized surviving context deleted both known databases,
and an independent database inventory confirmed absence. Known targets, whole
browser Jobs and the disposable profile were released and absence checked.

The initial context-loss attempt failed despite a successful normal component.
A diagnostic repeat captured Chrome's root-level detached-session routing error,
which omits the session identity. Applying the successful-reply session check to
that routing error closed the shared client and prevented fallback. The correction rejects only that pending
request for the observed unscoped routing error; successful replies, explicitly
foreign sessions and other unscoped errors retain strict identity checks. New
positive/negative controls and the affected fresh-profile run passed. Earlier
failures and their owned cleanup remain separate evidence.

The proportional closeout finding is specific: mocked evaluation alone did not
exercise the browser's detached-session error envelope. The actual fault case and
retained protocol packets now cover that interaction. This is not a measured
general review effectiveness claim or a reason for a universal browser campaign.
Compiler, test and cleanup observations are bounded spans, not total labor or
demonstrated savings. The file-origin, injected-quota and wider release limitations
above remain. Required staged gates and final hosted scope are reported by the PR;
their successes do not turn skipped native execution into qualification.

### Follow-up lifecycle controls and constructor coverage

The bounded follow-up finalizes the optional hold on native transaction
termination and handles early WebSocket connection closure. Focused fake-timer,
transaction and socket controls cover completion, abort, unsuppressed request
failure, unrelated-transaction ownership, genuine safety expiry, open/error/close/
deadline setup paths and listener transfer to the connected protocol client.
The test runner includes these controls; they do not launch a browser or service.

An isolated Rust 1.99.1 / Clippy 0.1.99 matrix established the constructor gap:
`from`, `default` and `collect` passed for both default-hasher HashSet and HashMap
under the earlier guard. Existing `new` rejections and explicit deterministic
hasher controls behaved as expected. A separate candidate with both collection
types prohibited rejected all six alternatives and both deterministic-hasher
controls; BTreeSet and BTreeMap controls passed.

The maintainer explicitly approved this stricter pure-library collection policy,
including rejection of explicit deterministic custom hashers. It is recorded in
the [coding standard](../engineering/CODING-STANDARD.md#rust) and implemented by
two type restrictions. Deterministic hash collections are not inherently impure;
the accepted restriction trades that flexibility for a simple conservative guard.
No custom compiler plugin, generic trait ban or collection rewrite was introduced.

The compiler controls now include both collections' `new`, `from`, `default`,
`collect` and explicit deterministic-hasher alias paths, paired with BTree
default/from/collect positive controls. All twelve program inputs match the
isolated candidate observations. Together with existing non-collection cases,
the guard has 23 compiler controls. These remain bounded active-target checks,
not a proof of whole-program purity or arbitrary inferred/transitive type use.

The initial seven intended lifecycle-controller failures are retained separately;
the corrected 22 protocol/cleanup/instrumentation controllers and 22 engineering
controllers passed. The three code commits passed their required staged gates,
including the final guard's 23 compiler controls. Final-head review and hosted
checks are reported separately in the PR.
Existing real-browser and compiler results above retain their original source
identities and are not reattributed to these changes. No unrelated native SQL,
full browser campaign, declared-compiler campaign or new CI service is selected
for this follow-up.

The closeout lesson is bounded: constructor-method restrictions left confirmed
implicit-seeding paths open. The isolated compiler matrix separated that defect
from the policy choice to exclude deterministic hashers as well. The maintainer
accepted the stricter policy explicitly; passing experiments alone did not make
that decision. Lifecycle controls exercise timer and listener ownership with
fakes and do not establish revised live-browser qualification. No general
detection coverage, execution-time saving or process effectiveness is inferred.
