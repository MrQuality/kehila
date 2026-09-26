# YAJA implementation backlog

Baseline: 2026-09-23. This proposed delivery plan links to the [requirements](requirements.md) and [decisions](decisions.md). Items are planning records, not existing GitHub issues. Dates and effort estimates remain unassigned.

## How to use this backlog

Each item links to requirements, dependencies, unresolved choices, and acceptance criteria. Engineering criteria are proposed reliability checks. Resolve questions when they affect an item; other work can proceed.

Statuses used here:

- **Documented:** the specified planning artifact has been written; this does not claim product implementation.
- **Ready to investigate:** useful evidence can be gathered now.
- **Needs decisions:** a product choice must be resolved for all or part of the item.
- **Waiting on dependencies:** implementation depends on earlier capabilities.

Backlog IDs are stable references, not strict execution order. The technical path must account for the compiler, schema, API, storage, event, and recovery work in [implementation status](../IMPLEMENTATION.md).

## Proposed milestones and order

| Milestone | Items / order | Observable outcome |
| --- | --- | --- |
| M0 — Document and establish the path | B-001, B-002; resolve the blocking subset of B-003/B-007 | A traceable baseline and an evidence-backed implementation plan. |
| M1 — Use YAJA to record its own work | B-003 and B-007; B-005 and B-004; B-006 | Create a project and tasks, configure phase-mapped statuses, record estimates and knowledge, and retrieve saved records locally. |
| M2 — Track shared resources and execution | B-008, B-009, B-010; integrate B-011 and B-012 | Reserve shared resources, report actuals, compare plans, complete/reopen safely, and inspect costs. |
| M3 — Plan across dates and sprints | B-013 and B-014; B-015, B-016, B-017 | Both sprint and Gantt planning with manual/automatic scheduling, multiple drivers, authorized overrides, and cost/time summaries. |
| M4 — Validate the full first release | B-018 | Demonstrated end-to-end behavior, local recovery, and an explicit record of remaining limitations. |

Within a milestone, independent work may proceed once its own blockers are resolved. Milestones divide the work while preserving the full release scope. Resource-backed lifecycle automation arrives after M1. M2's completion/cost behavior is accepted together once both B-011 and B-012 integrate.

<a id="b-001"></a>
## B-001 — Establish the product planning baseline

**Status:** Documented. **Dependencies:** None.

**Traceability:** All requirements and decisions; [D-014](decisions.md#d-014).

**Deliverable:** Requirements, decisions, open questions, backlog, a reading guide, and links from existing documentation.

**Acceptance:** Product scope, terminology, requirements, and unresolved choices are documented. Every requirement maps to backlog items, examples are marked illustrative, and links resolve. Implementation status is recorded separately.

<a id="b-002"></a>
## B-002 — Investigate and record the first-increment technical path

**Status:** Bounded investigation accepted for the first local task path; implementation remains in B-005 and related items. **Dependencies:** B-001.

**Spike record:** [SP-001](../spikes/SP-001-task-path.md), using the approved [spike procedure and template](../spikes/README.md). Keep experiment cases, run summaries, conclusions, and the next-session handoff there; acceptance criteria remain here.

**Latest evidence:** SP-001-R04 passed the six bounded cases and the full repository
suite on Windows/Podman using experimental Python adapters. The concrete design
and reproduction runner are in [the experiment](../../experiments/SP-001/README.md).
The 3.125 GiB total container ceilings fit the observed workstation. Explicit
PostgreSQL replica identity and task-only publication were required. [D-016](decisions.md#d-016)
records the accepted bounded product direction. These results do not establish
the future Go/Rust application's correctness or resource use.

**Traceability:** [R-001](requirements.md#r-001), [D-015](decisions.md#d-015). **Questions:** [Q-001](open-questions.md#q-001), [Q-002](open-questions.md#q-002), [Q-019](open-questions.md#q-019).

**Deliverable:** A concrete design for one create/update/read task path using the applicable architecture, including its error and recovery contracts. Investigate the actual database and event connections needed; keep experiments bounded to the proposed increment.

**Required preflight before execution:** Inspect and record the current host/CI resources and prerequisites before starting containers or tests. Check available memory, CPU, free disk space, and capacity for the planned containers, including CDC; installed versions and availability of Podman, its Compose provider, WSL/Linux runtime, Python, Git, Rust, and Go; required service ports, container image access, and OpenSearch's `vm.max_map_count` setting. Confirm the Podman machine and host-to-container connectivity are usable. Record any missing software, resource shortfall, or environmental blocker and resolve it before running the affected experiment. Repeat this preflight on each machine or CI environment used for reproducible results.

**Experiment setup:** Use Podman for this increment. Run the application and supporting services in containers; run test commands from the host or CI runner. Start with the existing PostgreSQL/FerretDB, NATS, and OpenSearch mapping rather than a database-engine comparison. Add the missing CDC service and any API/worker containers required by the selected task path. Pin service versions and make the experiments repeatable with automated setup, readiness checks, known test data, assertions, failure/restart cases, diagnostics, and teardown. The current CI job uses Docker, so its results do not establish that the Podman path works; provide a Podman-based repeatable run for this increment.

**Bounded cases and assumptions:** Check task create/update/read mapping, durable event delivery, eventual search visibility, duplicate/retried commands, and recovery when a worker or CDC/indexing component stops after a successful database write. Distinguish database acknowledgment, event publication, durable replay, and search visibility in the results. Treat the existing stack as the starting hypothesis, not a proven end-to-end path; document evidence and impact before proposing a different engine or architecture. Decide the stalled-pipeline API response before asserting it in a test.

**Local readiness note (recheck before running):** SP-001-R04 ran the expanded
stack on the 15.71 GiB Windows workstation using a dedicated Podman WSL machine.
Its containers have 3.125 GiB of enforced memory ceilings; setup requires at least
4224 MiB host available RAM after VM startup. The cases observed at least 2691 MiB
host available RAM and no container OOM flags. These are bounded prototype
observations, not production minimums. The experiment and its VM are now stopped.
Recheck resources, provider behavior, image access, connectivity, and OpenSearch
settings for every rerun. Diagnose observed interference before changing security
settings.

**Acceptance:** Record a passing preflight before each environment's experiment run; record blockers and defer affected experiments until they are resolved. Identify the roles of browser, API, business logic, storage, event delivery, and search/update delivery. Resolve the contradictory stalled-pipeline response before implementing the affected route. Record observed integration evidence and limitations. Document proposed architecture changes and their impact. Identify prerequisites for consistent future resource releases and cost adjustments. Kubernetes is outside the current release requirements.

<a id="b-003"></a>
## B-003 — Specify project/task contracts and workflow rules

**Status:** Needs decisions for fields/estimates; phase rules are ready to specify. **Dependencies:** B-001; align persistence contracts with B-002.

**Traceability:** [R-001](requirements.md#r-001), [R-002](requirements.md#r-002), [R-006](requirements.md#r-006), [R-007](requirements.md#r-007). **Questions:** [Q-006](open-questions.md#q-006), [Q-008](open-questions.md#q-008).

**Deliverable:** Project/task identity, minimum fields, estimation configuration, status-to-phase mapping, and explicit transition rules.

**Acceptance:** Multiple statuses can map to one system phase; a task cannot independently contradict its status's phase. The YAJA project can use hours and another project can select points without converting points to hours. Record what selects the current task. Invalid references and in-use status edits follow a decided policy. Ordinary status changes record no usage. Add pure rule checks for S-01's phase behavior.

<a id="b-004"></a>
## B-004 — Implement project/task operations and manual knowledge

**Status:** Waiting on dependencies and knowledge-format decisions. **Dependencies:** B-003, B-005; access checks from B-007.

**Traceability:** [R-001](requirements.md#r-001), [R-002](requirements.md#r-002), [R-005](requirements.md#r-005)–[R-007](requirements.md#r-007). **Questions:** [Q-008](open-questions.md#q-008), [Q-016](open-questions.md#q-016).

**Deliverable:** Create/retrieve/update projects and tasks, configure statuses, record estimates and all required knowledge categories, and link originating/follow-up tasks.

**Acceptance:** Save and retrieve a created-file reference, decision, lesson, insight, and follow-up origin link. Preserve them through completion/reopening. Do not create a dependency merely by linking a follow-up. Reject invalid status references. Resource effects are integrated later and not claimed by these operations alone. Validate real persistence for the routes provided.

<a id="b-005"></a>
## B-005 — Build durable local storage and the required delivery path

**Status:** First task-path direction accepted; implementation and recovery decisions remain. **Dependencies:** B-002, B-003; coordinate access boundaries with B-007.

**Traceability:** Supports [R-001](requirements.md#r-001), [R-005](requirements.md#r-005), [R-012](requirements.md#r-012); engineering prerequisites rather than new confirmed product semantics. **Questions:** [Q-001](open-questions.md#q-001), [Q-002](open-questions.md#q-002), [Q-020](open-questions.md#q-020).

**Deliverable:** Versioned storage representation, durable local configuration, the required command/read/update integrations, and basic backup/restore instructions.

**Proposed engineering acceptance:** Demonstrate actual task writes and reads, persistence through application restart and container recreation, and recovery appropriate to the agreed design. Prove relevant delivery acknowledgments/replay where used; existing connection probes are insufficient. Distinguish authoritative data from rebuildable projections. Use disposable test records until durability is verified. Record schema migration and backup format choices.

<a id="b-006"></a>
## B-006 — Deliver the first local project/task interface

**Status:** Waiting on dependencies. **Dependencies:** B-004, B-005, B-007.

**Traceability:** [R-001](requirements.md#r-001), [R-002](requirements.md#r-002), [R-005](requirements.md#r-005)–[R-007](requirements.md#r-007). **Scenario:** [S-10](requirements.md#s-10), first-increment portion only.

**Deliverable:** A local browser interface to create projects, configure statuses, identify current work, create/update tasks, and record knowledge and follow-ups.

**Acceptance:** Create and update records against real storage, then reload and retrieve them. Distinguish pending, saved, and failed changes. Derive phase from status. Identify resource and scheduling capabilities that remain unavailable, and record usability findings.

<a id="b-007"></a>
## B-007 — Establish identity and permission boundaries

**Status:** Needs decisions. **Dependencies:** B-002, B-003.

**Traceability:** [R-006](requirements.md#r-006), [R-021](requirements.md#r-021). **Questions:** [Q-002](open-questions.md#q-002), [Q-014](open-questions.md#q-014).

**Deliverable:** The agreed local identity/access model and enforceable permissions for configured operations, extended as resource and cost features arrive.

**Acceptance:** Project configuration changes and scheduling overrides enforce permissions on the authoritative side. Verify allowed and denied cases. Resource and rate visibility follow the access policy, including in local deployments.

<a id="b-008"></a>
## B-008 — Add shared named resources and calendars

**Status:** Needs decisions. **Dependencies:** B-005, B-007.

**Traceability:** [R-009](requirements.md#r-009), [R-010](requirements.md#r-010), [R-015](requirements.md#r-015). **Questions:** [Q-003](open-questions.md#q-003), [Q-014](open-questions.md#q-014).

**Deliverable:** Shared resources, resource types, availability windows, and calendar quantities using the agreed units.

**Acceptance:** Two projects reference the same named person or server and see the same relevant availability. Human, machine, and room capacity remain distinct. Availability exceptions and time boundaries follow the chosen calendar policy. Historical windows are not carried forward as new capacity. Calendar edits preserve necessary history.

<a id="b-009"></a>
## B-009 — Add requirements, reservations, and capacity checks

**Status:** Needs decisions and dependencies. **Dependencies:** B-004, B-008; consistency approach from B-002.

**Traceability:** [R-009](requirements.md#r-009)–[R-012](requirements.md#r-012), [R-015](requirements.md#r-015), [R-016](requirements.md#r-016). **Questions:** [Q-004](open-questions.md#q-004), [Q-009](open-questions.md#q-009), [Q-010](open-questions.md#q-010), [Q-012](open-questions.md#q-012).

**Deliverable:** Named resource requirements, dated reservations, preserved planning baseline, and project capacity checks; extend sprint checks with B-014.

**Acceptance:** Preserve original requirements while showing current reservations. Respect resource identity and dates on release. Reject or resolve incompatible simultaneous bookings according to the decided policy. Scheduling preserves required demand and distinguishes resource-hours from elapsed duration. Verify contention against actual storage/services, not only isolated calculations.

<a id="b-010"></a>
## B-010 — Report consumption and revise remaining work

**Status:** Needs decisions and dependencies. **Dependencies:** B-009.

**Traceability:** [R-003](requirements.md#r-003), [R-012](requirements.md#r-012)–[R-014](requirements.md#r-014), [R-019](requirements.md#r-019). **Questions:** [Q-005](open-questions.md#q-005), [Q-017](open-questions.md#q-017), [Q-018](open-questions.md#q-018).

**Deliverable:** Manual usage entries, timer workflow, resource-specific consumption, remaining-demand revision, and time displays.

**Acceptance:** Demonstrate both entry methods. Execute S-02 and S-03's independent accounting. Preserve original plans and actuals when remaining demand changes. No automatic completion on estimate exhaustion. Handle timer interruption, duplicate submissions, and corrections according to agreed rules. Decide automatic-reporting release scope explicitly; keep that decision separate from manual knowledge scope. Add an integration-specific child item if an automatic source is approved.

<a id="b-011"></a>
## B-011 — Integrate completion and reopening with resource release

**Status:** Waiting on dependencies and edge-case decisions. **Dependencies:** B-009, B-010; integrate release accounting with B-012 before accepting the complete feature.

**Traceability:** [R-007](requirements.md#r-007), [R-012](requirements.md#r-012), [R-015](requirements.md#r-015)–[R-018](requirements.md#r-018). **Questions:** [Q-008](open-questions.md#q-008), [Q-010](open-questions.md#q-010), [Q-012](open-questions.md#q-012), [Q-017](open-questions.md#q-017), [Q-019](open-questions.md#q-019).

**Deliverable:** Done transition release, historical retention, and Done → Active resource-plan review.

**Acceptance:** Execute S-03, S-04, and S-05. Reopening allows Active state but creates no new reservation before review. Another project's booking remains intact. Preserve charges/refunds and invoke the configured release policy. Proposed engineering checks must cover retry/failure recovery without duplicate release or refund. Until automatic scheduling exists, expose a manual reallocation workflow without claiming automatic capability.

<a id="b-012"></a>
## B-012 — Implement resource rates and cost accounting

**Status:** Needs accounting decisions; can develop alongside B-011 after shared contracts. **Dependencies:** B-009, B-010; shared transition contract with B-011, no requirement to finish B-011 first.

**Traceability:** [R-018](requirements.md#r-018), [R-025](requirements.md#r-025)–[R-028](requirements.md#r-028). **Questions:** [Q-014](open-questions.md#q-014), [Q-015](open-questions.md#q-015), [Q-019](open-questions.md#q-019). Q-021 is outside defined simple-policy scope unless expanded.

**Deliverable:** Effective-dated rates, cost-basis and release-policy configuration, immutable historical meaning, and task/project totals.

**Acceptance:** Execute S-08 and S-09 under the agreed recognition/currency rules. Compare planned, actual, remaining forecast, and variance. Reserved-time charges must not also be counted as consumed-time charges for the same commitment. Future release can free availability with or without reducing cost. Reopening records new costs separately and preserves old adjustments. Retry checks prevent duplicate financial adjustments.

<a id="b-013"></a>
## B-013 — Add dependencies and fixed milestones

**Status:** Needs dependency decisions. **Dependencies:** B-004, B-007.

**Traceability:** [R-008](requirements.md#r-008), [R-021](requirements.md#r-021), [R-022](requirements.md#r-022). **Questions:** [Q-011](open-questions.md#q-011), [Q-014](open-questions.md#q-014).

**Deliverable:** Explicit prerequisite links and permitted fixed-date constraints.

**Acceptance:** Keep origin links distinct from dependencies. Validate dependency structure according to decided rules. Demonstrate authorized/unauthorized date overrides and S-07's conflict explanation. Preserve fixed dates when forecasts slip.

<a id="b-014"></a>
## B-014 — Implement sprint planning and capacity limits

**Status:** Needs sprint/capacity decisions. **Dependencies:** B-009, B-010; coordinate dependency behavior with B-013.

**Traceability:** [R-002](requirements.md#r-002), [R-004](requirements.md#r-004), [R-010](requirements.md#r-010), [R-016](requirements.md#r-016). **Questions:** [Q-004](open-questions.md#q-004), [Q-006](open-questions.md#q-006), [Q-007](open-questions.md#q-007).

**Deliverable:** Sprint definition, task assignment, estimates/actuals, and per-sprint planning capacity alongside shared resource availability.

**Acceptance:** Sprint totals use the configured units and preserve estimation units. Remaining sprint planning capacity does not imply available resources. Show usable slack only within valid dates. Cross-boundary tasks and unfinished work follow the decided policy. Support hour-estimated YAJA and a separately point-estimated example project.

<a id="b-015"></a>
## B-015 — Implement resource-constrained scheduling

**Status:** Needs scheduling rules and dependencies. **Dependencies:** B-009–B-011, B-013, B-014.

**Traceability:** [R-008](requirements.md#r-008), [R-010](requirements.md#r-010), [R-011](requirements.md#r-011), [R-013](requirements.md#r-013), [R-015](requirements.md#r-015), [R-016](requirements.md#r-016), [R-020](requirements.md#r-020)–[R-024](requirements.md#r-024). **Questions:** [Q-003](open-questions.md#q-003)–[Q-007](open-questions.md#q-007) where applicable, [Q-009](open-questions.md#q-009), [Q-011](open-questions.md#q-011)–[Q-013](open-questions.md#q-013).

**Deliverable:** Manual conflict evaluation and automatic date calculation using multiple project-selected driving types, default Human, with named resource demands and all required availability constraints.

**Acceptance:** Demonstrate multiple drivers, non-driver blockage, revised remaining demand, fixed-date infeasibility, and shared resource contention. Dates follow explicit window/calendar rules. Respect review before reopened-task reservations. Preserve resource identity and time on release. Existing reservations and capacity limits constrain recalculation. Verify deterministic outcomes for the selected priority rules and actual concurrent reservation safety separately.

<a id="b-016"></a>
## B-016 — Deliver Gantt interaction and scheduling controls

**Status:** Waiting on dependencies. **Dependencies:** B-007, B-013–B-015.

**Traceability:** [R-004](requirements.md#r-004), [R-008](requirements.md#r-008), [R-020](requirements.md#r-020)–[R-024](requirements.md#r-024). **Questions:** [Q-007](open-questions.md#q-007), [Q-013](open-questions.md#q-013), [Q-014](open-questions.md#q-014).

**Deliverable:** Gantt dates/dependencies, project mode controls, item overrides, and clear conflict presentation linked with sprint planning.

**Acceptance:** Demonstrate both modes in the YAJA project and mixed automatic/fixed items. Changes appear consistently across the agreed shared task model. Show the cause of infeasibility and permitted corrective actions. Switching modes follows explicit rules. Permission denial is enforced beyond hiding controls in the interface.

<a id="b-017"></a>
## B-017 — Provide effort, capacity, and cost summaries

**Status:** Waiting on dependencies. **Dependencies:** B-010, B-012, B-014, B-015.

**Traceability:** [R-002](requirements.md#r-002), [R-003](requirements.md#r-003), [R-010](requirements.md#r-010), [R-012](requirements.md#r-012), [R-015](requirements.md#r-015), [R-025](requirements.md#r-025)–[R-028](requirements.md#r-028). **Questions:** [Q-005](open-questions.md#q-005), [Q-006](open-questions.md#q-006), [Q-015](open-questions.md#q-015).

**Deliverable:** Task/project/sprint views of estimates and actual time, remaining demand, usable future slack, historical unused time, and resource cost forecasts.

**Acceptance:** Label effort versus elapsed time; distinguish physical availability from planning limits; show cost according to basis and policy. Reconcile aggregates with source records. Preserve baselines and do not double-count resources shared across projects. Never present past unused hours as spendable future slack.

<a id="b-018"></a>
## B-018 — Verify, document, and use the complete local release

**Status:** Waiting on release scope and implementation. **Dependencies:** B-006 and B-008–B-017; resolved applicable release questions.

**Traceability:** R-001–R-028; [S-01](requirements.md#s-01)–[S-10](requirements.md#s-10). **Questions:** [Q-018](open-questions.md#q-018), [Q-020](open-questions.md#q-020), [Q-022](open-questions.md#q-022), plus any unresolved blocker inherited from earlier items.

**Deliverable:** A usable local release, setup and recovery documentation, and observed behavior and limitations.

**Acceptance:** Exercise both planning views, both scheduling modes, permission-controlled overrides, multiple drivers, shared resources, manual/timer usage, completion/reopening, and cost policies. Proposed reliability checks cover persistence through application/container and machine restarts, backup restoration into a clean instance, and retry/crash recovery for resource and cost operations. Record test results and usability findings. Performance and scale claims require measurements.

## Requirement-to-backlog traceability

Every confirmed requirement has an implementation destination. Multiple destinations indicate integration responsibility, not duplicate implementation.

| Requirement | Primary backlog items | Acceptance references |
| --- | --- | --- |
| [R-001](requirements.md#r-001) Local first use | B-002, B-004–B-006, B-018 | S-10 |
| [R-002](requirements.md#r-002) Project estimates | B-003, B-004, B-014, B-017 | S-02, S-10 |
| [R-003](requirements.md#r-003) Actual time | B-010, B-017 | S-02, S-10 |
| [R-004](requirements.md#r-004) Sprint and Gantt | B-014, B-016, B-018 | S-10 |
| [R-005](requirements.md#r-005) Knowledge | B-004, B-006 | S-01, S-10 |
| [R-006](requirements.md#r-006) Phase/status | B-003, B-004, B-007 | S-01 |
| [R-007](requirements.md#r-007) State versus usage | B-003, B-004, B-011 | S-01, S-03 |
| [R-008](requirements.md#r-008) Prerequisites | B-013, B-015, B-016 | S-07 |
| [R-009](requirements.md#r-009) Shared resources | B-008, B-009, B-011 | S-05 |
| [R-010](requirements.md#r-010) Capacity constraints | B-008, B-009, B-014, B-015, B-017 | S-04, S-06 |
| [R-011](requirements.md#r-011) Planning requirements | B-009, B-015 | S-06 |
| [R-012](requirements.md#r-012) Separate records | B-009–B-011, B-017 | S-02, S-09 |
| [R-013](requirements.md#r-013) Remaining demand | B-010, B-015 | S-02 |
| [R-014](requirements.md#r-014) Independent consumption | B-010, B-015 | S-03, S-06 |
| [R-015](requirements.md#r-015) Temporal limits | B-008, B-009, B-011, B-015, B-017 | S-04 |
| [R-016](requirements.md#r-016) Shared release | B-009, B-011, B-014, B-015 | S-04, S-05 |
| [R-017](requirements.md#r-017) Completion | B-011, B-012 | S-03, S-04 |
| [R-018](requirements.md#r-018) Reopening | B-011, B-012, B-015 | S-05, S-09 |
| [R-019](requirements.md#r-019) Reporting maturity | B-010, B-018 | Reporting-source scope decision in Q-018 |
| [R-020](requirements.md#r-020) Scheduling modes | B-015, B-016 | S-05, S-10 |
| [R-021](requirements.md#r-021) Authorized overrides | B-007, B-013, B-016 | S-07 |
| [R-022](requirements.md#r-022) Infeasible fixed date | B-013, B-015, B-016 | S-07 |
| [R-023](requirements.md#r-023) Multiple drivers | B-015, B-016 | S-06 |
| [R-024](requirements.md#r-024) Required non-drivers | B-015, B-016 | S-06 |
| [R-025](requirements.md#r-025) Historical rates | B-012, B-017 | S-08, S-09 |
| [R-026](requirements.md#r-026) Cost basis | B-012 | S-08 |
| [R-027](requirements.md#r-027) Release policy | B-011, B-012 | S-08 |
| [R-028](requirements.md#r-028) Cost summaries | B-012, B-017 | S-08, S-09 |

## Definition of a completed implementation item

The referenced behavior works through the interfaces the item promises; applicable product questions are resolved; meaningful acceptance and failure cases are verified; actual-service boundaries have actual-service evidence; documentation reflects delivered limits; and normal contribution/review requirements are met. A UI-only mockup, a passing connection probe, or a written acceptance scenario is not sufficient evidence for the complete feature.

Record scope changes as proposals and update the affected requirements and backlog items after review.
