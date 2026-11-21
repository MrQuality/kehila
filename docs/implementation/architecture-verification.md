# Architecture verification follow-up

Status: implementation in progress. Component evidence below is bounded; final
composition review and delivery qualification remain unfinished.

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

Candidate additional assertions include foundation installation rollback and
reinstallation rejection, deliberate payload-trigger mutation controls, and
Selection readiness rejection after replacing a trigger with an inert
`WHEN (false)` trigger. Complete the assertion comparison before choosing the
integrated set; filename counts do not establish coverage.

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

The separately prepared pure regression guard uses Cargo/Go metadata and compiler-resolved Clippy
checks rather than a custom Rust parser. It is not included in the structural SQL
checkpoint. The dependency allowlists require review
for additions. Renamed imports do not change the actual dependency name. Library
API checks cover the configured filesystem, environment, clock, process, network
and output paths while allowing in-memory writer traits and formatting. Compiler
controls require both deterministic positive cases and specific lint rejections.

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

Actual-browser additions, final composition review and the final PR summary
remain unfinished.
