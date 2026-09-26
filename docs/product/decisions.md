# YAJA product decision record

Baseline: 2026-09-23. This record explains the choices behind the [requirements](requirements.md). Status labels refer to the product planning baseline; delivery proposals and unresolved technical choices are identified separately.

<a id="d-001"></a>
## D-001 — Start with local project management

**Status: Confirmed.** Local project and task management is the initial use case, including tracking YAJA development. It covers current work, task knowledge, dependencies, effort, and dates. Broader deployment requirements remain open.

**Consequence:** Evaluate the first increment using a complete local task-management workflow. Access from other devices remains an open deployment choice. References: [R-001](requirements.md#r-001), [Q-002](open-questions.md#q-002).

<a id="d-002"></a>
## D-002 — Keep both planning approaches in the release

**Status: Confirmed.** The first release includes both sprint planning and a Gantt schedule. Projects select hours or story points for estimates; YAJA uses hours. Both manual time entries and timers are required.

**Consequence:** Delivery can be incremental, but release scope retains both views. Actual time and elapsed duration remain distinct. The relationship between task estimates and resource demand remains open. References: [R-002](requirements.md#r-002)–[R-004](requirements.md#r-004), [Q-006](open-questions.md#q-006).

<a id="d-003"></a>
## D-003 — Manually preserve the knowledge produced by tasks

**Status: Confirmed.** Record created files, decisions, lessons, insights, and linked follow-up tasks. Manual entry is sufficient for the first release.

**Consequence:** Automatic knowledge collection is unnecessary for initial scope. This choice does not settle automatic resource reporting, which is a separate issue. An origin link does not itself decide task order. References: [R-005](requirements.md#r-005), [R-019](requirements.md#r-019), [Q-016](open-questions.md#q-016), [Q-018](open-questions.md#q-018).

<a id="d-004"></a>
## D-004 — Configurable scheduling with authorized exceptions

**Status: Confirmed.** Both manual and automatic scheduling are project options. Automatic scheduling can coexist with individual fixed tasks or milestones when an authorized user overrides the default. If a predecessor makes a fixed milestone infeasible, keep the fixed date and show the conflict and cause.

**Rationale:** Both scheduling modes serve the release scope. A fixed date expresses a constraint even when the forecast cannot meet it; the conflict needs to remain visible.

**Consequence:** A valid configuration can contain an infeasible forecast; show the cause. Permission checks and physical availability checks remain necessary. References: [R-020](requirements.md#r-020)–[R-022](requirements.md#r-022), [Q-014](open-questions.md#q-014).

<a id="d-005"></a>
## D-005 — Capacity is heterogeneous and time-bound

**Status: Confirmed.** Resources include people, machines, and places. Project and sprint capacity rules must coexist with resource availability. Resource requirements are assigned during planning and treated as schedule constraints. Allocation and actual consumption remain separate.

**Rationale:** Weekly effort totals alone cannot represent machine or room availability, shared bookings, and dated reservations.

**Consequence:** Human-hours, machine-hours, and room-hours are not interchangeable. The specific calendar, resource quantities, planning limits, and reservation timing require design decisions. References: [R-009](requirements.md#r-009)–[R-012](requirements.md#r-012), [Q-003](open-questions.md#q-003), [Q-004](open-questions.md#q-004), [Q-010](open-questions.md#q-010).

<a id="d-006"></a>
## D-006 — Preserve baselines and revise remaining demand

**Status: Confirmed.** Keep planned allocation and consumption separately. Initialize remaining demand from the plan less reported use, and allow an explicit revised remainder. Eight planned hours and three consumed hours initially leave five; revising the remainder to seven produces a ten-hour forecast without rewriting the eight-hour plan.

**Consequence:** “Locked requirements” means honored by the scheduler, not immutable forever. Consumption alone cannot prove completion. References: [R-012](requirements.md#r-012)–[R-014](requirements.md#r-014), [Q-010](open-questions.md#q-010), [Q-017](open-questions.md#q-017).

<a id="d-007"></a>
## D-007 — Share availability across projects; never bank expired time

**Status: Confirmed.** Resources span projects. Released future reservations return to shared availability by default. Resource identity/eligibility and original time windows are retained. Time already passed cannot become future slack.

**Rationale:** Resource availability belongs to a named resource and a time window. A sprint or project balance alone cannot represent shared availability or distinguish future reservations from expired time.

**Consequence:** Releasing three future days when a sprint has one day left gives that sprint at most the usable portion within its dates. Later days remain later shared availability. Release does not create total physical capacity, assign work to another sprint, or transfer protected ownership. Protected project quotas remain an open scope decision. References: [R-010](requirements.md#r-010), [R-015](requirements.md#r-015), [R-016](requirements.md#r-016), [Q-004](open-questions.md#q-004), [Q-007](open-questions.md#q-007).

<a id="d-008"></a>
## D-008 — Resource consumption is independent

**Status: Confirmed.** A task can use four developer-hours and one of two allocated server-hours and still be complete. The unused future server reservation becomes slack on completion. No usage percentage from one resource establishes another resource's progress.

**Rationale:** Resource quantities do not establish execution order. Task lifecycle phases and resource timing are separate concepts.

**Consequence:** Independent consumption does not fully define reservation timing or overlap. Timing and overlap need explicit scheduling rules; resource-hours alone do not determine elapsed duration. References: [R-014](requirements.md#r-014), [R-024](requirements.md#r-024), [Q-009](open-questions.md#q-009).

<a id="d-009"></a>
## D-009 — Multiple project-level scheduling drivers

**Status: Confirmed.** Projects select one or more driving resource types, defaulting to Human. Tasks reference named resources. All required resources can constrain feasibility, including non-drivers.

**Rationale:** Projects select driver types, while tasks assign named resources. Supporting multiple types allows human and machine demand to influence dates. Task-level driver overrides remain an open proposal.

**Consequence:** A multi-driver algorithm and explicit resource-window model must be defined before predicting dates. Consumption still does not automatically complete a task. References: [R-023](requirements.md#r-023), [R-024](requirements.md#r-024), [Q-009](open-questions.md#q-009), [Q-014](open-questions.md#q-014).

<a id="d-010"></a>
## D-010 — Add resource costs without conflating cost and availability

**Status: Confirmed.** Each resource has an hourly cost and a consumed-time or reserved-time cost basis. Preserve historical rate applicability and the original estimate. Provide planned cost, actual cost, forecast-at-completion, and variance.

**Consequence:** A released reservation can still incur cost. The simple actual-cost formula “consumed hours × rate” applies to consumed-time resources, not all resources. Reservation costs must account for chargeable reserved hours. Currency, rate selection, recognition, and precise forecast formulas are open. References: [R-025](requirements.md#r-025), [R-026](requirements.md#r-026), [R-028](requirements.md#r-028), [Q-015](open-questions.md#q-015).

<a id="d-011"></a>
## D-011 — Per-resource refund policy for future releases

**Status: Confirmed.** Reserved-time resources support non-refundable and fully refundable-for-released-future-hours policies in the first release. A five-hour reservation at 20/hour with two hours used can retain a 100 charge or reduce to 40 when the three unused future hours are released, depending on policy.

**Consequence:** Availability is identical in those two release cases; accounting differs. Partial refunds and cancellation windows are not defined. Reopening preserves prior charges/refunds and accounts for new reservations separately. References: [R-018](requirements.md#r-018), [R-027](requirements.md#r-027), [Q-015](open-questions.md#q-015), [Q-021](open-questions.md#q-021).

<a id="d-012"></a>
## D-012 — Phase is system-wide; status belongs to a project

**Status: Confirmed.** System phases are New, Active, and Done. A project administrator defines statuses, each mapped to one phase. Multiple statuses may map to the same phase. The task's phase is derived from status.

**Consequence:** New → Ready may leave phase unchanged. Status changes do not record resource consumption. Entering Active does not start billing or record time merely by virtue of the phase change. A general status-action automation engine is outside the defined scope. References: [R-006](requirements.md#r-006), [R-007](requirements.md#r-007), [Q-008](open-questions.md#q-008).

<a id="d-013"></a>
## D-013 — Completion releases; reopening requires review

**Status: Confirmed.** Entering Done completes a task and releases unused future reservations, preserving plan, consumption, and cost history. Done → Active reopens the task without reclaiming released capacity. Mark its resource plan for review; confirm remaining needs before new reservations. Another project's subsequent booking must not be displaced.

**Consequence:** Reopening may happen after an erroneous completion or genuinely new work. Neither case allows remaining effort to be inferred safely from the original estimate. Rescheduling follows the project's mode after review. Reopening retains the earlier completion history. References: [R-017](requirements.md#r-017), [R-018](requirements.md#r-018), [Q-019](open-questions.md#q-019).

<a id="d-014"></a>
## D-014 — Move toward implementation and learn from usage

**Status: Incremental delivery direction confirmed; sequence proposed.** Start with a usable local task-management increment, then add shared resources, time tracking, lifecycle and cost behavior, and sprint/Gantt scheduling.

**Consequence:** Resolve blockers for each increment and use observed behavior to refine the plan. The full first-release scope remains in place. Dates and effort estimates are not yet assigned. References: [backlog](backlog.md), [Q-022](open-questions.md#q-022).

<a id="d-015"></a>
## D-015 — Existing architecture remains a reference, not a new product decision

**Status: Superseded for the first local task path by D-016; other capabilities remain proposals.** The v0.2 reference specifies React, a Go API, Rust workers, PostgreSQL through FerretDB, NATS, OpenSearch, and a Debezium-based change pipeline, with shared Rust query compilation. Current code implements only foundational components.

**Consequence:** The architecture remains a starting point for evaluating the first increment. Any changes need a documented rationale and impact assessment; the product plan does not select a replacement architecture. Kubernetes is not a release requirement. References: [v0.2 design](../reference/YAJA-v0.2.md), [Q-001](open-questions.md#q-001), [Q-019](open-questions.md#q-019).

<a id="d-016"></a>
## D-016 — Adopt the bounded local task path

**Status: Confirmed for the first local task-management increment.** Adopt the reference storage/event stack within the [SP-001 evidence boundary](../spikes/SP-001-task-path.md). An HTTP 200 from the existing task action means the authoritative database acknowledged the mutation and includes the persisted version. It does not promise search visibility. `sync_token: null` means no usable token was supplied, not that indexing is unhealthy. The browser displays “Saved — search update pending”, retains the saved version, and does not replace it with an older search result. An authoritative database-backed read remains independent of search; a missing search hit is not a missing task. The visibility wait ends when the projection reaches the saved version or newer. An uncertain write outcome is reconciled with the same operation ID. A future resource-creation route may use 201; reserve 202 for work whose execution is incomplete, such as an asynchronous saga. The experiment's 30-second delivery deadline is a test bound, not a product guarantee.

**Mutations:** Require an operation ID and expected version. Check for an identical recorded operation before rejecting a stale expected version. Replay returns the original successful result, even after later task updates; fetch current state separately. Reusing an ID for different content and a version conflict produce distinct 409 errors (`operation_id_reused`, `version_conflict`). Transport retries retain the ID; a changed intention or conflict resolution uses a new ID. Commit the mutation, version increment, and successful-operation record atomically. Define ID scope, canonical request comparison, replay retention and expiry behavior before implementing the durable ledger. The prototype's 16-operation limit is not a product limit; an unbounded embedded ledger is not acceptable.

**Installation and access:** Before accepting task mutations, automated, repeatable installation or migration must discover the actual task table and verify its replica identity and publication membership. `REPLICA IDENTITY FULL` is the initial validated choice; measure WAL/storage cost as records grow. Separate setup privileges from application privileges and fail readiness clearly on drift. Retest mapping and capture on relevant upgrades. Browser-facing access is host-only on loopback for this increment; database, broker, and search stay on internal networks unless administration specifically requires exposure. Host-only access is not an identity model. The browser-facing service still needs cross-origin and CSRF protection, including for unauthenticated requests.

**Boundary:** Authentication, startup policy, broader runtime support, collection lifecycle, schema evolution, deletion, WAL retention, backup/restore, poison-event recovery, and cross-document consistency remain open work. Single-task version checks and event delivery do not establish consistent reservation releases or cost adjustments. References: [B-002](backlog.md#b-002), [B-005](backlog.md#b-005), [B-007](backlog.md#b-007), [Q-001](open-questions.md#q-001), [Q-002](open-questions.md#q-002), [Q-019](open-questions.md#q-019), [Q-020](open-questions.md#q-020).

## How to change a decision

Record the revised behavior and reason, identify affected requirements and acceptance scenarios, and mark the older choice superseded instead of deleting its history. Keep unresolved proposals separate from confirmed decisions.
