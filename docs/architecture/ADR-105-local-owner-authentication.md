# ADR-105 — Local-owner authentication and trusted caller identity

Status: Accepted for the local MVP. Owner: project maintainer. Created / reviewed: 2026-10-19.
Acceptance: [explicit maintainer approval, 2026-10-19](https://github.com/MrQuality/kehila/issues/40#issuecomment-6262900825).
The maintainer accepted this local-MVP architecture and authorized the bounded
A0 transport/dependency investigation. A1-A5 implementation plans, dependency
adoption and production qualification retain their separate review/evidence gates.
Memory-heavy execution and certificate trust-store changes require separate
permission. This acceptance does not qualify any unexecuted security mechanism.
Single-active operation remains local-MVP-only; replicated deployment stays
open under B-032/#65. ADR-103 production adoption is not accepted here.
Related: D-016/D-017/D-037; B-007/B-020/B-022; Q-002/Q-014;
[#40](https://github.com/MrQuality/kehila/issues/40),
[#49 W2](https://github.com/MrQuality/kehila/issues/49), #11/#32 and #41.

## Context

The [SP-004 contract](../spikes/SP-004-actor-authority.md) defines stable actors,
current authority, original-scope replay and revocation/read ordering. PR #59
verified selected database fixtures; it supplied no caller authentication.
The current [Go API](../../go/io/task_api/server.go) proxies to an unauthenticated
[Rust worker](../../src/io/task_worker/src/main.rs) and queries search directly.
Their Host/origin/loopback checks do not identify an owner or authenticate a peer.

### Review summary

| Accepted local-MVP choice | Effect and remaining gate |
| --- | --- |
| Separate owner password | Store a salted Argon2id verifier; browser and CLI authenticate with the password. No external identity provider. |
| Rust authority boundary; one active instance for local MVP only | Terminate HTTPS, verify identity, enforce domain authority and mediate search within the Rust service. The local MVP singleton is not a final replica-count constraint; the boundary is accepted and implementation remains subject to scoped gates. |
| Local-MVP opaque browser/CLI sessions | Tokens carry no permissions. This profile keeps sessions in memory and ends them on restart; it does not supply multi-replica session continuity. |
| Local administrative enrollment/recovery | No public first-owner claim/reset endpoint. Recovery preserves the actor and history, replaces the password and revokes sessions. |
| Separate implementation gates | Certificate trust, dependency choices, schema/capabilities and negative/native tests need evidence before support. ADR-103 remains Proposed. |

## Decision — accepted for the local MVP

### Deployment scope and transition contract

The runtime protocol below is a **local-MVP deployment profile**, not the final
replicated architecture. A logical authority service implemented in Rust does
not imply a single pod. The installation advisory lock, runtime generation used
to exclude other serving instances, in-memory credential/CSRF registry, restart
logout, literal-loopback certificate profile and installation-local budgets apply
to this MVP profile. They cannot qualify a cluster by increasing replica count.
Do not weaken the local singleton fence to run multiple active instances without
a replacement protocol. Current code and actual supported deployment are unchanged.

[B-032 / #65](https://github.com/MrQuality/kehila/issues/65) owns the required
post-MVP design for multiple active replicas and secure remote deployment, before
claiming support for that final deployment profile. Its reviewed successor or
amendment ADR must define authoritative cross-replica sessions/CSRF/revocation,
expiry and clock/version semantics, coordination and actor contention, aggregate
admission/pools/hash limits, proxy/peer trust, secret/certificate rotation and
backend failure domains. A request must not require a particular pod; losing a
pod must not globally invalidate valid sessions. Mandatory sticky routing or a
pod-local authoritative credential registry cannot satisfy that requirement.
No shared store, token format, identity provider or cluster stack is selected here.

Migration must preserve stable actor, operation, replay/tombstone and attribution
contracts, independent authorization and ADR-104 edge ceilings. Specify explicit
transition re-login if needed, fence old runtimes and keep supported restore
credential invalidation. Replica-local runtime generations cannot silently become
global identity or credential versions. Native adapters must keep credential
resolution separate from domain identity and persistence; no domain contract may
require routing back to the issuing process. Prototype legacy paths remain isolated.
Review the new profile independently; this is no approval to implement it now.

### 1. Authority process and transport

For new native M1 routes, consolidate the public API, authentication, domain
command boundary and authorized read/search mediation in one Rust process.
Internal calls carry a typed private `ActorContext`, not an HTTP actor assertion.
Keep pure modules independent of runtime I/O. The current Go/Rust task prototype
and its frozen evidence remain unchanged until a separately reviewed transition.
No anonymous legacy route or direct search/worker listener may be exposed as an
alternate supported path. This does not adopt ADR-103 or implement W3/W7.
The supported profile must disable those legacy listeners/routes and reject a
configuration that shares its authoritative dataset with the anonymous prototype.
Acceptance must address the Go/Rust boundary in D-016/ADR-101 for future native
routes explicitly, preserving their data/replay guarantees and historical evidence;
any required product-record amendment accompanies acceptance rather than inference.

Use one configured HTTPS origin on literal loopback, initially
`https://127.0.0.1:<configured-port>`; bind only that address. Reject other Host,
authority, origin, port and forwarded-host assertions. No wildcard CORS, LAN
binding, forwarded identity, automatic redirect carrying credentials or HTTP
fallback. Do not infer owner identity from the socket address or TLS alone.

[ADR-106](ADR-106-local-container-transport.md), explicitly accepted on 2026-10-23
for the bounded disposable diagnostic proof only, amends the listener rule for
one specific container IPv4 address behind Windows loopback publication. Client
origin remains literal loopback and Rust still terminates TLS. It does not extend
production authentication support or qualify ingress, storage or resume behavior;
all other requirements here and execution gates retain their scope.

ADR-106's [2026-10-27 extension](ADR-106-local-container-transport.md#accepted-authenticated-development-extension-2026-10-27)
also accepts this bounded profile for synthetic, disposable authenticated tests.
It supplies no real-data, supported-installation or supported-suspend guarantee;
schema/capability review and execution/trust permissions remain separate.
The [2026-10-30 local-caller amendment](ADR-106-local-container-transport.md#accepted-local-caller-amendment-2026-10-30)
permits untrusted same-PC Windows/WSL/container callers to reach the application
in disposable authenticated tests only. It selects no supported-installation
exposure boundary and grants no caller authority.

The installer/operator supplies a server certificate trusted by the actual
browser and CLI, with the exact IP SAN and protected private key. Startup rejects
missing/expired/mismatched material. CLI verifies the configured trust and server
identity. Do not disable verification or ask users to bypass browser warnings.
Certificate issuance/trust/renewal and removal need a usable Windows/Podman proof
before the first implementation slice; Linux qualification is separate. No
certificate tool, package or trust-store installer is selected by this ADR.

Enforce one authority instance per installation with a dedicated PostgreSQL
session advisory lock and one durable installation-state row containing the
immutable owner binding and checked, non-wrapping `runtime_generation`. The row
is also the transactional takeover fence; the advisory connection alone cannot
authorize requests. Initial enrollment creates the singleton atomically.

All serving mutations, replay disclosure, session issuance/revocation and local
credential/permission management acquire locks in this order: installation row,
then every affected actor in `COLLATE "C"` actor-ID order, then operation/domain
locks. Normal operations take a shared row lock (`FOR SHARE`); takeover takes an
exclusive row lock (`FOR UPDATE`), then the owner actor fence, and advances the
generation atomically. No participant may acquire the installation lock after
an actor/domain lock or upgrade its shared installation lock to exclusive.
Any additional lock classes need an explicit compatible A1/W3 review.

After obtaining the installation and actor locks, a fresh READ COMMITTED
statement for serving work checks the durable runtime generation, credential generation/revision
and current authority before execution or replay disclosure. A snapshot taken
before a lock wait cannot authorize the operation. The shared installation lock
is retained through commit/rollback, so takeover waits for already-fenced work;
later old-runtime operations observe the new generation and deny. Retry starts
a new transaction and repeats all checks. Password hashing precedes these locks.
Recovery uses the same shared-installation/actor ordering and atomically advances
the owner's auth generation; it does not impersonate the serving runtime.

Reads check runtime/auth generation in their fresh authorization snapshot without
an exclusive fence. SP-004's bounded earlier-snapshot allowance also applies to
takeover/recovery during projected search; local flags cannot detect every
out-of-process change. Recheck local revocation/expiry and observed lock loss
before output, but do not claim a distributed post-search atomic check.
Observed loss or uncertain ownership of the advisory connection stops admission
and suppresses pending output. Reacquisition requires a fresh runtime and new
sessions. The advisory key, restricted functions/roles, statement isolation and
startup/unknown-commit cleanup require A1 schema review and native proof.

### 2. Owner enrollment, password and recovery

A separate local administrative command holds provisioning/recovery capability;
the serving process never receives that capability. Initial enrollment atomically
claims the one installation owner, creates its stable actor and initial creation
permission, and records attributable management history. A unique installation
claim serializes competing initializers. Retry after uncertain commit reconciles
that claim; it must not create another owner or silently replace credentials.
The tool reads secrets from a protected terminal, never arguments or environment.
Password login resolves this installed owner; a client username/actor ID cannot
select another principal.

Store only a salted Argon2id verifier, its allowlisted algorithm/policy version,
verifier revision and checked `auth_generation` linked to the durable actor.
Passwords never enter domain operation keys, fingerprints, events, logs or saved
results. Provisional cost: 64 MiB, three iterations, one lane, 16-byte random salt,
32-byte output. This is a proposed resource/security setting, not a measured
performance claim or the exact RFC 9106 recommended profile. Bound stored PHC
parameters before verification; corrupt/unsupported profiles fail closed rather
than allocating arbitrary memory or falling back to another algorithm.

Accepted password policy: NFC-normalize password input only; require 15–128
Unicode code points after normalization, with at most 1,024 decoded UTF-8 input
bytes and bounded normalization/output. No trimming, case folding, composition
rules or periodic password replacement. Permit paste/password managers. Reject
common/compromised choices using a versioned local blocklist with reviewed
provenance/license; never send passwords to a third party. Domain content retains
ADR-104's exact-string rules: password normalization cannot spread to codecs.

Password change requires current session, CSRF where applicable and current
password verification. Acquire the single hash permit once for sequential
current-password verification and replacement hashing; never release/reacquire
between them. Hash outside database fences, releasing the permit only after
the actual hash work ends, then acquire installation/actor locks and
recheck the exact verifier revision/generation before changing anything. Stale
verification fails this attempt without an automatic rehash loop. Increment
`auth_generation`, replace the verifier and audit atomically; old sessions fail
current-generation checks, including the requesting session. Reauthenticate.

Forgotten-password recovery requires the separate local administration capability,
not a browser flag or access to a loopback port. Preserve actor ID, grants,
operation identity and history; replace the password, advance generation and
record a distinct recovery event under the actor fence. It is intentional local
administrative authority, not proof that the old password was supplied. OS
account/administrator or authority-process compromise remains outside what this
password boundary can contain; ordinary other local callers still need secrets.

Credential changes do not use the business replay ledger or persist password
fingerprints. Their generation/revision CAS prevents stale replacement; clients
must not blindly retry after a lost response. Reconcile by fresh login with the
chosen password and authenticated credential metadata; recovery is explicit.

### 3. Browser and CLI credentials

Issue 32 CSPRNG bytes encoded as unpadded base64url. Retain only a token digest in
a bounded in-memory registry, with a distinct public session ID, actor ID,
credential kind, auth/runtime generation, monotonic issue/activity/deadline ticks
and a monotonic revocation flag. Neither the public session ID nor a digest is a
credential. Token matches do not carry permission verdicts. Use reviewed random,
digest and comparison primitives; no custom signature/MAC or JWT protocol.

Password verification runs outside the actor fence. Before issuing a session,
take the fence, recheck verifier revision/generation and reserve registry capacity.
Register the unpublished entry and audit issuance; return its secret only after
commit. Failure revokes/removes the unpublished entry. A later password change
can invalidate a token before delivery, which then requires new login. A lost
login response permits another bounded login, not replay of a stored secret.

Browser login uses a short-lived pre-session and server-held synchronizer token.
Generate an independent 32-byte CSRF secret; do not reuse the session credential.
Reuse a presented live pre-session for the same browser without extending its
original five-minute deadline. Reclaim expired/consumed entries before allocating;
use a bounded scan of at most 32 slots, not an unbounded expiry structure. New
allocations share an installation-wide five/minute token bucket, burst two,
separate from password login; reuse does not consume an allocation token. With
the proposed five-minute TTL this limits new live allocations to at most 27 in
one lifetime, below the 32-slot ceiling. Check both limits atomically; reject
excess with 429, without evicting live entries, extending expiry or allocating
per-client limiter state. Successful login consumes the pre-session only once.
Origin/browser checks constrain browser attacks but cannot authenticate another
local process. Repeated allocation traffic can exhaust the issuance allowance
and keep a new browser from signing in; this is an explicit residual local
denial-of-service risk, not an availability guarantee. Measure real login/retry
behavior under saturation; no supported result follows from the arithmetic alone.
Require exact configured Origin, JSON content type and the CSRF header on login
and every browser state change, including logout. Reject absent/null/foreign
Origin on those routes. Rotate pre-session state at successful login and issue
`__Host-kehila-session` with `Secure; HttpOnly; SameSite=Strict; Path=/`, no Domain.
Return the session's CSRF value only through same-origin, no-store responses.
Keep the independent CSRF secret retrievable in the bounded server-side
pre-session/browser-session entry; the digest-only rule applies to the bearer,
not this synchronizer value. Pre-session reuse returns its existing CSRF value.
An authenticated-page reload can retrieve the existing session CSRF value using
its HttpOnly cookie, with fresh credential/generation validation as for a read.
Retrieval is a dedicated read-only browser bootstrap operation: require a fixed
non-secret custom request header and `Sec-Fetch-Site: same-origin`, reject any
present nonmatching Origin and foreign/missing Fetch Metadata, and keep the exact
HTTPS authority and no-CORS policy. This read needs no previous CSRF value;
otherwise reload would create a circular dependency. Actual browser support
and header behavior must be qualified in A3, without a silent insecure fallback.
Return only CSRF data in a no-store response body, never a bearer or domain data.
Do not allocate a new entry, charge pre-session issuance, rotate the secret or
renew activity/expiry on retrieval. Concurrent tabs keep the same valid value;
rotate only when replacing the pre-session with a new authenticated session.
For an eligible retained revoked browser entry, retrieval returns only the CSRF
value needed to retry that same logout, under its existing generation/retention
checks. It cannot confer read/business authority. Unknown, expired, superseded
or reclaimed entries deny; retrieving a CSRF value never reconstructs them.
Do not expose the session credential to JavaScript, URL, localStorage or logs.
SameSite/Fetch Metadata are additional checks, not identity or CSRF substitutes.
Parse all Cookie fields according to the negotiated HTTP version; reject repeated
authentication-cookie names rather than selecting the first or last value.
Serve no third-party code on the login page; use output encoding, no-sniff,
same-origin resource policy and a CSP forbidding framing, objects and foreign
form/script targets. HttpOnly does not contain an XSS that can act as the owner.

CLI login prompts for the same owner password and uses a distinct HTTPS endpoint
with no Cookie, Origin or browser Fetch Metadata. It returns a CLI-kind bearer
credential; subsequent CLI calls require exactly one Authorization header.
Browser and CLI modes have explicit route/auth contracts: reject mixed, duplicate
or wrong-kind credentials and never downgrade an invalid browser credential to
an anonymous/CLI path. Missing Origin alone is not CLI authentication.

CLI secret storage is per OS user: DPAPI-protected content plus a restricted
owner ACL on Windows; a mode-0600 file in a mode-0700 owned directory on Linux
(not application-encrypted there). Verify ownership/permissions and reject
symlinks/reparse redirection; never fall back to a shared or public file. Do not
persist the password. These profiles require actual platform evidence; they do
not protect against code already controlling the same OS account.

Provisional lifetimes: 30-minute request inactivity and 12-hour absolute lifetime,
using monotonic ticks from the issuing runtime for both kinds. Only successful
authenticated business requests renew inactivity, not public health, failures or
keepalive routes. This measures request activity, not human keyboard activity;
background business polling can renew it. No refresh-token protocol. Restart
invalidates browser, CLI and pre-session state; the owner signs in again.

### 4. Authoritative admission, revocation and reads

Before any native pool acquisition or application/authentication worker scheduling,
validate bounded transport, original input and endpoint shape. Preserve ADR-104
ceilings: 1 MiB encoded records, 64 KiB decoded strings/keys, depth 16 and 1,024
collection entries, plus its existing node/duplicate rules. Auth endpoints use
the stricter provisional bounds below. No arbitrary JSON buffering before checks.

An early token lookup yields only a candidate credential reference. For mutations
and replay, acquire SP-004's actor fence, recheck the live registry entry,
monotonic expiry, actor/auth/runtime generation and current original-scope grants
before domain execution/result disclosure. Sample time after waiting for the
fence. Rollback/retry reloads authority; pre-queue checks cannot authorize work.
No Argon2, external search or unbounded I/O while holding this fence.

Individual logout/revocation marks the registry entry revoked while holding that
actor fence and records an audit event. Revocation is monotonic; audit failure
cannot resurrect an in-memory credential, and the operation reports failure.
Retained references observe the same revocation flag, not a cloned active value.
For current-session logout, retain bounded state `Revoked { audit_confirmed }`
on that same entry no later than its original idle/absolute deadline, without
renewal. Audit-confirmed entries may be reclaimed earlier for new-login capacity.
Only the exact retiring bearer/cookie may retry its own logout; browser retries
still require the retained CSRF secret and exact Origin. This exception grants
no business, read, password-change or other-session management authority.
Unknown, expired, superseded-generation or previous-runtime credentials return
401; they cannot reconstruct a retired registry entry. Reclaim expired entries
before session admission, with a scan bounded by the 64-entry registry. Pending
audit state counts against that cap and cannot outlive its original deadline.
After purging expired entries, new session admission must reclaim the oldest
audit-confirmed revoked entry if the 64 slots are otherwise full. Order by
monotonic revocation time, with public session ID as a deterministic tie-breaker;
scan at most 64 entries and atomically reserve the freed slot. Never reclaim an
active or unexpired audit-unconfirmed entry for capacity. Reclamation removes
lookup/retry state only: captured references remain irrevocably revoked and the
committed SQL audit is unchanged. No separate tombstone cache or token-to-audit
lookup is introduced. If only active/pending entries remain, return 429; pending
audit saturation retains the already-declared bounded availability trade-off.

Use one server-defined audit identity `(runtime_generation, public_session_id,
logout)` with a unique SQL constraint and fixed non-secret attributes. Retries
use the installation/actor lock order and check current generations; insert once
or confirm the identical committed event, never duplicate it. Mismatched stored
attributes are an integrity failure, not success. An uncertain SQL commit stays
unconfirmed until that lookup/insert establishes its outcome. Never store the
token, digest or CSRF secret in the audit record or business replay ledger.

Return 200 only when revocation and the audit are confirmed, including repeated
logout while that retained credential is eligible. Return 503 if local revocation
occurred but the audit is failed/unconfirmed; clients must present this as signed
out with audit unconfirmed, not as an active session. A client may retry only this
logout or discard its credential. It must never retry a business request to test
revocation. A subsequent 401 means no usable credential, not proof of an audit.
Response loss after commit is handled by the same stable audit identity while
the entry remains retained. Capacity reclamation ends that retry window: the
same token then receives 401, even when its audit committed. Clients treat this
as no usable credential and an unknown audit outcome, never as evidence that
logout or the audit failed. A reclaimed token cannot rebuild state or trigger
an unauthenticated SQL audit query. Retry guarantees are bounded by retention,
not a promise of 200 until the original session deadline.

This proposal deliberately permits a missing logout audit after client abandonment,
expiry, generation change or process failure before confirmation. There is no
durable retry queue or background reconciliation claim. Issuance, password change
and recovery still require their own committed audit before success. Guaranteed
logout-attempt auditing would require a separately reviewed durable protocol;
this availability/audit trade-off was included in the explicit local-MVP acceptance.
Logout success means later fence holders reject that credential. Password/global
revocation advances the durable generation under the same fence. Already committed
work remains committed; an uncertain response is not evidence of cancellation.

Authoritative reads check actor/auth/runtime generation, current grants and data
in one fresh consistent PostgreSQL snapshot as specified by SP-004. Check the
live registry/revocation flag and monotonic expiry again after that query before
releasing data; never reuse a cached successful credential verdict. A revocation
committed before the snapshot denies; a later one may conservatively deny too.
Projected reads authorize required scopes before search, release DB resources
before that call and enforce scoped hits/aggregates and the response deadline.
No exclusive actor fence or database transaction spans the search request.
Registry activity updates may only renew a still-live entry; they cannot grant
authority or undo a revocation/generation change. Registry locks span no awaits.

### 5. Provisional resource and failure contract

| Resource / operation | Provisional admission limit |
| --- | --- |
| Transport | 16 in-flight connections/requests; finite handshake/header deadlines; reject excess without an unbounded backlog |
| Auth JSON | 4 KiB encoded; exact endpoint keys; decoded password ≤1,024 bytes; no compression |
| Headers / target | 8 KiB aggregate headers; 2 KiB target; reject duplicate Authorization/CSRF fields and authentication-cookie names |
| Password verification | One 64-MiB Argon2 job, zero waiting jobs; five attempts/minute per installation, burst five; no IP-keyed unbounded map or permanent account lock |
| Authenticated management | Separate installation-wide five attempts/minute, burst five, for password change/revoke-other requests; login attempts cannot consume this bucket |
| Registry / pre-sessions | 64 total active/retained browser+CLI entries, 32 pre-sessions; pre-session TTL five minutes; reclaim expired entries then oldest audit-confirmed revoked entry for login capacity; never evict active or unexpired audit-pending entries |
| New pre-session allocations | Five/minute per installation, burst two; reuse without renewal; reclaim expired/consumed slots before atomic admission |
| Auth request | Two-second body deadline; five-second total work deadline including pool/hash wait; reject work whose dependencies cannot fit |
| Domain/read/search work | Retain declared SP-004/command-specific bounds; no larger budget or codec ceiling follows from this draft |

These are review inputs requiring measurement and memory permission before live
experiments, not supported operating claims. Saturation rejects before expensive
work. An Argon2 call cannot be assumed cancellable: retain its permit until it
actually finishes even if the caller times out; no detached unowned work. Request
limits, throttles and metadata survive neither a claim of anonymity nor arbitrary
client keys; restart abuse by an OS administrator is a separate threat boundary.
Browser and CLI password login share the unauthenticated login bucket. Validate
current session/CSRF before charging the authenticated management bucket; final
authority still requires post-fence checks. Current-session logout uses neither
password bucket nor the hash permit, but retains transport/database deadlines.
Both password login and authenticated password proof use the same one hash
permit: a busy worker returns 429 immediately, with no priority reservation or
waiting queue. Separate buckets prevent allowance consumption, not worker or
transport starvation; an active local caller can still deny self-service.
The five-second total budget includes both password-change hashes and database
work. Do not start the second hash after deadline/cancellation, publish a change
after expiry, or assume the first hash can be interrupted. Keep owned cleanup
until work ends and discard secret intermediates; no early permit reuse.
Measure complete dual-hash latency and process peak memory, not just the Argon2
memory parameter, before qualifying this budget. Corrupt/unsupported stored PHC
profiles are incoherent auth storage: return 503, never a wrong-password 401.
Use generic authentication errors without verifier/account details. Sensitive
responses are no-store; diagnostics/audit contain IDs/reasons, never secrets.

## Alternatives

| Option | Assessment for this proposal |
| --- | --- |
| Keep Go API + Rust authority with authenticated transport | Viable alternative: mutual TLS, authenticated peer roles, typed request/body binding and authoritative Rust credential checks. More certificate/forwarding/authorization interfaces; ordinary loopback headers are insufficient. Revisit if a demonstrated Go-edge requirement outweighs consolidation. |
| Separate OS-launcher identity or browser passkey | Different login experience; the maintainer preferred a separate Kehila password. OS administration remains enrollment/recovery authority, not automatic browser login. |
| External OIDC provider | Adds an issuer, availability and operating dependency for one local owner; future enterprise scope needs separate review. |
| Persistent sessions or self-contained tokens | Would add restart/restore/revocation semantics before needed; accept reauthentication on restart and keep permissions authoritative. |
| Anonymous local owner or client actor header | Does not meet the accepted identity/authority contract. |

## Consequences and impacts

- Security: removes actor-context network forwarding on the new native path;
  concentrates trust in the Rust authority and protected administrative tool.
  Password guessing, local throttling/availability, XSS, secret theft and OS/DB
  administrator compromise need their stated threat boundaries and tests.
  Sustained local traffic can deny browser issuance or hashing; logout revokes
  safely but its failed/unconfirmed audit may be lost under the declared lifecycle.
  Neither separate rate buckets nor volatile retry state guarantees availability
  or eventual audit delivery.
- Performance: no password verification per business request; one bounded login
  worker, bounded registry and indexed PostgreSQL authority checks. Values above
  need measured supported-host evidence before adoption.
- Operations/reversibility: HTTPS trust provisioning and restart reauthentication
  add user-visible setup. The old prototype cannot be an automatic fallback.
  Adopting the boundary change needs reviewed routing/dependency/migration work;
  reverting must not reopen anonymous access. No new dependency is installed here.
- Recovery, D-037/B-024/#60: supported restore must keep actor/operation/tombstone
  identities but invalidate all issued credentials and remove the restored password
  verifier until local owner recovery sets a new one. Keep the API auth-blocked
  during restore/re-enrollment. This is a mandatory downstream restore contract,
  not a deferred disclaimer. Automatic detection of an undeclared whole-database
  rollback is not supplied here; supported restore tooling and real-data gates
  must enforce the explicit workflow. Never treat a replayed old password as
  acceptable recovery. DPAPI blobs are not portable backups of identity.
- Languages, D-037/B-030/#61: IDs, routes and error codes remain locale independent;
  localize display later. Version password normalization separately from domain
  codecs. Initial UI may be English; supported languages/RTL remain post-MVP.
- Agents, D-037/B-031/#62: `ActorContext` uses stable actor/credential kind rather
  than assuming a browser username or OS account is every performer. CLI-kind
  credentials still authenticate the human owner, not a separate agent principal
  or delegated approval. Future agents need explicit enrollment/grants/attribution;
  do not share owner credentials or add a privileged database path for them.

## Verification and follow-up

### Remaining scale and security obligations

These are open design/evidence boundaries, not demonstrated runtime exploits:

| Concern | Unresolved obligation and owner |
| --- | --- |
| Cross-replica credentials and budgets | The local registry, restart logout, singleton and per-process limits do not supply replicated continuity or cluster-wide abuse/resource control. B-032/#65 must replace these profile mechanisms with a reviewed, failure-tested contract. |
| Actor-wide serialization | SP-004's actor fence serializes mutations/replay for the same owner, even across unrelated projects; more pods alone cannot remove that contention. B-032/#65 and B-025 require measurement and a correctness-preserving coordination decision before scale claims. The local protocol stays unchanged. |
| Remote exposure and identity scope | Loopback HTTPS, one owner/password and a local administrative tool establish no trusted ingress/peer model, remote administration, MFA, multi-user or tenancy support. #65/B-020/B-029 must decide applicability and controls before remote/replicated support; #40 still qualifies the local profile. |
| Unqualified local security mechanisms | AUTH-001 records a host-only certificate/transport subset. Supported installation, auth dependency choices, schema/function privileges, enrollment/recovery, secret storage, CSRF and actual revocation retain A0-A5/T01-T16 gates under #40/#49. The existing unauthenticated prototype is not upgraded by this ADR. |
| Availability, audit and recovery limits | Local traffic can deny login/self-service; failed logout audits may be lost. OS/admin compromise is outside password containment; undeclared whole-database rollback is not detected. Supported restore invalidation still needs proof. #40/#49 own local qualification; #60/#65 own broader recovery, audit and shared-backend availability decisions. Application replication alone does not qualify PostgreSQL, session infrastructure or search failover. |

MVP checks cannot close #65 or qualify these remote/replicated properties. No
general deadlock freedom, throughput, high availability or security certification
follows from this documentation. The separate-profile acceptance matrix must
remain visible during implementation and release review.

The [verification plan](../implementation/local-owner-authentication-verification.md)
maps the proposed mechanisms to bounded slices and failure/evidence gates under
#40/#49. The [AUTH-001 experiment](../../experiments/AUTH-001/README.md) records
transport-only observations; full authentication qualification remains outstanding.
A0 investigation is authorized by the acceptance above; ADR-103 disposition for
native adoption and separately approved A1-A5 implementation plans remain required.
Existing R01–R05 evidence and hosted CI do not qualify the mechanisms above.

## Primary design references

These sources inform the proposal; no ASVS/NIST conformance is claimed.

- [NIST password guidance](https://pages.nist.gov/800-63-4/sp800-63b/authenticators/):
  length, normalization and password handling.
- [OWASP password storage](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html)
  and [RFC 9106](https://www.rfc-editor.org/rfc/rfc9106.html): Argon2id guidance;
  the exact single-lane profile above is this proposal's choice.
- [OWASP session management](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html)
  and [CSRF prevention](https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html):
  opaque sessions, lifecycle and browser request protections.
- [OWASP TLS guidance](https://cheatsheetseries.owasp.org/cheatsheets/Transport_Layer_Security_Cheat_Sheet.html)
  and [TLS 1.3](https://www.rfc-editor.org/rfc/rfc8446): authenticated transport;
  the local certificate deployment still needs evidence.
- [Microsoft DPAPI](https://learn.microsoft.com/en-us/windows/win32/SecCrypto/example-c-program-using-cryptprotectdata):
  per-user protection; no cross-machine recovery or same-account malware guarantee.
