# SP-005 — Stable project-target allocation across retries

| Field | Value |
| --- | --- |
| Status / outcome | Architecture accepted; native structural and fixture-actor concurrency subsets passed; product/browser gates recorded separately |
| Owner / assessor | Project maintainer; bounded database execution completed, full allocation qualification pending |
| Created / last updated | 2026-10-08 / 2026-11-03 |
| Tracking issue | [41](https://github.com/MrQuality/kehila/issues/41) |
| Parent delivery issue | [#27](https://github.com/MrQuality/kehila/issues/27) |
| Prior evidence | [SP-002](SP-002-postgresql-project-create.md) |
| Effort limit / stopping condition | Proposed 60 minutes; confirm before execution. Stop on failed prerequisites or supported/bounded inconclusive conclusion. |

## Question, cases and execution plan

### Current bounded allocation slice, 2026-11-02

The maintainer explicitly [accepted ADR-103 and ADR-107 revision 3](https://github.com/MrQuality/kehila/issues/49#issuecomment-6573495351)
for the first authenticated create/read journey. The historical plan below sought
an architecture choice; that choice is now made. This slice must establish its
physical contract and failure evidence, not reopen the accepted protocol.
The native structural subset passed; complete allocation qualification remains pending.

**Objective:** verify that one trusted target and permanent operation belong to
one actor-scoped intent through concurrency, uncertainty and payload retirement.
Included: disposable native mapping/transaction harness, actual-role constraints,
typed replay and controlled faults. Excluded: public routes, auth extraction,
browser implementation, event consumers, production migration/installer and
multi-active authority. Frozen SP-002 evidence remains unchanged.

**Physical candidate for review:** store actor, family, intent handle, target
kind/project and permanent operation-core ID, all non-null. Use the existing
five-column attribution foreign key to bind core/family/kind/project/actor; enforce
ProjectCreate/project tags, unique actor/family/handle, unique target and the
accepted 32-character lowercase hexadecimal handle shape. Derive the operation
token from the referenced immutable core rather than maintaining a second copy.
Different-token use of an existing handle conflicts with that core's token.
Serving receives only the required mapping SELECT/INSERT, no mapping mutation,
actor administration, retirement or DDL. Exact SQL, installation/version handling,
actual-role privileges and harness interfaces remain review items before execution.

**Ownership:** use the accepted installation-before-actor fence order, resolve
the mapping after fresh authority checks, then the full operation key/core lock.
Mapping and complete creation commit together. Every collision/transient retry
rolls back and re-enters through the full lookup path, with all generated envelopes
admitted before the next creation database attempt. Historical reconciliation
generates no prospective current-domain seed and cannot fall through to creation.

| Case group | Controlled schedule / independent oracle |
| --- | --- |
| Constraints and privileges | As the serving role, reject wrong actor/family/target/core references, malformed/duplicate handles, duplicate targets and unauthorized UPDATE/DELETE; token disagreement conflicts without a duplicate mapping token field. |
| Same intent | Hold the actor fence, observe the second request waiting, release; identical requests produce one project/core/mapping and the saved result; changed payload/token conflicts. |
| Identity separation/collision | Independent actors and distinct intents cannot adopt another target/result. Inject a target collision, allow a same-actor competitor to commit during rollback, then prove full re-entry replays the winner. |
| Commit uncertainty | Barriers before persistence, before commit and after commit/before acknowledgement distinguish rollback from unknown outcome. Retry original identity; assert atomic rows and no second project. |
| Reconciliation/history | Absent is only a lookup-ordering observation. Changed current validation/seed cannot block stored-version replay; denied/revoked current authority discloses no saved content. |
| Expiry/retirement/recovery | Logical expiry never reexecutes. Retain mapping/core after retirement; mismatched/missing references fail closed. Fixture restoration checks coherence without claiming supported production restore. |
| Bounds and cleanup | Generated overflow prevents creation DB admission; deadline/cancellation retains uncertainty where applicable and joins owned work. Record final connections/locks and owned-resource cleanup. |

Before any live phase, prepare and separately review the harness, exact barriers,
resource/connection limits, per-case and overall deadlines and artifact handling.
The existing proposed 60-minute execution cap is not automatically approved by
ADR acceptance. Obtain resource permission and fresh headroom before services;
heavy phases remain serial. Stop at a failed prerequisite or harness failure,
retain failed evidence and repair without user-assisted cases. Do not retry until
green or treat simulated storage as native evidence.

Completion requires actual-role native observations at identified source/profile,
typed reconstruction of the single trusted result, all applicable case dispositions
and exact-head independent review. #41 stays open until that evidence is reviewed;
architecture acceptance alone completes no #27/#49 delivery. Concrete blocking
findings trigger only affected plan/schema changes, not a new generic campaign.

### Next adapter design gates

Before implementing the application adapter, record the transaction/API contract
and exact bounded test schedule under #41/#49. These are remaining obligations,
not defects established by the structural subset:

- Specify READ COMMITTED statement ordering and installation/actor/core fence
  ownership, including fresh authorization and lookup after waiting. Commit the
  mapping, operation and complete creation atomically. Classify named constraints;
  finish rollback before complete re-entry. Select retry/deadline budgets from
  relevant measurements rather than inventing fixed attempt counts.
- Define transport outcomes for equal replay, changed token/content, expired
  replay, absent reconciliation, temporary contention and unknown commit outcome.
  Preserve handle/token/content through recovery. An absent lookup cannot promise
  that another in-flight transaction will not commit. Exact HTTP statuses and
  payloads remain adapter review decisions; no endpoint is adopted here.
- Exercise the controlled schedules above with real connections: same-actor
  fence waits and duplicates, independent actors, target collisions with a winner
  during rollback/re-entry, rollback versus commit-before-response loss, and lock
  timeout/cancellation. Check independent final state and owned cleanup.
- Use independently generated secure-random handle/token values in the first-party
  client and persist them with exact content before transmission. The server can
  validate the accepted shape, not prove a hostile client's entropy or determine
  uniqueness before the transaction. Handle secrecy grants no authority. Define
  bounded admission and creation-abuse controls before exposing the route; no
  standalone reservation or arbitrary new numeric quota is introduced here.

**Parent:** #27. Related: #26, #11, #8. Follow-up to SP-002.

### Original pre-execution plan, 2026-10-08 (retained history)

1. **Goal & Context:** Define who allocates a trusted ProjectId and how retries recover the same target when a response is lost. Operation lookup includes typed target, so a retry must not allocate a new target and accidentally create another project.
2. **Definition of Done:**
   - [ ] Record a concrete allocation/reconciliation protocol and its trust boundary.
   - [ ] Test response loss before and after allocation/creation, actor-scoped retries and deliberate target collision.
   - [ ] Prove the retry retains actor/family/target/token; a different actor/token must not adopt a recorded success.
   - [ ] Publish SP-005 recommendation, rejected alternatives and native integration follow-up.
3. **Dependencies & Prerequisites:** #27 identity requirements, #26 scoped operation key, #11 trusted actor context and SP-002 collision/replay observations.
4. **Risks & Mitigations:** New targets on retries can evade same-key lookup. Never use a display prefix as identity. Make recovery explicit and preserve conflicting-success isolation; no blind uniqueness-error replay.
5. **Spikes & Open Questions:** Allocation timing, stable retry handle and ownership of the binding. Proposed effort limit: 60 minutes, to confirm before execution; stop with a bounded recommendation or a precise unresolved dependency.
6. **High-Level Architecture / File Changes:** docs/spikes/SP-005-project-target-allocation.md; bounded experiments/SP-005/ when executed. A prototype allocation table or endpoint is not an accepted product API.
7. **Verification & Testing Plan:** Synthetic allocation attempts, controlled response-loss barriers, repeated concurrent retries and independent final-state checks. Record scope and exact identity used in every attempt.

Historical status: planned. The accepted architecture and executed subsets below
supersede that planning status; completing this spike will not close #27.

## Environment and reproduction

The [executable structural subset](../../experiments/SP-005/README.md) implements
the immutable mapping overlay and actual-role SQL oracles. Its native run passed
on 2026-11-02. This is not a production migration or completed allocation adapter;
the later fixture-actor concurrency extension is recorded below. Uncertain
commit and product authority are separate qualification obligations.

## Run summaries

The native structural run used the documented pinned PostgreSQL image with
verified limits of 256 MiB, one CPU and 128 PIDs. Role/schema, creation-core
attribution and mutation-guard oracles passed; owned-container removal completed.
The runner records source fingerprints and unqualified work.

A later full shared-gate attempt passed its test phases but failed final snapshot
cleanup with a Windows sharing violation. No commit was created. A provider-only
observation reproduced blocked removal of its launch directory while running and
successful removal after shutdown. Lifecycle commands now use a stable account
directory. The later full containerized shared gate passed, including owned
runner, service, network, provider and source-snapshot cleanup; see the
[container verification record](../implementation/container-verification.md).
The particular historical locking process has not been identified.

## Conclusion and reuse boundary

ADR-107 revision 3 is accepted. The physical mapping structural subset and three
fixture-actor transaction schedules have native evidence. They do not qualify a
production allocation adapter or multi-actor HTTP. Reuse SP-002
within its documented baseline, role/helper, runtime and fault boundaries.
Further experiments must resolve the remaining uncertainty rather than repeat
engine feasibility.

## Decision and follow-up

Deliver a supported recommendation and retain rejected alternatives/limitations.
Update existing decisions/questions only when the maintainer accepts a choice.
The parent issue remains responsible for production implementation and regression
coverage; finishing this investigation does not close it.

## Handoff

Latest work: the native structural subset, three fixture-actor transaction
schedules and the shared containerized gate passed in separately recorded runs.
Keep required commit gates, exact-head review and CI before handoff. Product
uncertain commit and authenticated application integration have their own
[verification record](../implementation/project-authority-verification.md).
Windows browser qualification is explicitly deferred from this delivery and
remains unqualified. Broader machine calibration
is not an added completion gate.

## Server-owned pending creation amendment, 2026-11-03

The maintainer accepted cross-browser server-owned pending creation and authorized
its implementation as part of the [product authority delivery](../implementation/authenticated-project-authority.md).
The earlier immutable successful mapping evidence remains valid for its structural
scope; it does not qualify the new pending lifecycle. Register/discover/execute/
cancel, delayed execution after expiry, concurrent cancellation and uncertain
registration/commit require the actual product/native cases. No IndexedDB recovery
dependency remains. #41 stays open until its applicable allocation evidence passes.

## Fixture-actor concurrency evidence, 2026-11-03

All three opt-in SP-005 schedules passed: distinct targets committed independently
with identical token/handle text in separate actor scopes; a blocked target loser
received typed `23505/projects_pkey`, completely rolled back, released its actor
fence and re-entered with a fresh target; a waiting transaction committed its own
creation after the original holder rolled back. Independent final observations
required exact attribution and five new coherent project/core/payload/mapping
sets. Actor authorization flags were checked after each actor-row lock.

The full phase took 124.407 seconds through cleanup, with a 1.647-GiB sampled
aggregate peak and 4.804-GiB minimum available RAM. Owned resources were removed
and the initially stopped provider stopped. Earlier observer/protocol failures
remain failures; their diagnoses and the remote-stream ordering control are
described in the [executable record](../../experiments/SP-005/README.md#evidence-limits-and-next-cases).

The maintainer-selected
[layered split](../engineering/QUALIFICATION.md#native-project-creation-separate-database-and-http-evidence)
keeps these fixture actors at the database layer. Placeholder snapshot/payload
bytes do not establish typed M1V1 creation. ADR-105 keeps one installed HTTP
owner; same-owner Windows Chrome/Edge evidence and post-MVP multi-actor HTTP
under #65 remain separate.
