# B-003 project and work-item contract review

**Status:** Contract work in progress. The maintainer accepted the review
recommendations and the ten review resolutions on 2026-09-29; the remaining
project, identifier, field-kind, and M1 representation choices on 2026-09-30.
[D-018](decisions.md#d-018), [D-019](decisions.md#d-019),
[D-020](decisions.md#d-020), and [D-021](decisions.md#d-021) record those decisions.
The remaining specification work is listed below. This document does not
claim implemented behavior or a finalized storage format.

**Tracking:** [Issue #7](https://github.com/MrQuality/yaja/issues/7),
[B-003](backlog.md#b-003), [Q-008](open-questions.md#q-008), and
[Q-006](open-questions.md#q-006).

## Accepted contract direction

### Configuration ownership and consistency

Project configuration has explicit revisions. A successful work-item mutation
must satisfy the configuration governing its commit. Checking an item's version
alone is insufficient when validation also depends on configuration.

Visibility is separate from authorization. Field validity must distinguish
missing, null, empty, and invalid values according to field type. Hidden fields
retain their values and remain readable by authorized clients, but are omitted
from ordinary editing forms; ordinary writes to hidden fields are rejected.
Optional fields may be absent. Clearing produces absence rather than a second
stored null state. Required text contains non-whitespace text. Numeric zero is
a value. Every supplied value satisfies its declared type and constraints.
M1 custom-field kinds are text, number, Boolean, date, and single-choice.

A configuration change must not silently invalidate existing items. In
particular, making a field required needs authoritative evidence that applicable
items comply, together with protection against concurrent writes invalidating
that evidence. An eventually consistent search query is insufficient.

Removing workflow membership or revoking a type's permitted workflow requires
prior migration of affected items. Reject the configuration edit until those
items no longer depend on the removed membership or permission. B-003 defines
the invariant; B-005 must establish how the authoritative storage path enforces
it.

### Conversion and migration

Type conversion validates the complete destination state. When the existing
workflow is incompatible with the destination type, destination workflow and
status selection belong to the conversion operation. Validation and acceptance
must cover the resulting lifecycle transition as well as destination fields.

Workflow migration always obeys the system phase graph. Done to New is
prohibited, including through conversion or migration. Done to Active is
reopening and retains [R-018](requirements.md#r-018)'s review requirements.
For M1 a cross-phase migration requires both source and destination workflows
to permit the phase change, plus authorization to migrate. A same-phase
migration still requires authorization and valid destination references.

Conversion must preserve knowledge, follow-up provenance, and lifecycle history.
Removed source-type values are retained in conversion history even when absent
from the current representation. Product history is distinct from operation
replay retention. Physical representation and compaction belong to B-005.

### Archival and reference integrity

Existing references to archived configuration remain valid, and items can move
away from archived targets. New assignments to archived targets are rejected.
Before archiving an active default workflow or initial status, configure a valid
replacement. These rules apply to explicit assignments as well as defaults.

The exact meaning of a new assignment must be specified for each operation:
retaining an existing status during an unrelated edit must remain possible.
Archiving a work item clears any current-work selection of it and prevents new
selection. Archived items are read-only until restored and remain retrievable
through an explicit archive filter. Their history and relationships survive.
Archival alone does not complete an item, report usage, or release resources.
Its interaction with reservations must be specified before resource integration.

Project archival blocks item creation and project configuration edits. Items in
that project become read-only, and user current-work selections pointing to them
are cleared. Items, relationships, and history remain readable. Project
restoration makes only individually unarchived items editable again. Archival
itself produces no item lifecycle or resource effects. Storage coordination of
archive and selection changes is a B-005 acceptance requirement.

### Relationships

Cross-project relationships are allowed when the actor is authorized to access
both endpoints. Reject self-links and duplicate links of the same relationship
type between the same endpoints. Store one canonical relationship and derive
its inverse display. Endpoint archival preserves links. Relationship types have
stable identities and may be renamed; a referenced type cannot be deleted or
reinterpreted. Archive it or introduce a new type instead. Generic relationship
validity does not establish scheduling dependency validity; Q-011 owns those
additional rules.

### Identity and project history

Title is configurable, not mandatory structural identity. Display a stable
readable identifier when title is absent: a project prefix plus sequence number,
backed by a separate internal identity that survives display naming changes.
Projects have stable identity, name, readable identifier prefix, estimation
unit, configuration revision, and archival state. Archive projects containing
records instead of deleting them. Preserve configuration history needed to
interpret historical operations. Destructive history cleanup is outside B-003.
Issued readable IDs keep their original prefix and sequence when a project's
current prefix changes. Only future allocations use the new prefix. Old IDs
remain resolvable to stable internal identities; sequence allocation is
monotonic within each project and must be safe under concurrent creation.
Readable-ID lookup includes project identity. Prefixes and sequence numbers
may repeat across distinct projects; global readable-ID uniqueness is not an
M1 requirement.

### Estimates

A task estimate is an independent planning quantity in the project's selected
unit, hours or story points. Resource demand is recorded separately. Resource
usage does not automatically rewrite the task estimate, and the estimate does
not automatically determine resource demand. No points-to-hours conversion or
sum of heterogeneous resource-hours defines an estimate.

Use exact decimal estimates, allow zero, reject negatives, and distinguish zero
from absence (not estimated). M1 prohibits changing the project unit once an
estimate has been recorded, including after that estimate is cleared. Scheduling
must obtain explicit resource demand; its detailed rules remain with the
scheduling questions.

The M1 contract uses exact thousandths from `0` through
`999999.999`. It rejects signs, exponent notation, implicit rounding, and larger
values. Zero remains distinct from an absent estimate.

### Replay

Preserve [D-016](decisions.md#d-016) and [D-017](decisions.md#d-017): after access
checks, an identical recorded success is recognized before current version or
mutable business-rule validation. A configuration change cannot turn that
successful retry into a new execution or reject it under new field rules.
Changed content under the same operation ID remains a distinct conflict.

## Contract structure

The accepted ownership and revision policy guide the typed specification.
Names describe logical contracts and do not prescribe database collections,
HTTP routes, serialization, or Rust inheritance.

| Contract | Ownership and responsibility |
| --- | --- |
| Project | Stable identity, name, readable identifier prefix, estimate unit, configuration revision, archival state. |
| Project configuration revision | A coherent set of field definitions, type/workflow permissions, workflow membership, type default workflows, initial statuses, and transition restrictions. Each type may identify a text field for display title. |
| Work item | Stable identity, project and type references, workflow/status references, item version; phase derived from status. |
| Field definition | Stable identity, owning type, value kind, and validation constraints; display names are not identity. Once values exist, change of value kind is rejected; migrate to a new field and archive the old one. |
| Status | Stable project-scoped identity and phase mapping; display metadata is separate from identity. |
| Workflow | Project-scoped status membership, initial status, and phase-transition restrictions. |
| Current-work selection | User-scoped reference with its own concurrency boundary; selection does not mutate item lifecycle. |
| Relationship | Stable identity, relationship-type reference, and endpoint identities; informational links do not imply scheduling behavior. |

Reference invariant: an item's type, workflow, and status must resolve
within its project configuration, and the status must belong to its workflow.
Cross-project relationships are permitted; they do not weaken
configuration reference checks.

### Accepted configuration revision policy

Use one logical project configuration revision initially, rather than several
independently checked revisions. This reduces combinations of configuration
states a command can observe. It is not a decision to embed all configuration
or all items in one storage document.

A new command supplies the configuration revision it was prepared against.
If that revision is no longer current, return a configuration conflict and
require revalidation as a new intention. Do not silently reinterpret the
command under different field or workflow rules. This check follows successful
operation replay and does not replace the item's expected-version check.

The persisted success records which configuration revision governed acceptance.
The history contract must retain enough information to interpret that state;
the snapshot or reference representation is still to be designed.

Configuration edits likewise carry an expected configuration revision. For
changes affecting existing items, B-005 must provide a serialization mechanism
or another demonstrated protocol that closes the gap between validation and
commit. A revision recorded in a document, without that protocol, is not proof
of consistency. The existing single-task uniqueness checks do not establish it.

### Command boundaries proposed for review

| Operation | Required validation and outcome |
| --- | --- |
| Create item | Resolve active type, workflow and initial status; validate destination fields; record one accepted item version. |
| Edit fields | Validate allowed fields and resulting values; preserve workflow and type references. |
| Change status | Validate target membership and eligibility, phase restrictions, and lifecycle effects. |
| Convert type | Validate destination fields, workflow/status compatibility, preservation rules, and phase effects together. |
| Migrate workflow | Validate destination permissions/status and applicable migration restrictions; preserve history. |
| Change configuration | Check expected configuration revision and the effect on existing items and defaults. |
| Select current work | Validate selection eligibility and update user context independently of lifecycle. |
| Create relationship | Validate relationship type and endpoints under the decided integrity policy. |

Proposed failure categories distinguish item-version conflict, configuration
conflict, operation-ID reuse, invalid reference, archived target, invalid field
value, prohibited transition, and migration required. HTTP mapping and stable
wire error codes remain to be specified. A rejected operation must expose no
partial destination state or lifecycle effects. An uncertain transport outcome
is reconciled using the original operation ID.

### First typed rule slice: status and workflow commands

The Rust `task_contract::work_item` module implements pure decisions for normal
status changes and explicit workflow migration. Its inputs are a typed project
configuration, current WorkItem, command, optional recorded success, and the
caller-provided authorization result. The configuration carries project ID,
revision, statuses, workflows, WorkItem types, and field definitions. Each status carries one phase;
each workflow carries status membership, a New-phase initial status, permitted
cross-phase changes, and archival state. Each type lists permitted workflows.

The command carries an operation ID, expected item version, expected project
configuration revision, and one of two actions: change status within the current
workflow, or migrate to an explicit destination workflow/status. Neither action
accepts a caller-supplied phase. The result returns the derived phase and one
of three lifecycle effects: none, completion, or reopening. The phase in the
result is derived output, not an independently persisted WorkItem field.

The pure decision order is:

1. Reject unauthorized access. This receives an access decision from B-007;
   the module does not authenticate users or define grants.
2. If an authoritative success was found for this item and operation ID,
   replay its original result for an identical command; reject changed-content
   reuse. Replay precedes current configuration, version, and status rules.
3. Reject a stale configuration revision, then a stale item version.
4. Validate the configuration structure and current item references.
5. Validate target membership, type permission, archival eligibility, and
   system/workflow phase rules. Migration requires an authorization decision and
   both workflows' permission for a cross-phase move.
6. Produce one next-version change with its derived phase and lifecycle effect.
   The caller is responsible for committing that result atomically with its
   successful-operation record.

The module rejects duplicate status/workflow/type identities, missing or
non-New initial statuses, invalid membership, and forbidden phase permissions
in a proposed configuration snapshot. An archived current status remains valid;
movement to a different eligible status is permitted. New assignments to an
archived status or migration into an archived workflow are rejected. Same-phase
status changes have no lifecycle effect. Entering Done signals completion;
Done to Active signals reopening; Done to New is forbidden. The module does not
perform resource, usage, reservation, or cost effects.

The rule tests cover BC-01, BC-03, BC-05, BC-06, BC-11, BC-13, and the pure
portion of BC-14. They also cover archived-source movement, duplicate config
IDs, invalid workflow initial phase, missing references, and item-version
conflict. The existing single-task worker still uses its provisional mutation
contract. B-005 must integrate the new rules with authoritative configuration
loading, atomic persistence, and real concurrency checks.

### Second typed rule slice: project identity and archival

The Rust `task_contract::project` module models project archival, item access,
readable ID allocation, estimate values and unit locking, and populated-field
kind changes. Archiving emits a project-scoped instruction to clear current-work
selections. Restoring emits no lifecycle effect. Access is read-only while the
project is archived; an individually archived item remains read-only after
project restoration. Authorization is a separate concern.

Each issued ID stores the stable internal item and project identities together
with the prefix and sequence used at issue time. The allocator increments a
project-local sequence; prefix changes affect only later allocations. The pure
module accepts uppercase ASCII prefixes of 2–12 characters, beginning with a
letter. Readable IDs are resolved with project context; global lookup is not an
M1 contract. B-005 must serialize allocation and preserve old lookup entries.

The estimate representation uses exact decimal thousandths with an upper bound
of `999999.999` in hours or points. No float conversion or implicit rounding is
allowed. A monotonic `ever_estimated` flag locks unit changes after the first
accepted estimate, even if it was zero or is later cleared. B-005 must update
that flag coherently with the estimate write.

A populated field's value kind cannot be changed in place. The pure rule uses
an `ever_valued` input that must include historical values, not only current
visible values. A new field and explicit migration are required. The rule also
rejects configuration edits while the project is archived. Full field schema,
value validation, and migration commands remain to define.

The pure tests cover the rule portions of BC-19 and BC-21 through BC-23.
Storage coordination, old-ID resolution, cross-user selection clearing, and
authoritative historical-value checks are B-005 obligations.

### Third typed rule slice: field values and usage

The Rust `task_contract::field` module gives each field a stable identity and
owning WorkItem type, independent of its display name. A definition has one of
the five accepted value kinds, one usage mode, an archival flag, and stable
single-choice option identities. It validates ordinary `Keep`, `Set`, and
`Clear` edits: `Keep` retains hidden or archived values; `Set` and `Clear`
cannot change them. `Clear` produces absence, and a required field rejects
absence. Required text must contain non-whitespace content; numeric zero and
Boolean false are valid values. A single-choice assignment must name a current
non-archived option, while an existing archived option may be retained.

The pure rule for optional-to-required changes checks the full applicable
value set provided by its caller. B-005 must get that set authoritatively and
serialize the configuration change with concurrent item writes. A search
query or a detached snapshot cannot establish compliance. The rule tests cover
the pure portion of BC-02 and BC-15, plus type ownership, date boundaries,
choice archival, and similarly named fields on distinct WorkItem types.

The M1 representation accepts text values of at most 16 KiB of UTF-8 bytes,
signed numeric thousandths in `-999999.999` through `999999.999`, and Gregorian
dates in years 1 through 9999. Command payload limits and how archived field definitions
evolve remain distinct contract work.

### Fourth typed rule slice: relationships and current work

The Rust `task_contract::relationship` module derives canonical identity from
a relationship type and two project-qualified WorkItem endpoints. It rejects
self-links, duplicate canonical links, archived types or endpoints, and missing
access to either endpoint. Symmetric types sort endpoints into one stored key;
directed types preserve their canonical orientation, from which inverse display
can be derived. Existing links survive endpoint archival. A relationship's
stable record identity, project-defined type ownership, and type-specific
scheduling behavior remain further contract work.

The `current_work` module keeps one versioned selection per user. Choosing or
clearing it changes only user context. An unchanged selection is an idempotent
no-op. Selection checks its own version and rejects archived targets and denied
access; it does not inspect or mutate WorkItem phase, usage, timer, reservations,
or scheduling. When an item or project is archived, B-005 must coordinate the
affected selection clear with the archive change.

Pure tests cover BC-12 and the identity/access portion of BC-17. B-005 must
enforce relationship uniqueness under concurrent creation and valid endpoints
at commit; B-007 must supply authoritative access decisions for both endpoints.

### Fifth typed rule slice: configuration compatibility

An active WorkItem type names a default workflow that it permits and that is
not archived. Every workflow names an unarchived New-phase initial status in
its membership. Archiving an active type's default workflow therefore requires
selecting another eligible default first; archiving an initial status requires
selecting another eligible initial status first. An archived workflow can still
govern existing items and their status changes.

The Rust `task_contract::configuration_change` module validates an authorized
one-revision configuration edit against the complete set of affected items. It
rejects removing a type, workflow, status, workflow membership, or type/workflow
permission while an item still depends on it. It also rejects changing the
phase of a status used by an item, which would reinterpret that item's current
lifecycle state. Archived items remain in the dependency set. A new revision
cannot be accepted for an archived project.

These pure checks cover BC-07 and BC-08 and the in-use status policy in issue
#7. The item set must be authoritative and protected from concurrent writes
through commit; B-005 owns that enforcement. The rule does not implement a
project archive command, since archive changes project state rather than the
configuration schema.

### Sixth typed rule slice: type conversion

The Rust `task_contract::conversion` module accepts a complete destination
field-value set alongside explicit destination type, workflow, and status. A
conversion to another type cannot proceed with an incompatible workflow,
missing required destination field, archived destination, or prohibited phase
change. A workflow change also needs migration authorization and both workflows'
cross-phase permissions. Done to New remains forbidden; Done to Active produces
the reopening effect. The converted item keeps its stable identity and advances
one version. The result separates current destination values from a snapshot of
source values for conversion history. Successful operation replay precedes
changed configuration and item-version checks.

The source-value snapshot is contract output, not proof of durable retention.
B-005 must atomically store it with the converted item and keep it independently
of replay expiration. Knowledge, follow-up provenance, and lifecycle history
are outside the current pure WorkItem shape and must be preserved by the
persistence conversion transaction. The pure tests cover BC-04 and BC-05's
conversion path, the effect portion of BC-06, and the value/replay portions of
BC-09 and BC-20. Durable preservation remains unverified.

### Seventh typed rule slice: creation and field editing

The Rust `task_contract::item_mutation` module decides item creation and
ordinary field/estimate editing against one coherent project configuration
revision. Field definitions now belong to that revision, rather than arriving
as a separate conversion input. Each type may designate one of its text fields
as its display title. The title's usage remains configurable; an absent or
blank title displays the issued readable ID without changing stable identity.

Creation selects the type's default workflow unless an eligible workflow is
explicitly chosen, then derives its New-phase initial status. It validates all
initial fields, allocates the next project-scoped readable ID, creates item
version 1, and records the estimate-unit lock when an estimate is present.
The returned project state, item, content, ID allocation, and operation record
must be committed as one accepted result. The pure rule cannot prove durable
uniqueness or atomicity.

An edit carries sparse field changes, an estimate Keep/Set/Clear action, the
expected item version, and expected configuration revision. Omitted fields are
kept, including hidden values; explicit ordinary changes to hidden or archived
fields fail. The complete resulting values must satisfy required and typed
field rules. Editing preserves item type, workflow, status, and derived phase;
it emits no usage or resource effect. Create and edit replay identical recorded
successes after authorization and before mutable validation. B-005 must make
the required configuration, item, project, and operation writes coherent under
concurrency. The pure checks cover the creation/edit portions of BC-03,
BC-11, BC-13, BC-15, BC-18, and BC-19.

### Eighth typed rule slice: item and project archival

The Rust `task_contract::archive` module gives item and project archive/restore
commands operation identities, expected versions, typed results, and explicit
selection-clear effects. Item archival advances the item version, leaves its
type, workflow, status, phase, knowledge, history, and relationships untouched,
and returns an instruction to clear every user's current-work selection of
that item. Item restoration has no selection effect and is rejected while the
project is archived. Project archival advances the project configuration
revision, makes the project read-only through the existing eligibility rules,
and returns a project-wide selection-clear instruction. Project restoration
advances the revision without restoring individually archived items.

An already archived item or project cannot be archived again as a new
operation; restoring an active one is likewise invalid. Stale revisions
conflict. Identical recorded success replays after authorization and before
current revisions or archive rules, while changed content under the same
operation ID conflicts. The result carries no lifecycle, usage, reservation,
or scheduling effect. B-005 must coordinate the archive state, configuration
revision where applicable, affected user selections, and success record under
concurrency. The pure tests cover BC-16 and BC-21's eligibility and effect
portions; durable cross-user clearing remains unverified.

## Review resolutions

All ten resolutions were accepted by the maintainer on 2026-09-29. C-09 retains
the full configurable model in M1; no capability is deferred by this review.

| ID | Area | Accepted resolution |
| --- | --- | --- |
| C-01 | Configuration revision and stale commands | One logical project revision; reject stale new commands, preserving successful replay precedence. |
| C-02 | Configuration compatibility | Reject invalidating edits until affected items are migrated. |
| C-03 | Migration | Both workflows permit a cross-phase change; migration also requires authorization. |
| C-04 | Hidden fields and validity | Authorized reads remain possible; reject ordinary writes, preserve data, normalize clearing to absence, and validate values by type. Five initial custom-field kinds as above. |
| C-05 | Archival | Clear current selection, preserve links/history, expose archive retrieval, and require restoration before edits. No implicit lifecycle/resource effects. |
| C-06 | Relationships | Allow authorized cross-project links; reject self-links and duplicates; derive inverse display; preserve referenced identities and meanings. |
| C-07 | Identity | Configurable title; readable project-prefix/sequence identifier plus stable internal identity. |
| C-08 | Estimates | Exact nonnegative decimals; zero differs from absence; lock project unit after first recorded estimate. |
| C-09 | M1 scope | Full configurable model in M1, divided into implementation slices in the backlog. |
| C-10 | Project/history | Explicit project minimum fields; archive populated projects; retain configuration, conversion, knowledge, provenance, and lifecycle history. |

## Remaining specification work

These are narrower details, not a reopening of C-01 through C-10:

- Finish single-choice option administration and command payload limits.
- Complete command payloads, typed results, error precedence and stable codes,
  field-definition evolution rules, and the full phase-transition table.
- Specify ownership and administration of project-defined relationship types,
  stable relationship record IDs, and the inverse presentation contract.
- Demonstrate the consistency boundary for item/project archival plus selection
  clearing, and specify cross-project relationship creation plus
  endpoint/configuration changes.
- Implement meaningful pure contract checks and record results. Real-service
  enforcement is a B-005 obligation, not evidence supplied by these checks.

Routine representation choices should be proposed with concrete limits during
typed-contract design. Any new behavioral ambiguity must be identified rather
than silently treated as part of the maintainer's acceptance.

## Acceptance scenarios

These are specifications for future checks, not test results.

| ID | Setup and operation | Required result | Verification boundary |
| --- | --- | --- | --- |
| BC-01 | Change between two statuses mapped to New, then enter Active. | Phase derives from status; neither change records usage. | Pure rules; S-01. |
| BC-02 | Make a field required while a concurrent item edit removes its value. | The pair cannot both commit leaving an applicable item invalid. Either one is rejected/retried, or both are serialized into a valid result. | Pure rejection rules plus real storage contention in B-005. |
| BC-03 | Retry a successful item command after its field or workflow configuration changes. | Return the original successful result without reapplying the operation; changed payload under the same ID conflicts. | Pure replay precedence plus durable replay check. |
| BC-04 | Convert an item to a type that forbids its current workflow. | Reject an incomplete conversion; accept only a valid complete destination state. No partial conversion is visible. | Pure validation plus storage atomicity. |
| BC-05 | Attempt Done to New using a status change, workflow migration, or type conversion. | Reject all three paths. | Pure transition rules. |
| BC-06 | Migrate Done to Active through an otherwise permitted operation. | Recognize reopening and preserve prior history; resource reservations require review under R-018. | Pure effect classification; resource integration in B-011. |
| BC-07 | Archive an initial status or a default workflow without replacement. | Reject the configuration change. With valid replacement, existing references remain usable but new assignments to archived targets fail. | Pure configuration rules plus storage consistency. |
| BC-08 | Remove workflow membership or revoke a type's workflow permission while items still depend on it. | Reject silent invalidation; follow the decided compatibility/migration policy. | Pure rules plus authoritative reference check. |
| BC-09 | Convert an item with knowledge, follow-up provenance, and completion history. | Required information remains available and attributable to the same item. | Contract checks plus persistence verification. |
| BC-10 | Record hour estimates in one project and point estimates in another, then report resource usage. | Preserve separate estimate units and resource accounting; perform no implicit conversion. | Pure estimate rules after C-08. |
| BC-11 | Submit an item with a status outside its workflow or configuration references from another project. | Reject invalid references; never accept independently supplied contradictory phase. | Pure reference validation. |
| BC-12 | Select current work while another item is Active. | Selection alone changes no lifecycle state, usage, or reservations. | Pure selection rules after C-05. |
| BC-13 | Submit a new command under an older configuration revision, including an otherwise valid payload. | Configuration conflict; an identical recorded success still replays. | Pure validation order; durable configuration coordination in B-005. |
| BC-14 | Attempt cross-phase migration when only one workflow permits the change, or migration permission is absent. | Reject; both workflow restrictions and authorization must pass. | Pure rules; authoritative permission enforcement in B-007. |
| BC-15 | Hide a populated field, read it with authorization, and attempt an ordinary write. | Retain/read its value; reject the write; clearing an optional visible field produces absence. | Pure field rules plus persistence preservation. |
| BC-16 | Archive a currently selected item, then attempt to edit or select it. | Clear selection; reject edits/new selection until restoration; preserve links and history. | Pure eligibility plus coordinated persistence. |
| BC-17 | Create a cross-project link, its duplicate, an inverse presentation, and a self-link. | Authorized first link succeeds; duplicate and self-link fail; inverse display does not create a second record. | Pure canonical identity plus storage uniqueness/access checks. |
| BC-18 | Create an untitled item, rename a display prefix, and inspect history. | Readable fallback remains available; internal identity and history remain stable under the specified naming policy. | Pure identity policy after naming details are specified. |
| BC-19 | Record zero, omit an estimate, submit a negative estimate, and change units after clearing a recorded estimate. | Zero and absence differ; negative and post-use unit changes fail. | Pure estimate rules. |
| BC-20 | Convert an item and later expire its operation replay record. | Required history, including removed source-type values, remains available independently of replay retention. | Contract preservation plus B-005 retention/recovery checks. |
| BC-21 | Archive a project with active items and current-work selections, then restore it. | Clear selections; preserve readable items, links, and history; prohibit edits while archived; restore editability only for individually unarchived items. No lifecycle or resource effects. | Pure eligibility/effect rules plus B-005 coordinated persistence. |
| BC-22 | Issue an item ID, change the project prefix, then issue another ID. | The first issued ID remains stable and resolvable; the second uses the new prefix and next project sequence. | Pure allocation rule plus B-005 uniqueness and lookup checks. |
| BC-23 | Attempt to change a populated custom field from text to number. | Reject in-place kind change; allow an explicit new field and value migration while preserving old field history. | Pure configuration rule plus B-005 authoritative value check. |

## Completion and handoff

B-003 remains open until its remaining specification details are decided, the
complete typed contracts and errors are reviewed, and the required pure rule
checks pass.
Documentation acceptance alone does not satisfy issue #7's test requirement.

B-005 owns demonstrated commit-time configuration consistency, authoritative
reference checks, persistence, replay, and recovery. B-007 owns access enforcement.
B-011 owns integrated resource effects. Keep those implementation gates visible
without claiming they have been satisfied by this contract review.

The currently deployed worker still uses the earlier Rust task mutation rules,
which require a nonempty title and accept only the status strings Open and Done.
The new `work_item` module does not yet change that worker behavior. Later
integration must replace those provisional validation assumptions while
retaining successful replay precedence.
