# Architecture verification follow-up

Status: implementation planned; no new qualification is established by this record.

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
