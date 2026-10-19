# SP-004 — Trusted actor binding and initial-owner authority

| Field | Value |
| --- | --- |
| Status / outcome | Contract, serial baseline and bounded R04/R05 checks merged through PR #59; broader investigation incomplete |
| Owner / assessor | Project maintainer; remaining mechanism decisions and result assessment are separate |
| Created / last updated | 2026-10-08 / 2026-10-19 |
| Tracking issue | [40](https://github.com/MrQuality/kehila/issues/40) |
| Parent delivery issue | [#11](https://github.com/MrQuality/kehila/issues/11) |
| Prior evidence | [SP-002](SP-002-postgresql-project-create.md) |
| Effort limit / stopping condition | R04 continuation: approved 90-minute investigation budget, excluding setup/checks. Future mechanism-dependent runs need their own confirmed scope/budget. Stop on failed prerequisites or a supported/bounded inconclusive conclusion. |

## Question, cases and execution plan

**Parent:** #11. Related: #32, #49, #27, #41; historical #26. Follow-up to SP-002.

[PR #53](https://github.com/MrQuality/kehila/pull/53) was approved and merged on
2026-10-14 at `988631a353b3086769e302943fa0d7b7fbebffd1`. Its contract is the
reviewed baseline for this investigation. Authentication/session/channel
mechanisms were unselected in those experiments; accepted ADR-105 now selects
the local-MVP direction, with actual authentication evidence still outstanding.

## Source inventory

Inspected at main `2efd444e38a4d60eaeb195a00359813174aa3d23`. These are source
observations, not new runtime verification.

| Boundary | Existing behavior | Missing contract |
| --- | --- | --- |
| [Go API](../../go/io/task_api/server.go) | POST checks loopback Host, browser origin and a 4 KiB body bound; worker requests are freshly constructed without forwarding browser headers. Task/search reads have no actor check. | No owner authentication, session, current permissions or authenticated worker identity. Origin checking does not identify a caller. |
| [Rust worker](../../src/io/task_worker/src/main.rs) and [store](../../src/io/task_worker/src/lib.rs) | Internal listener checks Host, rejects browser context headers, bounds execution and serves the provisional FerretDB task path. Mutation replay is keyed by task and operation ID. | No authenticated actor or M1 scoped operation lookup. Loopback and an expected Host do not authenticate the Go process. |
| [Pure creation](../../src/pure/task_contract/src/project_create.rs) and [replay](../../src/pure/task_contract/src/operation.rs) | Creation accepts `authorized: bool`; `OperationKey` carries actor/family/target/token; replay requests current original grants. | Caller must supply trusted actor, authoritative scope and protected current grants. Public types and booleans are not proofs. |
| [Native actors/operations](../../storage/postgresql/operations.sql) | Actor has `active` and `can_create_project`; operation stores actor, original grant and policy version. | No sessions, enrollment, authentication provider or session-revocation protocol. |
| [Native grants](../../storage/postgresql/project_create.sql) and [capability](../../storage/postgresql/access.sql) | `initial_owner` profile, current grant row and append-only events; actor locking helper. Creator cannot administer actors or update grants. Grant-event guard permits only revision-one self-grant at creation. | No enforced current/event agreement, owner permission expansion or management/revocation capability. Later mutation cannot work by simply removing the creation-only guard. |

## Proposed contract for review

This section records the reviewed W2 contract baseline; it accepts no new ADR or
unselected authentication mechanism through experimental execution.
The accepted local-owner/loopback direction comes from D-016/D-017 and Q-002.
Current original-grant replay comes from D-032/D-033; ADR-104 forbids deriving
authority from codec admission or fingerprints. Existing APIs are not changed.

### Actor binding and routing

1. A trusted identity component verifies a browser session or CLI credential and
   constructs an immutable `ActorContext` containing stable actor identity,
   credential/session reference and validity information. Client-supplied actor,
   owner, role and grant assertions cannot construct it. The context contains no
   cached permission verdict that can bypass current authority checks.
2. Identity remains stable across login/session renewal and retry. Neither a
   session ID nor a database login becomes the operation's actor ID. One local
   installation has one authenticated owner; project scope still applies.
3. The command boundary derives family and typed target from the admitted
   command and checked allocation binding, then constructs the operation key
   using the verified actor. #41 owns stable target allocation/reconciliation.
   Never adopt an operation belonging to another actor or project after a
   uniqueness error. No client-provided fingerprint is identity evidence.
4. A process boundary must authenticate its peer and integrity-bind the actor
   context to the admitted command. Forwarding an actor header to the current
   unauthenticated worker is insufficient. The exact credential and internal
   channel mechanism remain explicit review decisions before implementation.
5. Protect authoritative and projected reads as well as commands. Authenticate
   CLI requests even when Origin is absent. Browser origin/CSRF validation is
   additional to session verification. Credential contents must not enter
   operation keys, fingerprints, results or public diagnostics.
6. ADR-104 edge admission still completes before native pool acquisition or
   application worker scheduling. If identity resolution needs storage, its
   per-request storage access follows bounded edge admission too. This ordering
   does not make malformed-input errors equivalent to authorization decisions.

If the selected credential or internal channel uses a MAC/signature, its
authenticated representation must be versioned, domain-separated and injective:
different typed field combinations cannot produce the same authenticated bytes.
Specify fixed-width or length-delimited fields, exact text encoding and request
target/body binding. Raw concatenation of variable actor/session/path fields is
insufficient. Algorithm, key provisioning/rotation, peer/audience binding and
replay behavior require security review with the chosen mechanism. These are
acceptance requirements, not selection of a custom MAC protocol. Authentication
of an envelope never replaces current permission/session-revocation checks.

### Initial-owner permission mapping reviewed in PR #53

The existing SQL profile is a storage label, not an implemented permission set.
The following mapping is the reviewed experimental contract baseline under Q-014.

| Action | Proposed current authority |
| --- | --- |
| New project creation and replay of its original creation | Active authenticated actor with current `can_create_project`; retain `project_create` as the original scope. Existing project ownership alone does not replace this grant. |
| Read project/items/history and corresponding projections | Active actor with an active `initial_owner` grant in that project. |
| Project/configuration administration, item mutation, knowledge and archive/restore | Active project owner may satisfy the applicable M1 command grants in that project; domain restrictions and retained original replay scope still apply. |
| Relationship/type use and cross-project Link | Evaluate the required grants separately in every owning/endpoint project. Ownership of one project gives no rights in another. |
| Status-group administration | Project administration can satisfy the accepted administration boundary; do not invent delegated memberships or weaken D-028's both-group rule. Delegated provisioning remains #32. |
| Actor/credential/grant administration | Dedicated reviewed management capability, unavailable to the serving creator role. Enrollment/recovery cannot be achieved through a public command that asserts ownership. |

Project creation must derive the initial grant and event from the same verified
actor and accepted result as the operation, project and revision. Bootstrap must
establish exactly one stable owner and its creation permission; retry, failed
bootstrap or recovery must not silently create a second owner.

### Stored permission policy and current authority

For the creation slice, require `grant_policy_version = 1` and the exact stored
scope `project_create`. Version one interprets that scope as current active-actor
creation permission; it does not substitute project ownership. The stored policy
version identifies how to interpret the original required permission scope, not
a historical permission verdict. Always evaluate that meaning against current
authoritative grants, including on full-record and tombstone replay.

The proposed `initial_owner` mapping above is the version-one profile meaning
for project-scoped commands. Persisted profile tags and permission meanings must
not be silently redefined during upgrades. Later policy evolution must explicitly
preserve interpretation of retained original scopes and define any profile
migration; revocation must still deny the affected permission on replay.
Unsupported stored versions, profiles or malformed scopes fail closed as internal
compatibility/integrity failures, not ordinary caller denial or permissive fallback.
Keep diagnostics distinct from a known valid scope whose current grant is absent.

Before multi-command schema evolution, #49 W5/#32 must specify a lossless,
versioned representation for group sets, conversion/Migration and cross-project
scopes. The current scalar creation scope is sufficient only for this slice.
Preserve original scope through payload retirement; neither changed retry content
nor a newer policy may replace it with an easier authorization requirement.

### Authority, history and revocation ownership

Reviewed mutation/replay protocol:

1. Begin a bounded transaction, lock the verified actor with `lock_actor`, and
   recheck actor liveness and the selected credential/session revocation state.
2. Perform trusted scoped operation lookup to obtain the stored original scope
   and its policy version for a full record or tombstone. For an unseen operation,
   derive the new required scope from the checked command. Reading this internal
   metadata is not disclosure of operation existence or its result to the caller.
3. Interpret that effective scope and evaluate current authority under the same
   fence. For project-scoped commands, verify matching current/latest-event
   profile, active state and version. A mismatch is an internal integrity failure
   and cannot grant access; an ordinary absent/inactive grant denies permission.
4. Only after applicable authority succeeds, evaluate/disclose replay, changed
   content, expiry and mutable domain outcomes. Protect authority through
   transaction completion, and reload it after rollback/retry.

Creation may check its constant current creation permission before lookup. This
optimization must not become a creation-permission gate for other commands or
evaluate a retry's new scope in place of a recorded original scope. Existing
creation ownership never substitutes for current `can_create_project`.

Serving verification must be bounded per required grant: use indexed current
and latest-event lookup for the requested scope, not COUNT/MAX over the actor's
entire retained grant history on every read or command. Management must advance
one version from the checked predecessor and append its matching event atomically,
with checked overflow and immutable history. Full gap/attribution verification
belongs to experimental setup, upgrade/restore qualification and explicit
integrity audits, where a complete history scan is declared and budgeted.
Detected corruption fails closed and requires an explicit repair decision.
Bounded serving checks do not claim to discover every earlier history gap;
permission decisions themselves are never cached to avoid these checks.

Every management operation changing an actor's authority or project grants must
acquire the same actor fence before writing. Append the next grant event and
update the current row atomically; preserve earlier history. Creation/replay
holding the fence may finish before revocation; revocation acquiring it first
forces subsequent creation/replay to observe denial. An operation already
committed is not rolled back by later revocation. Recheck after rollback/retry.
Multiple affected actors must be locked in a documented deterministic order.

Session logout/recovery must have a declared equivalent ordering: either share
this fence with authoritative credential-generation checks or provide another
demonstrated serialization protocol. A session check made before a queued
command is insufficient to promise immediate revocation. No session schema or
production management role is authorized by this proposal.

### Read authorization and concurrent revocation

Reviewed read semantics: authorize at a fresh PostgreSQL snapshot established for
that request. Any logout, credential revocation or permission change committed
before that snapshot must be observed. An authorized read may finish if revocation
commits after its snapshot; it does not promise revocation of bytes already being
returned. This ordering does not select the outstanding credential/session mechanisms.

Authoritative reads must check actor/session state, applicable grants and
current/latest-event agreement in the same snapshot as the returned authoritative
data: one read-only statement, or an explicitly bounded read-only transaction
using one consistent snapshot. Do not accidentally reuse a snapshot established
before the request. Reads need no exclusive actor fence under this proposed
contract; independent grant/event queries under READ COMMITTED alone cannot
establish a consistent authorization/data view. Mutation/replay fencing above
remains separate. PostgreSQL's [isolation rules](https://www.postgresql.org/docs/16/transaction-iso.html)
define the snapshot behavior; no concurrent run is claimed here.

Projected reads must obtain fresh authoritative read authorization for every
required project before querying OpenSearch. That authorization snapshot is the
read's permission ordering point, not an atomic PostgreSQL/OpenSearch snapshot.
Keep queries, hits and aggregates within the authorized scopes and require an
authoritative project binding for returned identities; client/search labels do
not establish permission. Do not carry an actor lock or database transaction
across the external search call. Bound query/result lifetime and discard results
after the request deadline. A later revocation may overlap an already-authorized
bounded read; a subsequent request must acquire fresh authority. Projection
freshness and rebuild/recovery remain #49 integration obligations.

### Remaining decisions for mechanism-dependent experiments

- The contract's owner mapping, actor-fence and read snapshot semantics were
  reviewed in PR #53. Amendments require explicit review.
- Choose the local enrollment/credential mechanism, browser session transport
  and authenticated CLI mechanism; define expiry, logout and recovery behavior,
  including how durable actor identity survives credential replacement.
- Choose authenticated Go-to-worker context transport, or consolidate that
  boundary; do not treat the current loopback listener as trusted identity.
- Review any subsequent experimental scope/time limit against the cases below.
  The first serial A05 subset uses a declared verified-identity fixture to test
  authority plumbing; it cannot qualify real authentication.
- Apply [D-037](../product/decisions.md#d-037): review recovery dependencies,
  localized display versus stable identity, and future machine-caller/delegation
  assumptions under B-024/#60, B-030/#61 and B-031/#62. Their broader delivery is
  post-MVP; the current one-owner scope and authorization contract remain.

These are owned by #40 under #11/#32. #49 owns runtime integration; #41 consumes
the reviewed stable actor identity. No extra historical-assessment issue is needed.

### Experimental schema boundary

The first implementation uses an explicit post-install overlay in a fresh disposable native
database. Keep all `storage/postgresql/*.sql` source files unchanged. First verify
the unmodified installer and its creation-only denials; only then apply the
versioned experimental overlay in that database. It is not a production upgrade.

The overlay plan must account for all affected constraints and permissions:

- Add isolated experimental credential/session objects after their mechanism
  is reviewed, and separate management capability/login roles. Serving creator
  privileges must not expand to UPDATE/DELETE or management operations/events.
- Permit a clearly experimental `sp004_grant_admin` project-target family and
  required scope, accounting for `operations_family_target`,
  `slice1_operation_family` and `slice1_operation_grant`. Do not masquerade grant
  management as an accepted configuration command or a production codec.
- Replace `slice1_grant_event` only in the disposable database with explicit
  creation versus management shapes. Preserve the original revision-one creation
  shape; management requires checked successor versions and matching current
  state. Retain actor/project references, composite operation attribution,
  append-only history and permanent-core/payload shape guards.
- For each grant change, lock manager and subject actors in deterministic order,
  verify management authority, insert the attributed experimental operation and
  its request/result payload, and advance current grant plus event atomically.
  Rollback must remove the complete attempted change. Test restricted serving
  attempts to manufacture management operations/events as well as grant UPDATE.

Before execution, review the exact DDL/privilege diff and fixture representation,
including how the changed guard set prevents serving-role management inserts.
All tested grants, credential state and command/replay fixtures must share the
same actor fence. Record overlay source alongside baseline source in every run.
Passing this experiment establishes only that declared overlay's mechanics;
#49/#32 still require a separate production migration and integration review.

## Revised pre-execution plan

1. **Goal & Context:** Use the reviewed PR #53 contract against Q-002/Q-014 to investigate actor binding and authority/revocation ownership for #49 W2. The inventory establishes the missing boundary; this is not another database feasibility investigation.
2. **Definition of Done:**
   - [x] Document the proposed actor/session contract and initial-owner permission mapping against B-003/B-007 for review.
   - [x] Record the first serial A05 subset, including its failed privilege attempt and corrected native evidence; R01-R03 retain their serial-only limits.
   - [ ] Demonstrate denied spoofed actor input, missing/expired identity and forbidden role escalation using a declared experimental integration.
   - [x] Demonstrate current grant/event agreement and atomic revocation under the declared management fixtures; R04 adds earlier-gap audits, growing-history observations and concurrent management cases.
   - [x] Repeat the selected database creation/replay versus revocation orderings with observed lock barriers: R04 records ten trials of each ordering plus rollback cases. Actual authentication remains unqualified.
   - [ ] Publish SP-004 recommendation and gaps; actual authentication and grant-management implementation remain open.
3. **Dependencies & Prerequisites:** Resolve the remaining mechanism decisions before dependent cases. Serial A05 uses only the declared verified-identity fixture. Use #11/#32 requirements, B-003, ADR-104 and SP-002's source-bound locking observations. #41 receives stable actor identity; its allocation protocol is not implemented here. Check available memory before compiler/service work; request headroom if the selected workload cannot fit.
4. **Risks & Mitigations:** Supplied identity, cached roles and database capability possession cannot establish caller authority. Actor-wide serialization is deliberately conservative and may limit concurrency; measure selected schedules rather than claiming throughput. Freeze affected-actor lock ordering and protect both the management caller and grant subject. Detect current/event mismatches and never repair them implicitly during serving.
5. **Spikes & Open Questions:** Proposed 90-minute investigation, excluding setup and required checks, after contract/decision review. Stop on failed prerequisites, a bounded conclusion or the time limit; unresolved real authentication remains an explicit blocked case. A synthetic identity fixture must not turn that case into a pass.
6. **High-Level Architecture / File Changes:** Keep this record canonical. The serial runner and approved H01/H02/C01-C04 continuation reside under `experiments/SP-004/`, with explicit disposable overlays and restricted roles. Later mechanism-dependent experiments retain their own review gates. Production migration and integration remain #49 obligations.
7. **Verification & Testing Plan:** Use the cases below with independent restricted connections and explicit barriers. Record actual source, image, platform, role, state before/after, observed waits, commit/rollback and primary diagnostics. A timeout is not evidence that the intended race was reached. Actual observations belong in the run summaries; this plan alone supplies no successful result.

Status: reviewed contract with serial and bounded history/concurrency evidence; the full investigation
is incomplete. Completing it leaves #11/#32 responsible for production access delivery.

### Required experimental cases (full-case acceptance remains outstanding)

Use typed pure replay fixtures and the experimental permission evaluator for
A06's composite-scope cases. Native creation replay uses the actual stored
creation scope; this overlay does not implement conversion/status command
persistence. Their production scope serialization remains the W5/#32 gate.

| Case | Controlled action | Required evidence |
| --- | --- | --- |
| A01: trusted binding | Verify owner identity, then supply a conflicting payload/header actor, role or grant assertion; repeat with missing/expired credentials. | Context derives only from the verifier; unauthenticated requests reach no command execution. A deterministic verifier fixture proves plumbing only; actual provider qualification is a separate result. |
| A02: internal peer and framing | Attempt direct worker submission and altered actor/context/request from an untrusted process. For a selected MAC/signature envelope, test distinct variable-field combinations that would collide under raw concatenation, domain/peer confusion and the specified replay behavior. | The selected channel rejects spoofing/tampering; distinct typed envelopes have distinct authenticated encodings. If no mechanism is selected, record blocked, not passed on the basis of Host/loopback checks. |
| A03: owner scope | Exercise active/inactive owner, creation disabled, absent/inactive project grant and another project. | Evaluate the proposed mapping independently in each scope; reject role escalation and cross-project access. No ownership inferred from an existing creation result. |
| A04: read/CLI/session | Test reads without identity, CLI without Origin, cross-origin commands and expired/logout/recovered sessions. Reach revocation-before-snapshot and snapshot-before-revocation for authoritative and delayed projected reads; mix allowed/denied project hits and cross the response deadline. | Earlier committed revocation denies. An already-authorized read may finish within its deadline, with consistent authoritative data and no out-of-scope hits/aggregates. Later reads reauthorize; timed-out results are discarded. Reads hold no exclusive actor fence across search. Recovery preserves actor identity; unselected policy remains blocked. |
| A05: grant history and overlay | Test the baseline denials, then overlay current/latest mismatch, an earlier event gap, serving management-operation/event inserts, grant UPDATE/history mutation, and management commit/rollback. Repeat bounded serving checks with growing history. | Current/latest mismatch fails internally without granting access. A declared full audit detects earlier gaps; serving checks do not scan all history. Restricted serving management writes fail. Authorized management advances exactly one matching attributed operation/payload/event/current version; rollback leaves all unchanged. |
| A06: replay precedence and policy | Retry full and tombstoned records with changed requested scope after revoking an original grant: conversion requiring Migration and a status move requiring both original groups. Include unknown stored policy/profile and creation's constant scope. | Original-scope denial precedes replay/conflict/expiry disclosure even if the retry's easier scope would pass. Original committed state is unchanged. Unknown policy/profile fails internally without fallback. Creation replay checks current creation authority; unrelated commands do not inherit that gate. |
| A07: revocation schedules | Run command-first and revocation-first for both creation and replay, with actor lock barriers; roll back each side and retry. | Reach and record all four orderings, proposed ten trials each. Command-first can finish before revocation; revocation-first denies. Rollback releases the fence and retry reloads authority; no partial grant/event state. |
| A08: session and multiple actors | Revoke a credential while work is queued; exercise two actors and management affecting both in opposite requested orders. | The declared session fence prevents stale authorization under its policy. Deterministic actor lock ordering is enforced; bounded failure rolls back before retry. Stable actor identity remains usable for #41 retry binding after reauthentication. |

For each new extension, add exact reproduction commands and pin the experimental
identity implementation, role/schema overlay, PostgreSQL image and lock/statement/
process deadlines. Start from fresh disposable state and retain negative-case
diagnostics and cleanup failures. Existing runner commands are linked below;
unexecuted cases remain acceptance requirements, not successful observations.

## Environment and reproduction

The first [serial A05 runner](../../experiments/SP-004/README.md) reuses the native
verifier's fresh/disposable container checks and pinned image. It runs the
unmodified baseline first, then the [experimental overlay](../../experiments/SP-004/overlay.sql).
The README specifies exact commands, role limits, fixture representation and
statement/lock/process budgets. The caller owns container cleanup. Check memory
before each compiler/native-service run; source and container limits do not
establish that the host has enough available memory.

## Run summaries

### Serial A05: SP-004-R01/R02, 2026-10-14

Baseline main: `988631a353b3086769e302943fa0d7b7fbebffd1`, with then-uncommitted
A05 sources identified by exact hashes in the [serial evidence](../../experiments/SP-004/results.serial.json).
Those exercised source files are preserved at commit `f12466e`; subsequent
removal of an unused signature constant does not change the exercised functions.
Windows/WSL2/Podman used the existing rootful connection, with an unprivileged,
networkless 256 MiB ephemeral container and PostgreSQL 16.15. Rootless startup
failed because memory controllers were unavailable; that failed container was
removed before the native attempts. This is not Linux-host qualification.

R01 passed the native baseline but failed with SQLSTATE 42501: the deferred
payload trigger ran under the manager login after its SECURITY DEFINER helper
returned and lacked operation-table reads. R02 passed after granting only
operation/payload SELECT needed by that trigger, retaining no direct mutation
rights. Both attempt containers were removed.

R02 covered baseline version-two event denial, separate creator/manager logins,
forbidden management operation/event INSERT, stale/no-op rejection, atomic
rollback, revocation/restoration, operation/payload/event/current attribution,
exact experimental request/result/digest coherence, current/latest disagreement
and history protection. Removing the management-operation guard made the
forbidden write succeed; the existing rejection oracle detected the mutation,
and its transaction rolled back. Six Python tests separately cover runner
prerequisite/failure propagation; they are not native SQL evidence.

This does not complete A05: earlier-history-gap audits, growing-history
measurements and multi-actor/concurrent changes remain outstanding. A01-A04 and
A06-A08 also remain outstanding; synthetic identity is not authentication and
the helper is not a general replay protocol.

Earlier staged attempts stopped at the existing Go API readiness check. Captured
child output identified a Windows bind collision after the test's probe/release
port selection. The worker/API now bind port zero atomically and report their
owned address; bounded discovery replaces the released-port probe while the
HTTP readiness and live checks remain required. Six discovery tests cover exact/
over-limit output, invalid/duplicate/partial reports and child failure; allowing
reported port zero was detected by an in-memory mutation without changing source.
The next live run passed its behavior checks but failed deleting the generated
Go executable while Windows still held it. The Go test runner and live runner
now share owned-directory cleanup with at most 30 seconds of retry for Windows
access/sharing/lock errors (5/32/33); other errors and exhausted retries fail the
gate. Seven tests cover ownership, retries, deadlines and preserved primary
failures. Those original source gates included 143 tooling tests (141 passed, two
POSIX-only skips), workspace Rust/Go checks, frozen oracles and live task-path
checks.

### Serial A05 extension: SP-004-R03, 2026-10-16

The [serial extension evidence](../../experiments/SP-004/results.serial-guards.json)
records a fresh Windows/WSL2/Podman PostgreSQL 16.15 run against the unchanged
native baseline and expanded experimental fixtures. A distinct active manager
with no creation permission revoked/restored the subject's grant. Request/result
subject identity, manager operation/event attribution, digests, successor
versions and final state/history counts were checked separately.
All six NULL arguments, unregistered/inactive/missing managers, a missing subject
and a subject without a grant were rejected with the specified SQLSTATE and
primary diagnostic. Rejection probes and a distinct-manager rollback retained
the prior committed grant/event/operation state.

Removing management-authority admission allowed an otherwise forbidden mutation;
the rejection oracle detected it. Replacing manager attribution with subject
attribution was detected by the distinct-identity oracle. Both probes rolled back;
final state and guard restoration were checked. The networkless 256 MiB container
was removed with absence confirmed, and the initially stopped exclusive VM was
stopped. These serial cases exercise distinct actor rows but do not establish
concurrent lock ordering, full A05, real authentication or Linux-host execution.
Earlier R01/R02 records and their exact source hashes remain historical.
R03's exact exercised source files are preserved at `da59c69`.

### Bounded history and authority continuation: SP-004-R04, 2026-10-17 UTC

The approved PR #58 plan merged in [PR #59](https://github.com/MrQuality/kehila/pull/59)
on 2026-10-18 at `d168256419d247033e9648fddc61e72835f12054`.
The [R04 evidence](../../experiments/SP-004/results.authority.json) records exact
source `42389e9a23796a60cf1deab9dc99c0fa0366e6ea`, exercised source-byte hashes,
PostgreSQL 16.15 on Windows/WSL2/Podman, observed blocking backend pairs and
confirmed cleanup. The networkless fixture used one CPU, a 256 MiB memory limit
and ephemeral storage; preflight observed 6.19 GiB available host RAM after VM
startup. This is a point observation, not a peak-memory or production-capacity claim.

H01 rejected an earlier gap while current/latest still agreed, altered earlier
attribution and a changed retained request. Every injection rolled back. Removing
the gap check was detected; the original audit and rejection were restored.
H02 audited every event at each size and retained all nine actual query plans,
including `project_grants_by_actor` and `project_grant_events_pkey` index use.

| History events | Current/latest execution, three observations (ms) | Full audit host round trip (s) |
| --- | --- | --- |
| 10 | 0.040–0.058 | 0.422 |
| 100 | 0.047–0.062 | 0.500 |
| 1,000 | 0.059–0.130 | 0.500 |

Query figures are PostgreSQL EXPLAIN execution times. Audit figures include
host/Podman/client overhead. These small fixture observations define no throughput
threshold and do not measure an application adapter.

C01/C02 reached all 40 main creation/replay schedules: ten command-first and ten
revocation-first trials for each operation. Four additional first-participant
rollbacks required fresh authority on retry. Eight project save/replay fixtures
tested both orderings and rollback, allowed control-project commands and creation
replay after project-grant removal. Exact operation/payload/project/revision/grant/
seed counts and request/result/actor attribution were checked.

C04 observed both actor lock orders with reversed manager/subject arguments,
rolled management changes back, and reached competing-writer commit/rollback
cases. Stale expected versions were rejected; a fresh retry advanced to version
three. The new creation-permission helper rejected NULL/invalid-token, authority,
missing-subject, stale/no-op and forged-log inputs without changing state.

Two further native mutations bypassed creation authority and reversed manager
lock order. Typed outcome/barrier oracles detected both, restored the functions
and reran intact cases. Lock-order evidence requires the later-actor barrier first.
A shortened-deadline case first reached an uncommitted management operation,
then verified backend/process removal and complete grant/event/operation rollback.
Startup failure cannot qualify that timeout probe. Actor permissions were restored.

A pre-commit attempt stopped before concurrency because PostgreSQL's composite
JSON aggregate emitted physical newlines. Cleanup succeeded. Metadata now uses
a compact JSONB root; the native regression retains the incompatible producer
without relaxing strict one-record decoding. The evidence preserves that failed
attempt's exact source hashes and diagnostic. Lightweight regressions also caught
Windows CRLF markers and prerequisite failures that could otherwise falsely
credit timeout/mutation evidence.

All four source commit gates passed; the final Windows staged matrix ran 261
tooling tests (258 passed, two POSIX-only skips and one unavailable Windows
symlink privilege), Rust/Go/static/frozen-oracle checks and every required live
phase. [Hosted verification](https://github.com/MrQuality/kehila/actions/runs/39067843956)
passed separately at `42389e9`. Hosted checks do not execute this opt-in native run.

R04 qualifies the declared database fixtures and platform. Credential/session/
peer mechanisms, read/projection races, composite original scopes, tombstones,
expiry and production codec/adapter adoption remain outside this evidence.
R01-R03 and their failures remain unchanged historical records.

### PR #59 review follow-up

Two focused source commits tighten the participant contract: `e01ef5e` validates
encoded commands before changing pending state and covers session bounds/backend
absence; `9919460` settles the watchdog before classifying teardown and retains
cleanup failures under the original body error or observed timeout. Startup
failures still release acquired files/processes. Production SQL is unchanged.

All 36 focused regressions passed. Eight targeted in-memory mutations were
detected: inverted backend absence, relaxed command size, removed in-flight and
pending-work guards, removed cumulative output bound, omitted watchdog startup,
omitted uncommitted-operation prerequisite and a weakened cleanup-failure oracle.
These are harness checks, separate from native PostgreSQL evidence. Deterministic
expiry-during-cancellation cases retain simultaneous supervisor/backend failures;
the timeout probe rejects incomplete prerequisites and dirty cleanup.

Both source commit gates passed. The latest Windows staged matrix ran 273 tooling
tests (270 passed, two POSIX-only skips and one unavailable symlink privilege),
plus the required Rust/Go/static/oracle and service phases. An earlier local gate
completed its test phases but failed to remove a locked temporary directory;
explicit VM ownership and shutdown allowed the unchanged staged retry to pass.

[R05](../../experiments/SP-004/results.authority-review.json) reran the complete
bounded native suite at `99194601c833d3aa7ee8aa58df2b6bf0e9e0361b`: PostgreSQL
16.15 on Windows/WSL2/Podman, 52 schedules, H01/H02, three native mutations,
role/input rejection and timeout rollback/cleanup all passed. The run took
688.031 seconds and confirmed participant/backend/container/VM cleanup. It retains
all nine query plans and losslessly encoded per-case state observations, with
runtime/test source hashes. [Hosted verification](https://github.com/MrQuality/kehila/actions/runs/39093737133)
also passed at that source; it does not execute the opt-in native suite. R04
remains unchanged. Authentication, broader command scopes, production adoption
and Linux-host native qualification remain outstanding under #40/#49.

## Conclusion and reuse boundary

Grant-management separation, explicit history audits, transactional attribution
and the selected actor-fence schedules are supported for the recorded fixtures.
Production schema, authentication, session, peer-channel and broader command/read
qualification remain separate obligations.
Reuse SP-002 only within its stated boundaries; ADR-103 remains Proposed.

<a id="next-bounded-slice-history-and-concurrent-authority"></a>
## Completed bounded slice: history and concurrent authority

**Status: plan approved through merged [PR #58](https://github.com/MrQuality/kehila/pull/58);
implementation authorized on 2026-10-17 and merged in
[PR #59](https://github.com/MrQuality/kehila/pull/59) on 2026-10-18.
Bounded native R04/R05 verification passed at the recorded sources above.**
This approved slice covered database permission-safety checks. Authentication
and application routes were outside that slice. PR #54 merged on
2026-10-16 at `7b7e78825e1b639e2a222bb1403d24b74e71665a`; its serial results are
the baseline. This plan adds no successful execution claim or architecture decision.
The maintainer owns scope approval and result assessment under SOP-001.

### Scope and authority distinctions

- Extend the existing disposable SP-004 overlay/runner after the unmodified
  native baseline and retained serial cases pass. Keep production SQL, codecs,
  application APIs and dependencies unchanged. Preserve R01-R03 evidence.
- Project access and project-creation permission are different scopes. Revoking
  an `initial_owner` project grant must deny that project's fixture command and
  replay; it must not be used as evidence that `project_create` is denied.
  Creation/replay denial instead changes the fixture actor's current
  `can_create_project` under the same actor fence, without changing its identity.
- A creation/replay driver must execute the reviewed lookup, current-authority
  and transaction ordering explicitly against native records. Project-scoped
  save/replay checks use clearly labelled fixtures, not an invented production
  mutation family. Neither constitutes application adapter integration.
- Any new actor-permission management helper is disposable test infrastructure.
  Review its exact role/DDL diff before execution: restricted manager capability,
  trusted fixture identity, deterministic manager/subject locks, no serving
  UPDATE rights, rollback and unchanged unrelated actor/project state. This
  selects no credential/session mechanism or production actor-history schema.

### Cases and required observations

| Case | Controlled action | Passing evidence |
| --- | --- | --- |
| H01: complete-history audit | Audit a coherent grant chain; then separately remove an earlier event while leaving current/latest equal, alter earlier attribution, and alter a retained request/digest association. Only the trusted fixture administrator may inject corruption. | The explicit audit accepts the coherent fixture and rejects each corruption for its specific reason. Verify version-one origin, every successor, operation/event/subject attribution and retained request/result/digest coherence. Roll back each injection and verify restoration. Current/latest lookup alone is not credited with detecting an earlier gap. |
| H02: growing history | Build valid chains of 10, 100 and 1,000 events with distinct operation tokens; run three current/latest checks and one explicit full audit per size. | Record exact row counts, indexed query shape/plans, timings and outcomes. Serving checks request only the indexed current row and latest event; full scans belong to the declared audit. These observations establish no throughput target. A bounded timeout is an incomplete measurement, not a pass. |
| C01: creation/revocation order | Reach creation-first and creation-permission-revocation-first using independent restricted sessions and the actor fence; ten trials of each. | A command holding the fence may commit before revocation. Earlier committed revocation denies the later command before persistence. Verify the complete operation/project/revision/grant state, with no partial accepted creation or unrelated changes. |
| C02: replay/revocation order | Seed a committed creation, then reach replay-first and creation-permission-revocation-first; ten trials of each. | Replay checks current creation authority before exposing saved outcomes. Earlier revocation denies even an identical retry, preserving the original committed record. Successful replay creates no second operation/project/event. |
| C03: rollback and project scope | For each C01/C02 ordering, roll back the first transaction and retry in a fresh bounded transaction. Separately repeat command-first and revocation-first with a project-grant save/replay fixture. | Rollback releases the fence; retry reloads current authority. Project-grant denial applies only to that project; another project's grant and creation authority remain independently evaluated. Fixture-only results stay distinct from native creation evidence. |
| C04: multiple actors | Use distinct managers/subjects, then concurrent management requests with the same affected actors supplied in opposite argument orders. Include rollback and expected-version contention. | Both acquire the complete actor set in C collation order before other rows. Record reached locks/waits; no lock-order inversion, partial operation/event/current change or duplicate successor is accepted. A stale expected version is rejected; retry requires fresh state. |

### Controlled scheduling, bounds and failure evidence

Use independent native PostgreSQL connections and explicit coordinator barriers.
Identify backend PIDs, observe the intended blocker with `pg_blocking_pids`, then
release commit/rollback and check both outcomes plus final durable state. Sleeps,
elapsed time or a lock timeout alone do not establish that a schedule was reached.
The coordinator may inspect lock metadata; application fixtures retain restricted
roles. No credentials or personal data belong in recorded diagnostics.

Concurrent participant bounds are 10 seconds for lock waits, 20 seconds per
statement, 30 seconds idle in transaction and 60 seconds per participant process.
The coordinator must release each barrier within the lock budget; missed barriers
fail the case. Keep existing serial-case bounds unchanged. On failure, cancel and
reap participants, roll back outstanding transactions, verify fixture state and
retain both primary and cleanup failures. No unbounded automatic retry is allowed.

Demonstrate that the negative oracles detect a skipped earlier-gap check and a
skipped post-fence current-authority check. Demonstrate that reversing actor lock
order is detected by the observed-lock/order oracle. Run each altered fixture only
in isolated disposable state, restore it, and rerun the intact case. A deadlock or
timeout without the expected lock evidence does not qualify mutation detection.

Approved continuation budget: 90 minutes excluding setup and mandatory checks;
stop at a prerequisite failure, exhausted budget or bounded conclusion. Report
unreached schedules and incomplete cases explicitly rather than expanding scope.
Before implementation is executed, publish exact reproduction commands, changed
fixture/helper sources and reviewed role grants. Pin PostgreSQL 16.15 by the
existing image digest, use one compiler job and the existing networkless 256 MiB
disposable container, and check host/disk/port prerequisites. **Ask the maintainer
for permission before memory-heavy builds, native tests or VM/service startup,
even when RAM appears sufficient.** After permission, require at least 3 GiB host
RAM available after VM startup and 2 GiB free disk; ask again if headroom is lost.

### Exit and retained work

Record source hashes, platform/image, attempted cases/trial counts, observed
barriers, SQLSTATE/primary diagnostics, final-state assertions, history measurements,
mutation results and confirmed participant/container cleanup. Keep native evidence
separate from orchestration unit tests and hosted checks. Update this record,
the spike register, #40/#49 and the implementation PR summary with actual results.
This slice can resolve specific A05/A07/A08 database cases only. Authentication,
session/internal-channel choices, remaining read/composite-scope/tombstone cases,
production schema/adapter adoption and supported-platform qualification remain
open. #40/#49 must not close on this bounded implementation or on partial experimental results.

## Decision and follow-up

Deliver a supported recommendation and retain rejected alternatives/limitations.
Update existing decisions/questions only when the maintainer accepts a choice.
The parent issue remains responsible for production implementation and regression
coverage; finishing this investigation does not close it.

## Next design slice: owner authentication and trusted caller identity

**Status:** [ADR-105](../architecture/ADR-105-local-owner-authentication.md) and its
[verification plan](../implementation/local-owner-authentication-verification.md)
record the accepted local-MVP direction under #40/#11, feeding #49 W2. The
maintainer explicitly accepted ADR-105 and authorized A0 on 2026-10-19;
[approval record](https://github.com/MrQuality/kehila/issues/40#issuecomment-6262900825).
No code, schema or authentication evidence is supplied by acceptance. ADR-103
and separately scoped A1-A5 implementation retain their own gates.
The single-active-runtime mechanism is a local-MVP profile only;
[B-032/#65](../product/backlog.md#b-032) owns final replicated service/session
design and migration. Existing actor-wide serialization needs separate contention
measurement; replica count does not imply concurrent mutations for one actor.

The [PR #64 follow-up](../implementation/local-owner-authentication-verification.md#pr-64-review-follow-up)
defines installation-before-actor takeover fencing, bounded password/pre-session
admission and retryable but not guaranteed logout audit delivery. These proposed
contracts and T01–T16 remain unexecuted; R01–R05 evidence is unchanged. Follow-up
also defines capacity reclamation for completed logouts and reload-safe CSRF
retrieval, without changing stable identity, revocation or existing read ordering.

Prepare a bounded proposal for owner enrollment/login, browser sessions and
CSRF/origin handling, authenticated CLI access, expiry/logout/recovery and
credential revocation. Compare authenticated Go-to-worker context transport
with consolidating that boundary. Specify trust, secret storage/lifecycle,
stable actor binding and concrete denial/recovery cases before implementation.
Test fixtures and Host/loopback checks cannot supply caller authentication.

Apply [D-037](../product/decisions.md#d-037) and the
[design review guidance](../architecture/README.md#post-mvp-design-considerations):
record recovery dependencies, display/locale independence and future agent
identity/delegation/attribution compatibility. Agent enrollment or execution
remains B-031/#62 post-MVP work. Retain the one-owner MVP contract, current-grant
checks and existing real-data gates. Detailed mechanisms need their own reviewed
proposal/ADR; this handoff selects none and supplies no implementation evidence.

## Handoff

The [history and concurrent-authority plan](#next-bounded-slice-history-and-concurrent-authority)
was approved through merged PR #58 and merged in PR #59 on 2026-10-18.
Its [R05 evidence](../../experiments/SP-004/results.authority-review.json) reverified
the bounded history/concurrency cases at `9919460`; R01-R04 remain historical.
The [next design slice](#next-design-slice-owner-authentication-and-trusted-caller-identity)
addresses real owner credentials and trusted caller identity with D-037 review inputs.
Actual credential/session/channel decisions, read/projection ordering and broader
replay scopes remain #40 acceptance gaps. #11/#32 retain production access,
#41 retains stable allocation, and #49 retains native integration. Heavy local
execution requires separate permission and fresh resource preflight.
