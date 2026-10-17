# SP-004: serial grant-management experiment

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
