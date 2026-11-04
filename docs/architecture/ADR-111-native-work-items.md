# ADR-111 - Native WorkItem commands and persistence

Status: Proposed, revision 2. Owner: project maintainer. Created: 2026-11-04.
Acceptance: pending; selection of the user outcome does not accept this design.
Related: R-001/002/006; D-019/021/023/032/033/034/037; B-003/004/005/006/007;
[#27](https://github.com/MrQuality/kehila/issues/27),
[#40](https://github.com/MrQuality/kehila/issues/40),
[#49](https://github.com/MrQuality/kehila/issues/49);
[delivery brief and source-first review](../implementation/native-work-items.md).

## Context

The selected outcome adds authenticated Task/Milestone create/read/list and
ordinary text/estimate editing to the native project authority. Accepted pure
rules already define the behavior. The product schema, operation slice guards,
effective serving profile and reader remain creation-only. Item commands also
need trusted identity before an uncertain response, explicit current grants and
atomic project/item/history/replay persistence.

This proposal applies only to a fresh synthetic development database. Existing
project creation codecs and ADR-107 behavior stay unchanged. ADR-108 upgrade and
ADR-109 delivery designs remain Proposed; no events or populated upgrades enter
this outcome. The exact SQL, codecs and harness need review after this decision.

## Proposed decision

### 1. Immutable recoverable item commands

Add an owner/project-scoped pending command lifecycle limited to ItemCreate and
ItemEdit. It is a separate product contract, not implicit adoption of generic
drafts under ADR-107. Save a command before explicit execution. Registration
commits immutable versioned content and independently generated 128-bit public
request locator and operation token. For creation, the server also allocates a
stable random item target in that registration; no item or readable sequence is
issued yet. Edit targets an existing authorized item. Never trust a caller's
actor, creation target, grants or supplied seed/configuration as authority.

The pending identity belongs to installation, owner, project and family. Another
authenticated session can list/open it. Registration response loss can leave a
discoverable command; submitting equal content again creates a distinct command,
never an implicit retry. The interface exposes uncertainty and discovery before
offering another registration. Login, reload, list/open and reconciliation do
not execute anything. Editing saved content requires a separate explicit command.

Prepared/completed/cancelled/expired states use the same serialization as item
execution. Cancellation committed first prevents execution; completed edits and
creates are reported, never undone. Missing or expired requests cannot recreate
their identity. Completion, the permanent operation binding and domain writes
commit together. Terminal content can be pruned under a bounded pending policy;
successful installation/owner/project/family/public-locator/target/token/core
binding remains permanent independently. After replay expiry, opening a
completed locator identifies its terminal state and item without disclosing an
expired saved result; authorized current item read is a separate operation.
Successful identity remains discoverable through bounded terminal summaries,
independently of pending-content pruning. Cancelled/never-accepted requests may
be pruned, but a missing locator can never execute or reconstruct a target.
Registration enforces unique generated locators and creation targets across
pending and existing items; collisions retry before registration commits.
An immutable registered target can never adopt another pending or existing item.
This is not a background queue, generic draft store or automatic conflict rebase.

### 2. Current authority and grants

For this one-owner profile, an active coherent `initial_owner` project grant
confers project Read, CreateItem and EditItem. Creation permission on the actor
alone does not confer these rights. Independent project-grant/latest-event
agreement and active actor/runtime/auth/session checks are mandatory. Existing
project creation replay still requires its original project-create permission.

Item operation cores retain distinct `item_create` or `item_edit` required grants,
policy version one and project/item targets. Replay rechecks that original
mutation permission, not read permission alone. Historical scope never derives
from mutable current item content. Registration/open/cancel/reconcile require
the command family's current project mutation authority; item/list reads require
Read. Unauthorized lookups disclose neither saved content nor conflict detail.

Mutations retain installation fence then actor fence, followed by a project row
lock, pending-command row and existing operation-core share lock, then item row
where applicable. Acquire no earlier lock after a later one. Re-read grants,
project/configuration and item after waits. All future project configuration and
grant writers must use compatible fences before their routes are exposed.
Retirement takes only core/payload locks and never later acquires project/actor
locks. Retain the single authority, shared bounded serving connection/permits and
original request deadline; no new independent queue/pool or multi-owner promise.

### 3. Two-stage admission for state-dependent results

Propose a narrow amendment to ADR-104 Contract 1 for these two WorkItem families.
Its original-byte byte/string/depth/node/collection/duplicate checks remain
mandatory before any database admission or application scheduling. Authentication,
CSRF/origin and closed endpoint shape remain independent controls.

Unlike trusted project seeding, an item result depends on current database state.
After edge admission, allow a bounded authorized read-only preparation snapshot
to materialize the saved immutable command and required project/configuration/
item state. First resolve its permanent operation binding with current original
mutation grants. Bound stored request/result materialization and historical
decoding before releasing any data, including on replay and rejection paths.
A completed success uses stored-version historical replay,
exact request comparison and expiry before any mutable domain decision or new
result generation. Logical expiry/retirement never falls through to creation.
Only a truly unseen executable request proceeds to fresh-state preparation.
This precedence applies to Execute as well as Reconcile; a separate reconciliation
endpoint is not a remedy for incorrect Execute replay ordering.
Preparation cannot create an operation, allocate a readable number or update
domain/pending state. Database materialization has byte/count sentinels, shared
capacity, cancellation and the original deadline. No lock survives preparation.

Run pure decisions and encode/admit the exact request, result and item-history
envelopes outside the database transaction before write admission. Reject NUL,
codec/JSONB expansion or domain failure for an unseen command without a write
transaction; that observation is ordered at its preparation snapshot, not a claim
that another in-flight execution cannot commit afterward. Reconcile
uses historical codecs and never needs current-domain preparation. On write
entry, reauthorize and look up completed identity before mutable comparisons;
compare the complete trusted dependency set to fresh protected state. A changed
configuration or item version conflicts. Changes to allocation/estimate project
state (`next_sequence` or monotonic `ever_estimated`) require full
rollback, fresh bounded preparation and full lookup reentry under the same
immutable command identity and deadline; never continue a failed transaction.
No arbitrary unlimited retry or automatic semantic rebase is permitted.
An already true `ever_estimated` can never be overwritten from a stale prepared
result. Every successful/replayed response retains final live-session and
expiry/revocation eligibility checks.

Creation-only routes retain their existing pre-database generated-envelope
admission. The exception permits only bounded read preparation for item commands;
generated overflow must still produce zero mutation/write-transaction admission.
Raw malformed/oversized ingress must produce zero database admission of any kind.
This boundary change requires explicit maintainer acceptance before implementation.

### 4. Fresh storage and capability evolution

Install a fresh product schema version three, preserving the shared version-one
foundation and version-two history. The version-three installer is not a migration
from populated version two. Version-three runtime readiness rejects missing,
mixed, earlier or unknown product profiles; no runtime schema repair.

Represent stable item identity and issued prefix/sequence separately from an
append-only sequence of item revisions and the current revision pointer. Revision
one and each edit retain full typed item data, estimate/unit, configuration
revision, actor and attributed operation identity. Preserve collection order.
Reference type/workflow/status/fields in their owning project/configuration.
Record historical usage independently of replay payload retention, including
fields later cleared. New edits cannot discard prior values/history.

Create atomically commits item identity/version one, issued number, project
next_sequence/monotonic ever_estimated, item history, permanent operation/payload
and command completion. Edit atomically advances item version, appends history,
sets ever_estimated if necessary and commits operation/payload/completion. Pure
domain output is the one source of accepted state; reload typed state and compare
before forcing deferred constraints and COMMIT. All errors roll back the action.

Add narrow owner-defined functions to lock the project and advance its sequence/
estimate flag, advance an item head and manage pending command transitions.
Functions use fixed trusted search paths, explicit PUBLIC revocation, exact
generation/grant checks and expected before-state. Serving has no broad UPDATE,
DELETE, TRUNCATE, actor administration, retirement or DDL privileges. New INSERT/
SELECT privileges and function signatures are enumerated in the schema review;
transition guards and deferred attribution enforce coherence at transaction end.
Update exact effective-privilege/startup checks and negative role tests together.

Creation snapshots and saved creation results remain immutable. The current
project reader must stop requiring mutable next_sequence/ever_estimated to equal
their creation-time values. Instead verify stable metadata/configuration against
the retained revision snapshot and verify mutable allocation/estimate state
against attributed item-history evidence. Readiness and targeted corruption
tests must detect backwards sequence, recycled issued numbers, false unit unlock
and missing history. Do not simply remove the equality check or edit old history.
Use maintained attributed current-state evidence and indexed bounded reads, not
an unbounded scan of all item history on every project read. The exact schema
review must specify which aggregate/head attribution and constraints establish
this agreement and the corruption cases they can detect; a cached value alone
is insufficient evidence of agreement.

### 5. Representation, replay and read/list behavior

Freeze separate version-one ItemCreate/ItemEdit exact-request encodings,
fingerprints and saved-result/item-revision envelopes. Preserve expected versions,
ordered sparse actions, exact strings and canonical decimal u64 values; estimate
thousandths use exact integer/string representations, never floating point.
Historical decode does not invoke current domain acceptance. Existing project
codecs remain byte/shape compatible. Reuse checked JSON admission primitives.

Retain D-034's immutable 90-day server-origin replay window, current original
grants, permanent operation fingerprints and no-gap payload retirement. Product
item history is separate from replay and survives retirement. Logical expiry
denies replay even before physical deletion; equal expired or retired requests
never execute. Retired changed requests still conflict. Production compactor
implementation and supported backup/restore remain separate; native owner-only
retirement and restored-identity fixtures qualify the bounded route semantics.

Individual reads and lists use a fresh coherent authorized snapshot and final
live session/monotonic-expiry check before response release. Project lists here
are WorkItems of one authorized project: keyset pagination by issued sequence,
fixed bounded page size and bounded summary fields. They are not search or a
cross-page stable snapshot. Full text/field payloads belong to bounded individual
reads. Cursors carry no authority; each page reauthorizes. Derive phase from the
referenced status and display title from configured fields, falling back to the
issued readable ID. Sparse editing changes neither status nor type/workflow.
Pending quota/page/TTL and list-summary limits are explicit containment policy,
not calibrated capacity or latency guarantees. Exact codec review must demonstrate
actual request/result/revision and PostgreSQL JSONB expansion bounds; the logical
1 MiB domain text allowance is not proof that a generated envelope fits 1 MiB.

## Alternatives

- Browser-held operation/creation identities with success-only server mappings
  reduce pending storage but do not recover a command after browser state loss.
  They also differ from the existing server-recovery product direction. No
  broader browser-persistence acceptance is inferred.
- Echoing complete project/configuration/item preparation state from the client
  can retain ADR-104's strict generated-before-any-database rule, but requires a
  second untrusted state protocol, potentially large payloads and stale-state
  reentry. It remains an alternative if the narrow preparation exception is
  rejected; caller snapshots can never confer authority.
- Running pure acceptance/encoding inside the write transaction is simpler but
  consumes fenced database work before generated-envelope admission. Rejected
  here in favor of bounded read preparation plus pre-write admission.
- Broad serving UPDATE or rewriting creation snapshots would simplify writes
  while weakening existing privilege/history guarantees. Neither is proposed.

## Consequences and impacts

- Security: new item authority and narrowly expanded read preparation are real
  changes. Exact functions/profile and new denial-of-service surfaces need
  review. One process still holds combined auth/project/item trust.
- Performance: mutations serialize by owner/project in this local profile.
  Preparation can repeat on contention within the original deadline. No throughput
  or measured capacity guarantee follows; measure relevant limits before claims.
- Operations: pending discovery distinguishes unknown from rejected. Cancellation
  joins owned work; uncertain COMMIT is reconciled with original identity. Preserve
  abnormal shutdown reporting and independent final resource observations.
- Migration/reversibility: fresh databases only. Keep previous source/history;
  reverting source cannot serve version-three data with a version-two runtime.
  No destructive downgrade or automatic import is supplied.
- Recovery: permanent identity, complete item history and issued numbers are
  authoritative state. Synthetic restart/retirement/copy evidence does not qualify
  supported backups or broader post-MVP disaster recovery.
- Languages/RTL: stable IDs and canonical exact values remain independent of
  localized display; preserve Unicode text and do not use display labels as keys.
- Agent execution: retain stable authenticated actor attribution independently
  of sessions. No machine credentials, delegation or multi-owner execution added.

## Verification and follow-up

The [delivery matrix](../implementation/native-work-items.md#verification-matrix)
defines nominal, boundary, authorization, contention, uncertainty, history,
retirement, admission and lifecycle evidence. Source-first review is required
before implementation; exact SQL/codecs and harness before execution. Existing
project/authentication checks remain regression controls. Required gates and
final exact-head composition review remain; no runtime evidence is claimed yet.
