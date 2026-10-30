# Disposable owner coordinator

Status: coordinator subset observed, 2026-10-29, at source `0c3706c`.
Source review, Linux/Windows Clippy, restored Linux regressions and the typed
native journey passed. Publication requires the normal gates and CI at the
proposed head; [PR #87](https://github.com/MrQuality/kehila/pull/87) records those
results separately. This is an internal part of the existing
[owner-login delivery](local-owner-authentication-delivery.md), not a completed
login capability, supported installation or production adoption.

## Owned work and replies

The Linux-only `application` module connects the credential engines and
[typed database transactions](local-owner-authentication-database-bridge.md).
A manager owns at most 16 accounted jobs/replies. Admission is non-waiting with
a bounded mailbox. The manager retains each admission permit through actual task
join, including completed or panicked tasks. Buffered replies and adapter delivery
guards share that permit, so joining a task does not release capacity while its
credential/reference remains outstanding. Ordinary receiver cancellation cannot
drop a database/hash future or free that work's slot early.

The serving connection is exclusive only through its SQL operation/transaction
and cleanup, not through password hashing. An exclusive hash-lane lease remains
held through actual OS-thread completion and join. Older cleanup cannot join or
cancel a newer hash job. A dirty lease stops the shared runtime before releasing
ownership. The worker checks the same irreversible stop state between stages.
Logout uses neither a password allowance nor the hash lane; it can reach the
serving connection while hashing runs, subject to the common finite resources.

Runtime stop rejects admission and drains jobs before joining the worker and
database drivers. Normal cleanup retains the concrete worker while asynchronously
waiting for its thread to finish, then joins without a shared guard. Exceptional
owner destruction closes liveness and requests task abort; it does not claim
observed normal joins or server rollback. An active hash/OS randomness call is
not assumed interruptible.

Cleanup now uses the first irreversible stop's five-second budget across the
coordinator and database drivers. Repeated stop calls cannot renew it. Driver
cleanup uses only the remaining time before aborting and observing joins;
coordinator cleanup reports an observed overrun as failure while retaining
ownership. This does not make an active OS entropy call or hash thread forcibly
interruptible, or prove a hard scheduling bound. The runnable drain measurement
remains an integration gate.

## Credential transitions

Login performs bounded pre-session/CSRF admission where applicable, charges the
installation login allowance, reads/adopts a strict verifier and verifies outside
SQL fences. After a fresh password fence it claims the browser pre-session,
reserves an unpublished registry entry and writes issuance audit. Only confirmed
commit permits pre-session consumption, publication and guarded secret delivery.
Confirmed rollback can release a still-live claim; uncertain outcomes discard it.
Unused committed issuance history cannot recreate/replay a bearer.

A reply's delivery guard retires its new session on failed/unconsumed delivery.
Secret access and exchange confirmation check current local eligibility,
liveness and the original deadline. The adapter must retain the reply/guard
through its own observed exchange outcome. A successful channel send is not
HTTP delivery evidence. A prepared HTTP adapter now calls this module in an
in-memory fixture; no serving listener calls it.

State/CSRF retrieval uses fresh session fencing without requiring an existing
CSRF value, avoiding a bootstrap cycle. It neither renews activity nor rotates
CSRF. Retained eligible logout state can retrieve its existing CSRF. Logout
revokes locally under the fence before audit and never restores authority after
audit failure. Its typed signed-out/audit-unconfirmed outcome survives generic
deadline/runtime-stop handling.

Password change checks the existing session/CSRF before charging the independent
management allowance, admits the replacement against the supplied blocklist,
then performs verification/replacement in one owned worker job. A fresh fence
checks the original proof before the audited CAS. After confirmed commit, while
retaining the serving lane, the registry retires/prunes all superseded-generation
entries and captured delivery references. Existing credentials are never rebound
to the new generation; failure to invalidate local state stops the runtime.

Source review caught the initial omission of this local generation invalidation:
SQL already denied old credentials, but cached delivery references and active
capacity could survive. A full-capacity transition regression and retained old
delivery assertions were added before execution qualification.

## Verification boundary

### Unknown-commit consumers, 2026-10-29

The opt-in native consumer journey passed against PostgreSQL 16.15/TLS 1.3.
A per-instance, test-only seam withholds confirmation after a real successful
commit and returns the existing `UnknownCommit` outcome before disarming the
normal pending guard. This is not a TCP/TLS fault injection or a claim about every
possible server outcome. The ordinary runtime-stop, suppression and cleanup paths
remain unchanged in production.

The journey confirmed issuance returns no credential and stops new admission,
logout retains its signed-out/audit-unconfirmed outcome despite runtime stop,
and password-change uncertainty reconciles through a fresh runtime and login:
the old password denies and the chosen replacement succeeds. Independent SQL
inspection found four runtime starts, four issuance events, one logout and one
password change, with the expected generation/revision and unchanged owner/grants.
All owned drivers/workers settled and the advisory lock was released. Each owner
was recreated within the same test process; this does not qualify process/container
restart, a serving HTTP listener or revoke-other consumers.

### Prepared HTTP boundary, 2026-10-29

The separate `http_auth` module admits only explicit browser/CLI routes before
coordinator submission: exact configured loopback HTTPS authority, closed method
and target, 8-KiB aggregate headers, 2-KiB target, 4-KiB JSON, canonical declared
length and no proxy assertions, compression or transfer coding. Browser changes
require exact Origin and CSRF, plus a fixed client header and same-origin Fetch
Metadata. CLI routes reject browser context and require exactly one bearer on
authenticated calls. All Cookie fields are inspected for repeated authentication
names; browser and CLI credentials never fall through into another mode.

Initial pre-session issuance and read-only CSRF retrieval are separate operations.
Retrieval cannot allocate, rotate or renew state. A supplied unknown pre-session
denies rather than silently allocating. A typed local lookup miss permits clearing
stale browser cookies for a subsequent explicit bootstrap; wrong password/CSRF
proof preserves valid cookies. In particular, a mistyped current password must
not discard the current session. Unconfirmed logout audit keeps the revoked
cookie available for its bounded retry.

The prepared dispatcher checks actual body length, a two-second body budget and
the caller's original absolute work deadline. Typed JSON decoding rejects
positional arrays, duplicate/unknown fields and wrong types before scheduling.
Owned password/body/response buffers are zeroizing; library scratch and headers
remain outside a universal wiping guarantee. Browser output contains only CSRF
or fixed status data, with protected HttpOnly cookies; only the separate CLI
login returns a bearer in JSON. Responses are no-store, no-CORS, no-sniff and
same-origin, with a restrictive CSP.

The dispatcher returns an owned exchange guard alongside the encoded response.
Its outbound gate checks current issued-session eligibility as well as runtime
liveness/deadline; generation retirement must suppress already-encoded material.
The future listener must use that gate through outbound delivery and retain the
guard through observed exchange/cleanup. Merely encoding, polling a body to its
end or confirming the in-memory fixture does not establish network delivery.

Current prepared-source checks passed: four Windows admission tests, Windows
Clippy, Linux Clippy, and Linux serial regressions (123 unit plus 20 transport;
five opt-in unit cases skipped). The modified native adapter journey also passed
against PostgreSQL 16.15/TLS 1.3: actual in-memory CLI/browser requests reached the
coordinator, response body/cookie material was checked, and the retained encoded
browser exchange denied outbound delivery and confirmation after password change.
The independent SQL history/generation/lock oracles passed unchanged. This is not
a live HTTP/browser observation. RAM preflight held earlier phases; the optimized
build and native run proceeded only after headroom recovered. Owned services were
cleaned up and generated private keys deleted. Publication requires mandatory
commit gates and final-head CI; PR #87 records those results separately. The earlier typed native
measurement below retains its original source limits.

A targeted mutation removing unknown-field rejection failed its intended JSON
oracle. An initial post-mutation rerun reused a stale test executable; cleaning
the experiment's compiled artifacts and rebuilding unchanged original source
restored the four-test/Clippy pass. Earlier failures also caught the valid browser
login falling through to rejection and a Linux closure error-type inference gap.
Independent source review corrected overly broad cookie clearing and a missing
issued-session check in the outbound guard before execution qualification.

Startup asset admission and uncertain-commit consumer qualification remain gates
before a serving listener is connected. The diagnostic endpoints remain unchanged.
Windows forwarding/client protection, actual browser behavior, process/container
restart and integrated transport qualification are still pending.

The opt-in `native_owned_login` passed using actual PostgreSQL 16.15/TLS 1.3 and a
synthetic blocklist. It exercises retained-reply capacity, abandoned receivers,
wrong-password rejection, typed CLI/browser pre-session/login/state transitions,
password change, generation invalidation, logout deduplication, runtime-owner
recreation with old-credential rejection
and an externally observed loss of the exact dedicated advisory-lock backend.
Its `confirm_exchange` calls simulate a typed adapter boundary; they are not
browser, CLI transport or HTTP evidence. Recreating the owner inside the same
test process is not an OS-process/container restart observation. Independent
SQL inspection confirmed the expected issuance/logout/password-change history,
deduplication, generations and unchanged owner activity/grants. Both normal
shutdown and the injected dedicated-connection failure released the advisory lock.

One controlled optimized run used Rust 1.93.1, optimization level 3, one Cargo job
and one release code-generation unit. Five measured login attempts, including
one wrong password, took 106.721–130.438 ms (median 112.300 ms). Password change
took 189.475 ms through its verification/replacement, fresh SQL and invalidation.
All typed requests completed within the unchanged five-second budget. Process
peak resident memory was 71,028 KiB (69.36 MiB), including the fixed 64-MiB hash
workspace, test harness and TLS/driver code. It excludes the PostgreSQL process.
These are single-journey observations, not a production benchmark or full HTTP
budget qualification: body admission, Windows forwarding, rendering and client
storage were not exercised. Peak RSS is not an allocation trace or container
memory-limit enforcement.

The restored Linux serial run passed 117 unit and 20 transport tests. Five
ordinary unit skips were the four opt-in database/coordinator fixtures and the
external corpus. This new coordinator fixture ran separately; earlier bridge and
corpus observations retain their original source limits. A targeted mutation
omitting captured-reference retirement failed the intended old-entry eligibility
assertion. It used an external copy; the successfully tested source/binary stayed
intact.

Initial checks exposed obsolete dead-code expectations, cross-module test-helper
visibility and a Windows-only unused startup constructor. These were corrected;
the constructor remains Linux/test-only, with no Windows backend added. Earlier
failed attempts and RAM-held phases remain retained rather than counted as passes.
An edit-script encoding error also changed seven Unicode test-literal lines,
causing four Linux regression failures. The exact committed literals were
restored without changing normalization/hash behavior or weakening the assertions.
Focused Windows normalization tests and the restored Linux serial regressions
passed. Portable Clippy and the focused Windows shared-worker-stop and
generation/capacity regressions passed. Linux Clippy passed; intermediate serial
phases held by the RAM minimum remain recorded separately.

Final commit gates and hosted verification remain required. The later startup
asset and revoke-other observations below supplement this initial journey;
provider isolation and browser/CLI integration remain unfinished. A4 domain authority and A5 supported restore retain
their separate gates. No full T01–T16 case is marked passed here.

## Revoke-other consumers, 2026-10-29

The coordinator now implements authenticated revoke-other with a current-password
check and a separate management allowance. It freezes at most 63 target public
IDs, kinds and the initiating session's original deadline. Local target eligibility
ends before audit writes. A confirmed rollback retains that batch for retry;
retry cannot select a later login or change the original revocation provenance.
Targets gain no own-logout retry right. Confirmed success clears the pending batch,
and a later explicit request may select a new batch.

The opt-in PostgreSQL 16.15/TLS 1.3 consumer test passed separately. A controlled
audit rejection rolled back SQL while selected sessions stayed revoked, including
a retained issuance reply. External SQL observations found zero revocation events
before allowing retry. Retry recorded the two original targets while a later login
stayed active; a new request revoked that later login. A final known commit with
its confirmation deliberately withheld stopped the runtime and preserved the typed
audit-unconfirmed result. Final SQL checks observed five issuance and four
revocation events, one initiating session, unchanged authentication/verifier
generations and release of the dedicated advisory lock. This is a consumer seam
after a real commit, not a simulated TLS wire failure.

Browser and CLI HTTP routes share the existing strict admission and closed
password object. The new route-specific admission/JSON regression passed on
Windows. Live HTTP delivery, user interface behavior, actual provider isolation,
remaining lifecycle measurements and exact-head publication gates remain required.
