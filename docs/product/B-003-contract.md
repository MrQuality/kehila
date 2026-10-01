# B-003 project and work-item contract review

**Status:** Typed rule slices implemented; final contract review remains open.
Project initialization, delegated administration boundaries, and the remaining
conversion and replay choices require resolution before B-003 is closed.
Implementation remains in the
linked backlog slices. The maintainer accepted the review recommendations and
ten review resolutions on 2026-09-29, then the project, identifier, field,
relationship, payload, and knowledge choices on 2026-09-30.
[D-018](decisions.md#d-018), [D-019](decisions.md#d-019),
[D-020](decisions.md#d-020), [D-021](decisions.md#d-021),
[D-022](decisions.md#d-022), [D-023](decisions.md#d-023), and
[D-024](decisions.md#d-024) record those decisions. This document specifies
logical behavior and pure decisions; it does not claim an implemented
configurable worker or a physical storage format.

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
The complete M1 cross-phase graph is New to Active, New to Done, Active to
New, Active to Done, and Done to Active. Each workflow configures a subset;
same-phase status changes require valid membership but no cross-phase edge.
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

Cross-project relationships are allowed under the grants below. Reject
self-links and duplicate links of the same relationship
type between the same endpoints. Store one canonical relationship and derive
its inverse display. Endpoint archival preserves links. Relationship types have
stable identities and may be renamed; a referenced type cannot be deleted or
reinterpreted. Archive it or introduce a new type instead. Generic relationship
validity does not establish scheduling dependency validity; Q-011 owns those
additional rules.

Each relationship type belongs to one project. At least one endpoint of a new
link must belong to that project; the canonical type identity includes its
owning project. Creating a link requires permission to use that type in its
owner project and Link permission on both endpoint items. Type administration
requires project configuration permission in the owner project. B-007 defines
grant assignment and authoritative enforcement. The pure decision accepts
those three creation grants as explicit inputs.

Relationship types are members of the owning project's single configuration
revision. A complete-revision command can add, rename, archive, or restore a
type. Its display name excludes control characters; its owner and stable ID
cannot change. A type with any historical link
cannot be removed or change directed/symmetric meaning; archiving preserves
existing links and blocks new ones. The configuration snapshot includes type
IDs ever used by links, including links to archived items and other projects.
B-005 must serialize this evidence with link creation and configuration writes.

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
| Project configuration revision | A coherent set of field definitions, type/workflow permissions, workflow membership, type default workflows, initial statuses, status groups, transition restrictions, and project-owned relationship types. Each type may identify a text field for display title. |
| Work item | Stable identity, project and type references, workflow/status references, item version; phase derived from status. |
| Field definition | Stable identity, owning type, application or project origin, value kind, and validation constraints; display names are not identity. Once values exist, change of value kind is rejected; migrate to a new field and archive the old one. |
| Status | Stable project-scoped identity and phase mapping, display name, optional status-group reference, and archival state. Display metadata is separate from identity. |
| Status group | Stable project-scoped identity, display name, and archival state; membership is held by statuses and has no lifecycle effect. |
| Workflow | Project-scoped status membership, initial status, and phase-transition restrictions. |
| Current-work selection | User-scoped reference with its own concurrency boundary; selection does not mutate item lifecycle. |
| Relationship | Stable identity, relationship-type reference, and endpoint identities; informational links do not imply scheduling behavior. |
| Knowledge entry | Stable identity, WorkItem identity, one of created file/decision/lesson/insight, validated value, and independent version history. A created file is a labeled external reference. |
| Follow-up origin | Stable directed WorkItem-to-WorkItem provenance, including cross-project links with Link grants on both endpoints; no scheduling edge. |

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
Each accepted project configuration revision is immutable history identified by
project and revision and retains the complete logical `Configuration` value.
The operation success refers to that revision. B-005 chooses the physical
snapshot/reference mapping and proves that replay and historical interpretation
survive compaction and restore.

Configuration edits likewise carry an expected configuration revision. For
changes affecting existing items, B-005 must provide a serialization mechanism
or another demonstrated protocol that closes the gap between validation and
commit. A revision recorded in a document, without that protocol, is not proof
of consistency. The existing single-task uniqueness checks do not establish it.

### Command boundaries

| Operation | Required validation and outcome |
| --- | --- |
| Create item | Resolve active type, workflow and initial status; validate destination fields; record one accepted item version. |
| Edit fields | Validate allowed fields and resulting values; preserve workflow and type references. |
| Change status | Validate target membership and eligibility, phase restrictions, and lifecycle effects. |
| Convert type | Validate destination fields, workflow/status compatibility, preservation rules, and phase effects together. |
| Migrate workflow | Validate destination permissions/status and applicable migration restrictions; preserve history. |
| Change configuration | Check expected configuration revision and the effect on existing items and defaults. |
| Change project metadata | Check one shared expected revision, name/prefix/unit rules and prior estimate use; advance Project and Configuration together. |
| Select current work | Validate selection eligibility and update user context independently of lifecycle. |
| Create relationship | Supply owner-qualified type and endpoint identities, expected revisions for both projects and versions for both items, operation ID, and trusted record ID; validate grants, current eligibility, and canonical uniqueness. |

Failure categories distinguish item-version conflict, configuration conflict,
operation-ID reuse, invalid reference, archived target, invalid field value,
prohibited transition, and migration required. The stable domain codes are
specified below; HTTP mapping remains B-004/B-006 integration work. A rejected
operation must expose no partial destination state or lifecycle effects. An
uncertain transport outcome is reconciled using the original operation ID.

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
that flag coherently with the estimate write. A concurrent first-estimate write
and unit change must serialize against the same authoritative project state;
the configuration revision alone does not detect a change to `ever_estimated`.

A populated field's value kind cannot be changed in place. The pure rule uses
an `ever_valued` input that must include historical values, not only current
visible values. A new field and explicit migration are required. The rule also
rejects configuration edits while the project is archived. Full field schema,
value validation, and migration commands are specified in the following slices.

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
Field and option display names reject control characters; stable IDs, rather
than display names, determine identity.

The pure rule for optional-to-required changes checks the full applicable
value set provided by its caller. B-005 must get that set authoritatively and
serialize the configuration change with concurrent item writes. A search
query or a detached snapshot cannot establish compliance. The rule tests cover
the pure portion of BC-02 and BC-15, plus type ownership, date boundaries,
choice archival, and similarly named fields on distinct WorkItem types.

The M1 representation accepts text values of at most 16 KiB of UTF-8 bytes,
signed numeric thousandths in `-999999.999` through `999999.999`, and Gregorian
dates in years 1 through 9999. The following slices specify command payload
limits and archived field-definition evolution.

### Fourth typed rule slice: relationships and current work

The Rust `task_contract::relationship` module derives a canonical uniqueness
key from an owner-project-qualified relationship type and two
project-qualified WorkItem endpoints. It rejects self-links, duplicate keys,
archived types or endpoints, and missing creation grants. A stored relationship
also has its own opaque, stable record ID, separate from the uniqueness key.
B-005 allocates that ID and enforces key uniqueness at commit. Neither a type
rename nor inverse display changes the record ID.

Symmetric types sort endpoints into one stored key. Directed types preserve
their `from` and `to` orientation. A view from `from` is outgoing; a view from
`to` is incoming. Both views carry the same record ID and other-endpoint
identity. Symmetric views have the same direction marker from either endpoint.
An unrelated viewer or mismatched type reference is invalid. The view is a
projection, never a second stored link; B-007 must authorize reads before
presenting it. Existing links survive endpoint archival. Type-specific
scheduling behavior remains separate contract work.

The `current_work` module keeps one versioned selection per user. Its typed
selection command carries user identity, operation ID, expected selection
version, and optional project-qualified item identity. Choosing or clearing
changes only user context. An unchanged selection is an idempotent no-op, but
its successful command is still recorded for replay. Current authorization
precedes recorded-success lookup; an identical recorded command returns its
original result even after later selection changes, while changed content under
the same operation ID conflicts. A new command checks the requested endpoint
against its identity, its own version, and target eligibility. It rejects
archived targets and denied access; it does not inspect or mutate WorkItem
phase, usage, timer, reservations, or scheduling. B-005 commits selection state
and successful-operation record atomically and coordinates affected selection
clears with item or project archival.

Pure tests cover BC-12 and the identity/access portion of BC-17. B-005 must
enforce relationship uniqueness under concurrent creation and valid endpoints
at commit; B-007 must supply authoritative access decisions for both endpoints.

The `task_contract::relationship_command` module specifies creation as one
typed command. It carries an operation ID, owner-qualified type, both endpoint
identities, expected configuration revisions for both endpoint projects, and
expected versions for both items. The relationship type's owner is one of
those projects, so its governing configuration revision is among the two.
The trusted boundary supplies the new opaque relationship record ID. A
same-project link reads one coherent project revision. Authorization to use
the type and Link both items is checked before replay. Identical recorded
success replays before current revisions or archival rules; changed content
under the same operation ID conflicts. A new command checks both project
revisions before both item versions, then reference identity, archival state,
and canonical uniqueness. Conflicts identify the stale project or item and
its current revision/version.

The pure inputs are assertions about authoritative reads, not an atomicity
mechanism. B-005 must load the type from the matched owner configuration,
protect both project and item states through commit, allocate a unique record
ID, enforce canonical-key uniqueness, and atomically record success. B-007
must supply current grants, including on replay. Pure tests cover BC-30.

### Fifth typed rule slice: configuration compatibility

Status groups are organizational metadata. A status may belong to one group or
none; any referenced group must exist in the same project configuration. A
referenced status may be renamed or moved to another group without changing its
identity or phase. Group names and status names are nonblank and bounded by 256
UTF-8 bytes; group IDs are unique within the project. A group's administration
grant is scoped by its stable identity and is supplied by B-007. Group archival
does not change the phase or validity of member statuses.

An active WorkItem type names a default workflow that it permits and that is
not archived. Every workflow names an unarchived New-phase initial status in
its membership. Archiving an active type's default workflow therefore requires
selecting another eligible default first; archiving an initial status requires
selecting another eligible initial status first. An archived workflow can still
govern existing items and their status changes.

The Rust `task_contract::configuration_change` module validates an authorized
one-revision configuration edit against a complete authoritative snapshot of
current items and their field values, plus status, workflow, type, field, and
relationship-type IDs ever used in accepted history. It
rejects removing a type, workflow, status, workflow membership, or type/workflow
permission while an item still depends on it. It also rejects changing the
phase of a status used by an item, which would reinterpret that item's current
lifecycle state. Archived items remain in the dependency set. A new revision
cannot be accepted for an archived project.

Historical use has a stronger rule than current-item compatibility. Once a
status has been referenced, its identity and phase mapping remain defined;
once a workflow or WorkItem type has been referenced, its identity remains
defined. After current items migrate away, these definitions may be archived
but cannot be removed, and the status phase cannot be remapped. B-005 must
provide complete historical-use evidence, including converted, archived, and
deleted-item history, and protect it through the configuration commit.

Configuration administration has a typed command carrying one operation ID,
expected revision, and a complete proposed configuration. Authorization is
checked first; identical recorded success replays before current revision or
compatibility checks, and reuse with changed content conflicts. A new command
checks its expected revision, validates the proposed complete state against
the authoritative snapshot, and returns that next revision. An empty operation
ID or a proposal that only increments the revision is invalid. B-005 must
atomically persist the new configuration, interpretable history, and successful
operation record while protecting the item snapshot from concurrent changes.

The same decision checks that every applicable current item satisfies a newly
required field. Existing values must remain valid under the proposed
definition; a hidden or archived field retains its value. Archiving a required
field ends its active required-value obligation without deleting old values.
A field's owning WorkItem type cannot change. Once a field ID has appeared in
accepted history, changing its value kind or removing its definition is
rejected; create a new field, migrate values, and archive the old definition.
An unused project-defined field may change kind or be removed. An
application-defined field cannot be removed, archived, or retyped through a
project configuration revision; it may be hidden. A revision cannot relabel a
field's origin or introduce a new application-defined field. Application
installation or upgrade supplies those definitions through a separate trusted
path. Renaming is independent of identity. The authoritative snapshot must be
checked and protected through commit, including concurrent changes to item
values and historical use.

These pure checks cover BC-02, BC-07, BC-08, BC-23, and the in-use status policy in issue
#7. The item set must be authoritative and protected from concurrent writes
through commit; B-005 owns that enforcement. The rule does not implement a
project archive command, since archive changes project state rather than the
configuration schema.

### Sixth typed rule slice: type conversion

The Rust `task_contract::conversion` module accepts a complete destination
field-value set alongside explicit destination type, workflow, and status. A
conversion to another type cannot proceed with an incompatible workflow,
missing required destination field, archived destination type, newly assigned
archived workflow/status, or prohibited phase change. Retaining the item's
existing archived workflow or status is valid when the destination type permits
those references. A workflow change also needs migration authorization and both workflows'
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

M1 create, edit, and conversion commands each carry no more than 128 supplied
field entries and 1 MiB of combined supplied UTF-8 text values. All supplied
edit entries count, including Keep and Clear; retained values do not count
against the edit. The 16 KiB per-text-value limit also applies. An oversized
new command returns `payload_limit_exceeded` after authorization, replay, and
current revision checks, without truncating data. This is a logical field
payload bound, separate from an adapter's encoded request-body limit.
A configuration with more than 128 active required fields for one WorkItem
type is invalid because no create or conversion command could satisfy it.

### Eighth typed rule slice: item and project archival

The Rust `task_contract::archive` module gives item and project archive/restore
commands operation identities, expected versions, typed results, and explicit
selection-clear effects. Item archival advances the item version, leaves its
type, workflow, status, phase, knowledge, history, and relationships untouched,
and returns versioned before/after mutations clearing every user's current-work
selection of that item. Item restoration has no selection effect and is rejected
while the project is archived. Project archival advances the configuration
revision in both the Project and Configuration representations, sets both
archival flags, makes the project read-only through the existing eligibility
rules, and returns versioned project-wide selection clears. Project restoration
advances both revisions and clears both archival flags without restoring
individually archived items or selections. Mismatched current Project and
Configuration revisions or archival flags are invalid inputs.

An already archived item or project cannot be archived again as a new
operation; restoring an active one is likewise invalid. Stale revisions
conflict. Identical recorded success replays after authorization and before
current revisions or archive rules, while changed content under the same
operation ID conflicts. The result carries no lifecycle, usage, reservation,
or scheduling effect. The supplied current-work snapshot must contain every
user who could select the affected item or project. A duplicate user context
or selection-version overflow rejects the whole command. B-005 must protect
the full selection set against concurrent additions or changes, compare each
before-version, and commit archive state, both project representations where
applicable, all selection clears, and the success record atomically. A replay
returns the original result without reapplying clears. The pure tests cover
BC-16, BC-21, and BC-31's decision portions; durable cross-user clearing
remains unverified.

The observable result is all-or-none. The reference storage design permits
single-document atomic writes and forbids database-level multi-document
transactions. B-005 must choose and verify a consistency protocol that makes
the archive and all affected selection clears appear together, including after
crashes, retries, concurrent selection changes, and recovery. A sequence of
independent acknowledged writes does not meet this contract. The protocol may
use a different physical representation or explicit in-progress state, but
must preserve the stated read and replay behavior. This storage choice remains
open; the pure archive decision alone does not demonstrate it.

### Ninth typed rule slice: single-choice option administration

Each single-choice option has a stable identity, a display name, and an archive
flag in its field definition. The Rust `task_contract::field_admin` module
defines Add, Rename, Archive, and Restore commands against the expected project
configuration revision. Option IDs cannot be reused or removed from a retained
single-choice field, including by complete configuration replacement. An unused
field may instead change kind or be removed; its former choice options then
belong to the retained configuration history. Renaming keeps the identity
referenced by current and historical values. Archiving rejects
new assignments while retaining existing values; restoration makes the same
option eligible for new assignments again. Names must contain non-whitespace
text, contain no control characters, and fit within 256 UTF-8 bytes. Identity
is determined by ID, not name.

Administration requires authorization, an active project and field, and a
single-choice field kind. An identical recorded success replays before mutable
revision or definition checks. The accepted result is a complete next
configuration revision; B-005 must commit it with the operation record and
coordinate it with concurrent item writes. Pure tests cover stable identity,
archive/restore behavior, invalid names and duplicate IDs, and replay.

### Tenth typed rule slice: stable domain error codes

The Rust `task_contract::error_code` module maps every current pure-rule error
to a stable snake-case domain code. The typed error still carries details such
as the current version or affected item. Adapters must use the code rather than
infer a category from message text. Shared failures have the same code across
commands: `unauthorized`, `operation_id_reused`,
`configuration_conflict`, `item_version_conflict`, `invalid_reference`, and
`invalid_configuration`. Separate codes preserve meaningful distinctions such
as `selection_version_conflict`, `required_field`, `archived_option`,
`duplicate_relationship`, `self_link`, `unrelated_type_owner`,
`prohibited_phase_change`,
`migration_required`, `payload_limit_exceeded`, and `estimate_unit_locked`.

For commands with operation replay, authorization is checked first; an
identical recorded success then replays before current revisions or business
rules, and changed content under the same operation ID conflicts. A new
command checks its expected configuration revision before item version where
both exist. It then validates configuration and references before accepting a
new state. Individual commands define the remaining eligibility precedence in
their typed decisions; the domain code does not change that order. Transport
status mapping, localized messages, and disclosure of version details under
authorization remain adapter work. Pure tests cover representative shared,
nested, and distinct code mappings.

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

### Knowledge value contract

[D-024](decisions.md#d-024) fixes four manually entered entry kinds. Created
files carry a nonblank label and an opaque locator; the application neither
uploads nor resolves the referenced content as part of this command. Decisions,
lessons, and insights carry nonblank text. The entry identity, owning item,
kind, and version are explicit; version starts at 1. A value cannot be used
under the wrong kind. Conversion retains the same entry and prior versions.

The initial typed limits are 16 KiB of UTF-8 bytes for a text entry, 256 bytes
for a file label, and 2048 bytes for its locator. Labels and locators exclude
control characters. These are contract design limits, not separately accepted
product decisions. Create names a trusted allocated entry ID, current item
version, kind, and value. Edit names the entry and its expected independent
version; it preserves kind and item ownership. Both carry an operation ID and
authenticated actor identity. Reading and editing knowledge on the item are
required for a new mutation and for replay. Unauthorized comes first, then a
previously recorded identical success replays before mutable validation;
changed content under its operation ID conflicts. A new command checks
identity, archival state, and expected version. No-op edits fail.

Each successful create or edit yields the current entry and an ordered revision
history. Each revision retains version, author, and value; edit appends one
revision and never rewrites prior ones. The input history must be complete and
match the current value. B-005 must commit the entry, appended revision, and
success record atomically with item archival eligibility and unique entry ID.
The pure result carries the complete history for contract inspection; a storage
adapter may append only the new revision while preserving the same invariant.
No knowledge mutation changes an item's status, phase, usage, or version.

### Logical payload bounds

A complete project configuration revision contains at most 256 statuses, 256
status groups, 128
workflows, 128 WorkItem types, 512 fields, 128 relationship types, and 256
choice options per field. The total UTF-8 byte length of every supplied string
occurrence, including repeated ID references, is at most 1 MiB. New commands
over this bound return `payload_limit_exceeded`; a recorded success still
replays first. Adding an option beyond the per-field count uses the same error.
A knowledge create/edit command changes one entry. Its text or
file-reference limits are given above; it has no multi-entry payload. A
relationship-type administration command is a complete configuration revision
and uses the same bound. HTTP encoded-body limits and decoding budgets are
transport contracts for B-004/B-006 and may be stricter or use chunked
administration later; they cannot silently reduce this logical M1 contract.

Project metadata changes replace name, readable-ID prefix, and estimate unit
together under the same project configuration revision. A name is nonblank,
free of control characters, and at most 256 UTF-8 bytes. Prefix rules and the
monotonic estimate-unit lock apply. A no-op is rejected. Project and matching
Configuration revisions advance once; issued readable IDs stay unchanged.
The public pure metadata command returns both revised values; separate
Project-only prefix and unit mutators are not supported.
Current project configuration authorization is checked before replay. B-005
must persist both records, revision history, and the success record together.

### Follow-up origin contract

A follow-up origin is an immutable directed record from one source WorkItem to
one follow-up WorkItem. The follow-up has at most one origin; one source may
have many follow-ups. Both endpoints may belong to different projects. A new
origin requires current Link permission on each item, current versions for
both items and both project revisions, live endpoints, and a unique record ID.
The source cannot be its own follow-up, and the complete ancestry of the
source cannot contain the proposed follow-up. This keeps provenance acyclic.
It is independent of project-defined relationship types and never supplies a
scheduling dependency or status transition.

Current Link grants are checked before replay. Identical recorded success
replays before mutable endpoint, version, and ancestry checks; changed content
under the same operation ID conflicts. B-005 must atomically protect both
endpoints, the child's unique origin, complete ancestry, and the operation
record. Archival and conversion preserve the origin and endpoint identities;
restoring an item does not create or remove origins.

## Contract boundary and downstream implementation

B-003 defines logical identities, values, revisions, authorization inputs,
replay order, validation, results, and pure effects. The typed commands cover
item create/edit/status/migration/conversion, project metadata and archival,
configuration and choice-option administration, current-work selection,
relationship creation and type administration, knowledge create/edit, and
follow-up origin creation. Knowledge entries have no independent deletion or
archival command in M1; WorkItem archival preserves them. The current worker
does not implement this configurable model.

Every new command checks current authorization before recorded-success replay;
the same operation ID with different typed content conflicts. A recorded
identical success precedes mutable version, archive, schema, and payload checks.
For a new command, expected project configuration revision precedes dependent
item/entry versions where applicable. Identity/coherence errors, archived
targets, invalid values, and operation-specific restrictions then follow the
order implemented in each pure decision. Stable domain error codes distinguish
these outcomes; B-004/B-006 map them to transport responses without changing
their meanings. No rejected command may expose partial effects.

B-005 must demonstrate configuration/item/relationship/selection consistency,
unique IDs and canonical keys, history retention, operation replay, and restore
under real contention. B-007 enforces current grants, including cross-project
checks. B-004/B-006 own encoded HTTP request limits and decoding, while
preserving the accepted logical command bounds. These are implementation gates
for supported M1 behavior, not incomplete B-003 policy decisions.

## Acceptance scenarios

These scenarios state logical acceptance. The pure checks cover the listed
pure decisions; the named downstream issues own the service-level evidence.

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
| BC-19 | Record zero, omit an estimate, submit a negative estimate, and change units after clearing a recorded estimate; race the first estimate against a unit change. | Zero and absence differ; negative and post-use unit changes fail. Under contention, both a first estimate and a conflicting unit change cannot commit. | Pure estimate rules; B-005 authoritative contention check. |
| BC-20 | Convert an item and later expire its operation replay record. | Required history, including removed source-type values, remains available independently of replay retention. | Contract preservation plus B-005 retention/recovery checks. |
| BC-21 | Archive a project with active items and current-work selections, then restore it. | Clear selections; preserve readable items, links, and history; prohibit edits while archived; restore editability only for individually unarchived items. No lifecycle or resource effects. | Pure eligibility/effect rules plus B-005 coordinated persistence. |
| BC-22 | Issue an item ID, change the project prefix, then issue another ID. | The first issued ID remains stable and resolvable; the second uses the new prefix and next project sequence. | Pure allocation rule plus B-005 uniqueness and lookup checks. |
| BC-23 | Attempt to change a populated custom field from text to number. | Reject in-place kind change; allow an explicit new field and value migration while preserving old field history. | Pure configuration rule plus B-005 authoritative value check. |
| BC-24 | Configure each possible cross-phase edge and change status within one phase. | Accept only the five system edges when the workflow permits them; reject Done to New and any configured same-phase edge. Same-phase status changes remain valid. | Pure phase graph and workflow checks. |
| BC-25 | Create a cross-project link using a type owned by either endpoint project, then try a type owned by a third project. | Accept the eligible types; reject the unrelated owner. Equal local type IDs from different owner projects have distinct canonical identities. | Pure relationship identity plus B-005 endpoint integrity. |
| BC-26 | Attempt link creation without type-use permission or Link permission on either endpoint. | Reject each missing grant; type administration requires project configuration permission in the owner project. | Pure grant inputs; B-007 authoritative enforcement. |
| BC-27 | View one directed link from each endpoint, then view a symmetric link from each endpoint. | Directed views are outgoing/incoming respectively; symmetric views share one direction marker. Each view carries the same stable record ID and names the other endpoint; no inverse record is stored. | Pure projection; B-005 persistence and B-007 read access. |
| BC-28 | Make a field required while one applicable item lacks a valid value; separately change or remove a field used in history after its current value was cleared. | Reject the required change until items comply; reject kind reinterpretation or removal of a historically used definition. Rename, hide, and archive preserve existing values. | Pure snapshot rules; B-005 authoritative snapshot and concurrent-commit protection. |
| BC-29 | Submit 129 field entries or over 1 MiB of aggregate supplied text in a create, edit, or conversion command. | Reject with `payload_limit_exceeded`; 128 entries and exactly 1 MiB pass the aggregate check, subject to other field rules. Identical recorded success replays first. | Pure command checks; transport limits and durable replay in B-004/B-005. |
| BC-30 | Create a cross-project link while either endpoint project revision or item version changes, then retry a recorded success. | A new stale command identifies the changed project or item and is rejected. Identical success replays after current authorization; changed content under the same operation ID conflicts. No link may commit against an archived endpoint, changed type, or duplicate canonical key. | Pure command checks; B-005 atomic multi-record contention and B-007 current grants. |
| BC-31 | Archive an item selected by two users, or archive a project with selected and unrelated users, while a selection changes concurrently. | Produce versioned clears only for affected users. The archive, matching Project/Configuration state where applicable, all clears, and success record commit together; otherwise none commit. Replay never reapplies clears. | Pure plan checks; B-005 authoritative selection set, contention, and atomicity. |
| BC-32 | Submit a complete compatible configuration revision, retry it after later changes, then reuse its operation ID with changed content. | Apply one revision; replay the original result after current authorization; reject changed content. Reject an empty operation ID or a revision-only no-op. | Pure command checks; B-005 durable revision/history and operation record. |
| BC-33 | Create and edit each knowledge kind, including a labeled file reference. | Keep kind and WorkItem ownership; append attributed versions; reject wrong kind, stale version, and invalid values. | Pure knowledge checks; B-005 retention and B-007 grants. |
| BC-34 | Create a cross-project follow-up with and without Link grants; attempt a second origin or cycle. | Require both grants; accept one directed acyclic origin; reject duplicate origin and cycle. Preserve provenance through archive/conversion. | Pure provenance checks; B-005 atomic ancestry and B-007 grants. |
| BC-35 | Rename/archive a historically used relationship type, then change its direction or remove it. | Permit rename/archive; reject reinterpretation and removal even after current links migrate away. Historical-use evidence remains authoritative. | Pure configuration checks; B-005 historical-use evidence. |
| BC-36 | Exceed complete-configuration count or text budget; retry a recorded success. | Reject a new oversized command with `payload_limit_exceeded`; replay an identical recorded success first. | Pure configuration checks; B-004/B-006 encoded-body limits. |
| BC-37 | Change project name, prefix, and unit in one command; retry after another revision. | Advance Project and Configuration once; preserve issued IDs; enforce estimate-unit lock and successful replay. | Pure project metadata checks; B-005 atomic persistence. |

## Completion and handoff

The implemented typed rule slices have passing pure checks on the contract
branch; final review choices remain open as stated above.
`python scripts/verify.py --pure`, `cargo clippy --locked -p
task_contract --tests -- -D warnings`, and `cargo fmt --all -- --check`
passed after the 2026-10-01 contract review changes. The pure tests exercise
S-01 phase behavior and the contract scenarios above at their stated pure
boundary. They do not establish
real-service enforcement.

B-005 owns demonstrated commit-time configuration consistency, authoritative
reference checks, persistence, replay, and recovery. B-007 owns access enforcement.
B-005 also owns relationship record ID allocation and canonical-key uniqueness;
B-007 owns read authorization for endpoint-relative relationship views.
B-011 owns integrated resource effects. Keep those implementation gates visible
without claiming they have been satisfied by this contract review.

The currently deployed worker still uses the earlier Rust task mutation rules,
which require a nonempty title and accept only the status strings Open and Done.
The new `work_item` module does not yet change that worker behavior. Later
integration must replace those provisional validation assumptions while
retaining successful replay precedence.
