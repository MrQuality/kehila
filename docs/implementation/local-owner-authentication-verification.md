# Local-owner authentication: proposed verification and delivery plan

Status: Proposed; unexecuted. Owner: project maintainer. Recorded: 2026-10-19.
Related: [ADR-105](../architecture/ADR-105-local-owner-authentication.md),
[#40](https://github.com/MrQuality/kehila/issues/40),
[#49 W2](https://github.com/MrQuality/kehila/issues/49), B-007/B-020/B-022.

This plan makes the proposed password/authority boundary reviewable. It supplies
no new authentication evidence. The existing worker remains unauthenticated and
on the provisional storage path. ADR-103 and ADR-105 remain independently Proposed.

## Review choices before implementation

- [ ] Approve or amend consolidating the future native API/authentication/domain
  authority in Rust; retain Go + authenticated transport as the explicit alternative.
- [ ] Approve the separate-password profile, restart reauthentication, CLI login,
  local administrative enrollment/recovery and restored-password invalidation.
- [ ] Review HTTPS certificate trust/renewal UX and the provisional size, lifetime,
  hash-cost, registry, throttling and deadline values; do not call them measured.
- [ ] Review residual local login/self-service denial and permitted loss of an
  unconfirmed logout audit after abandonment, expiry or process failure.
- [ ] Select and review dependencies and native capability/schema changes before
  code. Approval of the login preference does not approve any of these choices.

## Proposed slices and gates

| Slice | Boundary and exit evidence |
| --- | --- |
| A0: local transport and dependency proof | Review a bounded prototype scope first. Demonstrate actual browser/CLI certificate validation, strict origin/host binding, key protection and renewal on Windows/Podman; qualify Linux separately. Review TLS, Argon2id, NFC, randomness, digest/comparison, zeroization and protected-file/DPAPI library candidates under the dependency policy. No trust-warning bypass or raw unsafe platform implementation. Stop if no usable secure installation path is supported. |
| A1: owner and generation management | Review versioned auth storage/capabilities, the singleton installation row, shared/exclusive installation-before-actor lock protocol and unique logout audit identity. Disposable native tests cover atomic single-owner enrollment, unknown-commit reconciliation, narrow serving versus administration rights, password/generation CAS, post-wait takeover fencing, audit deduplication, failed startup and preserved actor/grants/history on recovery. No public enrollment/reset or provisioning secret in the server. |
| A2: credential and admission core | Pure policy/registry tests with fake monotonic time, followed by real randomness/adapters. Cover normalized password bounds, corrupt PHC parameter rejection, one owned hash job, bounded registries, strict credential kinds, finite expiry/revocation and no secret diagnostics. Private constructors prevent deserializing a trusted ActorContext from a client claim. |
| A3: browser and CLI authentication | Run actual TLS/browser/HTTP and restricted PostgreSQL boundaries for pre-session/CSRF, password verification, secure cookies, CLI secret storage, logout/password change, rejection/saturation and loss of the runtime lock. Pure HTTP mocks alone do not qualify browser certificate/cookie behavior or Windows permissions. |
| A4: authoritative command and read integration | Bind actual credentials to SP-004 fences/snapshots; prove queued revocation, original-scope replay, mixed authorized/denied search scopes and no transaction across search. Coordinate #41 stable retry targets before #49 W3 interfaces. Integrating the real native adapter/production adoption retains its own ADR-103/W3/W7 gates. |
| A5: supported recovery and operating evidence | Demonstrate process restart reauthentication, stale CLI file rejection, supported clean restore with auth blocked and fresh owner-password enrollment, actor/operation/tombstone preservation, then supported operating limits. Baseline real-data gates remain; broader DR/languages/agent delivery stays #60/#61/#62 post-MVP. |

Keep each slice in small commits with exact source/platform evidence and primary
failure diagnostics. Experimental schema stays a labelled disposable overlay until
native production adoption and migrations are approved. Do not silently expand
the existing serving role, change codecs, expose legacy routes or close #40/#49.

## Proposed route and credential contracts

Paths are review inputs, not an existing supported API. Each endpoint has a closed
JSON shape and rejects duplicate/unknown fields; project codecs retain ADR-104.

| Operation | Proposed caller contract |
| --- | --- |
| Public static login / minimal health | Same configured HTTPS authority; no owner/task/configuration or secret diagnostics. No authenticated fallback on these routes. |
| Browser pre-session | Same-origin client gets a bounded, five-minute server-side pre-session, secure HttpOnly cookie and separate CSRF value. Reuse a live presented pre-session without renewal; new allocations share five/minute, burst two. Atomically reclaim expired/consumed slots before admission; saturation returns 429 without live eviction. It grants no identity; sustained local allocation traffic can still deny new browser login. |
| Browser password login | Exact Origin, pre-session cookie, synchronizer header and `{password}`. Consume/rotate pre-session on success and issue a fresh browser-kind session. Failed attempts cannot fix a chosen session ID. |
| Browser CSRF retrieval / reload | Reuse returns the existing server-held CSRF secret, distinct from the digest-only bearer. Read-only browser bootstrap requires the cookie, fixed non-secret custom header and same-origin Fetch Metadata; any present Origin must match. Authenticated retrieval revalidates the credential/generations as a read. No old CSRF is required; no CORS, bearer/domain data, secret rotation, new allocation or activity/expiry renewal. Responses are no-store. Eligible retained revoked cookies can retrieve only their same-logout CSRF; expired/superseded/reclaimed credentials deny. |
| CLI password login | Separate route, `{password}`, no Cookie/Origin/Fetch Metadata. The owner enters the password on the terminal; no arguments/environment/history. Return a CLI-kind bearer and store only that bearer in the protected per-user profile. |
| Business reads / commands | Explicit browser-cookie or CLI-bearer adapter; reject mixed/wrong-kind/duplicate credentials. Browser state changes require current CSRF and exact Origin. Resolve authority from the server's stable actor binding and fresh SQL state. |
| Current-session logout | Credential-specific route/mode; CSRF for browser, including restricted retries by the same revoked credential. 200 confirms revocation and one audit; 503 after local revocation means signed out with audit unconfirmed. 401 for an unknown/expired/superseded or capacity-reclaimed credential proves no audit outcome. Retained state never renews and only authorizes this same logout; confirmed entries may be reclaimed for login capacity. Clients may retry it or discard credentials. Closing a browser is not authoritative logout. |
| Password change / revoke other sessions | Current credential, browser CSRF where used, and current password proof; never a client actor assertion. Recheck verifier revision after the actor fence. Password change advances durable generation and requires new login. Public session IDs are management selectors, not credentials. |
| Administrative enrollment / forgotten-password recovery | Local tool with separate provisioning/recovery capability, protected input, actor-preserving changes and explicit audit. No unauthenticated HTTP equivalent. |

The normal runtime may use narrowly reviewed auth/password-change functions; it
must not load owner-bootstrap or recovery credentials. Native functions, actor
lock order and audit events need an explicit rights/migration review in A1.
Authentication management is separate from the WorkItem business replay ledger;
no raw password, password fingerprint, PHC verifier or bearer is a replay payload.

Proposed stable error classes: malformed/duplicate/mixed credential input is
400 `invalid_auth_request`; wrong password/token or expired credential is 401
`authentication_failed`; origin/CSRF rejection is 403 `request_origin_denied`;
throttle/hash/registry capacity is 429 `auth_capacity_exceeded`; unavailable or
incoherent auth storage/runtime is 503 `auth_unavailable`. No class exposes a
verifier, secret or password-match detail. Preserve domain conflict/denial and
integrity semantics separately; an unrelated failure cannot pass an auth oracle.

## Required failure and ordering evidence

| Case | Required observation |
| --- | --- |
| T01: enrollment | Concurrent/uncertain initializers result in one durable owner and one stable actor; failed init changes nothing and retry cannot reset an existing password. Serving/client roles cannot provision or recover. |
| T02: password policy | Exact normalization/version behavior, 15/128 code-point and byte/encoded bounds, paste/spaces/non-Latin examples, denylisted passwords, wrong password, unknown verifier version and oversized PHC memory parameters. Reject malformed input before pool/hash admission. |
| T03: transport | Real browser/CLI reject wrong/untrusted/expired certificates, wrong SAN/Host/origin and redirects; altered forwarded headers do not choose identity/origin. No supported anonymous worker, search or HTTP downgrade path exists. |
| T04: browser requests | Reject missing/null/foreign Origin, absent/wrong/stale CSRF, login fixation, hostile form/CORS attempts, duplicate cookies and mixed auth. Inspect actual Secure/HttpOnly/SameSite/Path/no-Domain and no-store behavior. No secrets in URLs, logs or browser persistent storage. |
| T05: CLI | Missing Origin plus absent/bad bearer denies. Reject browser-kind tokens, Cookie/Origin/Fetch Metadata and duplicate Authorization. Verify Windows ACL/DPAPI ownership and Linux mode/ownership/redirection refusal; password is not persisted. |
| T06: expiry and bounds | Fake-clock boundaries plus actual restart: idle/absolute limits, no renewal from rejected/public/keepalive requests, no resurrection from stale activity updates, capacity rejection without active-session eviction. Password change retains one hash permit across verify+hash; busy login/management receives 429 with no queue. Login saturation cannot charge the authenticated management bucket, and logout needs neither bucket nor hashing. Cancel/expire between hashes: no second hash or update; expire during hashing: retain ownership until real completion. Measure complete dual-hash latency and process peak memory under the proposed five-second budget; no availability guarantee under sustained local contention. |
| T07: session revocation | Reach the actual actor barrier: command first can finish before logout; logout first denies queued create/replay. A captured registry reference observes revocation. Repeat password/global generation change, rollback/failure and fresh retry; deleting a registry key without revoking a retained reference is a detected fault. |
| T08: read/snapshot | Credential/grant/data checks use a fresh consistent snapshot. Logout committed before the snapshot denies even if the registry reference was captured earlier; no stale cached successful verdict. Test both read/revocation orderings and bounded output. Search scopes/hits/aggregates are authorized, with no actor/DB lock spanning search. |
| T09: runtime takeover | Prove installation shared/exclusive locking before sorted actor and domain locks. Hold an old mutation's shared installation fence: takeover waits through its commit/rollback. Commit takeover first: an old queued mutation/replay/issuance denies after a fresh post-wait generation check. Exercise recovery and reversed multi-actor arguments, aborted startup and unknown takeover commit. Reject stale pre-wait snapshots and lock upgrades. Loss of the advisory connection stops admission/output; fresh old-process read snapshots deny after takeover. Delayed projected reads retain only the declared earlier-snapshot allowance. No silent session reconstruction/reacquisition. |
| T10: credential change / failure | Race password verification against another password change/recovery; stale revisions deny without automatic rehash. Corrupt/unsupported stored PHC yields 503, wrong password 401. Failed audit/commit never reactivates revoked memory state. Lost responses do not trigger blind password-change replay or leak verifier metadata. |
| T11: restart / restore | Old browser/CLI/pre-session tokens fail after restart. Restore to a clean installation keeps stable owner/operation/tombstone identities, removes the restored password verifier, requires authorized fresh enrollment and exposes no authenticated data beforehand. Old passwords/tokens cannot regain access through supported restore. |
| T12: role and secret boundaries | Restricted runtime cannot claim/change owner identity, use provisioning/recovery capability or bypass grants. Fault/redaction tests seed recognizable test secrets and prove that responses, telemetry, audit, operation fingerprints and retained artifacts exclude them. No live secrets in evidence. |
| T13: pre-session saturation | Concurrent new allocations cannot exceed the shared bucket or 32 slots. Reuse preserves original expiry and does not charge issuance allowance; consume once on successful login. Reach exact expiry/reclamation boundaries and bound scans/limiter metadata. Under repeated allocation attempts, demonstrate both existing-browser reuse and 429 for denied new issuance; recovery after traffic stops requires bounded refill, not restart. Record residual denial under sustained traffic rather than claiming reaping fixes it. |
| T14: logout audit lifecycle | Fail the audit after local revocation: business/read/management immediately deny and logout returns 503. Same-token/browser-CSRF retries can only finish that logout. Lose a successful response or simulate unknown SQL commit: the stable unique identity yields exactly one matching event and 200; conflicting attributes fail integrity checks. Committed retries return 200 only while retained and eligible; capacity-reclaimed tokens return 401 with no audit conclusion or SQL lookup. No retry/expiry/generation change/crash may resurrect the token or leak slots. Test bounded reclamation and explicit audit loss after abandoning pending state; no eventual-delivery claim. Restart invalidates all credentials, including pending logout retries. |
| T15: browser reload / CSRF | Real-browser reload retains the HttpOnly cookie and retrieves the same usable CSRF value for pre-session login, active-session state changes and eligible revoked-session logout retry. Two tabs retain valid values; retrieval neither rotates nor renews expiry/activity or consumes issuance allowance. Reject missing custom header, foreign/missing Fetch Metadata, mismatching Origin and wrong/expired/superseded/reclaimed credentials; no bootstrap/CSRF circular dependency, bearer exposure or domain-data response. Check no-store, fresh active-session authority, and denial at exact expiry and after takeover/reclamation. |
| T16: completed-logout capacity | Reach 64 successful login/business/logout cycles inside the original deadlines while respecting the login throttle. The 65th login reclaims the oldest confirmed revoked entry and succeeds without exceeding 64 entries or evicting active/pending state. Test mixed active, confirmed and pending populations, concurrent admission/retry/confirmation and atomic slot reservation. Captured references stay revoked. Before reclamation the old logout can return 200; after reclamation it returns 401, preserves its one committed audit and performs no token-based SQL lookup. All-active/pending saturation remains bounded 429. |

Deliberately weaken specific guards in isolated fixtures and prove their intended
oracle fails: cached pre-queue session validity, retained-reference revocation,
runtime-generation mismatch, mixed browser/CLI fallback and normalization/PHC
resource bounds. A missed prerequisite, startup failure, generic timeout or
unrelated SQL error cannot qualify a negative case. Restore original code/state
and rerun intact cases. Finite tests do not prove general security or linearizability.

## Evidence and execution control

- [ ] Confirm the slice scope and bounded investigation budget before execution.
- [ ] Ask permission before compiler/native/browser-service or other memory-heavy
  work, then measure available resources and serialize required service phases.
- [ ] Record source, OS/browser/runtime, image/library/tool versions, role matrix,
  exact reached barriers, results, failed attempts and confirmed cleanup.
- [ ] Measure password-hash latency and peak resource use, admission saturation,
  lock/connection loss, restore behavior and dependency failure; never present
  provisional settings as measured operating limits.
- [ ] Apply required staged/hosted checks and separate actual native/browser
  evidence. Hosted pure/tooling success alone qualifies neither TLS installation,
  database revocation nor supported real-data use.
- [ ] Update #40/#49, the canonical SP-004 record and PR summary with reached cases
  and remaining gates. Do not close either issue on this proposed design.

## PR #64 review follow-up

The [review self-reminder](https://github.com/MrQuality/kehila/pull/64#issuecomment-6261598730)
is addressed in the proposal, not qualified by execution:

| Concern | Proposed disposition / required evidence |
| --- | --- |
| High/P1: takeover fencing | Installation shared/exclusive fence, deterministic actor order and post-wait fresh checks; T09. |
| Medium/P1: password-change admission | One composite hash permit, separate management allowance, explicit busy rejection/deadline and residual contention; T06/T10. |
| Medium/P2: pre-session saturation | Reuse without renewal, bounded reclamation and shared issuance limit; sustained local denial remains explicit; T13. |
| Medium/P1: logout audit failure | Same-logout-only retry, unique audit identity, finite retention and permitted unconfirmed audit loss; T07/T14. |
| Medium/P1: completed logout capacity | Reclaim oldest audit-confirmed revoked entry under capacity pressure; retry becomes 401 after reclamation without implying audit failure; T14/T16. |
| Low/P2: CSRF after browser reload | Return existing server-held CSRF for pre-session/active-session bootstrap and eligible same-logout retries; no rotation, renewal or cross-tab invalidation; T15. |

The [second self-reminder](https://github.com/MrQuality/kehila/pull/64#issuecomment-6262224047)
adds the last two refinements. Confirmed logout retention is best effort for
retry convenience, not a reason to reject new login when a reclaimable slot
exists. CSRF retrieval is an explicit read-only exception to requiring an old
CSRF value, with same-origin/custom-header and credential checks; browser proof
remains required. Neither refinement supplies implementation evidence.

ADR-105 stays Proposed. In particular, the local availability and audit-loss
trade-offs require review before acceptance. All T01–T16 cases remain unexecuted;
#40/#49 stay open and no runtime/schema/dependency change follows from these edits.

## D-037 follow-through

Recovery (#60) receives an explicit actor-preserving re-enrollment contract,
not portable live credentials or silently restored old passwords. Languages (#61)
receive stable IDs/codes and a versioned password-normalization boundary distinct
from domain data. Agent execution (#62) receives a typed actor/credential seam;
human-owner CLI tokens are not agent identities, delegation or approvals. None of
these post-MVP features is implemented or accepted as a detailed design here.
