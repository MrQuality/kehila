# YAJA implementation backlog

Baseline: 2026-09-23. This proposed delivery plan links to the [requirements](requirements.md) and [decisions](decisions.md). Backlog IDs are planning references; linked GitHub issues track execution. Dates and effort estimates remain unassigned.

## How to use this backlog

Each item links to requirements, dependencies, unresolved choices, and acceptance criteria. Engineering criteria are proposed reliability checks. Resolve questions when they affect an item; other work can proceed.

Statuses used here:

- **Documented:** the specified planning artifact has been written; this does not claim product implementation.
- **Ready to investigate:** useful evidence can be gathered now.
- **Needs decisions:** a product choice must be resolved for all or part of the item.
- **Waiting on dependencies:** implementation depends on earlier capabilities.

Backlog IDs are stable references, not strict execution order. The technical path must account for the compiler, schema, API, storage, event, and recovery work in [implementation status](../IMPLEMENTATION.md).

## Proposed milestones and order

These M0–M4 labels are **product delivery milestones**, not the independent
[engineering maturity gates](../engineering/ENGINEERING-STANDARD.md). Existing
M1-01–M1-08 child IDs retain their product meaning. Engineering Preview/Alpha/Beta
qualification requires its own cumulative evidence and does not reduce product scope.

| Milestone | Items / order | Observable outcome |
| --- | --- | --- |
| M0 — Document and establish the path | B-001, B-002; resolve the blocking subset of B-003/B-007 | A traceable baseline and an evidence-backed implementation plan. |
| M1 — Use YAJA to record its own work | B-003 and B-007; B-005 and B-004; B-006, divided into M1-01–M1-08 | Create projects and work items with the full configurable model, including fields, types, workflows, conversion, migration, relationships, administration, estimates, and knowledge; retrieve saved records locally. |
| M2 — Track shared resources and execution | B-008, B-009, B-010; integrate B-011 and B-012 | Reserve shared resources, report actuals, compare plans, complete/reopen safely, and inspect costs. |
| M3 — Plan across dates and sprints | B-013 and B-014; B-015, B-016, B-017 | Both sprint and Gantt planning with manual/automatic scheduling, multiple drivers, authorized overrides, and cost/time summaries. |
| M4 — Validate the full first release | B-018 | Demonstrated end-to-end behavior, local recovery, and an explicit record of remaining limitations. |

Within a milestone, independent work may proceed once its own blockers are resolved. Milestones divide the work while preserving the full release scope. Resource-backed lifecycle automation arrives after M1. M2's completion/cost behavior is accepted together once both B-011 and B-012 integrate. [D-019](decisions.md#d-019) retains the full configurable model in M1; the [implementation slices](#m1-configurable-model) divide that work without reducing scope.

<a id="m1-configurable-model"></a>
## M1 configurable-model implementation slices

**Scope accepted 2026-09-29; implementation pending.** These child IDs
divide existing backlog responsibilities and link to GitHub issues.
B-003 defines the shared contracts and pure rules. Each slice includes
applicable API, storage, access, and interface integration through its parents.

| Child ID | Parent / dependencies | Deliverable and acceptance |
| --- | --- | --- |
| [M1-01](https://github.com/MrQuality/yaja/issues/26) | B-005; B-003 | Configuration revisions, history, reference integrity, and coordinated command acceptance. Track and protect historical status, status-group assignment/grant, workflow, type, field, and relationship-type use even after current items migrate away. Demonstrate configuration/item contention, replay after configuration changes, and recovery without partial accepted state. Use actor/family/target-scoped operation lookup, freeze versioned exact-request fingerprints, and demonstrate 90-day replay, gap-free compaction to permanent tombstones, and tombstone backup/restore. Establish a consistency protocol for archive and current-selection clearing under the single-document storage constraint; test crash, retry, concurrent selection, and recovery behavior before exposing archival routes. |
| [M1-02](https://github.com/MrQuality/yaja/issues/27) | B-004/B-006; M1-01, B-007 | Project and WorkItem identity, Task/Milestone, estimates, replayable current selection, archival/restoration. Integrate the accepted trusted revision-one Task/Milestone seed and replayable project creation command. Verify untitled-item display, zero versus absent estimates, unit locking, selection retry after uncertain response, and selection clearing under the accepted project archival contract. |
| [M1-03](https://github.com/MrQuality/yaja/issues/28) | B-004/B-006; M1-02 | Project-defined types, application/custom fields, hidden/optional/required modes, and five initial value kinds. Protect application-defined fields from project removal, archival, and retyping; specify trusted upgrade of built-in definitions. Verify hidden-value preservation/write rejection, required-field changes under contention, and safe definition evolution. |
| [M1-04](https://github.com/MrQuality/yaja/issues/29) | B-004/B-006; M1-03 | Multiple workflows, statuses, defaults, permitted workflows per type, phase restrictions, and configuration archival. Verify phase derivation, initial/default replacements, reference preservation, and zero implicit usage. |
| [M1-05](https://github.com/MrQuality/yaja/issues/30) | B-004/B-006; M1-04 | Explicit type conversion and workflow migration. Verify complete destination validation, both-workflow phase restrictions, current original migration grants on replay, retained archived references, source-snapshot validation, preserved source values/history, and no partial conversion. |
| [M1-06](https://github.com/MrQuality/yaja/issues/31) | B-004/B-006; M1-02, B-003 | Relationship types and links, knowledge, and follow-up provenance under the resolved Q-016 contract. Verify cross-project authorization, canonical duplicate prevention, self-link rejection, inverse display, archival preservation, and history surviving conversion. Final integration includes M1-05. |
| [M1-07](https://github.com/MrQuality/yaja/issues/32) | B-007/B-006; B-003, Q-014 | Configuration permissions and delegated administration, including status-group administration. Implement the accepted group-scoped status grants and project administration boundary; provision initial project access, define revocation, and verify authoritative allow/deny behavior across all slices. Initial access enforcement is required before dependent routes are exposed. |
| [M1-08](https://github.com/MrQuality/yaja/issues/33) | B-006 with B-004/B-005/B-007; M1-01–M1-07 | Integrate the full configurable model into the local interface. Demonstrate administration, edits, conversion, migration, knowledge, relationships, reload, conflict recovery, and archival. Retain B-005 durability and authenticated-access gates before real data. |

M1-07's initial access boundary accompanies the early slices; its delegated
administration surface expands with them. Q-016 is resolved for M1; Q-014 grant
assignment and revocation remain B-007 work. Resource reservations, costs, and
scheduling retain their later milestone placement. Their lifecycle effects
must not be claimed by M1 checks.

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

**Status:** Accepted typed policies and pure checks are implemented on the B-003
branch; ready for final maintainer review.
Initialization, delegated status administration, and conversion review policies
are specified, including group lifecycle in [D-031](decisions.md#d-031).
Operation identity, replay authorization, and expiry have typed enforcement in
[D-032](decisions.md#d-032) through [D-034](decisions.md#d-034).
[D-030](decisions.md#d-030) confirms that active
and archived definitions both consume configuration capacity. B-004/B-005/B-006/B-007 implement and
verify the behavior. [D-018](decisions.md#d-018)
records the accepted review direction; [D-019](decisions.md#d-019) resolves the
ten review areas and retains the full M1 scope. [D-020](decisions.md#d-020) and
[D-021](decisions.md#d-021) settle project archival, readable IDs, and M1 value
limits. [D-022](decisions.md#d-022) settles the phase graph, relationship-type
ownership, and link-creation grants. [D-023](decisions.md#d-023) bounds item
field payloads; [D-024](decisions.md#d-024) settles knowledge and follow-up
provenance. The [contract review](B-003-contract.md) records accepted logical
rules, including [D-025](decisions.md#d-025) through [D-029](decisions.md#d-029)
for conversion, choice feasibility, delegated grants, and project initialization,
alongside
limits, typed commands, pure-check evidence, and downstream consistency gates.
Durable records and the configurable worker are not implemented here.
**Dependencies:** B-001; align persistence contracts with B-002.

**Traceability:** [R-001](requirements.md#r-001), [R-002](requirements.md#r-002), [R-006](requirements.md#r-006), [R-007](requirements.md#r-007). **Questions:** [Q-006](open-questions.md#q-006), [Q-008](open-questions.md#q-008).

**Deliverable:** Project/task identity, minimum fields, estimation configuration, status-to-phase mapping, and explicit transition rules.

**Acceptance:** Multiple statuses can map to one system phase; a task cannot independently contradict its status's phase. The YAJA project can use hours and another project can select points without converting points to hours. Record what selects the current task. Invalid references and in-use status edits follow a decided policy. Ordinary status changes record no usage. Add pure rule checks for S-01's phase behavior.

<a id="b-004"></a>
## B-004 — Implement project/task operations and manual knowledge

**Status:** Knowledge and workflow contracts are specified; implementation waits on B-005 storage and B-007 access work. **Dependencies:** B-003, B-005; access checks from B-007.

**Traceability:** [R-001](requirements.md#r-001), [R-002](requirements.md#r-002), [R-005](requirements.md#r-005)–[R-007](requirements.md#r-007). **Questions:** [Q-008](open-questions.md#q-008), [Q-016](open-questions.md#q-016).

**Deliverable:** Create/retrieve/update projects and tasks, configure statuses, record estimates and all required knowledge categories, and link originating/follow-up tasks.

**Acceptance:** Save and retrieve a created-file reference, decision, lesson, insight, and follow-up origin link. Preserve them through completion/reopening. Do not create a dependency merely by linking a follow-up. Reject invalid status references. Resource effects are integrated later and not claimed by these operations alone. Validate real persistence for the routes provided.

<a id="b-005"></a>
## B-005 — Build durable local storage and the required delivery path

**Status:** First task-path direction accepted; implementation and recovery decisions remain. **Dependencies:** B-002, B-003; coordinate access boundaries with B-007.

**Traceability:** Supports [R-001](requirements.md#r-001), [R-005](requirements.md#r-005), [R-012](requirements.md#r-012); engineering prerequisites rather than new confirmed product semantics. **Questions:** [Q-001](open-questions.md#q-001), [Q-002](open-questions.md#q-002), [Q-019](open-questions.md#q-019), [Q-020](open-questions.md#q-020).

**Deliverable:** Versioned storage representation, durable local configuration, the required command/read/update integrations, and basic backup/restore instructions.

**Proposed engineering acceptance:** Demonstrate actual task writes and reads and no acknowledged-save loss through process crash, application/container recreation, and ordinary machine restart. Test clean-instance restore of configuration and replay records followed by search reconstruction; report automatic daily backup age and failures, and keep an off-machine copy before claiming machine-loss recovery. Check stale browser versions and retry IDs after restoring an older backup. Prove relevant delivery acknowledgments/replay where used; existing connection probes are insufficient. For immutable operation records, prove concurrent next-version exclusion, non-regressing search, safe compaction of inactive tasks, index rebuild during writes, and practical project lists as history grows. Distinguish authoritative data from rebuildable projections. Use disposable test records until durability is verified. Record schema migration, backup format, and an operating budget for task count, memory, disk, startup, and save latency. See [D-017](decisions.md#d-017).

Before item or project archival is exposed, specify the physical representation
and commit/recovery protocol for [BC-31](B-003-contract.md#acceptance-scenarios).
Show that readers never observe an accepted archive with a stale current-work
selection, or an accepted selection clear with an uncommitted archive. Exercise
crashes between physical steps, racing selection changes, duplicate retries,
and restore from a backup. The chosen protocol must respect the reference
design's ban on multi-document database transactions.

<a id="b-006"></a>
## B-006 — Deliver the first local project/task interface

**Status:** Waiting on dependencies. **Dependencies:** B-004, B-005, B-007.

**Traceability:** [R-001](requirements.md#r-001), [R-002](requirements.md#r-002), [R-005](requirements.md#r-005)–[R-007](requirements.md#r-007). **Scenario:** [S-10](requirements.md#s-10), first-increment portion only.

**Deliverable:** A local browser interface to create projects, configure statuses, identify current work, create/update tasks, and record knowledge and follow-ups.

**Acceptance:** Create and update records against real storage, then reload and retrieve them. Distinguish pending, saved, and failed changes. Derive phase from status. Identify resource and scheduling capabilities that remain unavailable, and record usability findings.

<a id="b-007"></a>
## B-007 — Establish identity and permission boundaries

**Status:** One authenticated local owner accepted; onboarding, session expiry/recovery, browser CSRF, CLI access, and enforcement remain to design and implement. **Dependencies:** B-002, B-003.

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

## Engineering maturity initiatives

These grouped initiatives extend existing items rather than duplicate their
implementation. Engineering requirement IDs, exact first gates and evidence gaps
are maintained in the [engineering register](../engineering/ENGINEERING-STANDARD.md).
All are owned by the project maintainer until delegated. No release or security
compliance is implied by a documented planning artifact.

<a id="b-019"></a>
### B-019 — Engineering governance and verification baseline

**Status:** Documented framework and implemented structural checks; qualification pending.
**Dependencies:** B-001/B-002 and contribution policy.
**Traceability:** GOV-001/GOV-002, ARC-001, TEST-001, DOC-001, CODE-001.
**Deliverable:** Standards/register, ADR boundary, Definition of Done, shared
metadata validation and useful language checks.
The [coding standard](../engineering/CODING-STANDARD.md) applies to new/materially
changed code and inventories current gaps. Qualify boundary checks, Python static
tooling and declared Rust compiler support incrementally; use B-022 for TypeScript/
Go contract work and B-023 for failure-diagnostic retention. Do not require an
unrelated legacy-code sweep to complete a scoped change.
**Acceptance:** Regression tests reject invalid IDs/links/gaps; final full CI
evidence is linked; remote protection/required checks and bypass restrictions are
inspected; maintainer records final PR assessment. Preserve sole-contributor policy.

<a id="b-020"></a>
### B-020 — Security baseline and incident qualification

**Status:** Needs decisions and implementation. **Dependencies:** B-007, B-021/B-022.
**Traceability:** SEC-001–SEC-005, INC-001; Q-014.
**Deliverable:** Reviewed threat model, integrated identity/session/project grants,
SAST/secret scans and exact versioned ASVS L1/OSPS L1 Alpha matrices; L2 Production
assessment and supported-release incident workflow later.
**Acceptance:** Negative auth/IDOR/cross-project/replay tests; real seeded scanner
detection, actionable triage, secure deployment and expiring exceptions; enabled
private reporting and exercised patch/advisory/disclosure/postmortem workflow.
**Tooling gap:** Qualify CodeQL/equivalent and secret scanning against polyglot
source, hosted availability and maintainable fixtures before adding CI jobs.

<a id="b-021"></a>
### B-021 — Dependency and repository supply-chain controls

**Status:** Ready to investigate. **Dependencies:** B-019, B-020 for triage.
**Traceability:** DEP-001, SEC-004/SEC-005, SUPPLY-001/SUPPLY-004.
**Deliverable:** Direct/transitive/image/tool inventory, license/notices review,
advisory scanning/update cadence and owned expiring exceptions; read-only scheduled
Scorecard where practical.
**Acceptance:** Validate cargo audit/govulncheck (and future JS dependency) coverage,
pin scanner/tool inputs, distinguish advisory/network failure from clean findings,
and demonstrate detection/expiry. Qualify GitHub dependency review availability
and avoid redundant scanners. Assess immutable release action/image/toolchain pins
with deliberate update procedure. No silent permanent vulnerability ignores.

<a id="b-022"></a>
### B-022 — API and event contract baseline

**Status:** Waiting on dependencies. **Dependencies:** B-003/B-004/B-005/B-007.
**Traceability:** API-001/API-002, EVENT-001, COMPAT-001, LIMIT-001; D-016/D-032–D-034.
**Deliverable:** OpenAPI specification and cross-language conformance; stable
errors, auth/grants, pagination/filter/sort, replay/conflict, limits/versioning;
event schema/metadata, future-schema/poison/gap handling and compatibility policy.
**Acceptance:** Positive/negative/old-client tests; unchanged-ID retry versus
changed-intent distinction; monotonic projection/client versions; rebuild and
quarantine/replay evidence. Preserve experimental error semantics without claiming
they are already a supported API. Actual route integration remains B-004/B-005.

<a id="b-023"></a>
### B-023 — Integrity, concurrency, properties and fault qualification

**Status:** Waiting on dependencies; bounded current regressions retained.
**Dependencies:** B-005/B-022; Q-019 for cross-record protocols.
**Traceability:** DATA-001, EVENT-001, TEST-002–TEST-004, REL-002.
**Deliverable:** Seeded properties/parser fuzz corpus, deterministic/model and
sustained concurrency campaigns, critical-path E2E and hypothesis-driven fault
matrix from QUALIFICATION.md.
**Acceptance:** Detect duplicate effects, stale acceptance/projection, acknowledged
loss and partial cross-record acceptance under explicit interruption barriers;
test malformed/duplicate/reordered/delayed events and dependency/resource failures.
Use real boundaries, bounded tests and no retry-until-pass. Finite tests remain
evidence, not linearizability proof. Pure B-003 rules must gain storage evidence.

<a id="b-024"></a>
### B-024 — Durable installation, backup, restore and migrations

**Status:** Needs decisions. **Dependencies:** B-005, Q-019/Q-020, B-023.
**Traceability:** PERSIST-001, REC-001–REC-003, MIG-001, OPS-002; D-017/D-034.
**Deliverable:** Supported restart survival, automated visible daily backup and
off-failure-domain copy, clean-instance restore including configuration/replay/
tombstones, search rebuild and supported upgrade/migration qualification.
**Acceptance:** Independent restored integrity checks, stale-client/retry-ID
reconciliation, failure/sleep/destination cases, realistic-volume interrupted
migrations and timed approved RPO/RTO/DR exercises. Index setup alone is not a
migration framework; backup existence alone is not recovery evidence.

<a id="b-025"></a>
### B-025 — Performance envelope and resource qualification

**Status:** Needs decisions and measurement. **Dependencies:** B-005/B-022, Q-022.
**Traceability:** PERF-001/PERF-002, LIMIT-001.
**Deliverable:** Repeatable Go/Rust workload baseline and later published envelope,
including history growth, fields, clients, throughput, rebuild and startup.
**Acceptance:** p50/p95/p99, errors, saturation, CPU/memory/disk and propagation
measurements with fixtures/runtime/hardware; maximum/over-limit tests. Values are
measured, not inferred from prototype ceilings or enterprise-scale aspirations.

<a id="b-026"></a>
### B-026 — Telemetry, SLIs/SLOs and reliability assessment

**Status:** Needs decisions and implementation. **Dependencies:** B-022/B-025, Q-020/Q-022.
**Traceability:** OBS-001, REL-001/REL-002.
**Deliverable:** OTel-compatible logs/metrics/trace context, first-class projection
lag, redaction/cardinality policy, operational queries and approved SLI/SLO targets.
**Acceptance:** Propagation/metric tests, dependency/freshness alerts, declared
windows/populations and achieved-window Production evidence. No final targets
are invented here. Link recovery targets to B-024.

<a id="b-027"></a>
### B-027 — Operability, configuration and UI qualification

**Status:** Waiting on dependencies. **Dependencies:** B-005/B-006/B-007/B-024/B-026.
**Traceability:** OPS-001/OPS-002, MIG-001, ACCESS-001.
**Deliverable:** Safe validated configuration/secret schema, capability-specific
health/degradation, tested runbooks and later WCAG 2.2 AA web UI evidence.
**Acceptance:** Invalid/unsupported configuration rejection, failure-matrix tests,
operator-run install/upgrade/restore/repair/rotation/crash exercises; manual and
automated accessibility checks once real UI exists. A config-validation CLI is
a design candidate, not an obligation to implement a speculative command now.

<a id="b-028"></a>
### B-028 — Supported release and artifact pipeline

**Status:** Waiting on dependencies. **Dependencies:** B-018/B-020/B-021/B-024/B-026.
**Traceability:** COMPAT-001/COMPAT-002, SUPPLY-001–SUPPLY-003, INC-001.
**Deliverable:** Immutable controlled hosted build, supported matrix, changelog/
deprecation, artifact tests/checksums/SBOM/provenance/signing/packaging and consumer
verification; SLSA 1.2 Build L2 assessment by Beta.
**Acceptance:** Artifact-bound SBOM and authenticated provenance verification,
tampering rejection, signing/rotation/revocation procedure, supported upgrade/API/
event compatibility tests and final commit-specific cumulative gate assessment.
No publication is required by this planning task; Build L3 remains later hardening.

<a id="b-029"></a>
### B-029 — Enterprise scope and qualification candidates

**Status:** Needs decisions; later target outside current implementation scope.
**Dependencies:** Production qualification and explicit customer/product decisions.
**Traceability:** ENT-001.
**Deliverable:** Justified applicability for OIDC, SAML/SSO, MFA, RBAC/ABAC, audit,
HA/scaling, multi-node recovery, stronger backups, hardening, external assessment
and compliance evidence; isolation only if multi-tenancy is actually adopted.
**Acceptance:** Customer-relevant threat/operating models and independent security,
failure/recovery/isolation evidence. No speculative enterprise features or formal
certification claims are introduced by the standard.
