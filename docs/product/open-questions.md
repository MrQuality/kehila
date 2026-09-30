# YAJA open questions and implementation blockers

Baseline: 2026-09-23. These product and technical choices remain unresolved. Each question links to the earliest affected backlog items so independent work can proceed.

Backlog IDs link to the [implementation backlog](backlog.md). Proposals below are explicitly optional until accepted. Confirmed requirements remain in force while details are investigated.

<a id="q-001"></a>
## Q-001 — Technical path for the first usable increment

**Resolved first-increment choice:** [D-016](decisions.md#d-016) adopts the reference task path, resolves the contradictory `CDC_PIPELINE_STALLED` response in favor of an acknowledged 200 with pending search visibility, and accepts operation IDs, expected versions, distinct conflicts, and explicit replication provisioning. A null synchronization token does not diagnose indexer health. The 30-second experiment deadline is not a product delivery guarantee.

**Known:** The design includes separated write/read paths, independent durable change capture, shared query compilation, single-document mutations, and asynchronous multi-document workflows. The repository lacks a usable API/UI and the full pipeline. No architecture replacement has been approved.

**Remaining design:** [D-017](decisions.md#d-017) sets a provisional 90-day post-commit replay period, separate from unseen-ID admission and physical retention. Prove safe compaction, current-task preservation, concurrent next-version writes, non-regressing projection, index rebuild during writes, and practical project lists before real data. Product history needs a separate retention policy. Validate mapping/provisioning drift and measure `REPLICA IDENTITY FULL` WAL cost. Broader schema lifecycle, deletion, WAL retention, and recovery remain open. A broker handshake is not evidence of durable delivery or database correctness.

**Evidence available:** [SP-001-R04](../spikes/SP-001-task-path.md) passed bounded
save/delivery/replay/conflict cases with Python boundary adapters and the reference
services. The product direction is accepted; Go/Rust implementation correctness and
long-lived operation retention are not established by the experiment.

**Blocks:** [B-002](backlog.md#b-002), then [B-005](backlog.md#b-005)/[B-006](backlog.md#b-006). Does not block documenting domain rules.

<a id="q-002"></a>
## Q-002 — Local runtime and access boundary

**Resolved first-increment choice:** [D-016](decisions.md#d-016) limits browser-facing access to host loopback. LAN support is deferred. Keep database, broker, and search on internal container networks absent a specific administrative need.

**Resolved first-supported direction:** One authenticated local owner and Windows/Podman with manual startup. Define onboarding, session expiry, access recovery, protection for reads/configuration, browser CSRF/origin policy, and authenticated CLI behavior before support. The launcher needs start, stop, status, actionable readiness failures, and non-destructive repeatable upgrades. Automatic startup and LAN access are deferred. Broader runtime support remains unverified; Kubernetes is not currently required. See [D-017](decisions.md#d-017).

**Evidence available:** [SP-001-R04](../spikes/SP-001-task-path.md) ran locally on
Windows/Podman with loopback ports and 3.125 GiB of container ceilings. This does
not establish LAN access, authentication, Linux CI, or supported deployment policy.

**Blocks:** [B-002](backlog.md#b-002), [B-005](backlog.md#b-005), [B-007](backlog.md#b-007), [B-018](backlog.md#b-018).

<a id="q-003"></a>
## Q-003 — Resource units and availability calendars

**Question:** Are resources exclusive, divisible, or concurrently usable up to a quantity? How are recurring availability, exceptions, time zones, holidays, maintenance, and unavailable periods represented? Can resource types be defined by users? What are the permitted units beyond hours?

**Known:** Resources are heterogeneous, time-bound, and shared across projects. Resource identity/eligibility and time windows must survive release.

**Blocks:** [B-008](backlog.md#b-008), [B-009](backlog.md#b-009), [B-015](backlog.md#b-015).

<a id="q-004"></a>
## Q-004 — Project limits, sprint limits, and protected capacity

**Question:** What does project-defined capacity mean per resource type: an upper planning limit, a budget, an entitlement, or a combination? How does sprint capacity constrain it? Are limits hard or advisory? Are protected project allocations needed in the first release?

**Known:** Shared availability is the default. Project/sprint limits are separate from physical availability. Protected quotas remain a proposal. The current release model uses shared availability.

**Blocks:** Capacity portions of [B-009](backlog.md#b-009), [B-014](backlog.md#b-014), [B-015](backlog.md#b-015). Basic resource identity can proceed without this answer.

<a id="q-005"></a>
## Q-005 — Meaning of days and time presentation

**Question:** Does “days” mean conversion to working days, totals grouped by calendar date, elapsed calendar days, or multiple clearly labeled views? If converting, what determines working-day length for each resource? What precision and rounding apply?

**Known:** Hours/days visibility and separation of work time from elapsed duration are required. Working-day length remains undefined.

**Blocks:** Time display in [B-010](backlog.md#b-010), date calculations in [B-015](backlog.md#b-015).

<a id="q-006"></a>
## Q-006 — Task estimate versus resource demand

**Question:** Is an hour-based task estimate independent of resource requirements, derived from driving resources, or something else? What should happen when they disagree? How does a point-estimated task acquire resource demand for scheduling?

**Accepted direction:** Projects select hours or points, and YAJA uses hours. A task estimate is an independent planning quantity; resource demand is recorded separately. There is no implicit points-to-hours conversion or aggregation of heterogeneous resource-hours into a task estimate. See [D-018](decisions.md#d-018).

**Further resolution, 2026-09-29:** [D-019](decisions.md#d-019) selects exact nonnegative decimals, distinguishes zero from absence, and prohibits M1 project-unit changes after the first recorded estimate. [D-021](decisions.md#d-021) fixes M1 precision and bounds. **Remaining:** Detailed resource-demand input for scheduling. See C-08 and the remaining specification work in the [B-003 contract](B-003-contract.md).

**Blocks:** Estimate semantics in [B-003](backlog.md#b-003)/[B-004](backlog.md#b-004), scheduling in [B-015](backlog.md#b-015). Status/phase logic can proceed independently.

<a id="q-007"></a>
## Q-007 — Sprint membership and boundary behavior

**Question:** How long are sprints? Can they overlap? Can tasks or reservations span sprint boundaries, and does that need permission? What happens to incomplete work at sprint end? Can a task belong to more than one sprint? How should sprint and Gantt edits interact?

**Known:** Both views are required. Released capacity retains dates and is not automatically assigned to a later sprint. The example of a reservation spanning a sprint illustrated release behavior; it did not settle permission to create such reservations.

**Proposed:** Use the same task records in both planning views; no duplicate task copies.

**Blocks:** [B-014](backlog.md#b-014), [B-016](backlog.md#b-016), relevant [B-015](backlog.md#b-015) rules.

<a id="q-008"></a>
## Q-008 — Task fields and workflow administration

**Question:** What minimum fields identify a task and milestone? How is the current task selected? Can multiple tasks be active? Which transitions are allowed? How do cancellation, Done → New, deletion of an in-use status, and remapping a status to another phase behave?

**Known:** New/Active/Done are system phases; phase derives from status. Done → Active has explicit reopening rules. A complete hierarchy, transition-permission matrix, or cancellation state was not agreed.

**Proposed M1 baseline, pending contract review:** Project identity, name, estimation unit, and configurable statuses; task identity, project, title, description, optional estimate, status reference, and version; structured knowledge entries and follow-up origin links; explicit current-work selection independent of Active. Use stable status IDs; permit renaming but initially block deletion of referenced statuses and changes to their phase mapping. Specify reopening and other transitions; leave unresolved transitions unavailable. Finalize this model before a durable record format is committed. M1 includes knowledge and follow-ups; the first release still includes sprint, Gantt, resource, and scheduling commitments.

**Blocks:** [B-003](backlog.md#b-003), [B-004](backlog.md#b-004), parts of [B-011](backlog.md#b-011).

**Contract review started, 2026-09-29:** [D-018](decisions.md#d-018) accepts
configuration revision consistency, complete destination validation for
conversion, mandatory preservation of knowledge/provenance/lifecycle history,
archival reference protections, and system phase enforcement across migration
and conversion. Done to New is prohibited. The earlier M1 baseline above remains
historical proposal context, not a finalized record format. The
[B-003 contract review](B-003-contract.md) records review areas C-01 through C-10.
**Further resolution, 2026-09-29:** [D-019](decisions.md#d-019) accepts all ten
resolutions, retaining the full configurable model in M1 and splitting its
implementation. Scenarios BC-01 through BC-20 specify acceptance. Q-008 remains
open for exact value/identity limits, project archival details, field-definition
evolution, complete typed commands/errors, and contract checks; the accepted
review decisions are not pending reconfirmation.

**Further resolution, 2026-09-30:** [D-020](decisions.md#d-020) fixes project
archival and restoration effects, preserves issued readable IDs across prefix
changes, and rejects in-place value-kind changes for populated fields. The
[contract review](B-003-contract.md) adds BC-21 through BC-23. Q-008 remains
open for complete typed commands and remaining administration/checks.
**Further resolution, 2026-09-30:** [D-021](decisions.md#d-021) fixes the
project-scoped readable-ID lookup policy and M1 value limits.
**Further resolution, 2026-09-30:** [D-022](decisions.md#d-022) fixes the full
M1 phase graph and relationship-type ownership. Q-008 remains open for the
remaining typed command and representation details in B-003.
**Contract progress, 2026-09-30:** [B-003](B-003-contract.md) now requires a
complete item-value and historical-field-use snapshot for configuration edits.
The pure rule checks required-field changes and rejects kind reinterpretation
or removal after historical use; B-005 must enforce snapshot completeness and
commit-time consistency.
**Further resolution, 2026-09-30:** [D-023](decisions.md#d-023) fixes the M1
item create/edit/conversion field payload limit at 128 entries and 1 MiB of
supplied text. Other command and transport limits remain open.

<a id="q-009"></a>
## Q-009 — Scheduling demand, windows, and multiple drivers

**Question:** How does a task specify when it needs each resource? Can demand be split across intervals? Which resources require overlap or uninterrupted availability? How do multiple driving demands determine dates and which reservation moves are allowed?

**Known:** Consumption is independent, multiple driver types are permitted, and non-drivers can constrain feasibility. These rules do not specify time-window relationships. An internal task-phase sequence is undefined, and resource-hours do not directly determine elapsed duration.

**Next evidence:** Validate a concrete resource-window representation against the examples in S-03 and S-06, then specify scheduling rules. The model must express resource timing independently of task lifecycle phases.

**Blocks:** [B-009](backlog.md#b-009), [B-015](backlog.md#b-015).

<a id="q-010"></a>
## Q-010 — Requirements, reservations, and revisions

**Question:** At what planning action does a requirement become a firm dated reservation? Who may revise required resources or remaining demand? Which plan revisions are retained? Are unscheduled tasks allowed? What happens if a named resource is replaced?

**Known:** The scheduler must honor requirements; original plan and actuals remain separate; revising remaining demand is allowed. “Locked” does not establish an unchangeable plan. Reservations after reopening require review.

**Blocks:** [B-009](backlog.md#b-009), [B-011](backlog.md#b-011), [B-012](backlog.md#b-012).

<a id="q-011"></a>
## Q-011 — Dependency semantics

**Question:** Which dependency types are supported initially? How are lag, cycles, completed predecessors, and reopened predecessors treated? Can dependencies cross projects?

**Known:** Prerequisites matter and fixed-date conflicts must be explained. Follow-up origin links and scheduling dependencies are separate.

**Blocks:** [B-013](backlog.md#b-013), [B-015](backlog.md#b-015).

<a id="q-012"></a>
## Q-012 — Cross-project contention and concurrent reservations

**Question:** How are competing requests prioritized? Can a project's automatic recalculation move another project's existing reservations, and under whose permission? What counts as allowed overlap for a divisible resource? How are simultaneous reservation requests resolved consistently?

**Known:** Reopening cannot reclaim another task's booking. Physical availability must be checked across projects. No project-priority or general preemption policy has been agreed.

**Proposed engineering criterion:** Never confirm incompatible reservations because competing requests were checked against stale availability. Investigate a concurrency design consistent with the architecture.

**Blocks:** [B-009](backlog.md#b-009), [B-011](backlog.md#b-011), [B-015](backlog.md#b-015).

<a id="q-013"></a>
## Q-013 — Recalculation triggers and mode switches

**Question:** Which changes trigger automatic recalculation: reported usage, revised demand, availability edits, dependency changes, rate edits, or other events? Is recalculation previewed or immediately applied? How does switching manual/automatic mode affect existing dates and overrides? How are users told about changes?

**Known:** Driving resource demand influences scheduling; other required resources constrain it; fixed dates persist. Cost changes alone were not agreed as a reason to move tasks.

**Blocks:** [B-015](backlog.md#b-015), [B-016](backlog.md#b-016).

<a id="q-014"></a>
## Q-014 — Permissions and override scope

**Question:** What identities, project roles, and grants exist? Who can manage statuses, shared resources, rates, usage, and scheduling exceptions? Who may view costs? Which item dates can be fixed and how are overrides removed? Are task-specific scheduling-driver overrides needed?

**Known:** Project administrators define statuses, and scheduling overrides require permission, including in local deployments. Task-specific driver overrides remain an open proposal.

**Further resolution, 2026-09-30:** [D-022](decisions.md#d-022) fixes link
creation grants: type use in its owning project and Link on each endpoint item.
Type administration requires project configuration permission in the owner
project. B-007 still defines grant assignment, revocation, and enforcement;
the other permissions in this question remain open.

**Blocks:** [B-007](backlog.md#b-007), permissions in [B-008](backlog.md#b-008)/[B-012](backlog.md#b-012)/[B-016](backlog.md#b-016).

<a id="q-015"></a>
## Q-015 — Currency, applicable rates, and cost recognition

**Question:** Single or multiple currencies? What selects the applicable rate for a reservation, usage, or refund? How are spans across rate changes handled? When does reserved-time cost become “actual”? How do forecasts count existing non-refundable commitments without double-counting remaining work? What precision and rounding apply?

**Known:** Effective-dated rates, original cost estimates, two cost bases, and two release policies are required. Historical actuals must retain their recorded values. Resource cost is not a complete financial accounting system or external billing integration.

**Blocks:** [B-012](backlog.md#b-012), cost summaries in [B-017](backlog.md#b-017).

<a id="q-016"></a>
## Q-016 — Knowledge and file-reference structure

**Question:** Separate entry types or a structured task note? Are created files stored as repository paths, commit links, attachments, or another reference? Who can edit entries and is revision history needed? Can a follow-up belong to another project?

**Known:** All five knowledge categories in R-005 are required and manual entry is sufficient. File upload, automatic scanning, and commit integration are not implied.

**Blocks:** Knowledge implementation in [B-004](backlog.md#b-004).

<a id="q-017"></a>
## Q-017 — Usage corrections, overrun, and timers

**Question:** How are running timers handled at completion/restart? Can they overlap? How are late, backdated, duplicate, or corrected reports handled? What happens when usage exceeds allocation or is reported after completion? How should remaining demand behave at zero while a task stays Active?

**Known:** Manual entries and timers are required, actuals are independent per resource, and estimate exhaustion is not completion. Overrun must remain visible without triggering completion.

**Blocks:** [B-010](backlog.md#b-010), [B-011](backlog.md#b-011), relevant accounting in [B-012](backlog.md#b-012).

<a id="q-018"></a>
## Q-018 — Automatic resource-consumption reporting

**Question:** Required in the first release or later? Which source reports which resources, with what identity, reconciliation, and duplicate handling? What does project maturity configure?

**Known:** Automatic reporting was described as a possible usage source. Accepting manual knowledge records did not settle this. Timers are confirmed independently.

**Blocks:** Automatic reporting scope in [B-010](backlog.md#b-010), final scope acceptance in [B-018](backlog.md#b-018). Does not block manual usage and timer implementation once their own questions are resolved.

<a id="q-019"></a>
## Q-019 — Consistency across completion, reservations, and costs

**Question:** How will a task transition, multiple releases, cost adjustments, and cross-project availability stay consistent under retries, competing requests, or process failure?

**Known:** The v0.2 design forbids multi-document transactions and proposes asynchronous sagas. Completion/reopening spans multiple kinds of records. Simply updating them one after another without a recovery design would not establish the agreed behavior.

**Next evidence:** Specify stable operation identities, observable intermediate/failure states, and recovery behavior; verify them against actual services. These are engineering proposals, not prescribed database tables or a chosen algorithm.

**Evidence boundary:** [SP-001's design handoff](../../experiments/SP-001/README.md#design-handoff)
identifies prerequisites for reservation/cost ledgers and compensating actions.
Its passing single-task cases do not settle cross-document consistency.

**Blocks:** Architecture disposition in [B-002](backlog.md#b-002), integrated [B-011](backlog.md#b-011)/[B-012](backlog.md#b-012).

<a id="q-020"></a>
## Q-020 — Local durability and recovery objectives

**Question:** What data-loss tolerance, backup destination/frequency, restore workflow, and upgrade/migration behavior should be supported? What resource use and startup time are acceptable for a supported local installation?

**Accepted direction:** No acknowledged-save loss through process crash, container recreation, or ordinary machine restart. Automatic daily backups must show age and failure status. Test clean-instance restore of configuration and replay records, then reconstruct search. Machine-loss recovery requires an off-machine copy. Test stale browser versions and retry IDs against restored data. Daily backups do not guarantee a 24-hour recovery point when the machine sleeps or backup delivery fails. Exact restore/data-loss targets, destination, migration procedure, and operating budget remain open; see [D-017](decisions.md#d-017).

**Blocks:** [B-005](backlog.md#b-005), [B-018](backlog.md#b-018). Disposable-data development can proceed without pretending to meet these gates.

<a id="q-021"></a>
## Q-021 — Cost policies beyond the two accepted choices

**Question:** Are partial refunds, cancellation windows, minimum charges, taxes, non-resource expenses, or external billing needed, and when?

**Known:** None is specified. The first defined behavior is consumed/reserved basis with non-refundable or fully refundable future-release policy. Do not expand implementation to additional pricing rules without a decision.

**Blocks:** Nothing in the accepted simple-policy implementation; reassess only if scope expands.

<a id="q-022"></a>
## Q-022 — Delivery commitment and remaining release scope

**Question:** What development capacity is available, what defines a usable milestone, and which unresolved capabilities must ship in the first release? What measurable scale or performance targets apply to that release?

**Known:** Both planning views and the confirmed resource/scheduling/cost behaviors remain in scope. Target dates, development capacity, sprint length, expected task count, and release performance targets remain undefined. The architecture's “millions of tasks” objective is not a measured first-release acceptance target.

**Blocks:** Date/effort commitments and final [B-018](backlog.md#b-018) sign-off. It does not prevent bounded implementation of agreed behavior.
