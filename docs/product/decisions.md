# Kehila product decision record

Baseline: 2026-09-23. This record explains the choices behind the [requirements](requirements.md). Status labels refer to the product planning baseline; delivery proposals and unresolved technical choices are identified separately.

<a id="d-001"></a>
## D-001 — Start with local project management

**Status: Confirmed.** Local project and task management is the initial use case, including tracking Kehila development. It covers current work, task knowledge, dependencies, effort, and dates. Broader deployment requirements remain open.

**Consequence:** Evaluate the first increment using a complete local task-management workflow. Access from other devices remains an open deployment choice. References: [R-001](requirements.md#r-001), [Q-002](open-questions.md#q-002).

<a id="d-002"></a>
## D-002 — Keep both planning approaches in the release

**Status: Confirmed.** The first release includes both sprint planning and a Gantt schedule. Projects select hours or story points for estimates; Kehila uses hours. Both manual time entries and timers are required.

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

**Consequence:** The architecture remains a starting point for evaluating the first increment. Any changes need a documented rationale and impact assessment; the product plan does not select a replacement architecture. Kubernetes is not a release requirement. References: [v0.2 design](../reference/Kehila-v0.2.md), [Q-001](open-questions.md#q-001), [Q-019](open-questions.md#q-019).

<a id="d-016"></a>
## D-016 — Adopt the bounded local task path

**Status: Confirmed for the first local task-management increment.** Adopt the reference storage/event stack within the [SP-001 evidence boundary](../spikes/SP-001-task-path.md). An HTTP 200 from the existing task action means the authoritative database acknowledged the mutation and includes the persisted version. It does not promise search visibility. `sync_token: null` means no usable token was supplied, not that indexing is unhealthy. The browser displays “Saved — search update pending”, retains the saved version, and does not replace it with an older search result. An authoritative database-backed read remains independent of search; a missing search hit is not a missing task. The visibility wait ends when the projection reaches the saved version or newer. An uncertain write outcome is reconciled with the same operation ID. A future resource-creation route may use 201; reserve 202 for work whose execution is incomplete, such as an asynchronous saga. The experiment's 30-second delivery deadline is a test bound, not a product guarantee.

**Mutations:** Require an operation ID and expected version. Check for an identical recorded operation before rejecting a stale expected version. Replay returns the original successful result, even after later task updates; fetch current state separately. Reusing an ID for different content and a version conflict produce distinct 409 errors (`operation_id_reused`, `version_conflict`). Transport retries retain the ID; a changed intention or conflict resolution uses a new ID. Commit the mutation, version increment, and successful-operation record atomically. Define ID scope, canonical request comparison, replay retention and expiry behavior before implementing the durable ledger. The prototype's 16-operation limit is not a product limit; an unbounded embedded ledger is not acceptable.

**Installation and access:** Before accepting task mutations, automated, repeatable installation or migration must discover the actual task table and verify its replica identity and publication membership. `REPLICA IDENTITY FULL` is the initial validated choice; measure WAL/storage cost as records grow. Separate setup privileges from application privileges and fail readiness clearly on drift. Retest mapping and capture on relevant upgrades. Browser-facing access is host-only on loopback for this increment; database, broker, and search stay on internal networks unless administration specifically requires exposure. Host-only access is not an identity model. The browser-facing service still needs cross-origin and CSRF protection, including for unauthenticated requests.

**Boundary:** Authentication, startup policy, broader runtime support, collection lifecycle, schema evolution, deletion, WAL retention, backup/restore, poison-event recovery, and cross-document consistency remain open work. Single-task version checks and event delivery do not establish consistent reservation releases or cost adjustments. References: [B-002](backlog.md#b-002), [B-005](backlog.md#b-005), [B-007](backlog.md#b-007), [Q-001](open-questions.md#q-001), [Q-002](open-questions.md#q-002), [Q-019](open-questions.md#q-019), [Q-020](open-questions.md#q-020).

<a id="d-017"></a>
## D-017 — Bound the first local operation and runtime policies

**Status: Accepted direction with provisional limits and explicit release gates.** Keep immutable successful-operation records as the bounded single-task implementation candidate; do not extend that layout to reservations or costs before [Q-019](open-questions.md#q-019) is resolved. Before real data, demonstrate one accepted next version under concurrent writes, no search regression from duplicate/older events, compaction that preserves inactive current tasks, a fresh index rebuild after compaction during new writes, and practical project task lists as history grows. Search visibility requires an actual search query or a tested refresh-aware mechanism; document GET alone is insufficient.

**Operation lifetime:** Distinguish admission age of an unseen ID, replay retention after server commit, and physical record retention. The provisional replay promise is 90 days after successful save, configurable internally. An expired unknown ID is rejected, never silently executed. A recorded success is recognized before current-version and other mutable business rules, after authentication and authorization once those exist. Ordinary rejected attempts need not be retained; an uncertain transport outcome is not a confirmed rejection. Replay retention must not erase product history, plans, knowledge, completion history, or costs. The provisional worker currently retains all operation records physically; its admission age is also set to 90 days pending usage evidence.

**Local access and recovery:** The first supported interface has one authenticated owner, with defined onboarding, session expiry, and access recovery; protect reads and configuration as well as writes. Browser-session CSRF/origin policy and authenticated CLI behavior need separate contracts. Windows/Podman is the first supported runtime, with manual start, stop, status, readiness diagnostics, and repeatable installation/upgrades that preserve data. Before real data, test acknowledged-save survival across process crash, container recreation, and ordinary machine restart; automatic daily backups with visible age/failure; and a clean-instance restore of configuration and replay records followed by search reconstruction. Machine-loss recovery requires a copy outside the machine's failure boundary. A daily schedule alone does not guarantee a 24-hour recovery point when the laptop sleeps or a job fails. Restore must reject or reconcile stale browser versions and retry IDs rather than silently accepting them as new intentions.

**Pending search:** Keep the authoritative saved version and “Saved — search update pending”; offer “Refresh saved task” independently of index repair. Failed event delivery needs bounded retries, durable failure reporting, and a recovery path that accounts for retained PostgreSQL WAL. Define an operating budget for task count, memory, disk growth, startup, and save latency before calling the installation comfortable for daily use. References: [B-003](backlog.md#b-003), [B-005](backlog.md#b-005), [B-007](backlog.md#b-007), [Q-008](open-questions.md#q-008), [Q-020](open-questions.md#q-020), [Q-022](open-questions.md#q-022).

<a id="d-018"></a>
## D-018 — Establish configuration consistency and work-item contract boundaries

**Status: Direction accepted by the maintainer on 2026-09-29; detailed contract
choices remain open.** Give project configuration explicit revisions. Accepted
work-item writes must satisfy the configuration governing their commit; item
version checks alone do not establish that guarantee. Configuration edits must
not silently invalidate existing items. Field visibility is separate from
authorization, and authoritative validation must distinguish missing, null,
empty, and invalid values by field type.

Type conversion validates a complete destination state, including destination
workflow/status when needed. Conversion preserves knowledge, follow-up
provenance, and lifecycle history. Migration and conversion obey system phase
rules: Done to New is prohibited, and Done to Active is reopening. Additional
workflow restrictions on migration remain to be decided.

Retain existing references to archived configuration and permit movement away
from it; reject new assignments to archived targets. Require valid replacements
before archiving an active default workflow or initial status. Workflow
membership removal and permission revocation require an explicit compatibility
or migration policy.

A task estimate is an independent planning quantity in the project's selected
unit. Resource demand is separate; there is no implicit points-to-hours
conversion or aggregation of heterogeneous resource-hours into an estimate.
Numeric representation and changing a project's estimate unit remain open.

**Consequence:** Preserve D-016/D-017 replay precedence even after configuration
changes. B-003 defines command invariants; B-005 must demonstrate enforcement
under concurrent configuration and item writes. The existing single-task path
does not establish this consistency. M1 scope, hidden-field behavior, migration
restrictions, relationship integrity, and remaining representation choices are
tracked in the [B-003 contract review](B-003-contract.md). Acceptance of this
direction does not finalize those proposals or claim implementation.

References: [Q-006](open-questions.md#q-006), [Q-008](open-questions.md#q-008),
[B-003](backlog.md#b-003), [R-002](requirements.md#r-002),
[R-005](requirements.md#r-005), [R-006](requirements.md#r-006), and
[R-018](requirements.md#r-018).

<a id="d-019"></a>
## D-019 — Resolve the B-003 review and retain the full configurable M1

**Status: Accepted by the maintainer on 2026-09-29.** This decision resolves
the ten review areas C-01 through C-10 in the
[B-003 contract](B-003-contract.md), refining D-018. Exact representation limits,
remaining command details, and implementation evidence are still outstanding.

- Use one logical configuration revision per project. Reject new commands
  prepared against a stale revision and require revalidation as a new intention.
  Preserve D-016/D-017 replay precedence for recorded successes.
- Reject configuration changes that invalidate dependent items until those
  items are migrated. Cross-phase workflow migration requires permission and
  both source and destination workflows to permit the phase change.
- Hidden fields retain data, remain readable by authorized clients, and reject
  ordinary writes. Optional fields may be absent; clearing normalizes to absence.
  Required text contains non-whitespace text; zero is a value. Initial custom
  field kinds are text, number, Boolean, date, and single-choice.
- Item archival clears current selection, prevents new selection, preserves
  relationships/history, and permits explicit archive retrieval. Archived items
  are read-only until restored. Archival has no implicit lifecycle or resource
  effects; integration with reservations needs later specification.
- Permit cross-project relationships with authorization for both endpoints.
  Reject self-links and duplicates; derive inverse display from one canonical
  relationship. Endpoint archival preserves links. Referenced relationship
  types may be renamed but not deleted or reinterpreted; archive or replace them.
- Keep title configurable. Use a readable project-prefix/sequence identifier
  alongside stable internal identity. Project minimum fields are identity, name,
  readable prefix, estimation unit, configuration revision, and archival state.
- Estimates use exact nonnegative decimals. Zero differs from not estimated.
  M1 prohibits changing the project unit after any estimate has been recorded.
- Archive populated projects instead of deleting them. Preserve configuration
  history needed to interpret prior operations, removed source-type values in
  conversion history, knowledge, provenance, and lifecycle history independently
  of replay retention. Destructive history cleanup is outside B-003.

**M1 scope:** Implement the full proposed configurable model: Task and Milestone,
project-defined types and custom fields, configurable field usage, workflows and
statuses, type conversion, workflow migration, current-work selection, typed
relationships and relationship-type administration, knowledge, estimates, and
delegated administration. Split delivery into
[M1 implementation slices](backlog.md#m1-configurable-model) without deferring
these capabilities beyond M1. The alternative of a smaller configurable subset
in M1 was not accepted.

**Consequence:** B-003 remains contract work, with typed definitions and pure
checks outstanding. B-004/B-005/B-006/B-007 implement and integrate the model.
Detailed access grants and knowledge structure still require their respective
questions to be resolved. M1 does not absorb later resource/scheduling features
merely because configuration and relationships are included.

<a id="d-020"></a>
## D-020 — Preserve issued identifiers and archive projects without rewriting items

**Status: Accepted by the maintainer on 2026-09-30.** Archiving a project blocks
new items and project configuration edits, makes its existing items read-only,
and clears current-work selections that point into it. Existing items,
relationships, and history remain readable. Restoring the project makes items
that were not individually archived editable again. Project archival alone
does not change item phase, report usage, release resources, or invoke scheduling.

An issued readable work-item ID retains its original prefix and sequence.
Changing a project's current prefix affects only future IDs. Earlier IDs
continue to resolve to the same stable internal WorkItem identity. Allocation
must be unique and monotonic within the project; an authoritative allocator is
needed before issuing IDs in concurrent requests.

Changing a field's value kind after values exist is rejected. Define a new
field, migrate values explicitly, and archive the old field. Renaming a field
and changing its hidden/optional/required usage are separate operations subject
to existing validation and history rules. This policy preserves the meaning of
historical values rather than reinterpreting them under a new type.

**Consequence:** [B-003](backlog.md#b-003) defines typed rules and pure tests.
[B-005](backlog.md#b-005) must coordinate project archival with user selections,
provide durable readable-ID allocation and old-ID resolution, and enforce
field changes against authoritative values under concurrency. Exact prefix,
field-value, and estimate limits remain contract details. References:
[R-001](requirements.md#r-001), [R-005](requirements.md#r-005),
[Q-008](open-questions.md#q-008), and the
[contract review](B-003-contract.md).

<a id="d-021"></a>
## D-021 — Scope readable IDs by project and bound M1 field values

**Status: Accepted by the maintainer on 2026-09-30.** A readable WorkItem ID
is resolved together with its project identity. Different projects may use the
same prefix and sequence. An issued ID keeps its original prefix and sequence
when the project's current prefix changes; its stable internal identity remains
the record reference. A global readable-ID namespace is not required in M1.

M1 uses exact decimal thousandths for estimates from `0` through
`999999.999`, and for general numeric fields from `-999999.999` through
`999999.999`. Text field values contain at most 16 KiB of UTF-8 bytes. Dates
use the Gregorian calendar in years 1 through 9999. Project prefixes contain
2 through 12 uppercase ASCII letters or digits and begin with a letter. The
contract rejects values outside these limits without rounding or truncation.

**Consequence:** These limits are part of [B-003](backlog.md#b-003)'s typed
contract, with pure boundary checks. [B-005](backlog.md#b-005) must preserve
exact values, project-scoped lookup, and issued-ID stability in durable storage.
Command-size limits and resource-demand representation remain separate work.
References: [Q-006](open-questions.md#q-006),
[Q-008](open-questions.md#q-008), [R-002](requirements.md#r-002), and
[the contract review](B-003-contract.md).

<a id="d-022"></a>
## D-022 — Bound the phase graph and scope relationship types and grants

**Status: Accepted by the maintainer on 2026-09-30.** The system allows five
cross-phase edges: New to Active, New to Done, Active to New, Active to Done,
and Done to Active. Done to New is forbidden. A workflow may allow a subset of
those edges. Changing status within the same phase is allowed when the target
status belongs to the workflow. Cross-phase workflow migration requires both
workflows to allow the edge and separate migration authorization.

A project-defined relationship type belongs to one project and may link two
WorkItems only when at least one endpoint belongs to that project. The type's
identity includes its owning project, so equal local type IDs in different
projects do not collide. Cross-project links are allowed. Creating a link
requires permission to use the type in its owning project and explicit Link
permission on each endpoint item. Administering the type requires project
configuration permission in its owning project. B-007 defines grants and
enforces these checks at the access boundary; B-005 enforces endpoint integrity
and canonical uniqueness under concurrent creation.

**Consequence:** [B-003](backlog.md#b-003) carries these rules in its typed
contract and pure decision checks. References: [Q-008](open-questions.md#q-008),
[Q-014](open-questions.md#q-014), [B-003 contract](B-003-contract.md).

<a id="d-023"></a>
## D-023 — Bound M1 item field payloads

**Status: Accepted by the maintainer on 2026-09-30.** A new WorkItem create,
field edit, or type conversion command supplies at most 128 field entries and
at most 1 MiB (1,048,576 bytes) of UTF-8 text values in total. An edit counts
all supplied field updates, including Keep and Clear; only supplied Set text
values contribute to the text-byte sum. A create or conversion counts every
supplied field entry and its text value. Existing retained values do not count
against an edit's command payload. The existing 16 KiB limit for each text
value still applies. Reject an oversized new command without truncation or
rounding, using `payload_limit_exceeded`. An identical previously recorded
success still replays before current payload validation.
One WorkItem type may not define more than 128 active required fields: such a
configuration would make item creation impossible under this command bound.

**Consequence:** B-003 has a pure, encoding-independent bound for these three
item commands. HTTP body limits, configuration and knowledge payloads, and
storage record budgets need their own contracts. References:
[Q-008](open-questions.md#q-008), [B-003](backlog.md#b-003), and
[the contract review](B-003-contract.md).

<a id="d-024"></a>
## D-024 — Keep typed, versioned knowledge and cross-project follow-ups

**Status: Accepted by the maintainer on 2026-09-30.** M1 stores created files,
decisions, lessons, and insights as separate typed entries attached to a
WorkItem. A created file is a label and an external reference such as a
repository path, commit link, or URL. The reference does not imply uploaded
content or automatic retrieval. Edits retain prior versions. A follow-up
WorkItem may belong to another project when the actor has Link permission on
both items. Provenance is a distinct origin relationship, retained through
conversion and archival; it does not imply task ordering.

**Consequence:** B-003 defines the typed entries, edit/history and follow-up
commands. B-005 must retain versions and provenance durably; B-007 supplies
current grants. Entry edit grants and exact command boundaries are specified
in the B-003 contract. References: [Q-016](open-questions.md#q-016),
[R-005](requirements.md#r-005), [B-003](backlog.md#b-003).

<a id="d-025"></a>
## D-025 — Retain existing archived references during type conversion

**Status: Accepted by the maintainer on 2026-10-01.** A conversion may retain
the item's current archived workflow or status when the destination type
permits those references. A different destination type must be active, and a
new assignment to a different archived workflow or status is rejected.
Workflow changes retain the existing migration authorization and phase rules.

**Consequence:** B-003 distinguishes retained references from new assignments
in its pure conversion decision and regression tests. B-005/M1-05 must enforce
the same eligibility at commit time.

<a id="d-026"></a>
## D-026 — Reject malformed source snapshots during normal conversion

**Status: Accepted by the maintainer on 2026-10-01.** Normal conversion rejects
unknown source fields, fields owned by another type, invalid values, and missing
required source values. The error identifies the field and reason. Valid hidden
and archived source values remain eligible for retained conversion history.
Repair requires an explicit recovery path outside normal conversion.

**Consequence:** B-003 validates source snapshots with existing-value rules;
B-005/M1-05 supplies complete authoritative values and preserves their history.

<a id="d-027"></a>
## D-027 — Keep required choice fields satisfiable

**Status: Accepted by the maintainer on 2026-10-01.** An active required
single-choice field on an active WorkItem type needs at least one active option.
Archiving the last active option is rejected until a replacement is available.
The same rule applies to complete configuration replacement. Optional, hidden,
or archived fields and archived types may retain no active options.

**Consequence:** B-003 validates this invariant in the complete configuration,
so option administration and configuration replacement enforce the same rule.
B-005 must serialize configuration changes with item mutations.

<a id="d-028"></a>
## D-028 — Delegate status administration by group

**Status: Accepted by the maintainer on 2026-10-01.** A status-group grant permits
status creation, renaming, and archival in that group. Moving a status requires
both source and destination group grants. Project administration retains phase
remapping, workflow membership and defaults, and group lifecycle. Other project
configuration changes continue to require project administration.

**Consequence:** B-003 retains the required permission scope with successful
configuration commands and checks current grants before replay. B-007 owns
grant provisioning and revocation; B-005 coordinates authorization and commit.

<a id="d-029"></a>
## D-029 — Create projects with a minimal trusted seed

**Status: Accepted by the maintainer on 2026-10-01.** Project creation is a typed,
replayable command. The initial application seed supplies Task and Milestone,
optional title and description text fields per type, and one shared
New/Active/Done workflow. Estimates remain independent. Creation starts at
configuration revision one; replay returns the recorded seed.

**Consequence:** B-003 provides trusted initialization and validates its complete
configuration. B-005 persists creation coherently; M1-02 integrates the command,
and B-007 provisions creation authorization and initial project access.

<a id="d-030"></a>
## D-030 — Count all retained definitions toward configuration limits

**Status: Accepted by the maintainer on 2026-10-01.** M1 configuration count and
text limits include retained active and archived definitions and choice options.
Archival does not free capacity. Removal frees capacity only where the existing
reference, historical-use, and application-field protection rules permit it.

**Consequence:** B-003 retains bounded complete configuration snapshots and
tests archived entries at every count boundary. The bounds do not authorize
history cleanup or limit the number of historical revisions; B-005 owns their
durable representation.

<a id="d-031"></a>
## D-031 — Archive status groups without cascading to statuses

**Status: Accepted by the maintainer on 2026-10-01.** Archived groups reject new
statuses and moves into them. Existing statuses remain usable and may be renamed
or archived. Project administrators may restore groups. Group archival has no
implicit status or WorkItem effect. Group IDs used in assignments or grants
remain retained and cannot be deleted or reused.

**Consequence:** B-003 checks new membership and historical group identities.
B-005 and B-007 must maintain authoritative assignment and grant-use evidence
and protect it through configuration acceptance.

<a id="d-032"></a>
## D-032 — Scope bounded opaque operation IDs and compare exact requests

**Status: Accepted by the maintainer on 2026-10-01.** M1 uses case-sensitive
opaque tokens of 1–128 printable ASCII bytes without whitespace. Lookup is
scoped by authenticated actor, command family, and target identity. Exact typed
content, including collection order and expected versions/revisions, determines
whether a retry is identical. IDs and values are not normalized.

**Consequence:** B-003 supplies typed lookup keys and success envelopes. M1
domain commands validate token bounds. B-005/B-007 must use the shared scoped
lookup before domain acceptance; the provisional worker remains unchanged.

<a id="d-033"></a>
## D-033 — Require current original mutation grants before replay

**Status: Accepted by the maintainer on 2026-10-01.** Replay checks all current
grants required by the original successful mutation using its retained scope.
This includes Migration permission for a conversion that changed workflows.
Read permission alone does not authorize replay. Revocation may deny a retry
without changing whether its original operation committed.

**Consequence:** B-003's shared success envelope retains the permission scope;
conversion retains its original migration requirement. B-007 resolves current
grants and B-005 protects their validation through command acceptance.

<a id="d-034"></a>
## D-034 — Set 90-day M1 replay and retain operation tombstones

**Status: Accepted by the maintainer on 2026-10-01.** Extend D-017's provisional
task-path direction to all M1 commands. **Origin revised by the maintainer on
2026-10-08:** full replay expires 90 days (7,776,000,000 milliseconds) after the
server-recorded operation timestamp, sampled near the end of the successful
transaction. There is no additional grace or post-commit adjustment. The remaining
persistence/commit delay slightly reduces replay time after commit; this is
accepted. A rejected or rolled-back transaction establishes no successful record.
At expiry an identical retry returns `replay_expired` and never executes again.
Compaction retains a permanent scoped operation tombstone, versioned request
fingerprint, and original grant scope. Changed-content reuse still conflicts.
Product history remains separate; unseen opaque IDs have no timestamp admission
rule. Full replay retention is bounded, but tombstone storage grows over time.

**Consequence:** B-003 supplies full/tombstone lookup and compaction decisions.
B-004/B-005 must freeze the exact-request fingerprint codec and prove durable
replacement, restoration, and no duplicate execution before exposing M1 routes.
Use checked timestamp arithmetic and the recorded deadline for logical expiry,
even when physical payload cleanup runs later. This revision applies to M1's
shared contract, not a claim of changed provisional-worker behavior.

**Bounded verification:** [SP-002](../spikes/SP-002-postgresql-project-create.md)
tested recorded-time expiry, retirement/replay races, irreversible tombstones
and logical restore on native PostgreSQL with explicit QA helpers. This supports
the tested policy mechanics; it does not deliver final codecs, authenticated
routes or a production compactor, and does not change this accepted decision.

<a id="d-035"></a>
## D-035 — Name the project and repository Kehila

**Status: Accepted by the maintainer on 2026-10-04.** Use Kehila as the project
name, `MrQuality/kehila` as the existing repository's new name, `kehila` for
package/import names, and `KEHILA_*` for configuration environment variables.
The name reflects community, shared purpose, mutual responsibility, and
interconnected work. The local checkout folder follows the repository name.

**Consequence:** Update current source, documentation, repository links, and
editable GitHub branding together. Preserve existing database, volume,
container-project, and Podman machine identifiers to avoid a data migration.
Preserve historical commits, comments, logs, and dated test evidence. The rename
does not change domain behavior, maturity, storage contracts, or release scope.
The [branding policy](../BRANDING.md#rename-and-retained-storage-identities)
records namespace migration and retained identifiers.

## How to change a decision

Record the revised behavior and reason, identify affected requirements and acceptance scenarios, and mark the older choice superseded instead of deleting its history. Keep unresolved proposals separate from confirmed decisions.
