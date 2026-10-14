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

### Authority, history and revocation ownership

Proposed native protocol: begin a bounded transaction, lock the verified actor
with `lock_actor`, re-read active/current creation authority, then perform scoped
operation lookup. For project-scoped commands, load current grant rows and their
matching latest events under the same authority fence. Require matching profile,
active state and version; missing/mismatched history is an integrity failure and
must never grant access. Use the stored original scope for replay. Do not return
replay, changed-content, expiry or mutable business details before the applicable
current authority succeeds.

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

### Decisions needed before experimental execution

- Approve or amend the owner mapping above and the actor-fence protocol.
- Choose the local enrollment/credential mechanism, browser session transport
  and authenticated CLI mechanism; define expiry, logout and recovery behavior,
  including how durable actor identity survives credential replacement.
- Choose authenticated Go-to-worker context transport, or consolidate that
  boundary; do not treat the current loopback listener as trusted identity.
- Approve the bounded experimental scope/time limit below. Synthetic verified
  identity can test authority plumbing but cannot qualify real authentication.

These are owned by #40 under #11/#32. #49 owns runtime integration; #41 consumes
the reviewed stable actor identity. No extra historical-assessment issue is needed.

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
6. **High-Level Architecture / File Changes:** Keep this record canonical. A later approved experiment may add isolated `experiments/SP-004/` code, fixtures and a bounded runner. Session/grant-management schema must be isolated experimental objects, with separate serving/management logins. Do not edit the production installer, remove its creation-only guards, or give creator UPDATE rights. Such fixture results cannot qualify a production migration; #49 needs a separate reviewed schema evolution plan.
7. **Verification & Testing Plan:** Use the cases below with independent restricted connections and explicit barriers. Record actual source, image, platform, role, state before/after, observed waits, commit/rollback and primary diagnostics. A timeout is not evidence that the intended race was reached. No experiment or service execution has occurred for this proposal.

Status: contract proposed for review; experiment not executed. Completing this
spike leaves #11/#32 responsible for production access delivery.

### Required experimental cases (not executed)

| Case | Controlled action | Required evidence |
| --- | --- | --- |
| A01: trusted binding | Verify owner identity, then supply a conflicting payload/header actor, role or grant assertion; repeat with missing/expired credentials. | Context derives only from the verifier; unauthenticated requests reach no command execution. A deterministic verifier fixture proves plumbing only; actual provider qualification is a separate result. |
| A02: internal peer | Attempt direct worker submission and altered actor/context from an untrusted process. | The selected internal channel rejects it. If no mechanism is selected, record blocked, not passed on the basis of Host/loopback checks. |
| A03: owner scope | Exercise active/inactive owner, creation disabled, absent/inactive project grant and another project. | Evaluate the proposed mapping independently in each scope; reject role escalation and cross-project access. No ownership inferred from an existing creation result. |
| A04: read/CLI/session | Try authoritative and projected reads without identity; CLI without Origin; cross-origin browser command; expired/logout/recovered session. | Authenticated CLI and reads use the declared policy; browser checks are additional. Recovery preserves actor ID and invalidates replaced credentials under the approved ordering. Unselected policy remains blocked. |
| A05: grant history | Remove/mismatch current grant versus latest event; attempt serving-role grant UPDATE and history mutation; commit/roll back management changes. | Mismatch fails without allowing access. Restricted serving writes fail; successful management writes atomically append one matching event and advance the current version. Rollback leaves both unchanged. |
| A06: replay precedence | Retry an original success, changed request and expired/tombstoned record after original authority is revoked. | Current original-grant denial precedes replay/conflict/expiry disclosure; the original committed operation remains unchanged. Creation replay checks current creation authority, not a newly invented ownership substitute. |
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
