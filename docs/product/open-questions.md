# YAJA open questions and implementation blockers

Baseline: 2026-09-23. These product and technical choices remain unresolved. Each question links to the earliest affected backlog items so independent work can proceed.

Backlog IDs link to the [implementation backlog](backlog.md). Proposals below are explicitly optional until accepted. Confirmed requirements remain in force while details are investigated.

<a id="q-001"></a>
## Q-001 — Technical path for the first usable increment

**Question:** Which parts of the v0.2 design are required in the first increment, and are any concrete changes needed? Validate the selected database mapping and command-to-save-to-read/update path. The design's `CDC_PIPELINE_STALLED` response is contradictory (503 versus 200 with a null token). Resolve the applicable contract before implementing that route.

**Known:** The design includes separated write/read paths, independent durable change capture, shared query compilation, single-document mutations, and asynchronous multi-document workflows. The repository lacks a usable API/UI and the full pipeline. No architecture replacement has been approved.

**Next evidence:** Inventory contracts and run targeted real-service experiments. Present any proposed deviation with impact and alternatives. A broker handshake is not evidence of durable delivery or database correctness.

**Evidence available:** [SP-001-R04](../spikes/SP-001-task-path.md) passed bounded
save/delivery/replay/conflict cases with Python boundary adapters and the reference
services. Assess its proposed 200/pending save response and explicit replication
provisioning before adopting them for the Go/Rust implementation.

**Blocks:** [B-002](backlog.md#b-002), then [B-005](backlog.md#b-005)/[B-006](backlog.md#b-006). Does not block documenting domain rules.

<a id="q-002"></a>
## Q-002 — Local runtime and access boundary

**Question:** Browser access only on the host, or also from other devices? Is Kubernetes needed for the supported deployment model? Which supported local runtime, authentication boundary, and startup behavior should be delivered?

**Known:** Local operation is required and Compose is provided today. The supported application runtime and network-access boundary remain undecided; Kubernetes is not currently required.

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

**Known:** Projects select hours or points, and YAJA uses hours. The relationship between task estimates and resource demand remains unresolved. There is no defined points-to-hours conversion.

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

**Blocks:** [B-003](backlog.md#b-003), [B-004](backlog.md#b-004), parts of [B-011](backlog.md#b-011).

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

**Known:** Current development storage is documented as ephemeral. Keeping real project records requires a deliberate durability design. Persistence and a tested restore are proposed release checks; exact targets remain open.

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
