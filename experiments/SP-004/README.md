# SP-004: grant-management and authority experiments

This is the first executable A05 subset of [#40](https://github.com/MrQuality/kehila/issues/40)
and the [SP-004 contract](../../docs/spikes/SP-004-actor-authority.md), following
merged [PR #53](https://github.com/MrQuality/kehila/pull/53). It does not complete
SP-004, provide an authentication mechanism or change production SQL files.

## Overlay and trust boundary

The runner requires a fresh disposable container and first runs the existing
native verifier, including its production-codec bridge. The baseline must reject
a version-two creation grant event before the experimental overlay is applied.
The [overlay](overlay.sql) then admits only creation and `sp004_grant_admin`
operation families, keeping permanent operation/payload and attribution guards.
This narrower family set is experimental, not schema compatibility evidence.

`sp004_manager_test` is a dedicated backend login with only the management helper
capability and operation/payload SELECT needed by deferred payload triggers at
commit. It has no direct grant UPDATE or operation/event INSERT privileges.
The SECURITY DEFINER helper locks the manager/subject actor set in C order,
checks the declared management fixture, current/latest-event coherence and
expected version, then writes one attributed operation/payload/event/current
change. Creator INSERT rights are explicitly barred from management shapes.
The helper's owner remains the trusted native schema owner; schema administration
can bypass guards and is outside the serving threat boundary.

The manager's stable actor ID is a **verified-identity fixture**, not a credential.
The backend capability holder is trusted to supply it. No browser session,
authenticated process channel, credential enrollment/expiry/recovery, or spoofed
end-user rejection is established by these database tests. Their mechanisms
remain decisions in #40/#11; they are not selected by this experiment.

Request/result fixtures have a clearly experimental command family and shape.
PostgreSQL 16 JSON-array text frames the fixed typed request; its built-in
[SHA-256 function](https://www.postgresql.org/docs/16/functions-binarystring.html)
hashes those stored bytes. This is not project_create_codec version-one output,
an authenticated MAC, a general replay implementation or a production codec.
Each call is a new grant mutation; caller-side uncertain-outcome retry/replay
remains outside this serial subset. Native foreign keys and payload guards are
retained; scalar current/latest checks are bounded, while test-only COUNT checks
inspect the deliberately small final fixture state.

## Reproduction

Use Python 3.10+, Cargo/Rust and the pinned PostgreSQL 16 image in the
[native testing guide](../../storage/postgresql/README.md#disposable-native-verification).
Before execution, review overlay roles, trigger guards, helper and fixture scope.
Check memory after Podman starts: at least 3 GiB available host RAM and 2 GiB free
disk are required. Use one compiler job. No new language dependency is introduced.

Create the documented networkless 256 MiB ephemeral container with a unique name,
then run these individual commands (replace the explicit disposable name):

```text
podman exec kehila-sp004-test pg_isready -U postgres
python experiments/SP-004/run.py --disposable-container kehila-sp004-test
podman rm -f kehila-sp004-test
```

The caller owns container creation and cleanup on success or failure; the runner
neither starts services nor resets existing databases. It reuses the native
verifier's strict container and fresh-database checks, 60-second process bound,
specific SQLSTATE/primary-diagnostic rejection checks and platform/resource
preflight. A05 statements use 10-second statement/idle-transaction and 2-second
lock budgets. An explicit Podman connection is available through the same
`--podman-connection` option. Save stdout/stderr locally to retain failed attempts;
the final JSON success summary includes the tested source-file fingerprints.
Invalid container/connection names fail before resource preflight or database
setup. CLI failures retain chained and labelled cleanup diagnostics; normal
interruption returns 130 without a traceback. The caller still owns the container.

## Acceptance and remaining scope

The serial cases check baseline denial, independent serving/management login
privileges, forged management INSERT denial, stale/no-op rejection, full rollback,
revocation/restoration and attributed digest/request/result coherence, plus
current/latest corruption failure and retained history protection.

The serial extension supplies a distinct active `admin_actor` with no creation
permission. It checks every NULL argument, unregistered/inactive/missing manager,
missing subject and existing subject without a grant. Rejections and an explicit
distinct-manager rollback must retain the prior grant, event and operation state.
Distinct-manager revoke/restore then verifies the manager's operation/event
attribution separately from the subject's request/result identity, exact digest,
successor versions and final current/history counts. These are serial cases;
they do not qualify concurrent actor-lock ordering or real authentication.

Python unit tests cover orchestration failure propagation and unsafe-input/resource
gates. They do not establish that SQL executes correctly. Real PostgreSQL evidence
must be recorded separately with source revision, platform, image/version and
cleanup. A failed or unperformed native run must remain explicit.

A01-A04/A06-A08, earlier-history-gap audit, growing-history measurements,
multi-actor concurrency and actual session/logout ordering remain #40 work.
Passing this subset does not qualify full A05, production migration/adapter,
generic scope persistence or Linux-host execution. No other issue is completed
by adding or running this prototype.

The [R03 serial extension record](results.serial-guards.json) preserves the
2026-10-16 Windows/WSL2/Podman PostgreSQL 16.15 run, exact tested source hashes,
authority/adverse-attribution mutation detection and confirmed cleanup. R01/R02's
[historical record](results.serial.json) and source hashes remain unchanged.

## History audit extension

The opt-in `--history` flag runs H01/H02 after the unchanged baseline and serial
cases. It installs an explicit experimental full-history audit, rejects earlier
gaps and attribution/request corruption, rolls every injection back, and checks
an executable gap-check mutation followed by restoration. The creation origin
is checked as the exact frozen structural fixture, not as production codec bytes.
Management events require the experimental request/result/digest representation.
Retired management payloads are outside this fixture's audit contract.

At 10, 100 and 1,000 events, it records three actual current/latest query plans
and a full-audit count/elapsed observation. The serving-shaped query requests one
indexed current row and latest event; the explicit audit scans retained history.
Timings include the host/Podman round trip where labelled; no throughput threshold
or production capacity claim is made. Native execution and actual results remain
separate from the lightweight evidence-framing unit tests.

## Concurrent authority extension

Add `--concurrency` to run C01-C04 after the native baseline and serial cases;
combine it with `--history` for the complete bounded follow-up. Use the same
explicit disposable-container and connection arguments shown above. The runner
does not start a VM or create/remove a container. Obtain maintainer permission
before heavy local execution, then repeat resource preflight after VM startup.

The [additional overlay](concurrent.sql) is experimental. The manager receives
only a restricted helper to change creation permission, with checked prior state,
manager/subject actor locks and an attributed fixture log in the same transaction.
It receives no direct actor UPDATE rights; serving roles cannot call that helper.
Fixture tokens use the bounded ASCII operation-token domain. Native NULL,
authority, missing-subject, stale/no-op and forged-log probes cover the new helper.
The structural creation fixture uses the restricted native creator connection,
actor fence, current creation authority and scoped stored-operation lookup. Its
minimal seed, experimental request/result and fixed replay clocks do not qualify
production codecs, request admission, replay expiry or a complete configurable
model. The separate project save/replay table models permission ordering only;
it is not WorkItem persistence. Verified actor identities remain trusted fixtures.

Each participant owns one native session and at most one in-flight SQL command,
with a 60-second lifetime and 4 KiB command/64 KiB output limits. Statements have
20-second deadlines, lock waits 10 seconds, and idle transactions 30 seconds.
The coordinator observes the intended `pg_blocking_pids` relation within five
seconds before releasing commit/rollback. Sleeps and timeout errors cannot
substitute for that evidence. Participant cleanup terminates only its uniquely
named backend, reaps the owned local process tree and confirms backend absence.
Body and cleanup failures remain distinct and retained.
An owned watchdog enforces the participant deadline even while the caller is idle.
The command bound counts UTF-8 bytes including the psql completion marker; rejected
oversized commands leave the participant idle. Cleanup cancels and joins the
watchdog before selecting the outcome. An existing body error remains primary;
otherwise observed expiry remains primary, with supervisor/backend cleanup errors
retained separately. Startup failure also cleans up any resources already acquired.
Cleanup uses PostgreSQL's [bounded termination wait](https://www.postgresql.org/docs/16/functions-admin.html)
before checking backend absence. A native shortened-deadline injection verifies
rollback and backend/process cleanup; it is not a timing or throughput measurement.

Creation and replay each run ten trials in both orderings. Additional cases
roll back each first participant and retry with fresh authority, distinguish
project-grant revocation from creation permission, retain another project's
grant, and check operation/payload/project/revision/grant/seed counts. Multi-actor
cases gate the later actor and probe the earlier one to observe lock order,
including reversed manager/subject arguments, rollback, stale-version rejection
and a fresh-version retry. Native mutations remove creation-authority admission
and reverse management lock order; the specific outcome/barrier oracles must
detect both, restore the original functions and rerun intact cases.
Lock-order mutation evidence requires the later-actor barrier first. Timeout
evidence requires the uncommitted management operation to have been reached;
startup failures cannot qualify either probe. Scope checks execute an allowed
control-project command and creation replay after project-grant removal.

[R05 evidence](results.authority-review.json) reverified the corrected session
harness and complete bounded suite at `9919460`. [R04](results.authority.json)
remains unchanged historical evidence; both records retain their exact scope.
