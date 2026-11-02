# Authenticated project creation: integration contract

Revision: 5, 2026-11-03. Status: implementation authorized; integrated qualification pending;
ADR-103 and ADR-107 are Accepted for the bounded outcome below. No implementation or route
qualification claimed. Owning delivery: [#49](https://github.com/MrQuality/kehila/issues/49),
with authority [#40](https://github.com/MrQuality/kehila/issues/40) and allocation
[#41](https://github.com/MrQuality/kehila/issues/41).

## Outcome and authority

**Current decision, 2026-11-03:** implementation of the integrated outcome is
authorized, including server-owned pending creations, explicit execution and
confirmed cancellation under ADR-107 revision 4. The
[product authority contract](authenticated-project-authority.md) owns the concrete
implementation and evidence plan. Browser-local storage is not authoritative
recovery. Local browsers are excluded from the current resource profile; the
Windows qualification gate is retained while external verification is assessed.

**Preceding architectural decision, 2026-11-02:** the maintainer explicitly
[accepted ADR-103 and ADR-107 revision 3](https://github.com/MrQuality/kehila/issues/49#issuecomment-6573495351)
at source `5345c9b0eaf51935786d17626ea83245b5f5ab98` for this fresh-database,
Windows-browser creation/read-back outcome. This supersedes their earlier pending
labels in dated framing records below. #41 physical allocation/reconciliation
verification precedes adapters. Lifecycle-event consumers, populated upgrades
and online migrations are excluded from the first journey; ADR-108/109 remain
Proposed. Exact schema/dependency/implementation reviews, heavy execution, trust
changes, retirement/restore and route/real-data qualification remain separate.

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
| Native physical foundation | ADR-103 Accepted for the bounded integration, 2026-11-02 | Exact auth/allocation/read schema and dependency implementation review remain. |
| Creation has stable actor/family/target/token identity | Accepted D-032 and ADR-107, amended by server-owned pending revision 4 | Verify pending lifecycle, physical binding and fault cases under #41 before finalizing adapters. |
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

ADR-103 is Accepted for the bounded integration. The next implementation packet must identify the exact
baseline plus proposed auth/allocation/read schema changes, not ask whether
native PostgreSQL is desirable in general. Product SQL and function changes require
their own reviewed migration/version contract before implementation.

## Creation, replay and reads

The accepted allocation contract is [ADR-107](../architecture/ADR-107-project-creation-intents.md).
Its mapping resolves a browser's stable intent to the trusted target. It does
not replace the accepted complete operation lookup key or authorize any access.
Execution requires an existing server-owned pending entry and exact immutable
content agreement. The following creation transaction is not a registration
endpoint: a missing/cancelled/expired entry cannot fall through to fresh creation.

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
   Resolve the pending entry, require an executable state and exact immutable
   content agreement before considering fresh persistence. For replay,
   use its stored target and full operation key, current original grants, stored
   codecs and exact typed intent. Never apply current domain rules to historical
   successful content. No Project lock is required merely to replay creation.
5. For a fresh intent in creation mode, use the prepared result only if authoritative checks agree.
   Commit pending completion reference, allocation mapping, permanent operation,
   relational project/configuration,
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

The 2026-11-03 accepted ADR-107 amendment replaces browser-persistent recovery
with server-owned pending creation. See the [product authority contract](authenticated-project-authority.md)
for implementation/review gates. The browser interface remains the intended client,
but local browser execution is excluded from the selected verification profile.


Serve the product page and API at one verified HTTPS origin. Retain independent
CSRF, strict origin/authority handling, protected cookies and no-store sensitive
responses. Initial project form fields are name, prefix and estimate unit; trusted
seed definitions, actor identity and permissions are never client inputs.

Before execution, save immutable versioned form values and server-generated
identity in an authenticated server-owned pending creation. Saving creates no
project. Current authorization applies independently to registration, paginated
list/open, explicit execution, reconciliation and cancellation. Another browser
can discover/open the same entry after reauthentication; IndexedDB is not required.
Nothing executes on login, reload or GET. Distinct registrations of equal values
remain distinct entries. The server cannot recover content it never received.

Execution echoes the opened immutable values solely as untrusted input for
original-byte and generated-envelope admission before execution database/worker
work. Fresh fenced authorization and exact saved-content comparison remain
mandatory. The echo never overrides authoritative identity/content. Historical
reconciliation needs no prospective current-domain acceptance and cannot fall
through to creation. Missing/cancelled/expired entries cannot be reconstructed
by delayed execution; changed values require a separate explicit registration.

Cancellation and execution share serialization. A committed cancellation prevents
later execution; a committed project cannot be undone by cancellation. Response
loss retains uncertainty until server reconciliation. Pending quotas, pagination,
terminal retention and payload handling must be bounded separately from permanent
successful operation/mapping identity. A browser record or response is not a
permission grant. Owner/installation binding is derived from current authority.

After confirmed creation, show authoritative read-back and its failure separately
from the already committed create result. English display is sufficient initially;
IDs, error codes, codecs and numeric values remain locale independent. Future
agents require their own credentials/delegation; the owner cookie is no agent API.

## Native adoption questions in progress

On 2026-11-02 the maintainer authorized an initial draft addressing the following
questions and explicitly left its scope unfinished for further discussion. This
section historically extended the adoption assessment without accepting those
ADRs. The later explicit decision above accepts only ADR-103/107 for the bounded
outcome. Its original source baseline is merge `69115ca805c2997c89476c7dd5153fa21b715ee9`.

| Question | Retained contract / initial proposal | Concrete completion evidence and remaining decision |
| --- | --- | --- |
| Concurrent initialization under contention | Shared installation fence before exclusive actor fence; fresh authority and intent lookup after waits; operation-core protection for replay; target uniqueness across actors. Retry only classified transient failures as whole transactions after safe rollback, within one budget; never retry authorization, changed-intent or unexpected integrity failures as success candidates. | Observe same-intent and changed-intent races, cross-actor target collisions, revoke/retire contention, lock timeout/deadlock and cleanup. Select aggregate admission, maximum inflight/waiting work, total request/lock/statement deadlines and retry budget from measured cases; no throughput claim yet. |
| Constraints change while older workers remain live | Recommend maintenance-only compatibility under Proposed ADR-108. A startup version check cannot exclude already-running writers. | Define and test enforceable migration/writer exclusion and failure recovery before populated upgrade support. Schema version, compiled domain rules, API version and stored codec versions have separate compatibility obligations. |
| Commit succeeds but NATS is unavailable | PostgreSQL success remains success. Proposed ADR-109 compares durable outbox handoff with CDC; no native event publisher is delivered. | Event consumers are excluded from the accepted first journey; select handoff and verify commit/publish crash gaps, duplicates, backlog admission and restore before later dependent features. |
| Timed-out creation is retried during/after commit | Accepted ADR-107 retains handle/token/exact payload, current authorization, repeated fenced resolution and historical-only reconciliation. Client timeout and absent-at-lookup do not prove rollback. | Barrier-controlled inflight retry, committed-response loss, rollback, collision re-entry, changed content, expiry and restore cases under #41/#49. No new identity on uncertainty. |
| Initial snapshots cause latency or storage growth | Generate/admit prospective records before DB admission; atomically persist and verify required initial history. State Zero here means trusted revision-one configuration, not arbitrary bulk trees. | Measure generation/encoding, transaction/reload, tail latency, memory, bytes/project, indexes and WAL plus retained history growth. Distinguish retained data from dead-row bloat. Agree budgets; representation changes need separate approval, not post-success snapshot deferral. |
| Deep/oversized hostile input | Accepted ADR-104 original-byte admission: 1 MiB record, 64 KiB decoded key/string, depth 16, 1,024 entries/collection, 16,384 values; early decoded duplicate rejection. Generated envelopes also bounded. | Actual streaming/misleading-length/limit/escaped-key tests must show zero per-request pool or application-worker admission for rejection. Measure aggregate concurrency and parser/response memory; codec ceilings do not imply zero allocation or bounded total process memory. |

Existing pure/native fixture evidence is regression input, not performance or
product-route qualification for these cases. Prepare bounded targeted harnesses
before requesting service/resources or user-assisted execution; this matrix is
not authorization for another broad campaign.

Before enabling the affected later capabilities, resolve the maintenance fence/compatibility contract
and event delivery mechanism when applicable,
the operating budgets and any additional maintainer questions. The future
implementation plan must identify which cases gate its precise claim and retain
unmet adoption/route/real-data obligations. Do not mark the draft ready or infer
architectural acceptance because these initial documents exist.

## Additional questions: wire, deployment and consumer boundaries

The maintainer subsequently added eleven questions. They extend this draft's
assessment, including consumer behavior and broker privileges; they do not add
those subsystems to the authorized implementation outcome or finish this PR's
scope. Event-facing details live in Proposed ADR-109; active-load migration
alternatives live in Proposed ADR-108. The following distinctions constrain the
remaining design and verification work:

- **Concurrent gateways and identical tokens:** operation identity is scoped,
  never derived from arrival time. A retry needs the same authenticated actor,
  intent handle, token and exact payload. Token equality alone with a distinct
  intent/target intentionally does not mean the same operation under ADR-107.
  Repeated requests serialize at the authority/database fences and repeat lookup;
  unique-key/candidate collisions are typed outcomes with bounded whole-transaction
  handling, never unwrap/panic paths. Random IDs do not make collision impossible.
  The local MVP still has one active Rust authority. Multiple active authorities
  need #65's replacement session/fencing/budget contract; extra gateways cannot
  silently adopt that unsupported profile.
- **Disconnected or partially committed intent:** the mapping, creation state,
  initial history and result commit together; a known rollback leaves none of that
  accepted action. A lost response is an unknown observation, not a third partially
  committed business state. Reconcile the original identity under current authority;
  an absent lookup cannot prove another inflight request will not commit. Incoherent
  committed records fail closed as corruption and need a reviewed recovery path,
  not an automatic new allocation. Required event handoff follows ADR-109's chosen
  durable protocol, rather than being inferred from client acknowledgement.
- **Pure-codec CPU and memory:** ADR-104 bounds actual bytes, nesting, values,
  collections and strings, including original duplicate detection. The current
  implementation builds a tree and may use escaped-string scratch. Its resource
  limits do not prove a hard elapsed-time bound or whole-process heap cap. Prepare
  adversarial CPU/peak-memory measurements and aggregate ingress limits. An async
  timeout cannot preempt arbitrary synchronous decoding; account for owned work
  until completion. A hard execution deadline would require an explicitly reviewed
  cooperative budget or isolation mechanism, not a deadline wrapper alone.
- **Go/Rust field parity:** the accepted native path terminates at Rust; no new
  Go decode/remarshal hop is selected. The legacy Go mutation route forwards bounded
  raw bytes. If a future supported boundary uses both languages, define one versioned
  wire contract with exact case-sensitive fields and per-field missing/null/unknown
  policy, then test both adapters against the same raw-byte positive/negative golden
  corpus. Include escaped duplicates, trailing data, numeric extremes, enum tags and
  Unicode. Reject information-losing parsing before canonical validation; generated
  schemas and round trips alone do not prove parity. Do not change frozen legacy
  behavior or version-one records by applying new struct defaults retroactively.

Before final scope acceptance, specify which event consumers and language boundaries
are actually required. The new questions cannot silently turn the first local journey
into a replicated gateway, rolling-migration or general messaging-platform delivery.

## Completion and rollout evidence

| Gate | Concrete required evidence | Current disposition |
| --- | --- | --- |
| Design and implementation gates | ADR-103 and ADR-107 accepted 2026-11-02; exact auth/allocation/read schema, product graph/MSRV and bounded adapter plan remain | Architectural choices accepted for the bounded outcome; implementation evidence remains pending. |
| Allocation #41 | Lost responses before/after commit, concurrent identical/changed intents, actor isolation, target collision, tombstone/restore integrity | Structural mapping subset passed; concurrency, uncertainty, authenticated authority and application-adapter qualification remain pending. |
| Ingress | Actual reads with misleading lengths, duplicates/escaped keys and every codec limit; zero pool/application-work admission on rejection; generated-envelope overflow before DB | Required before route exposure. |
| Authority #40 | Missing/spoofed credentials, wrong scope, current revocation, creation versus replay grants, read snapshot races and post-query expiry | Required on real integrated paths. |
| Browser binding/lifecycle | Current session/CSRF authority, immutable server-pending discovery across browsers, explicit execution/cancel, changed-owner rejection and retained response uncertainty | Contract before browser/API implementation; actual browser cases before qualification. |
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
