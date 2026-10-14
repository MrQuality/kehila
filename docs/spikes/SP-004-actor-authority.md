# SP-004 — Trusted actor binding and initial-owner authority

| Field | Value |
| --- | --- |
| Status / outcome | Contract proposal for review / no experiment results |
| Owner / assessor | Project maintainer; execution and assessment to be confirmed before starting |
| Created / last updated | 2026-10-08 / 2026-10-14 |
| Tracking issue | [40](https://github.com/MrQuality/kehila/issues/40) |
| Parent delivery issue | [#11](https://github.com/MrQuality/kehila/issues/11) |
| Prior evidence | [SP-002](SP-002-postgresql-project-create.md) |
| Effort limit / stopping condition | Proposed 90 minutes; confirm before execution. Stop on failed prerequisites or supported/bounded inconclusive conclusion. |

## Question, cases and execution plan

**Parent:** #11. Related: #32, #49, #27, #41; historical #26. Follow-up to SP-002.

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

This section proposes the W2 boundary; it accepts no new ADR or product decision.
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

### Proposed initial-owner permission mapping

The existing SQL profile is a storage label, not an implemented permission set.
The following mapping requires maintainer review under Q-014.

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

Proposed mutation/replay protocol:

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

Proposed read semantics: authorize at a fresh PostgreSQL snapshot established for
that request. Any logout, credential revocation or permission change committed
before that snapshot must be observed. An authorized read may finish if revocation
commits after its snapshot; it does not promise revocation of bytes already being
returned. This ordering requires maintainer review alongside session semantics.

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

### Decisions needed before experimental execution

- Approve or amend the owner mapping, actor-fence protocol and proposed read
  snapshot/revocation semantics above.
- Choose the local enrollment/credential mechanism, browser session transport
  and authenticated CLI mechanism; define expiry, logout and recovery behavior,
  including how durable actor identity survives credential replacement.
- Choose authenticated Go-to-worker context transport, or consolidate that
  boundary; do not treat the current loopback listener as trusted identity.
- Approve the bounded experimental scope/time limit below. Synthetic verified
  identity can test authority plumbing but cannot qualify real authentication.

These are owned by #40 under #11/#32. #49 owns runtime integration; #41 consumes
the reviewed stable actor identity. No extra historical-assessment issue is needed.

### Experimental schema boundary

Proposed choice: an explicit post-install overlay in a fresh disposable native
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

1. **Goal & Context:** Review the proposed contract above against Q-002/Q-014, then investigate actor binding and authority/revocation ownership for #49 W2. The inventory establishes the missing boundary; this is not another database feasibility investigation.
2. **Definition of Done:**
   - [x] Document the proposed actor/session contract and initial-owner permission mapping against B-003/B-007 for review.
   - [ ] Demonstrate denied spoofed actor input, missing/expired identity and forbidden role escalation using a declared experimental integration.
   - [ ] Demonstrate current grant/event agreement and atomic revocation in a separately scoped management prototype; retained history alone must not authorize.
   - [ ] Repeat selected creation/replay versus revocation orderings; preserve observed lock barriers.
   - [ ] Publish SP-004 recommendation and gaps; actual authentication and grant-management implementation remain open.
3. **Dependencies & Prerequisites:** Resolve the decisions above before execution. Use #11/#32 requirements, B-003, ADR-104 and SP-002's source-bound locking observations. #41 receives stable actor identity; its allocation protocol is not implemented here. Check available memory before compiler/service work; request headroom if the selected workload cannot fit.
4. **Risks & Mitigations:** Supplied identity, cached roles and database capability possession cannot establish caller authority. Actor-wide serialization is deliberately conservative and may limit concurrency; measure selected schedules rather than claiming throughput. Freeze affected-actor lock ordering and protect both the management caller and grant subject. Detect current/event mismatches and never repair them implicitly during serving.
5. **Spikes & Open Questions:** Proposed 90-minute investigation, excluding setup and required checks, after contract/decision review. Stop on failed prerequisites, a bounded conclusion or the time limit; unresolved real authentication remains an explicit blocked case. A synthetic identity fixture must not turn that case into a pass.
6. **High-Level Architecture / File Changes:** Keep this record canonical. A later approved experiment may add isolated `experiments/SP-004/` code, fixtures and a bounded runner using the explicit disposable overlay above. Production SQL files stay unchanged; the changed experimental guard set and role limits require their own review before execution. Such results cannot qualify a production migration; #49 retains that separate gate.
7. **Verification & Testing Plan:** Use the cases below with independent restricted connections and explicit barriers. Record actual source, image, platform, role, state before/after, observed waits, commit/rollback and primary diagnostics. A timeout is not evidence that the intended race was reached. No experiment or service execution has occurred for this proposal.

Status: contract proposed for review; experiment not executed. Completing this
spike leaves #11/#32 responsible for production access delivery.

### Required experimental cases (not executed)

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

Before an approved run, add exact reproduction commands and pin the experimental
identity implementation, role/schema overlay, PostgreSQL image and lock/statement/
process deadlines. Start from fresh disposable state and retain negative-case
diagnostics and cleanup failures. No executable command is claimed until that
runner exists. These cases define acceptance, not successful observations.

## Environment and reproduction

Not executed. Use Podman for database/service cases and the public contribution
and testing guides. Record tools, image pins, scope, timeouts, fixtures, commands,
barriers and fresh preflight before starting. Experimental artifacts belong in
`experiments/SP-004/` once needed; no executable prototype is claimed here.

## Run summaries

No runs. Expected cases in the plan are planned observations, not test results.

## Conclusion and reuse boundary

No conclusion or accepted architecture choice. Reuse SP-002 only within its stated
baseline, role/helper, runtime and fault boundaries. Further experiments must
resolve this record's stated uncertainty rather than repeat engine feasibility.

## Decision and follow-up

Deliver a supported recommendation and retain rejected alternatives/limitations.
Update existing decisions/questions only when the maintainer accepts a choice.
The parent issue remains responsible for production implementation and regression
coverage; finishing this investigation does not close it.

## Handoff

Latest work: source inventory, proposed actor/owner/revocation contract and
negative/concurrent cases. Next action: maintainer review of the proposed mapping,
credential/session/channel decisions and bounded experimental scope. After that
review, prepare the runner and measure memory before execution. #40 stays open
until the scoped investigation has results and an explicit disposition; #11/#32
retain production access delivery and #49 retains native integration.
