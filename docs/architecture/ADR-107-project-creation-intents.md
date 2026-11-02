# ADR-107 - Stable project creation intents

Status: Accepted, decision revision 3. Owner: project maintainer. Accepted: 2026-11-02.
Acceptance: [explicit maintainer decision](https://github.com/MrQuality/kehila/issues/49#issuecomment-6573495351),
identifying revision 3 at source `5345c9b0eaf51935786d17626ea83245b5f5ab98`.
Scope: allocation/reconciliation for the bounded fresh-database local integration.
Physical schema, adapter/dependency review, native evidence and execution permissions
remain separate. This metadata records acceptance, not a new protocol revision.
Related: D-029/D-032/D-033/D-034; ADR-103/104/105;
[#41](https://github.com/MrQuality/kehila/issues/41),
[#49](https://github.com/MrQuality/kehila/issues/49),
[integration contract](../implementation/authenticated-project-integration.md).

## Context

ProjectCreate requires a trusted ProjectId, but a browser losing the first
response may never learn that ID. The permanent operation key includes actor,
family, target and token. Generating a new target on retry can create another
project while bypassing the original lookup. SP-005 remains planned and supplies
no execution evidence. This record now supplies the accepted mechanism.

The maintainer selected browser-first integration with fresh native PostgreSQL
and isolated prototype data on 2026-11-02, then explicitly accepted this allocation
contract and ADR-103 for that outcome. The version-one command codec is unchanged.

## Decision

Use a browser-generated creation-intent handle as a locator, separate from the
existing operation token. The backend owns target allocation. Store the mapping
from authenticated actor + ProjectCreate family + intent handle to trusted
ProjectId + operation token + permanent operation core in the same transaction
as successful project creation. No standalone reservation endpoint or independently
committed reservation is introduced.

The handle and operation token are independently generated opaque values, each
128 random bits encoded as 32 lowercase hexadecimal ASCII characters. These
proposed wire bounds fit existing operation-token bounds. Randomness and shape
are not authentication. The browser persists both and exact form content before
first transmission. A changed installation/owner binding cannot reuse the record.
The proposed request carries these two fields, name, prefix and estimate unit;
it accepts no caller ProjectId, actor, seed or grant assertion. Endpoint paths
and response/error representations remain part of the reviewed adapter/API plan.

Use explicit creation and reconciliation transport modes. Both require bounded
original-byte edge admission and independent current caller authorization.
Reconciliation uses retained version-one field shapes and historical request
decoding, without current business validation, seed generation or prospective
envelopes. It can only resolve/replay an existing intent. Missing mapping returns
absence at the protected lookup's ordering point; it cannot fall through to
fresh creation or promise another in-flight request will not commit later.

Creation mode prepares a fresh random 128-bit ProjectId candidate in the same
canonical textual shape after edge admission. Construct and admit the prospective
command and all generated persistence envelopes before any per-request database
admission, as ADR-104 requires. Preparation is not a grant or accepted command.
It may be discarded if the intent already has a target. A fresh candidate's
failure cannot block the separate historical reconciliation mode. Transport mode
does not enter the existing binary request codec or change operation identity.

In the transaction:

1. Acquire installation and actor fences in ADR-105 order and check current
   credential/generations, actor active state and creation/replay authority.
   No mapping lookup may disclose a target before current authority succeeds.
2. Look up the intent only within that authenticated actor and command family.
   An existing mapping must have the same operation token. Resolve its target
   and perform the complete accepted actor/family/target/token operation lookup,
   with current original-grant authorization and stored-version exact-content
   comparison. A different token is an intent conflict, never permission to
   replace the binding. Changed content conflicts; expired equal content returns
   `replay_expired` and cannot execute again.
3. For a missing mapping in creation mode, verify the prepared target is unused and run current
   pure acceptance against authoritative facts. Persist the mapping and complete
   creation result atomically. Use uniqueness constraints as backstops. Commit
   only if all typed reconstruction and deferred constraints succeed.
4. On uncertain commit/response, reconcile the same handle, token and content. A
   committed mapping recovers the original target; rollback left no allocation.
   A mapping whose core is missing or whose tuple disagrees is storage corruption,
   not an invitation to create again.

After an absent reconciliation, creation is a separate attempt with the same
handle/token/content, fresh preadmission and repeated fenced lookup. Current
fresh-acceptance rejection cannot establish whether an earlier or concurrent
attempt committed. Retain uncertain intent metadata and allow reconciliation;
never replace identity just to bypass that rejection. An integration test must
change fresh domain/seed acceptance and show historical reconciliation still
returns the accepted success independently.

The immutable mapping needs a unique actor/family/handle key, a unique target
binding and an enforceable link to the matching permanent operation tuple/core.
It must not point at another actor, family, target or token. Physical SQL/indexes,
schema version and restricted grants require review before implementation. The
serving capability may insert/read it as part of creation, never update/delete it.
Do not grant extra actor or operation UPDATE privileges to implement its lookup.

The existing native five-column attribution foreign key covers operation core ID,
family, target kind, project and actor, but excludes the token. Reusing it alone
cannot prove equality of a separately stored mapping token. The reviewed schema
must either enforce token agreement with the immutable core through an appropriate
constraint, or derive the token from that referenced core instead of storing an
independently inconsistent copy. This is an integrity obligation, not selection
of physical SQL; verify mismatched-token rejection or impossibility as the actual
serving role before schema adoption.

The actor fence serializes same-actor cooperating requests. A target collision
across actors rolls back the entire transaction and retries with a fresh candidate
only within the reviewed request budget, after generated-envelope admission again.
A deliberate collision never adopts the other project or saved result. Constraint
failures must be classified by named constraint; unexpected integrity failures
are not blindly retried. No retry count or collision test has yet been qualified.

## Identity and lifetime

The locator supplements the operation key; it does not replace target scoping
or make tokens globally unique per actor. The same operation token with a distinct
intent/target remains a different scoped operation. The same handle/token cannot
be repurposed after failure uncertainty, expiry, compaction or restart.

Retain the successful mapping permanently with the operation core, including
after payload retirement. Preserve both in backup/restore. Do not expire a mapping
after 90 days or recycle its handle, since doing so could admit duplicate execution.
Allocation adds no permanent rows for rejected or rolled-back attempts; successful
operation storage still grows permanently. Capacity exhaustion rejects before
commit and cannot reclaim identity by silently deleting mappings.

Browser persistence contains project form metadata but no authentication secrets.
Bound outstanding local records and require explicit new creation after discard;
discard cannot establish that the original action rolled back. No automatic retry
under a newly authenticated different actor is permitted.

## Alternatives

| Alternative | Disposition for this proposal |
| --- | --- |
| Browser supplies arbitrary ProjectId | Does not establish trusted target ownership; collisions and adoption still need an allocation contract. |
| Generate a target for every received request | Fails lost-response reconciliation because retries can evade the original operation key. |
| Use operation token alone as target locator | Adds actor/family-wide token uniqueness beyond the accepted target-scoped key; avoid changing that shared contract implicitly. |
| Separate durable reservation endpoint | Viable if reservation must precede creation; adds abandoned-reservation lifetime, extra request/reconciliation and privilege contracts unnecessary for this atomic creation proposal. |
| Deterministically derive target from a handle | Requires a reviewed namespace, collision and possibly secret/key-version lifecycle; does not eliminate actor/operation authorization or permanent identity obligations. |
| Return the new target before durable creation | A lost response still loses the target; a returned target does not prove commit. |

## Consequences

One successful creation transaction owns allocation and project acceptance. The
browser can retry before learning its target without changing the existing binary
codec. In exchange, a new immutable locator and browser persistence protocol need
schema/API review and native failure tests. No new external service is required.

The serving backend remains trusted to attribute accepted writes. The mapping
does not protect against schema owners/administrators or supply tenancy support.
Current owner identity, original replay grants and independent read permission
remain authoritative. Future agents need their own actor/credential enrollment,
not access to the owner's pending-intent storage.

Recovery must restore mappings, cores, payloads/tombstones and project attribution
coherently while invalidating credentials under ADR-105. An older restored backup
can lack a later acknowledged intent: permanent mapping in the restored database
does not detect that external history. Supported restore/stale-client reconciliation
must resolve that case before real data; this proposal makes no cross-restore
exactly-once claim. Identifiers and codes are locale independent; display and
translation do not change identity or canonical request content.

## Required validation and acceptance gate

SP-005/#41 must verify actual-role native constraints, atomicity and reconstruction,
lost responses before and after commit, concurrent identical/changed requests,
different actors/tokens/handles, controlled target collisions, uncertain commit,
restart, logical expiry, retirement and restored mapping/core integrity. Observe
lock barriers and final authoritative state, rather than infer commit from HTTP.
Edge tests must prove rejected input schedules no application work or database
admission; all generated envelopes must be admitted before fresh persistence work.

Review browser persistence failure/reload and reauthentication without silent
identity replacement. Review same-origin concurrent tabs, bounded local storage,
and duplicate UI submission separately from server-side concurrency correctness.
Use the integration contract's authenticated binding-discovery and immutable
pending-intent lifecycle requirements; raw identity metadata cannot authorize
requests. Test visible pre-transmission persistence failure without treating a
later storage error as proof an earlier uncertain request did not commit.

No experiment, native qualification, schema migration or endpoint implementation
is delivered here. The next bounded experiment plan must state harnesses,
resource limits, fault barriers and stopping conditions before execution approval.
The explicit acceptance above identifies revision 3; it completes neither #41
nor production route readiness. A merged proposal was not its acceptance source.
