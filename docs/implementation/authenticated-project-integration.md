# Authenticated project creation: integration contract

Revision: 3, 2026-11-02. Status: Proposed design; no implementation or route
qualification claimed. Owning delivery: [#49](https://github.com/MrQuality/kehila/issues/49),
with authority [#40](https://github.com/MrQuality/kehila/issues/40) and allocation
[#41](https://github.com/MrQuality/kehila/issues/41).

## Outcome and authority

An owner signs in through the first-party browser on Windows, creates a project
with its trusted M1V1 configuration, then reloads it through an independently
authorized PostgreSQL read. An uncertain response is reconciled without creating
another project. Unauthorized callers cannot read, create or replay protected data.

On 2026-11-02 the maintainer explicitly selected a browser-first journey against
fresh native PostgreSQL, keeping prototype data separate, and authorized design
of this integration contract. That direction does not accept ADR-103 wholesale,
an allocation protocol, production dependencies, supported installation, migration,
heavy execution or certificate trust changes. Architectural acceptance and merging
remain separate maintainer decisions.

This design is the next step from the bounded owner-login delivery merged in
[#87](https://github.com/MrQuality/kehila/pull/87). It does not describe that
experiment as product authentication. The current anonymous Go/worker task path
does not implement this outcome.

Included: source adoption, owner/actor and capability binding, stable creation
identity, native persistence/replay, authoritative project/configuration read-back,
browser retry behavior, legacy isolation and an evidence/rollout map. Excluded:
legacy import, work-item commands, search, delegated administration UI, supported
installer, production compactor implementation and replicated deployment.
Exclusion from this design delivery does not waive their applicable rollout gates.

## Evidence and consequential premises

Assessment baseline: source `c95d9c010f3e10f84e8e4f264344cd7518fd8e59`.

| Premise | Status and source | Consequence / disposition |
| --- | --- | --- |
| Browser-first, fresh native database, prototype data isolated | Explicit maintainer design direction, 2026-11-02 | Plan that path; no migration design is inferred. |
| New native routes belong in the Rust authority | Accepted ADR-105 and D-016 native amendment | No forwarded actor header or anonymous worker/search alternative. |
| Native physical foundation is adopted for product service | Unresolved; ADR-103 remains Proposed | Obtain exact adoption/schema decision before dependent product implementation. |
| Creation has stable actor/family/target/token identity | Accepted D-032; allocation unresolved in SP-005/#41 | Review ADR-107 and verify fault cases before finalizing adapter interfaces. |
| Existing codecs/schema supply a complete adapter | Contradicted by W3/W7 and inspected source | Implement transaction, admission, reconstruction and startup ownership explicitly. |
| AUTH-001 evidence transfers unchanged to product source | Unsupported; isolated manifest and experimental modules | Review actual extracted graph, capabilities and changed integration boundaries. |
| Disposable container approval establishes supported installation | Contradicted by ADR-106 | Keep synthetic development and supported rollout gates separate. |
| Successful creation establishes permission to read or replay | Contradicted by ADR-104/105 and D-033 | Reauthorize both using their own current-authority contracts. |

The [successor plan](m1-01-remaining-delivery.md) retains W2-W7 requirements.
The [creation protocol](../architecture/M1-project-create-protocol.md) and
[SP-004](../spikes/SP-004-actor-authority.md) supply the retained transaction and
authority constraints. Proposed details below are not additional acceptance evidence.

## Inspected source and proposed ownership

| Existing source | Actual boundary | Proposed product ownership |
| --- | --- | --- |
| `src/pure/task_contract/src/project_create.rs` | Pure creation decision and trusted M1V1 seed; no authenticated caller or storage | Retain domain ownership; I/O adapters supply trusted facts. |
| `src/pure/project_create_codec/` | Versioned request/result/snapshot codecs and bounded duplicate-aware JSON parsing | Reuse directly without changing version-one representations. |
| `storage/postgresql/` | Fresh native baseline, owner/creator roles, actor/core locks and creation guards | Reviewed schema evolution owns auth, allocation and read/startup compatibility; no experiment installer at runtime. |
| `experiments/AUTH-001/src/{password,session,pre_session,secret}.rs` | Experimental credential and registry logic | Extract deliberately into product-owned authentication modules; keep domain identity independent of session representation. |
| AUTH-001 application/database/startup/owner-server modules | Linux auth coordinator, owned jobs/connections, protected startup and HTTPS | Product Rust authority owns these lifetimes and domain adapters; eliminate diagnostic-only serving modes. |
| `experiments/AUTH-001/storage/` | Disposable auth overlay already referencing `kehila.actors` | Review product migration and capability functions; a shared actor foreign key alone does not prove integrated permission correctness. |
| AUTH-001 browser handlers | Login/session-management UI | Product browser adds project form/read-back and durable pending-intent UX; credentials remain in protected cookies. |
| `go/io/task_api/server.go`, `src/io/task_worker/src/main.rs` | Anonymous legacy proxy/worker and direct search mediation | Remain isolated historical prototype; no supported fallback or shared authoritative dataset. |

No product dependency may import experimental source through a path dependency,
`include!` or runtime installer invocation. Copying the whole experiment and
changing its name would retain diagnostic modes and implicit assumptions.
Extract only required ownership units and carry relevant regression tests with them.
Exact crate names and file subdivision are implementation choices after contract review.

The isolated graph declares Rust 1.88; the root default is 1.75. Neither arbitrary
downgrading nor a workspace-wide bump is selected here. Review the actual product
package MSRV, feature/lockfile graph, licenses and advisories under the dependency
policy before adoption. Point-in-time experimental checks do not clear production
dependencies or the advisory work retained under #40.

## Schema, startup and privileges

Propose one dedicated fresh PostgreSQL 16 UTF8 database and role namespace for
this native installation. No reuse of populated prototype state, automatic import,
destructive reset or silent adoption of existing roles/schema is permitted.

Keep schema ownership NOLOGIN and unavailable to serving connections. Keep local
enrollment/recovery separate from serving, and retain the existing creator
capability's lack of actor administration, history mutation, payload deletion,
retirement and DDL. Authentication management uses narrowly reviewed functions;
it must not grant creation workers direct credential or permission mutation rights.
The actor bound at enrollment is the same stable actor used by project attribution.
Creation permission and initial current project grant are explicit stored facts.

The existing overlay uses `auth001_admin`/`auth001_runtime` roles and an `auth001`
schema; those experimental names and grants are not a product migration. The
integrated proposal must enumerate function owners, PUBLIC revocations, fixed
search paths, connection-role membership and exact effective privileges. If one
trusted process holds both auth and creation capabilities, document that combined
trust rather than claiming process isolation between its modules.

Before serving, verify schema/version compatibility and required capabilities,
owner binding, protected TLS/database assets and the single-active runtime fence.
Missing, mixed or unsupported state fails readiness closed. Runtime startup does
not run schema changes. A runtime login cannot repair its own privileges.
Order installation fencing before actor locking, as required by ADR-105; preserve
generation rechecks after waits and owned connection cleanup on cancellation.

ADR-103 remains Proposed. The next acceptance packet must identify the exact
baseline plus proposed auth/allocation/read schema changes, not ask whether
native PostgreSQL is desirable in general. Product SQL and function changes require
their own reviewed migration/version contract before implementation.

## Creation, replay and reads

The proposed allocation contract is [ADR-107](../architecture/ADR-107-project-creation-intents.md).
Its mapping resolves a browser's stable intent to the trusted target. It does
not replace the accepted complete operation lookup key or authorize any access.

1. Bound actual transport bytes and parse original JSON with ADR-104's duplicate,
   decoded string/key, depth, collection and node ceilings before per-request
   database admission or application-worker scheduling. Validate exact endpoint
   fields and reject caller actor/grant/seed assertions.
2. Select an explicit transport mode, creation or reconciliation. Reconciliation
   only resolves an existing intent and performs historical replay; it generates
   no prospective seed/envelopes and never falls through to creation. Parse its
   retained version-one field shapes without current business-domain validation.
   A missing mapping reports absence at the authorized lookup ordering point,
   not proof that another in-flight request cannot subsequently commit.
3. For creation mode only, construct the checked native command and trusted
   seed/result candidate, and admit request, saved-result and both snapshot envelopes
   before database admission. This is preparation, never authority or acceptance.
   Candidate randomness must use bounded owned work after edge admission. No
   caller-generated target or display prefix becomes trusted identity.
4. In either mode enter the ADR-105 installation/actor fence and recheck current credential,
   runtime/auth generations, actor active state and creation permission after waits.
   Resolve an existing intent before considering fresh persistence. For replay,
   use its stored target and full operation key, current original grants, stored
   codecs and exact typed intent. Never apply current domain rules to historical
   successful content. No Project lock is required merely to replay creation.
5. For a fresh intent in creation mode, use the prepared result only if authoritative checks agree.
   Commit allocation mapping, permanent operation, relational project/configuration,
   matching current grant/event, revision-one snapshots and saved result atomically.
   Reload typed state and compare with the single trusted result; force deferred
   constraints before commit. All errors roll back the entire action.
6. Sample the server operation timestamp near the persistence batch and preserve
   the accepted checked 90-day deadline. Return confirmed success after commit;
   commit/response uncertainty retains the same intent and operation identity.
   Complete-transaction retries stay inside one deadline; no arbitrary retry
   count or measured latency guarantee is established by this proposal.

Reconciliation must remain available even when the same form would fail current
fresh acceptance or generated-envelope admission. Creation mode may discard its
prepared candidate when a concurrent request already committed the intent, but
it is not the sole historical replay path. Transport mode is not part of the
canonical operation identity and changes no version-one binary request fields.
Existing-intent replay writes no new result envelope. A fresh candidate prepared
in creation mode is discarded when the stored target resolves differently.
Its limits cannot justify accepting corrupt/oversized stored data: bound database
materialization and use versioned historical decoders before releasing replay.
Any collision retry requiring a changed candidate must release/roll back database
work and re-admit all generated envelopes before new database admission.

Read-back is a separate protected operation. In one fresh consistent PostgreSQL
snapshot, check actor/auth/runtime generation, current project read grant and
current/latest-event agreement together with project/configuration data. Apply
the ADR-105 live-registry/revocation and monotonic-expiry check again before release.
Do not reuse the creation response or a cached successful credential verdict as
proof of current read permission. Revocation committed before the snapshot denies;
a later overlapping revocation has the accepted bounded-read ordering semantics.
No exclusive actor fence spans response delivery, and no search dependency is needed.

## Browser behavior

Serve the product page and API at one verified HTTPS origin. Retain independent
CSRF, strict origin/authority handling, protected cookies and no-store sensitive
responses. Initial project form fields are name, prefix and estimate unit; trusted
seed definitions, actor identity and permissions are never client inputs.

Before first transmission, persist a bounded pending-intent record containing its
handle, operation token and exact form values. Proposed storage is IndexedDB,
scoped to this origin and the authenticated installation/actor. Never store
passwords, cookies or CSRF secrets there. Local project metadata is sensitive
application data: bound retained pending entries and provide explicit discard.
Failure to persist blocks submission rather than silently losing retry identity.

Before selecting or creating a pending record, the product browser must obtain
an authenticated, stable installation/actor binding from the authority. The
current experimental state response reports only active/inactive and cannot
provide that binding. Its product representation may be opaque; raw IDs and a
particular endpoint are not required. Associate the binding coherently with the
same authenticated session and CSRF proof used for submission, and refresh that
association after reauthentication or session replacement. Missing or mismatched
binding blocks pending-intent submission. A binding is comparison metadata, never
a credential, permission grant or trusted caller assertion; the server still
derives actor identity and current authority independently. The bounded adapter
plan must define the discovery/proof contract before browser/API implementation.

If persistence fails before first transmission, show that creation was not sent
because its retry identity could not be saved, and offer retrying local persistence
after site storage becomes available. Never silently submit from volatile state.
This message cannot be used for an already transmitted uncertain intent: a later
local storage error does not establish that its earlier request failed to commit.

After a lost response or reload, show the unresolved intent and reconcile with
exactly the same identity/content after reauthentication when needed. An absent
result permits a separate explicit creation attempt with that identity, subject
to fresh preadmission and a repeated fenced lookup. A fresh-acceptance rejection
does not prove that a previous or concurrent attempt did not commit; retain the
pending record and offer reconciliation. Do not automatically
submit pending work at login, replace its token, or relabel an unknown outcome as
rollback. Deliberate new creation requires a new explicit user action. Discarding
local retry metadata cannot promise the original transaction did not commit.

Expose two distinct actions:

- **Reconcile or retry pending intent:** retain the immutable original handle,
  token and exact payload. Reconcile first after uncertainty; an absent result
  permits the separate creation attempt described above. Editing a separate
  form draft must not mutate the saved pending record.
- **Start a separate creation:** require explicit user action and a fresh handle
  and token. If the user discards an uncertain pending record, warn that its
  original project may already exist and that another submission may create a
  second project. Discard is not cancellation or proof of rollback.

Reject a different installation/actor binding; do not send an old owner's pending
intent using a new owner's authority. Same-origin scripts/XSS can access browser
metadata; local persistence is not a separate confidentiality boundary.

After confirmed creation, show authoritative read-back and its failure separately
from the already committed create result. English display is sufficient initially;
IDs, error codes, codecs and numeric values remain locale independent. Future
agents require their own credentials/delegation; the owner cookie is no agent API.

## Completion and rollout evidence

| Gate | Concrete required evidence | Current disposition |
| --- | --- | --- |
| Design adoption | Exact ADR-103/schema disposition, ADR-107 decision, reviewed product graph/MSRV and bounded adapter plan | Pending; this PR supplies a proposal only. |
| Allocation #41 | Lost responses before/after commit, concurrent identical/changed intents, actor isolation, target collision, tombstone/restore integrity | Planned; no SP-005 run claimed. |
| Ingress | Actual reads with misleading lengths, duplicates/escaped keys and every codec limit; zero pool/application-work admission on rejection; generated-envelope overflow before DB | Required before route exposure. |
| Authority #40 | Missing/spoofed credentials, wrong scope, current revocation, creation versus replay grants, read snapshot races and post-query expiry | Required on real integrated paths. |
| Browser binding/lifecycle | Authenticated stable binding associated with session/CSRF, changed-binding rejection, immutable retry versus separate creation, visible pre-transmission persistence failure and retained uncertainty after transmission | Contract before browser/API implementation; actual browser cases before qualification. |
| Persistence #49 | Actual-role privileges/startup rejection; fault at each write boundary; full typed reload and frozen codec oracles; concurrent/uncertain commit reconciliation | Required; experiment evidence is scoped regression input. |
| Replay/retirement/recovery | Logical 90-day expiry, permanent no-reexecution identity, gap-free retirement and restored allocation/core/tombstone coherence | Retained W4/W7/D-034 route-exposure gates; no compactor or restore completion inferred. |
| Development journey | Windows browser -> verified TLS -> Linux container authority -> native create -> fresh authorized read, with all source/profile identities recorded | Synthetic disposable data only; execute only with resource/trust permissions. |
| Supported rollout | Explicit supported networking/secrets/startup/interrupt policy; no anonymous alternate route; applicable restart/restore/durability and operating limits | ADR-106 alone is insufficient; installer/real-data gates remain unfulfilled. |

Resource preparation must retain bounded HTTP, hashing, registry, database,
command/read and cleanup ownership. The adapter plan must state aggregate pool
and concurrency limits, maximum materialized stored/response records, clock/lock/
statement/transaction deadlines and cancellation outcomes before execution. Do
not multiply existing per-module limits into an unreviewed total. Admission rejects
excess work; permits remain owned until blocking work actually finishes. Measure
the complete integrated workload rather than transferring isolated auth timings.

Order work as contract/adoption, offline implementation and harness review,
automated relevant checks, then one prepared final user-assisted campaign. Retain
valid evidence; rerun only affected cases and controls. This ordering grants no
new resource or trust permission and skips no mandatory commit/CI gate.

The operation is not complete merely because this design merges or a synthetic
happy path passes. Keep the applicable W4/W7 exposure gates above until evidence
supports the actual delivery claim. #40/#41/#49 stay open for their full scope.
Reassess PR boundaries when allocation, schema or recovery discoveries introduce
substantial independent work; keep coupled transaction/runtime/client work coherent.

## Framing and review record

The early separate-context validator received the original integration request
and inspected ADR-103 through ADR-106, product/replay records, SP-005 and actual
source before receiving the candidate. It preserved Phase A, then supported
assessment/design only. A second pass incorporated the explicit browser/fresh-DB
design decision. Material dispositions: allocation remains undecided; read authority
is separate; product graph review is required; ADR-106 does not authorize supported
installation; restore/retirement remain rollout gates. These constraints are carried
above. The author subsequently read live #41/#49 and confirmed their open scope;
historical progress text is not a new acceptance decision.

The [PR #88 follow-up reminder](https://github.com/MrQuality/kehila/pull/88#issuecomment-6569113657)
records the maintainer-authorized contract clarifications: authenticated browser
binding discovery, explicit pending/separate creation actions, visible persistence
failure and token integrity in the future schema. Revision 3 incorporates these
without accepting an endpoint, schema, runtime implementation or either Proposed ADR.

Before publication, record the final candidate/revision checked, validator findings
and dispositions, actual documentation checks, and exact-head technical review in
the owning PR. Revalidate at adoption, changed actor/data/profile boundaries and
final publication. Agent review establishes neither maintainer architectural
acceptance nor permission to merge.
