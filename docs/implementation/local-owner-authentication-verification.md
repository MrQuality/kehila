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
- [ ] Select and review dependencies and native capability/schema changes before
  code. Approval of the login preference does not approve any of these choices.

## Proposed slices and gates

| Slice | Boundary and exit evidence |
| --- | --- |
| A0: local transport and dependency proof | Review a bounded prototype scope first. Demonstrate actual browser/CLI certificate validation, strict origin/host binding, key protection and renewal on Windows/Podman; qualify Linux separately. Review TLS, Argon2id, NFC, randomness, digest/comparison, zeroization and protected-file/DPAPI library candidates under the dependency policy. No trust-warning bypass or raw unsafe platform implementation. Stop if no usable secure installation path is supported. |
| A1: owner and generation management | Review the versioned auth schema/capability and startup-singleton protocol. Disposable native tests cover atomic single-owner enrollment, unknown-commit reconciliation, narrow serving versus administration rights, password/generation CAS, takeover fencing, failed startup and preserved actor/grants/history on recovery. No public enrollment/reset or provisioning secret in the server. |
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
| CLI password login | Separate route, `{password}`, no Cookie/Origin/Fetch Metadata. The owner enters the password on the terminal; no arguments/environment/history. Return a CLI-kind bearer and store only that bearer in the protected per-user profile. |
| Business reads / commands | Explicit browser-cookie or CLI-bearer adapter; reject mixed/wrong-kind/duplicate credentials. Browser state changes require current CSRF and exact Origin. Resolve authority from the server's stable actor binding and fresh SQL state. |
| Current-session logout | Credential-specific route/mode; CSRF for browser. Mark that same registry entry revoked under the actor fence, with audit/failure semantics in ADR-105. Closing a browser is not authoritative logout. |
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
| T06: expiry and bounds | Fake-clock boundaries plus actual restart: idle/absolute limits, no renewal from rejected/public/keepalive requests, no resurrection from stale activity updates, capacity rejection without eviction. Password change retains one hash permit across verify+hash; busy login/management receives 429 with no queue. Login saturation cannot charge the authenticated management bucket, and logout needs neither bucket nor hashing. Cancel/expire between hashes: no second hash or update; expire during hashing: retain ownership until real completion. Measure complete dual-hash latency and process peak memory under the proposed five-second budget; no availability guarantee under sustained local contention. |
| T07: session revocation | Reach the actual actor barrier: command first can finish before logout; logout first denies queued create/replay. A captured registry reference observes revocation. Repeat password/global generation change, rollback/failure and fresh retry; deleting a registry key without revoking a retained reference is a detected fault. |
| T08: read/snapshot | Credential/grant/data checks use a fresh consistent snapshot. Logout committed before the snapshot denies even if the registry reference was captured earlier; no stale cached successful verdict. Test both read/revocation orderings and bounded output. Search scopes/hits/aggregates are authorized, with no actor/DB lock spanning search. |
| T09: runtime takeover | Prove installation shared/exclusive locking before sorted actor and domain locks. Hold an old mutation's shared installation fence: takeover waits through its commit/rollback. Commit takeover first: an old queued mutation/replay/issuance denies after a fresh post-wait generation check. Exercise recovery and reversed multi-actor arguments, aborted startup and unknown takeover commit. Reject stale pre-wait snapshots and lock upgrades. Loss of the advisory connection stops admission/output; fresh old-process read snapshots deny after takeover. Delayed projected reads retain only the declared earlier-snapshot allowance. No silent session reconstruction/reacquisition. |
| T10: credential change / failure | Race password verification against another password change/recovery; stale revisions deny without automatic rehash. Corrupt/unsupported stored PHC yields 503, wrong password 401. Failed audit/commit never reactivates revoked memory state. Lost responses do not trigger blind password-change replay or leak verifier metadata. |
| T11: restart / restore | Old browser/CLI/pre-session tokens fail after restart. Restore to a clean installation keeps stable owner/operation/tombstone identities, removes the restored password verifier, requires authorized fresh enrollment and exposes no authenticated data beforehand. Old passwords/tokens cannot regain access through supported restore. |
| T12: role and secret boundaries | Restricted runtime cannot claim/change owner identity, use provisioning/recovery capability or bypass grants. Fault/redaction tests seed recognizable test secrets and prove that responses, telemetry, audit, operation fingerprints and retained artifacts exclude them. No live secrets in evidence. |
| T13: pre-session saturation | Concurrent new allocations cannot exceed the shared bucket or 32 slots. Reuse preserves original expiry and does not charge issuance allowance; consume once on successful login. Reach exact expiry/reclamation boundaries and bound scans/limiter metadata. Under repeated allocation attempts, demonstrate both existing-browser reuse and 429 for denied new issuance; recovery after traffic stops requires bounded refill, not restart. Record residual denial under sustained traffic rather than claiming reaping fixes it. |

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

## D-037 follow-through

Recovery (#60) receives an explicit actor-preserving re-enrollment contract,
not portable live credentials or silently restored old passwords. Languages (#61)
receive stable IDs/codes and a versioned password-normalization boundary distinct
from domain data. Agent execution (#62) receives a typed actor/credential seam;
human-owner CLI tokens are not agent identities, delegation or approvals. None of
these post-MVP features is implemented or accepted as a detailed design here.
