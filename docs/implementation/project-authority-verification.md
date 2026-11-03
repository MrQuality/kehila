# Product authority verification

Implementation and qualification are in progress. These observations belong to
the source revisions exercised during development; they are not a combined
final-head test total or supported installation qualification.

## Executed component checks

The extracted authentication baseline passed 135 authority tests with ten
fixture-dependent tests ignored. An earlier failed run corrupted Unicode while
copying formatter output; the source bytes were restored and the passing rerun
used an explicit UTF-8 boundary. That failure remains part of the development
record.

A later selected component run passed 148 authority tests with ten ignored and
13 simulated-DOM interface cases. Subsequent interface changes passed 14 cases
on Node 24.22.2. These cover server discovery, immutable request identity,
uncertain registration/execution, binding failure, bounded response decoding and
fresh-read failure after confirmed creation. They do not exercise a browser.

The certificate deadline regression passed after removing the inherited
three-minute process cap. The deadline remains bounded by the initially admitted
certificate interval and monotonic ceiling, with periodic UTC validity checks.
This does not establish long-running availability or suspend/resume behavior.

Strict authority Clippy checks passed with warnings denied. The selected external
password corpus matched its published pin and passed the product admission test.
Thirteen verification-matrix tests and nine native-fixture admission/cleanup
tests passed; those Python tests use engine doubles rather than live containers.

## Actual PostgreSQL and HTTP/TLS

A fresh product schema and restricted runtime role were exercised against
PostgreSQL over verified TLS. The native journey used the actual listener,
TCP/HTTP/TLS clients and coordinator, with separate authenticated sessions:

- Register, discover and open the same immutable request from another session.
- Reject changed execution values; create once, replay the saved result and
  independently decode/read project and configuration state.
- Keep completed identity when cancellation is requested; reject execution of a
  separately cancelled request.
- Lose confirmation after an actual COMMIT, restart the authority, log in again,
  reconcile the retained request and read its committed project.
- Deliver both logout and revoke-other uncertainty replies as HTTP 503 with
  `audit_confirmed: false`, preserving normal strict transport cleanup.

Independent SQL observation found two coherent successful projects and matching
cores, payloads, mappings, revisions, grants and grant events. Both retained
pending entries were completed. Runtime connections and advisory locks were zero
after shutdown. Cancelled never-created entries were legitimately pruned by
later registration; they are not expected to remain in the final inventory.

The latest successful lint/corpus/native phase took 210.688 seconds including
owned cleanup. Its sampled aggregate memory peak was 3.352 GiB and minimum
available host RAM was 2.644 GiB. These are development-machine observations,
not capacity targets or guaranteed maxima.

Earlier fixture failures remain distinct: PostgreSQL rejected key permissions
lost during a host copy; a later observer expected a cancelled entry that the
specified pruning policy had removed. Both harness defects were corrected before
the affected reruns. An audit-uncertainty transport failure was reproduced before
the response-delivery correction. Missing Clippy and a Python harness name error
also stopped their phases before native execution. Successful later phases do
not turn those earlier attempts into passes.

## Controlled native schedules

The subsequent public native-only container path passed its ten fixture
admission/cleanup tests and actual provisioned journey. It added these controlled
schedules to the earlier observations:

- Equal and changed HTTP/TLS retries, plus cancellation, queued behind a held
  creation transaction: one creation, matching replay, changed-content conflict
  and completed cancellation result.
- Cancellation held before commit while execution waits: committed cancellation
  prevents creation.
- Two sealed coordinator commands encounter the same existing-target collision.
  After the first fully rolls back and releases the serving mutex, the second
  commits its next admitted candidate. The resumed first request replays that
  winner. This portion is a coordinator/database test, not HTTP preparation or
  random target generation, and uses one authority rather than separate writers.
- Original-byte/domain rejection counters, generated-envelope overflow before
  database admission, adapter receiver cancellation with a held actual worker
  thread, shared capacity, and joined orderly-stop/task-abort cleanup.

The independent observer required three coherent successful projects and matching
permanent records, all three retained requests completed, and zero runtime
connections/advisory locks. The public path verified its source copy unchanged.
It took 246.109 seconds including cleanup, peaked at 3.275 GiB sampled aggregate
memory, and retained at least 2.119 GiB available RAM. An earlier admission test
incorrectly expected 400 for a body correctly rejected as 413; that failed attempt
was preserved and cleaned up before the case-specific expectation was corrected.
Adapter cancellation is distinct from an actual TCP disconnect observation.

The later native extension passed lost registration and cancellation confirmation
checks across restart. The entire saved registration form was recovered; the
committed cancellation stayed terminal. Its baseline observer required three
successes and four pending entries: three completed and one cancelled.

The serial disposable-clock extension then passed actual runtime checks for an
execution waiting behind an actor permission update, independent project read
without creation permission, and the permitted earlier read snapshot followed by
a read observing actor deactivation. It also passed terminal pending expiry after
rewind, rollback of native writes when completion expires, denial before replay
origin followed by recovery, replay expiry before payload removal, owner-only
retirement and continued expiry after rewind. Runtime access to clock controls,
clock-function alteration and retirement was denied. Invalid/missing fixture
clock state failed closed. The original clock function was restored with its
identity, owner, ACL and definition checked.

The separate final observer required three successes, two payloads, one retired
core, three completed entries and one expired entry, coherent retirement and zero
runtime connections/locks. This phase took 253.610 seconds including cleanup,
peaked at 3.312 GiB sampled aggregate memory and reached 1.986 GiB available RAM.
That crossed the configured 2 GiB warning threshold; it did not cross the 6 GiB
aggregate stop limit or signal Windows low memory. These are fixture observations,
not a production administrative interface, production-clock qualification or
backup/restore evidence.

## Quiescent copy and authentication regressions

The copied-database extension passed on PostgreSQL 16. With the source authority
joined, it captured synthetic records, schema properties, owners/effective ACLs,
types, columns, functions, indexes, trigger enabled state, constraint validation
and inheritance properties, and sequence state. A custom dump restored into a
fresh database in the same disposable cluster in one error-stopping transaction.
Comparison completed before admitting an authority to that database.

An earlier exact comparison stopped because PostgreSQL flattened associative
Boolean grouping in four deparsed CHECK definitions. No constraint was repaired
or removed. The comparison now reparses CHECK definitions on empty temporary
objects, preserving original metadata and retaining raw definitions separately.
Positive controls cover the observed scalar/phase regrouping; negative controls
require changed scalar bounds and removed phase transitions to remain different.
Repeated source capture checks stability and absence of captured state changes.
The first canonicalizer stopped on a reused temporary-domain literal OID. A small
diagnostic reproduced that domain behavior, but failed its broader assertion
because the table lookup refreshed correctly. It remains a failed diagnostic.
The corrected helper resolves each scratch object's live identity explicitly.

The fresh authority advanced the copied runtime generation, rejected the former
volatile browser credential, authenticated anew, read full typed project and
configuration state, and reconciled full and retired successes without creating
another project. A new pending request demonstrated sequence continuation beyond
the copied last-issued number, then was cancelled. Independent restored-state
checks required three successes, two payloads, one retired core, three completed
entries and one cancelled entry, with coherent identity and zero runtime
connections/locks.

This is a quiescent synthetic copy in one cluster. It does not establish supported
recovery, credential-safe disaster recovery or authority exclusion between
databases. The fixed cluster roles and copied password verifier were deliberately
retained. Old credential rejection establishes volatile session restart isolation.

Three additional provisioned authentication tests passed afterward: constructed
idle/absolute CLI expiry over TLS, a fully delivered login surviving missing peer
TLS closure, and issuance/password-change commit uncertainty. The missing closure
remains a typed transport rejection; it is not classified as graceful shutdown.
The password-changing case ran last. The final source observer confirmed retained
domain totals and joined runtime cleanup.

That successful native phase took 334.859 seconds including cleanup, peaked at
3.328 GiB sampled aggregate memory and retained at least 2.300 GiB available RAM.
The preceding Rust quality run passed strict authority Clippy and 148 library tests
with 13 provisioned cases ignored by the ordinary unit invocation. Selected
provisioned cases above were executed separately; these figures are not a combined
final-head total. Restored/authentication cases were not reached by the earlier
failed copy attempts.

## Protected startup in the selected Linux container profile

The actual product binary passed a separate startup campaign with fresh TLS and
database assets. A temporary provisioner assigned the four secret files to UID
and GID 10001 with mode 0400, then exited before the runtime started. The runtime
used two read-only volumes, UID/GID/groups 10001, no capabilities, no-new-privileges,
256 MiB memory, one CPU and 128 PIDs. Its listener bound an assigned private IPv4
address with `ip_nonlocal_bind=0`; attempted secret modification failed read-only.

Incorrect file metadata and an excess database UPDATE privilege each caused
startup exit 1 with the application endpoint unreachable. The selected protected
startup asset test also passed. The normal process then completed verified TLS
1.3 login, registration, creation, replay, independently decoded read and logout.
SIGTERM produced exit 0. An active connection check confirmed unreachability
after stop; the independent observer required one coherent project and zero
runtime database connections or advisory locks. Owned containers, volumes and
network were removed, and the initially stopped provider was stopped again.

The first campaign stopped because its negative connection checker did not
recognize the observed host-unreachable error after the expected application
failure. That attempt remains failed. The corrected campaign reused compiled
artifacts only after checking the complete source tree, immutable image and saved
artifact digests; it provisioned fresh runtime assets. It passed in 431.687 seconds
including cleanup, with 1.964 GiB sampled aggregate memory and at least 3.735 GiB
available RAM. This does not qualify Windows browser access, host-port forwarding
or a supported installation procedure.

## Write-prefix rollback and route authorization

The native extension passed 28 attempted row-write fault points across 15 table/
operation groups. A complete dry trace also failed after the written result had
been reconstructed and deferred checks entered. Each nonretryable injected
PostgreSQL error required confirmed rollback, unchanged business records and
pending content, released capacity, no idle transaction, and independently
acquirable installation/actor fences. The database log's complete ordered trace
matched the trusted seed's expected writes. Injection was then removed before
successful creation. These are BEFORE-write prefix failures plus one complete-set
rollback control, not failures after each individual write committed.

All eight project routes denied missing/spoofed identity, crossed CSRF, revoked
session and inactive owner cases, returning only a closed nonsecret error object.
The six creation-related routes also denied an owner lacking creation permission;
binding discovery and independently authorized read remained available. Independent
business-state comparisons checked that denials caused no domain/pending changes.
Wrong-owner proof and pending-owner foreign-key rejection passed separately; they
are not positive multi-actor HTTP qualification.

The initial extension failed with a test-thread stack overflow in a later logout
uncertainty exchange. The failed run and cleanup are retained. Equal future-object
sizes did not explain it. An exact-profile compiled comparison measured the
scenario poll frame growing from 1,155,608 to 1,261,800 bytes. Keeping the awaited
HTTP test exchange in owned heap storage reduced that frame to 196,120 bytes and
the `call` frame from 43,160 to 22,312 bytes. Lower shared exchange frames stayed
unchanged. The affected journey then passed with the original stack limit. This
is a measured debug-test repair, not a production stack qualification.

That later campaign also passed the clock, restored-database, CLI expiry,
missing-peer-close, unknown-commit and owned-login fixtures and their reached
independent observers. It subsequently failed in the newly exercised revoke-others
fixture: retained browser issuance plus two password checks took 5.594 seconds
before inspecting a reply with a five-second submission deadline. The runtime had
remained live. Moving the rejected-password control before retaining that reply
preserved its revocation assertions without extending a product deadline. That
attempt did not reach the three later database-lifetime cases.

The overall campaign remains failed despite these passed subsets. It took 480.969
seconds through cleanup, peaked at 3.416 GiB sampled aggregate memory and retained
at least 2.916 GiB available RAM, with no resource stop. Owned dependencies and the
initially stopped provider were stopped/removed. It passed strict authority Clippy,
148 ordinary library tests and 14 Python harness checks before provisioned work;
13 ignored cases in the ordinary invocation are not a combined qualification total.

The affected four-case rerun subsequently passed with fresh isolated fixture
databases, strict authority Clippy and source-preservation checks. Revoke-others
retained local revocation after audit rejection, retried the original batch without
revoking a later login, and stopped after unknown confirmation. The native fence
test checked actual rollback absence and cancelled a borrowed write while its
transaction wrapper remained alive. The cancelled credential-read test observed
the actual administrative blocker before dropping the request. Both cancellation
cases denied reuse and joined the two database drivers. The lifetime test decoded
authentication/revision values beyond signed 64-bit range, denied administrator
role assumption, and observed irreversible stop and joined drivers after loss of
the exact current authority backend. Each independent observer required no domain
data and no remaining runtime connections/advisory locks in that database.

This affected rerun took 351.453 seconds including cleanup, peaked at 3.473 GiB
sampled aggregate memory and retained at least 3.175 GiB available RAM. It did not
repeat the seven previously passed native modes or qualify a whole final source
suite. The earlier failed phase and deadline diagnosis remain preserved.

## Runtime checkpoint mandatory gate

The exact staged runtime checkpoint passed the full mandatory containerized hook
on 2026-11-03. All eleven provisioned fixture modes passed in that invocation:
journey, clock, restored, authentication expiry/delivery/unknown confirmation,
owned login, revoke-others, fenced cancellation, cancelled read and database
lifetime. Independent final-state/connection/lock observations, source preservation,
the shared legacy checks and owned cleanup also passed. Ordinary ignored-test
counts are separate; this invocation does not qualify Windows browsers or repeat
the separately provisioned corpus/protected-startup observations.

The gate took 502.969 seconds through cleanup, peaked at 4.961 GiB sampled
aggregate memory and retained at least 1.754 GiB available RAM. The configured
2-GiB reserve warning fired; neither the aggregate stop nor Windows low-memory
stop fired. The initially stopped provider was stopped at completion. This is
checkpoint evidence, not a claim that subsequent source has already passed CI.

## Separate fixture-actor database qualification

SP-005's three fixture-actor transaction schedules passed on 2026-11-03 under
the maintainer-selected
[layered split](../engineering/QUALIFICATION.md#native-project-creation-separate-database-and-http-evidence).
Distinct actors committed distinct targets with identical scoped token/handle
text; a target loser fully rolled back after typed primary-key rejection and
re-entered with a fresh target; a waiting creator succeeded after the original
holder rolled back. Independent observations checked actual blocking, visibility,
released actor fences and final attribution/cardinality. The
[SP-005 record](../../experiments/SP-005/README.md#evidence-limits-and-next-cases)
retains the failed attempts, protocol diagnosis, timings and evidence limits.

These are database transactions with fixture actors and placeholder payloads.
They neither add a second HTTP owner nor establish typed product creation or
Chrome/Edge behavior. End-to-end multi-actor HTTP remains post-MVP #65.

## Remaining delivery gates

- Execute the new CI job against the proposed head.
- Complete the intended Windows Chrome/Edge journey with normal TLS validation.
  Local browsers are excluded from the current resource profile. A hosted or
  separate-machine route still needs its concrete execution/trust authorization.
- Pass required commit/CI gates and independent review of the exact proposed head.

The legacy Go API/worker remains a separate unauthenticated prototype path.
Authentication in this authority does not grant or protect operations through
that legacy path. The PR remains draft while agreed delivery gates are unfinished.
