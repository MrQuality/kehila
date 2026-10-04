# Kehila product requirements

Baseline: 2026-09-23. These requirements describe planned product behavior. The [planning guide](README.md) explains status labels, [open questions](open-questions.md) record unresolved choices, and the [backlog](backlog.md) maps requirements to implementation work.

## 1. Purpose and vocabulary

Kehila supports local project planning: identifying current and upcoming work, retaining task knowledge, understanding dependencies, and tracking effort and dates.

| Term | Meaning in this baseline |
| --- | --- |
| Project | A collection of work with its own workflow and planning configuration. |
| Task | A work item with an estimate, status, knowledge, and potentially resource requirements and reservations. M1 minimum fields are defined by the [B-003 contract](B-003-contract.md); a broader hierarchy remains open. |
| Phase | One of the system-wide lifecycle categories New, Active, Done. It is not a sequential subtask or a resource-execution stage. |
| Status | A project-defined workflow state mapped to exactly one phase. |
| Sprint | A bounded planning period with its own capacity. Duration, calendar, and rollover rules remain open. |
| Gantt view | A project timeline representing work dates and dependencies. Exact interaction design remains open. |
| Resource type | A category such as Human, Machine, or Room. Project scheduling drivers select types. |
| Named resource | A particular person, server, room, or other resource assigned to a task. |
| Capacity | How much of a resource is available within a specified time window and unit. It is not an undated balance of reusable hours. |
| Requirement / remaining demand | Capacity a task needs / still needs. A requirement is not automatically a dated reservation. |
| Reservation / allocation | Capacity committed to a task at specified times. Requirements describe demand; reservations assign that demand to dates. |
| Consumption | Actual reported resource usage, recorded separately from plans and reservations. |
| Released capacity / slack | Previously reserved future capacity made available again on its actual dates, subject to resource eligibility and other reservations. |
| Past unused allocation | Reserved capacity whose availability window passed without use. Historical unused time cannot be rescheduled into the future. |
| Scheduling driver | A project-selected resource type whose remaining demand and availability inform task scheduling. Other required resources still constrain feasibility. |
| Fixed-date override | An authorized item-level departure from automatic scheduling, such as preserving a milestone date. |
| Resource cost | Cost derived from a resource's hourly rate and configured consumed-time or reserved-time basis. It is not necessarily the entire project budget. |

Effort, elapsed duration, resource-hours, and money are separate quantities. Four hours of work spread across four dates is not four days of effort. Human-hours and room-hours cannot be exchanged. Story points do not automatically convert to resource-hours or cost.

## 2. User, task, and workflow requirements

<a id="r-001"></a>
### R-001 — Initial use and local operation

Support local project and task management, including tracking Kehila development. Identify current work, upcoming work, and progress. Broader deployment requirements and access from other devices remain open ([Q-002](open-questions.md#q-002)).

M1 includes the full configurable model accepted in [D-019](decisions.md#d-019),
including custom fields/types, workflow migration, type conversion, relationship
administration, and delegated administration. See the
[implementation slices](backlog.md#m1-configurable-model). Titles are configurable;
readable project-prefix/sequence identifiers supplement stable internal identity.
Current-work selection is independent of lifecycle. Archiving an item clears its
selection and makes it read-only until restored, preserving relationships and
history without implicit lifecycle or resource effects.

<a id="r-002"></a>
### R-002 — Project-configurable estimates

Projects choose hours or story points as their estimation unit. The Kehila project uses hours. Estimates describe expected work during planning and are independent of separately recorded resource demand ([D-018](decisions.md#d-018)). Resource usage does not automatically rewrite the task estimate. Heterogeneous resource-hours cannot be summed into a task estimate, and no points-to-hours conversion is defined. Estimates use exact nonnegative decimals; zero differs from absence. M1 prohibits project-unit changes after the first recorded estimate ([D-019](decisions.md#d-019)). Exact precision and bounds remain to specify ([Q-006](open-questions.md#q-006)).

<a id="r-003"></a>
### R-003 — Actual time tracking and presentation

Support both manual time entries and a start/stop timer. Show actual work time at useful resolutions, including hours and days, and distinguish it from elapsed calendar duration. Day length, grouping, rounding, overlapping timers, and corrections need definitions ([Q-005](open-questions.md#q-005), [Q-017](open-questions.md#q-017)).

<a id="r-004"></a>
### R-004 — Both sprint and Gantt planning

Both planning approaches are required in the first release and for the Kehila project. Delivering only one does not satisfy release scope. Using the same underlying tasks in both views is the proposed design; precise interaction and sprint-boundary rules remain open ([Q-007](open-questions.md#q-007)).

<a id="r-005"></a>
### R-005 — Task knowledge and originating work

Manually record files created, decisions, lessons learned, new insights, and follow-up tasks produced by execution. A follow-up must be linked to the task that revealed it. Keep this information available after completion and reopening. Automatic knowledge capture is not required in the first release. [D-024](decisions.md#d-024) and the [B-003 contract](B-003-contract.md#knowledge-value-contract) define typed, versioned entries and labeled file references; [Q-016](open-questions.md#q-016) records the resolution.

<a id="r-006"></a>
### R-006 — Shared phases and configurable statuses

New, Active, and Done are system-wide phases shared across projects. Project administrators define statuses mapped to those phases. Multiple statuses may map to the same phase: for example, New → New and Ready → New. Each status maps to exactly one phase; a task's phase is derived from its current status, never independently edited. In progress → Active and Completed → Done are illustrative mappings, not mandated status names.

<a id="r-007"></a>
### R-007 — Workflow state is not resource usage

A status change does not itself consume time. Moving within a phase does not automatically change allocations or consumption. Entering Active does not imply any work was performed. Usage comes from reporting. Entering Done and reopening have the explicit resource effects in R-017 and R-018. Other status-triggered resource automation is outside the defined scope. [Q-008](open-questions.md#q-008) and the [B-003 contract](B-003-contract.md) resolve M1 status changes, in-use edits, phase transitions, and archival behavior.

<a id="r-008"></a>
### R-008 — Task ordering and prerequisites

Support understanding what comes first and which work depends on other work. Scheduling must account for prerequisites. Dependency types, lag, cycles, and the effect of predecessor completion or reopening need definition ([Q-011](open-questions.md#q-011)). A follow-up origin link is not automatically a scheduling dependency.

## 3. Resource requirements, capacity, and lifecycle

<a id="r-009"></a>
### R-009 — Resources shared across projects

The same named resource can be required by tasks in different projects. Its availability and reservations must be considered across those projects. Resources include humans, machines/servers, and places/rooms; do not hard-code all capacity as human effort. Cross-project conflict resolution is open ([Q-012](open-questions.md#q-012)).

<a id="r-010"></a>
### R-010 — Resource, project, and sprint capacity

Represent dated resource availability plus project-defined capacity rules and each sprint's capacity. Shared physical availability and project/sprint planning limits are distinct: spare sprint capacity does not prove a resource is available. A scheduled reservation must satisfy the applicable constraints. Exact units, limits, calendar inheritance, and whether project settings are planning budgets or entitlements remain open ([Q-003](open-questions.md#q-003), [Q-004](open-questions.md#q-004)). Projects do not receive protected ownership by default.

<a id="r-011"></a>
### R-011 — Establish requirements during planning

During planning, assign named resources and their required capacities. Resource requirements constrain the schedule; fitting a date cannot reduce demand automatically. “Locked requirements” does not mean estimates can never change: revised remaining demand is explicitly allowed under R-013. Exact reservation timing, editing permissions, and audit behavior are open ([Q-010](open-questions.md#q-010)). No internal multi-stage task model is required by this rule.

<a id="r-012"></a>
### R-012 — Preserve plan and actuals separately

Retain the original planned allocation, actual consumption, and revised remaining demand as distinct records. Actual reporting must not overwrite the original baseline. Completion and reopening preserve this history. The number of retained plan revisions and correction semantics are open ([Q-010](open-questions.md#q-010), [Q-017](open-questions.md#q-017)).

<a id="r-013"></a>
### R-013 — Update remaining demand without declaring completion

If eight hours are planned and three are consumed, initially calculate five remaining. Allow the user to revise that remainder, for example to seven hours after discovering more work. In that example, the original plan remains eight, consumed remains three, and current forecast becomes ten hours for that resource. Schedule the revised remaining work against future availability. Exhausting an estimate does not automatically complete the task; overrun behavior is open ([Q-017](open-questions.md#q-017)).

<a id="r-014"></a>
### R-014 — Independent resource consumption

Track each resource independently. A task may allocate four developer-hours and two server-hours, consume four and one respectively, and still be complete. One resource's consumption does not establish another's usage or progress. The remaining server-hour is reusable only if its reservation is in the future. Do not infer sequential stages or simultaneous resource usage from these quantities; detailed timing constraints remain open ([Q-009](open-questions.md#q-009)).

<a id="r-015"></a>
### R-015 — Temporal limits on reusable capacity

Release increases available capacity, not total physical capacity. Released time retains resource identity or valid eligibility and its availability window. It cannot be transformed into another resource type or shifted to another date. If three days were reserved last week and only one was consumed, the remaining two days are past unused allocation. They do not increase this week's availability.

<a id="r-016"></a>
### R-016 — Shared release pool and sprint boundaries

Unused future reservations return to shared availability by default, where eligible tasks across projects can reserve them. Reservations retain their resource and dates; release does not change other projects' commitments. If three future room-days are released but the current sprint has only one day left, only the portion within that sprint is available to it. Cross-sprint reservation permission and protected quotas remain open ([Q-007](open-questions.md#q-007), [Q-004](open-questions.md#q-004)).

<a id="r-017"></a>
### R-017 — Completion

Moving to a status mapped to Done completes the task. Preserve its plan and consumption; close outstanding demand; release unused future reservations; apply each resource's release-cost policy. Completion does not imply all allocated resources were consumed. Past unused reservations remain historical. Retry safety and consistency across task, reservation, and cost updates are engineering concerns in [Q-019](open-questions.md#q-019).

<a id="r-018"></a>
### R-018 — Reopening

Moving from Done to Active reopens the task. Preserve earlier completion, consumption, releases, charges, and refunds. Do not reclaim released reservations, even if they once belonged to this task. Mark the remaining resource plan as needing review and require confirmation or revision of remaining demand before creating new reservations. Remaining demand cannot safely be inferred from the original estimate minus actuals. Automatic scheduling finds feasible capacity after confirmation; manual scheduling exposes requirements and conflicts for user planning. Authorized fixed dates remain respected. Done → New is prohibited, including through workflow migration or type conversion ([D-018](decisions.md#d-018)).

<a id="r-019"></a>
### R-019 — Usage-reporting maturity

The product model allows user-reported usage and automatic reporting depending on project maturity. Manual knowledge recording is a separate requirement and does not settle automatic usage-reporting scope. Both manual entries and timers are confirmed; the automatic reporting sources, permissions, and first-release inclusion remain open ([Q-018](open-questions.md#q-018)).

## 4. Scheduling behavior

<a id="r-020"></a>
### R-020 — Project scheduling mode

Project configuration must offer both manual and automatic scheduling, including for Kehila. Manual mode exposes conflicts for user adjustment; automatic mode recalculates affected dates using the agreed constraints. Mode-switch effects, previews, and recalculation triggers are open ([Q-013](open-questions.md#q-013)).

<a id="r-021"></a>
### R-021 — Authorized task and milestone overrides

An authorized user can override project-default automatic behavior for an individual task or milestone, including preserving a fixed milestone date. A project can mix automatic scheduling and fixed items. Check the user's permission; the exact roles, grants, override types, and removal process remain open ([Q-014](open-questions.md#q-014)). Permission checks apply to local deployments as well.

<a id="r-022"></a>
### R-022 — Preserve fixed dates and explain infeasibility

If a prerequisite is forecast to finish after a dependent fixed-date milestone, preserve the milestone date and show the scheduling conflict and its cause. Keep the fixed date while allowing the valid schedule change and displaying the conflict. This does not waive permissions, invalid-input checks, or physical reservation constraints. The user can adjust date, scope, or dependencies. See S-07.

<a id="r-023"></a>
### R-023 — Multiple scheduling drivers

A project can select multiple resource types as scheduling drivers; the default is Human. A task refers to named resources. Remaining demand and availability of driving resources influence its schedule; for example, a project can select both Human and Machine. Usage updates the corresponding resource's remaining demand rather than a universal completion percentage. Multi-driver scheduling mechanics remain open ([Q-009](open-questions.md#q-009)); task-specific driver overrides are a separate proposal ([Q-014](open-questions.md#q-014)).

<a id="r-024"></a>
### R-024 — All required resources constrain feasibility

Even a non-driving required resource can force rescheduling when unavailable. For example, if a server is required for the remaining work, its unavailability can block developer work without its consumption measuring developer progress. How the task specifies required time windows or overlap remains open; internal task phases are not specified by this requirement ([Q-009](open-questions.md#q-009)).

## 5. Resource cost accounting

<a id="r-025"></a>
### R-025 — Hourly rates and historical stability

Resources have hourly costs for project estimates and cost calculation. Retain effective-dated rates and the original planned cost so later rate edits leave historical actual costs unchanged. Which event selects a reservation's rate, future forecast repricing, currency, precision, and visibility remain open ([Q-015](open-questions.md#q-015)).

<a id="r-026"></a>
### R-026 — Per-resource cost basis

Support consumed-time and reserved-time costing in the first release. Under consumed-time costing, usage determines cost. Under reserved-time costing, chargeable reservation time determines cost, including unused time where it remains chargeable. Availability and accounting are separate: releasing a future reservation can free capacity without removing its charge. Do not bill both consumed and reserved time for the same resource commitment merely because both records exist.

<a id="r-027"></a>
### R-027 — Per-resource release-cost policy

For reserved-time resources support two policies: non-refundable, retaining the full reservation charge; and fully refundable for released unused future hours, reducing cost by the applicable charge for those hours. Past unused time does not become released future time. Complex cancellation windows, partial refunds, minimum charges, and external payments were not defined ([Q-021](open-questions.md#q-021)). These rules describe internal cost accounting, not an instruction to execute payments.

<a id="r-028"></a>
### R-028 — Cost summaries and variance

Support planned resource cost, actual resource cost, forecast cost at completion, and variance against the original plan. Aggregate costs attributable to a task and its project without double-counting shared resources. For consumed-time resources, planned cost is planned resource-hours times rate; actual cost is consumed hours times rate; forecast is actual cost plus remaining forecast cost. For reserved-time resources, use chargeable reservations and release adjustments instead; consumed hours alone do not define actual cost. Recognition timing and treatment of existing non-refundable commitments in forecast totals need definition ([Q-015](open-questions.md#q-015)). Resource costs do not claim to cover all project expenses.

## 6. Worked acceptance scenarios

All quantities below are illustrative. These are future acceptance specifications, not evidence of implemented or tested application behavior. Local storage and recovery criteria in S-10 are proposed engineering release gates.

<a id="s-01"></a>
### S-01 — Project workflow and task knowledge

Given New and Ready statuses both map to New, moving a task between them leaves its derived phase New and does not record consumption. Moving it to a status mapped to Active still records no usage by itself. The user can attach created-file references, a decision, a lesson, an insight, and a linked follow-up task. Origin linkage does not automatically impose a dependency. Covers R-005–R-008.

<a id="s-02"></a>
### S-02 — Revising remaining work

Given eight human-hours planned and three reported, initial remaining demand is five. Revising remaining demand to seven preserves eight planned and three consumed and produces ten forecast human-hours. No task completion is inferred. Covers R-002, R-003, R-012, R-013.

<a id="s-03"></a>
### S-03 — Independent usage and early completion

Given four developer-hours and two server-hours allocated, the task completes after consuming four and one respectively. Entering Done closes remaining demand. If the unused server-hour is reserved for a future window, release it in that window. Keep planned, consumed, and released quantities distinct. Covers R-007, R-014–R-017.

<a id="s-04"></a>
### S-04 — Past unused capacity and a sprint boundary

Case A: three days were reserved in a week that has ended, and one was consumed. Record two past unused days; make no future hours available.

Case B: a five-day room reservation has consumed two days; three future days remain, but the sprint ends after the next day. Release the future reservations on their original dates. Only the portion inside the sprint can help that sprint. The later portion is shared availability on later dates, not current-sprint capacity. This case illustrates release accounting; permission to create the original cross-boundary reservation is still Q-007. Covers R-010, R-015, R-016.

<a id="s-05"></a>
### S-05 — Reopen after another project takes capacity

Task A in project A completes, releasing a future server reservation. Task B in project B reserves that time. A returns to Active: retain A's history, do not displace B, and mark A's resource plan for review. After the user confirms A's remaining demand, use project scheduling mode to find or manually arrange new capacity. Retain old charges/refunds and calculate new reservations separately. Covers R-009, R-017, R-018, R-020, R-026, R-027.

<a id="s-06"></a>
### S-06 — Multiple drivers and a required non-driver

Given Human and Machine as drivers, report human and machine usage independently and update each remaining demand. A required room that is unavailable must make the affected proposed work infeasible even if Room is not a driver. Exact dates cannot be asserted until Q-009 defines time-window constraints. Covers R-011, R-014, R-023, R-024.

<a id="s-07"></a>
### S-07 — A fixed milestone becomes infeasible

An authorized user fixes a milestone for Friday. A prerequisite is forecast to complete the following Monday. Preserve Friday, flag infeasibility, and identify the prerequisite. Reject unauthorized attempts to create the override, but do not reject the prerequisite's valid change merely to conceal the conflict. Covers R-008, R-021, R-022.

<a id="s-08"></a>
### S-08 — Cost basis and reservation release

Use an illustrative rate of 20 currency units/hour, constant throughout the example. Five hours are reserved, two used, and three unused future hours released. Under consumed-time costing the usage cost is 40. Under reserved-time non-refundable costing the charge remains 100; under the fully refundable release policy it becomes 40. In both reservation-policy cases the same three future hours become available. This says nothing about when accounting recognition occurs; that remains Q-015. Covers R-025–R-028.

<a id="s-09"></a>
### S-09 — Historical rate and cost preservation

After recording usage and cost at an applicable rate, configure a different rate for a later period. Earlier actuals and the original planned estimate remain unchanged. Reopening must not erase earlier charges or refunds. Exact rate selection for reservations spanning periods needs Q-015. Covers R-012, R-018, R-025, R-028.

<a id="s-10"></a>
### S-10 — Proposed first-use and reliability walkthrough

Create the Kehila project with hour estimates; configure statuses; create tasks with knowledge and links; change a status; reload and retrieve the records. In the later full-release walkthrough, plan the work in both sprint and Gantt views, allocate shared resources, record manual and timed usage, complete and reopen a task, and inspect resource/cost history. Before relying on real project data, verify persistence across application/container recreation and machine restart, then restore a backup into a clean instance. Recovery scope and acceptable loss remain to be agreed in Q-020. Covers R-001–R-005 and the integrated lifecycle; proposed reliability work is B-005/B-018.

## 7. Release boundaries

The first release remains broader than the first increment. Both planning views, both scheduling modes, permitted fixed-date overrides, independent resources and consumption, manual/timer reporting, completion/reopening, and the two cost bases/release policies are in the agreed planning scope. Incremental delivery preserves this release scope.

Detailed design and acceptance of automatic usage reporting, estimation/resource linkage, calendars, allocation timing, multi-driver scheduling, permissions, and cost recognition are open. No release date or effort estimate has been agreed. No implementation or test pass is asserted by this document.
