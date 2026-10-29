# Disposable owner coordinator

Status: coordinator subset observed, 2026-10-29. Source review, Linux/Windows
Clippy, restored Linux regressions and the typed native journey passed. Final
commit gates and hosted verification remain pending. This is an internal part of the existing
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
HTTP delivery evidence. No HTTP adapter currently uses this module.

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

Final commit gates and hosted verification remain required. Startup file/corpus admission, revoke-other batching,
unknown-commit consumer qualification, provider isolation and browser/CLI
integration remain unfinished. A4 domain authority and A5 supported restore retain
their separate gates. No full T01–T16 case is marked passed here.
