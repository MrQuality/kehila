# Testing

The [test strategy](TEST_STRATEGY.md) describes the proposed long-term quality
approach. This guide records executable commands, current coverage, and limits.

[ADR-110](architecture/ADR-110-hosted-verification-testbench.md) and its
[research and risk plan](implementation/hosted-verification-testbench.md) propose
a hosted Linux-container bench with local Windows/WSL browsers. They activate no
workflow or transport and do not replace the required local or CI checks below.

For material deliveries, follow the accepted
[environment/harness admission controls](procedures/engineering-delivery.md#environment-and-harness-admission).
Check uncertain protocol, classification, permissions, deadline and cleanup mechanisms
with focused controls before expensive/user-assisted work. Record actual failing
stages and retain valid unaffected evidence; this neither changes required check
classification nor makes a probe qualify an unexecuted integration/browser journey.

For bounded technical investigations, see the approved [spike procedure and
register](spikes/README.md). Spike records distinguish planned cases from observed
results and link experiments to decisions and reusable regression coverage.

## Automated checks and user-assisted final verification

On 2026-11-06 the maintainer directed risk-based testing during development and
full testing only when the delivery goal is finished. Select development checks
from the changed invariant, its consumers and the consequence of failure. Use
real service-backed tests when the question concerns storage, authorization,
atomicity or concurrent behavior; pure tests and mocks cannot establish those
contracts. Broaden affected coverage when shared contracts change or contrary
evidence invalidates an earlier result. Do not repeat unrelated journeys simply
because another implementation checkpoint has been reached.

In the existing delivery record, name the selected checks and their decisive
oracles, retained evidence and its validity conditions, and deferred final cases.
Reserve full applicable qualification and final exact-head composition review for
the completed goal. A component checkpoint does not redefine that goal. Deferred
checks remain incomplete, and failures remain retained evidence.

The hook/CI selectors below distinguish development from final qualification.
They provide an automated backstop; they cannot infer every affected contract or
replace risk-selected boundary tests. Do not bypass them or describe a focused
development run as a full gate pass. Resource, container, privacy and trust
restrictions continue to apply.

## Risk-based selection and fault testing

Risk-based testing is in use as a manual selection practice with automated
execution and coarse scheduling backstops. The hook/CI modes distinguish
development from final qualification; focused native selectors execute explicitly
chosen cases. They do not automatically infer changed contracts, consequences or
all affected tests. The broader [test strategy](TEST_STRATEGY.md) remains Proposed.

For material changes, record the changed contract, consequential failure mechanism,
selected checks and decisive oracles, reusable evidence/validity conditions and
deferred final checks in the existing
[delivery record](procedures/engineering-delivery-record.md#risk-and-review-map).
Trace producers, consumers, permission boundaries and old assertions as well as
edited files. Prioritize consequence and uncertainty; avoid an automatic test
matrix or a numerical score that substitutes for judgment. Run cheap relevant
format/lint checks early where practical, then the smallest meaningful affected
behavior and integration checks. Required backstops still apply.

Test allowed and denied behavior against the current contract. Keep unrelated
inputs valid when varying one rejection condition, and use a positive control
where it establishes reachability. Require evidence of the intended rejection
boundary and unchanged durable state when applicable. A parse failure, timeout
or unrelated error cannot qualify a different security or integrity rejection.
Use actual relevant roles, storage and transport when those interactions are the
question; passing component tests alone does not establish their composition.

Controlled fault injection is a technique within this risk-based selection.
Existing native fixtures exercise selected reply loss, restart, backend loss,
disposable database-clock changes and interleavings; shared integration checks
exercise selected dependency stalls/unavailability. See the bounded
[WorkItem qualification](implementation/native-work-items.md#final-qualification-2026-11-06).
This is not a general chaos platform or exhaustive failure qualification.

When a risk warrants an automated fault experiment, identify the invariant and
permitted outcomes, applicable no-fault control, exact owned target and observed
fault activation. Check application results and independent durable state where
relevant, then joining, cleanup and recovery within bounded resources/time.
Record source/environment, fault point and observed order; retain a seed when
used, without claiming it reproduces uncontrolled OS/network scheduling. Keep
fault-injection, application and cleanup failures distinct; preserve first failures.
Use existing fixtures where practical. Broader randomized, multi-fault or long
campaigns need a separately justified scope and applicable execution authority.
No new chaos automation or default campaign is introduced by this guidance.

Schedule relevant bounded fault checks during development and include applicable
regressions in final qualification. Do not reopen a completed goal merely to add
unrelated fault cases. The author and relevant reviewer own selection and evidence
adequacy; the maintainer retains architectural and other nondelegated acceptance.

### User-assisted final verification

The maintainer directed this scheduling rule on 2026-10-31: checks that require
someone to operate a separate device, handle an operating-system confirmation
or make a temporary trust change belong to final qualification before the PR
is ready for review. They are not routine per-commit checks. User attention and
machine resources are part of the verification budget.

First prepare and review the implementation and test harnesses, complete
applicable automated checks, and resolve known harness failures. Automated
checks may include owned service-backed tests; separate execution and resource
permissions still apply. Then arrange one bounded user-assisted campaign with
a short checklist, prerequisites, expected duration, exact permissions and
cleanup. Group related cases within the approved scope and time window.

Retain passed evidence while its relevant source and environment premises remain
valid. A material change requires the affected cases and necessary positive
controls to be revalidated; an unrelated prose edit does not require another
manual campaign. Preserve failed attempts and state what invalidates or limits
earlier results. If a test harness fails during user-assisted verification, stop
that campaign, repair and check the harness without user involvement, and
schedule the affected rerun rather than repeatedly debugging through prompts.

This is a scheduling and evidence rule, not new semantic enforcement in CI.
Deferred or unexecuted cases remain incomplete. It neither grants security or
trust permissions nor authorizes changing a PR's status.

## Commands

The shared development backstop is `python scripts/verify.py --development` on
the CI host, or `python scripts/container_verify.py --development` through the
exported-source Podman procedure on Windows. It runs tooling/static checks,
page controls, Go/frozen checks and Rust workspace tests excluding the live-NATS
test crate. It does not provision the external service stack or run the native
PostgreSQL journeys. Some unit tests own local listeners; this is not a claim of
pure execution or absence of runtime activity. Reassess the service-independent
workspace inventory when adding tests/crates. Select affected real service cases
separately during development.

For a focused native WorkItem development check, the exported-source Podman
runner accepts `--native-item item-read` or `--native-item item-pending`.
Repeat `--native-item` to select both; duplicates and unknown cases fail before
runtime entry. Cases run in the fixture's canonical order, each with its isolated
pristine database, native runtime path and independent SQL observer. The runner
retains its compilation, source checks, execution budgets and owned cleanup.
Its output and retained failure diagnostics identify the focused scope.

This selects WorkItem journeys only; `item-write`, `item-mutation` and
`item-retry` are also available. The isolated `item-retry` case schedules three
real operational-head invalidations around one pending creation. It checks
initial-inclusive preparation/write counts, the exact exhaustion response,
unchanged request identity/deadline/admission, no victim acceptance, and explicit
same-identity recovery after reconciliation. Competing commits are observed
separately from the victim's state; a final SQL observer checks durable history
and authority release. These controls test provisional containment, not a
supported contention capacity or latency guarantee.

`--native-item item-capacity` selects an untimed input and observer validity
control. It uses four separately authenticated sessions, the unchanged default
project seed, and explicitly synthetic configurations with four or 128 text
fields per item type. Today's project-creation UI/API seeds two text fields per
type; the larger configurations are test-only capacity inputs, initialized
through the existing fenced storage writer. They add no product configuration
feature. The case checks full-field edits and exact reads, bounded request-stage
observations, verified TLS, independent durable-state SQL and owned shutdown.

Its four-request read burst retains submission rejections without retrying them.
Successful responses must match their requested snapshots; HTTP 429 counts must
agree with observed submission-queue rejection counts. The current nonblocking
submission channel has one queued-message slot, separate from the sixteen-job
admission limit. A separate untimed read checks recovery after the burst. Passing
this control establishes input/observer validity, not a supported four-request
capacity, latency result or completed capacity study. Timed workload results and
their acceptance must be assessed separately.

Linux test observations record elapsed wall time and OS-worker thread CPU time
separately. Thread CPU intervals begin and end inside the same synchronous work
closure; skipped work has no CPU measurement. Ordinary queue and storage spans
have no CPU value. Incomplete work stays distinguishable during unwinding, and
invalid numeric observations invalidate the cell. These controls do not establish
clock calibration or negligible observer overhead.

Focused execution supplies neither the other native journeys nor shared full
verification. `--native-authority` and the default full runner retain their
complete native sequence. Mocked controls cover selector orchestration; later
[WorkItem evidence](implementation/native-work-items.md#current-implementation-checkpoint-2026-11-06)
records actual focused mutation/read execution and independent observers,
superseding the earlier pending-execution status. That evidence has its stated
source/fixture limits; availability of a selector does not qualify future changes.
Use the existing exported-source and explicit dependency-preparation procedure;
do not run the container verifier directly from a checkout.

Full and pure verification validate the [engineering register](engineering/ENGINEERING-STANDARD.md)
and run Go formatting/vet checks in addition to the existing matrix. CI retains
Rust formatting and Clippy checks. Run these individually with:

```text
python scripts/check_engineering.py
python scripts/go_static.py
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
```

After editing `docs/engineering/requirements.json` or the authored Markdown
preamble, regenerate with `python scripts/check_engineering.py --write`.
`--gate E1` (or E2–E5) additionally rejects recorded mandatory cumulative gaps
and reports recommendations/options without blocking; it does not approve
evidence, justified SHOULD departures or releases. Product M* IDs are not accepted
as engineering gates.
Register regression tests cover stale generation, ID uniqueness, invalid metadata,
missing evidence and broken references; Go static-check tests cover failing format/vet.
Go formatting, vet and the default test runner share the explicit module inventory
in `scripts/go_inventory.py`. It must agree with `go.work` and every module/source
under `go/`; omitted modules or out-of-module Go files fail verification. Add a new
module to both the inventory and workspace before relying on shared checks.
See [qualification](engineering/QUALIFICATION.md) for future property, fault,
performance, recovery and release campaigns and their current evidence boundaries.

Run the shared host matrix with a ready container engine:

```text
python scripts/setup_env.py --hooks-only
python scripts/verify.py
```

Native product qualification also requires the exported-source container fixture
and its independent SQL observers. The default full container gate includes that
sequence; CI retains a separate required native job. Host infrastructure readiness
alone does not qualify the native authority. See the
[retirement dispositions](implementation/ferretdb-retirement.md).

### Standalone A0 verification boundary

AUTH-001 is a separate Cargo workspace. `scripts/verify.py` and the staged
pre-commit verifier currently run the root workspace and do not invoke the
experiment's manifest. CI runs its formatting, clippy and tests in a separate
step. A successful shared staged gate therefore does not establish AUTH-001
verification; retain the separately executed, source-bound results.

The [documented standalone command](../experiments/AUTH-001/README.md#run) and
CI use `--all-targets -- --test-threads=1`. Record actual target/harness selection
when comparing results; a pass under one profile does not qualify another.
The [2026-10-26 patch record](implementation/local-owner-authentication-a0.md#advisory-follow-up)
distinguishes default-harness Windows runs from serial Linux CI and leaves
matching serial Windows verification unqualified.

The maintainer/#40 owns a **Medium/P2** follow-up under B-019: use one standalone
check from the staged local and CI paths. Acceptance requires evidence that a
failing staged AUTH-001 case fails the local check, both paths run its manifest,
and external build outputs, bounded execution and cleanup remain preserved.
This records the gap; no verifier or test behavior is changed here.

The experimental [Windows owner client](../experiments/AUTH-001/clients/windows/README.md)
has an independent `build.ps1 -Test` path using installed .NET Framework. Give it
a protected output directory outside the checkout. Its local DPAPI/file and
closed-response tests contact no service and install no trust. A separate hosted
Windows job is configured for those checks; it does not qualify TLS negotiation,
browser behavior or the Linux application-container path, and does not resolve
the standalone Rust staged-entry-point gap above.

The separate [A1 disposable SQL proof](../experiments/AUTH-001/storage/README.md)
is opt-in and outside both the Cargo checks and shared verifier. Its native
PostgreSQL observations, selected resource profile and source hashes must be
retained separately; a normal local/hosted gate does not execute its SQL oracles.
It does not qualify password/session/HTTP behavior or the rest of A1–A3.

### Owned test services

Full verification first runs tooling/static checks, precompiles the complete
Rust workspace test executables, executes the offline workspace tests with
`--workspace --exclude event_dispatcher`, runs frozen oracles and Go tests, and
completes workspace compilation before live services. Child commands use `CARGO_BUILD_JOBS=1` and append
`-p=1` to existing `GOFLAGS`; compiler parallelism is bounded without reducing
test counts or changing application concurrency.

Only then does it start unique, disposable Compose projects, one phase at a time:

| Phase | Running services | Required checks |
| --- | --- | --- |
| Broker execution | NATS | TCP readiness, account API probe and all `event_dispatcher` tests, including the live broker round trip |
| Search readiness | OpenSearch | TCP readiness and yellow/green HTTP cluster health |
| Database infrastructure | PostgreSQL | TCP readiness; native product behavior uses the separate isolated PostgreSQL TLS fixture and state observers |

The runner verifies one running owned container for each requested service,
then confirms no owned containers remain after teardown. Each project is removed
with its volumes before the next phase starts. The
runner registers cleanup before startup, so partial starts, failed checks and
child-command timeouts still clean up. Failure diagnostics are printed before
teardown (`logs --tail 200`, with a 30-second command limit). This bounds
lines per container, not bytes per line. Cleanup failures remain errors: the
primary failure and every labelled Compose/VM/directory cleanup failure are
retained and printed without rewriting the original exception cause. The shared
reporter is used at both verifier and child-check CLI boundaries, including
normal interruption (exit 130); argument-parser exit codes remain unchanged.
Resource-owning checks use an explicit cleanup owner: the body exception remains
primary and each callback failure is attached with its resource label, even if
the callback suppresses its Python context or supplies its own cause. All
registered callbacks run. On a successful body, the first cleanup failure is
primary. The reporter follows unsuppressed Python contexts with cycle/duplicate
protection; it does not undo deliberate context suppression.
The native verifier owns a 600-second work budget (`--timeout`, positive and at most
600). The Windows container lane uses configured finite gate/command allowances,
with separate explicit dependency preparation; see the container design below.
Every work command receives the smaller of its own timeout and the
remaining budget. Resource-owning Python checks (Go tests, SP-002/SP-003 check
drivers) receive a same-host monotonic
deadline through `KEHILA_VERIFICATION_DEADLINE`. Their parent waits while they
time out leaf work and unwind their own directories and processes.
The parent does not kill these Python owners
at the work deadline. Standalone commands keep their existing local timeouts.
Diagnostics and teardown use their separate bounded command
limits after work expires. The staged hook passes this budget and waits for the
owner to finish cleanup; it does not kill the owner at 600 seconds. Cleanup can
extend total elapsed time beyond the work budget. Normal interruption unwinds
these contexts. Forced process termination or host
failure can prevent cleanup; inspect resources carrying the printed unique
`kehila-test-...` project label before removing any leftover test project.

The Windows container coordinator can additionally write a new external host
record with `--ownership-file`. A supervising caller can supply its own fresh
32-character lowercase hexadecimal `--resource-owner` (or
`KEHILA_VERIFICATION_RESOURCE_OWNER`) to bind that record to the launched run.
The owner is validated and recorded before executor resource creation; this
does not imply validation before VM startup. Keep records outside the source
checkout: they include machine-local routing/source metadata and are never
copied into the application container.

The version-one record reserves the applicable finite service groups before
creation. A reservation becomes actionable only after its namespace is verified
empty and that authorization is persisted before Compose startup. Tracked normal
cleanup also records partial-start identities before deleting their containers.
Recovery requires a fresh caller-bound record, the original source/route/owner
bindings, and confirmed termination/join of the lifecycle process and its
descendants. Use the same bounded `ComposeResources` instance across recovery
of all groups to retain one absolute deadline. Recovery validates resources
before removal, targets full container/network IDs and refuses replacements,
foreign endpoints, binds or named volumes. Anonymous volumes remain tied to
their exact container; known retained volumes prevent a successful cleanup
result. Old owner-only records are not version-one service recovery authority.
A recovered resource set does not change the original verification failure
into a pass. These controls cover surviving host supervision, not arbitrary
host failure or a general resource-pruning command.

Default engine selection tries a ready Podman runtime, then Docker. Use
`--engine docker` or `--engine podman` to select explicitly. On Windows an
explicit stopped Podman machine can be started for the run:

```text
python scripts/verify.py --engine podman --podman-machine NAME
```

Use the name reported by `podman machine list`. The VM starts after builds;
shared VMs remain running, including those started by this command. Automatic
shutdown requires an explicit exclusive-use declaration:

```text
python scripts/verify.py --engine podman --podman-machine NAME --exclusive-podman-machine
```

This declares that no other program or user will use the VM during the run.
The machine must initially be stopped. A nonblocking OS file lease serializes
verifiers using the same OS account and explicit machine name. The lock lives
under `.kehila-verifier-locks` in the OS-resolved account profile (Windows
`FOLDERID_Profile`, POSIX account-database home), independently of shell username,
home and temporary-directory variables. The Windows profile also avoids
per-application [LocalAppData redirection](https://docs.python.org/3.10/using/windows.html#redirection-of-local-data-registry-and-temporary-paths).
The directory must not be redirected;
on POSIX it must belong to the account and exclude group/other permissions.
Lock files persist after release so another owner cannot lock a replacement
inode. Stop older verifier versions before using the new lease namespace; their
temporary-directory locks do not coordinate with it. The lease cannot prevent
unrelated Podman clients from using that VM. Do not select exclusive mode for a shared VM.
An unexpected active container in either store fails exclusive cleanup and leaves
the VM running. Without the exclusive declaration, idle snapshots never authorize
shutdown. An exclusively owned partial start receives a bounded stop attempt;
cleanup failures remain visible alongside the original failure.

Before startup, the runner requires the machine-created `NAME` and `NAME-root`
connections and checks their loopback SSH host/port/user and distinct service
socket paths. Rootful access must use `/run/podman/podman.sock`; rootless access
must use `/run/user/<uid>/podman/podman.sock`. The chosen verified connection is
carried explicitly to Compose and container inspection without changing global
connection defaults. `CONTAINER_CONNECTION` can select either verified store;
a foreign connection or `CONTAINER_HOST` combined with an explicit machine is
rejected. Owned mode also rejects ambient `DOCKER_HOST`/`DOCKER_CONTEXT` and
`CONTAINER_HOST`; without an explicit machine it rejects `CONTAINER_CONNECTION`.
Unset these overrides or use caller-managed external services. Without a named
machine, native Podman must report `serviceIsRemote=false` and subsequent commands
use `--remote=false`. Docker's effective saved context must identify a local Unix
socket or Windows named pipe; the validated context is pinned for Compose and
inspection. Remote Docker contexts are rejected even when no override variable
is set. Engine/machine/mode and static routing conflicts fail before builds;
saved daemon metadata is checked before container creation.

For commit hooks, set `KEHILA_TEST_ENGINE` and optionally
`KEHILA_TEST_PODMAN_MACHINE` in the shell that invokes Git; the verifier uses them
as defaults for the equivalent CLI arguments. Set
`KEHILA_TEST_EXCLUSIVE_PODMAN_MACHINE=1` only for an initially stopped VM reserved
exclusively for that verification run (default `0`). External-services mode
rejects machine-ownership settings instead of silently ignoring them. CI selects Docker and uses this
same lifecycle for both shared and staged verification. The staged source
export, pure/full classification and failure checks remain required.

The commit hook preserves an explicit `CARGO_TARGET_DIR`; use an absolute path
to keep build artifacts in a caller-selected directory outside the checkout.
Without that setting it retains the checkout's `target` cache. Either choice
still verifies only the isolated staged source snapshot.

The shared host ports must be free. Existing development containers are neither
borrowed nor stopped by the owned mode. To keep a manually managed disposable
development stack running, use these commands instead:

```text
python scripts/setup_env.py --start
python scripts/verify.py --external-services
```

That explicit mode performs all full checks against existing services and leaves
their lifecycle to the caller. It does not provide the overlap reduction of the
owned mode. Both full modes require the default local test endpoints:
`KEHILA_NATS_ADDRESS=127.0.0.1:4222`, `KEHILA_NATS_HOST=127.0.0.1` and
`KEHILA_NATS_PORT=4222` (unset variables use these defaults). Overrides are rejected
before builds so readiness, the Python account probe and Rust broker tests agree
about their targets. `--external-services` changes lifecycle ownership; it does
not add custom endpoint support. Use standalone `nats_probe.py` for custom broker
endpoints. Native PostgreSQL investigations retain
their separate documented container/resource preflight and qualification scope.

Generated SP-002 check executables now use the shared owned temporary-directory
cleanup policy on success and failure. The frozen oracle source/fixtures and
historical evidence remain unchanged. Current SP-002/SP-003 Python support
scripts participate in deadline handling; their behavior is separate
from those recorded native runs.

### Lifecycle verification record

These dated records preserve their actual source and earlier service composition,
including the retired task prototype. Use the current matrix above and the
retirement record for active paths; a historical pass is not current-head
qualification.

The source at `20c1664` passed the full staged gate on Windows/WSL2/Podman on
2026-10-14: 170 tooling tests (168 passed, two POSIX-only skips), all Rust
workspace tests with NATS, Go checks/tests, frozen oracles, OpenSearch readiness
and the complete live task-path checks. The gate began with a stopped explicit
VM; each owned service group was removed and the VM stopped automatically.
An additional real NATS run deliberately raised a test failure after startup:
the original failure survived, its project was removed, and the VM stopped.

The first lifecycle attempt passed its live checks but failed snapshot cleanup
because `podman machine ssh` created a literal Windows `NUL` known-hosts file.
VM cleanup now uses registered Podman connections with checked machine/store
endpoints. Regression tests additionally reject missing owned containers,
remaining containers after successful-looking teardown, foreign/missing machine
connections, and cleanup errors that would conceal test failures. Pure/docs and
staged-selection regression checks remain in the shared suite. These results
verify test lifecycle behavior, not a controlled memory benchmark or a new
native application adapter. Tracking: [#55](https://github.com/MrQuality/kehila/issues/55).

Review follow-up at `5ce6875` passed the full staged gate on
2026-10-14: 198 tooling tests (196 passed, two POSIX-only skips), the complete
Rust workspace split between offline and broker phases, Go/static/frozen-oracle
checks, OpenSearch TCP/HTTP readiness and all live task-path checks. Every owned
group was removed and the exclusively reserved VM stopped. Nine targeted fault
mutations were each detected by the regression suite.

An additional native Podman/NATS check rejected a foreign registered connection
before VM startup. Injecting an elapsed time beyond the work deadline after real
NATS startup preserved the deadline error, removed the owned project and stopped
the exclusive VM. This checks cleanup after controlled deadline exhaustion; it
is not a 600-second wall-clock experiment or a forced-termination guarantee.

One intermediate full gate passed its Go tests but failed temporary executable
cleanup with Windows sharing violation 32 after the existing 30-second retry
bound. The unchanged retry passed; no timeout, cleanup policy or test was
weakened. The final source gate passed. Obsolete generated SP-002 executables
were removed; future check runs clean their own temporary outputs on success or
failure. No controlled memory benchmark is claimed.

Further review fixes at `c941ab8` passed all five focused source-commit gates;
the final full staged gate ran 223 tooling tests (221 passed, two POSIX-only
Windows skips), the complete Rust/Go/static/frozen-oracle matrix and every
required service check. Seven targeted fault mutations were detected, covering
rootful selection, host-override rejection, URI metadata, per-user lease
identity, log limits, remote Docker context rejection and the owner wait policy.

An additional real PostgreSQL/FerretDB run injected elapsed time past the child
work deadline after both Rust workers were ready. The cooperative child stopped
both workers and successfully dropped its unique test collection. Its failure
propagated to the parent, which printed bounded Compose diagnostics, removed the
temporary binaries and owned service group, and stopped the exclusive VM. This
is controlled deadline-exhaustion evidence, not a 600-second wait or a
forced-termination test. Nested-failure regressions separately confirm that
Compose, VM and directory cleanup failures all remain visible with the primary
failure, including at child CLI boundaries. The earlier Windows sharing-lock
failure has not been attributed to orphaned processes. A subsequent implicit-
context and caller-environment isolation regressions raised the tooling count
to 225 (223 passed, two Windows POSIX-only skips). Matrix fixtures establish
their own routing defaults, so standalone endpoint settings do not invalidate
pure/docs tooling tests; the actual full CLI still rejects nondefault targets.
One intermediate gate passed all Go tests but failed temporary-executable
cleanup after the existing 30-second Windows sharing-lock retry bound. No
surviving test executable process was found; the directory was removable on
subsequent bounded cleanup. The unchanged source retry passed the complete full
staged gate. The cleanup timeout and policy were not weakened.

The next follow-up covers callback errors with suppressed contexts/explicit
causes, stable account-profile leases, explicit dual-store routing/environment
forwarding, VM teardown after the parent work deadline and the Windows/Cargo
timeout matrix. All five previously surviving targeted mutations now fail their
intended assertions. Tooling fixtures isolate inherited verification deadlines,
including an expired value, while deliberate inheritance tests and production
configuration validation remain active. SP-003 shares the directory owner with
an explicit five-second retry budget; failed pure cases print a single diagnostic
line limited to 4096 UTF-8 bytes, excluding its newline. These checks do not
establish crash/forced-termination cleanup guarantees.

Source `00e02fe` passed the full staged Windows/WSL2/Podman gate on 2026-10-15:
240 tooling tests (238 passed, two POSIX-only skips), all Rust/Go/static/frozen-
oracle checks and every required service phase. The same tooling suite passed
with inherited `KEHILA_VERIFICATION_DEADLINE=0`. Windows installer and packaged
Python resolved the same account-profile lease path. Four earlier gate attempts
failed on Windows Go-executable sharing locks after testing (one also failed a
startup timing assertion); no surviving test process or matching file handle was
found afterward. The files became removable later. Full gates then passed with
isolated temporary storage and serial Go package work, keeping the existing
30-second cleanup policy and all checks. A lease-development gate also rejected
LocalAppData redirection before VM startup; the profile-based lease passed.

At the same source, the controlled PostgreSQL/FerretDB deadline check was rerun
after both real Rust workers became ready. Injected elapsed time expired the
child's inherited deadline; both workers stopped, collection cleanup succeeded,
the failure propagated, bounded diagnostics ran, temporary outputs and the owned
service group were removed, and the exclusive VM stopped. This checks the new
callback owner against real services; it remains controlled deadline-exhaustion
evidence rather than a 600-second wall-clock timeout or forced termination.

The combined PR #54 source at `abc45fa` passed the full staged
Windows/WSL2/Podman gate on 2026-10-16: 247 tooling tests (244 passed; two
POSIX-only skips and one unavailable Windows symlink-creation privilege),
Rust/Go/static/frozen-oracle checks and every required live phase. Literal
loopback-only Go listener admission, explicit SP-004 connection routing, CLI
cause/interruption reporting, bounded crash diagnostics and canonical cleanup
refusal have direct regressions. Six targeted Python/Go mutations were detected;
failure injection skips overlay DDL so its error text cannot substitute for a
reached SQL case. These tooling checks are separate from opt-in native evidence.

SP-004's [R03 record](../experiments/SP-004/results.serial-guards.json) preserves
the serial distinct-manager/guard extension executed against PostgreSQL 16.15,
its exact source hashes and confirmed cleanup. The source files are preserved
at `da59c69`. Removing management-authority admission and swapping manager/subject
attribution were detected in real database transactions; both rolled back and
state/guard restoration was checked. Those serial results did not qualify
concurrency/authentication or Linux-host native execution. See the
[canonical scope and remaining cases](spikes/SP-004-actor-authority.md).

PR #59's bounded follow-up adds explicit full-history audits and controlled
creation/replay versus revocation schedules. After preparing the documented
fresh disposable fixture, invoke both extensions with:

```text
python experiments/SP-004/run.py --disposable-container kehila-sp004-test --history --concurrency
```

Select `--podman-connection` explicitly when needed. The
[runner guide](../experiments/SP-004/README.md#concurrent-authority-extension)
defines restricted fixture roles, process/SQL/output bounds, observed lock
barriers and negative-oracle prerequisites. Compiler and native execution need
prior maintainer permission and a fresh memory check. Native evidence is separate
from the shared staged/hosted matrix and does not qualify real authentication or
Linux-host native execution.

[R04](../experiments/SP-004/results.authority.json) passed the bounded H01/H02 and
C01-C04 fixture checks on PostgreSQL 16.15/Windows/WSL2/Podman at `42389e9`, with
52 scheduled cases, observed actor barriers, three native mutation checks and
confirmed timeout/backend/container/VM cleanup. The record preserves a failed
multiline metadata attempt and its regression, exact exercised hashes and all
query plans. [The canonical summary](spikes/SP-004-actor-authority.md#bounded-history-and-authority-continuation-sp-004-r04-2026-10-17-utc)
states resource/timing observations and retained qualification limits. R04's
source staged verification ran 261 tooling tests (258 passed, three documented
Windows platform/privilege skips), plus all required shared checks.

The [PR #59 review follow-up](spikes/SP-004-actor-authority.md#pr-59-review-follow-up)
adds command-state, backend-absence, watchdog lifecycle and timeout-oracle
regressions: 36 focused tests passed and eight targeted in-memory mutations were
detected. Both source commit gates passed; the latest ran 273 tooling tests
(270 passed, the same three Windows skips) and the required shared/service checks.
Those tests do not replace the separately recorded native PostgreSQL run.
[Native R05](../experiments/SP-004/results.authority-review.json) separately passed
the complete bounded history/authority suite at `9919460` on PostgreSQL 16.15 /
Windows / WSL2 / Podman, including all 52 schedules and confirmed cleanup.
R04 remains an unchanged historical record; the same qualification limits apply.

Verification provider timeouts are defined in
[`config/verification-defaults.json`](../config/verification-defaults.json).
For a machine-specific override, set `KEHILA_VERIFICATION_CONFIG` to an absolute
JSON file outside the checkout. Settings cover provider start/stop and the
container runner image, memory, CPU, PID and stop budgets, finite gate/command and
dependency-preparation allowances, plus optional host
memory guards for lifecycle controllers; omitted values inherit
upstream defaults. The [container gate design](implementation/container-verification.md)
describes the execution boundary and provisioning requirements.
Unknown or duplicate keys, non-finite/non-positive values and missing files fail
closed. Machine overrides are rejected in CI. For example:

```json
{"machine_start_seconds": 240}
```

This example is a provisional allowance, not a measured startup requirement.
Effective start/stop settings are captured before machine startup, so changing
the local file during a run cannot prevent cleanup. Startup still respects the
remaining total work deadline; a larger startup setting does not extend the
configured gate. The container lane's `gate_seconds` and `command_seconds` default
to 600 and 300 respectively. `dependency_prepare_seconds` defaults to 1800 and
applies only to explicit `--prepare-dependencies`, which must precede pure/full
container verification. Cleanup retains a separate finite stop budget. The
30-second snapshot cleanup retry policy is unchanged. Configuring timeouts
does not waive required checks, resource approvals or the memory floor.

Machine calibration and an inventory of other policy constants remain follow-up
work after the current delivery: measure startup/stop and phase timings together
with memory before choosing durable local values. Broader calibration is not a
prerequisite added to this PR. A longer timeout is not a remedy for an unidentified
snapshot-directory lock.

Task-path boundary failures report a case label and preserve the underlying
exception. On failure, the harness emits up to 8 KiB from each owned process log
after process and log cleanup. A diagnostic-output failure remains attached to
the original failure. Expected HTTP statuses and stalled-body deadlines remain
mandatory; a connection reset does not satisfy the expected HTTP 408 response.
The staged hook retries transient Windows snapshot cleanup locks within a bounded
budget and preserves verification failures alongside cleanup failures. Scope
selection and export use the same captured immutable Git tree through a temporary
index outside the payload. Final real-index and HEAD comparisons still reject a
changed proposal; a copied-source byte manifest verifies container transfer. Provider
lifecycle commands use the stable OS account directory, since a shared provider
may outlive the verifier's staged snapshot. Compose/service commands retain their
source working directory. A Windows observation reproduced blocked removal of a
provider's launch directory while running and successful removal after shutdown;
the specific locking process was not identified. Longer cleanup retries are not
used to substitute for correct lifetime ownership.

On Windows, the staged hook uses the container verification lane and requires an
explicit immutable toolchain image in `runner_image` or
`KEHILA_VERIFICATION_IMAGE`. Host code controls resources and
source copying; toolchain commands and runtime tests execute in the runner.
There is no host fallback. The existing Linux CI gate and Windows-client CI job
remain mandatory. Linux runner results do not qualify Windows-only APIs.

Run unit tests without external services:

```text
python scripts/verify.py --pure
```

Run lightweight documentation verification with:

```text
python scripts/verify.py --docs
```

This mode runs Python tooling regressions, naming and engineering metadata/
generated-document checks without compilers or external services. CI keeps the
always-triggered `verify` job and selects this shortcut only after a valid diff
shows known ordinary prose documents. Engineering register/preamble/generated
standard inputs cannot take it. Non-prose, unknown and empty diffs select the
development backstop for validated draft PRs and full verification for ready PRs
or main pushes. Code deletions/renames cannot become prose-only by similarity.
For unknown draft paths, the backstop covers its explicit inventories only.
The author must trace affected contracts and select relevant checks; it does not
prove coverage of an arbitrary new subsystem. Hold affected dependent work when
that scope cannot be established. This deliberately avoids an automatic full
campaign during unfinished development; it does not reduce final qualification.
Malformed events, invalid draft state and unavailable comparison bases fail
selection explicitly. No workflow-level path ignore or commit-message bypass is
added. Lightweight results establish only their documented scope.

At the finished goal, an authenticated author requests full draft-PR qualification
by applying the exact `qualification` label. This `pull_request: labeled` event
forces full checks even for prose/empty diffs. It uses the event's added label,
not the PR's retained label list: later draft updates remain development runs.
Remove/reapply the label for a fresh final request, or rerun prior evidence only
when its exact source remains applicable. Do not add an automated token-driven
label step or mark the PR ready to obtain its first full qualification. Resolve
merge conflicts before using PR workflows.

Record the checked-out head SHA, event/action, mode and workflow URL with final
evidence. The workflow checks out the PR head, not GitHub's default merge ref.
The native job explicitly defers its journeys outside full scope; its successful
development job status is not executed native evidence. A readiness event runs
full qualification conservatively and can repeat the already completed final
run; it does not replace the full pass required before readiness.

The full suite runs Python tests for the development tools, service readiness
checks, the NATS account API check, Rust workspace tests, and Go tests. Missing
tools, unavailable services, and test failures produce a nonzero exit status.

Both modes also run `python scripts/check_branding.py`. It rejects the retired
project spelling and former package/import, environment, repository and design
document identifiers, plus the earlier retired JQL-related identifiers, in paths
and UTF-8 text. Its explicit inventory covers public root files, source, packages,
Go modules, documentation, scripts, tests, experiments, GitHub configuration,
tracked commit-hook entry points, `.gitignore`, `.gitattributes` and `.env.example`.
The inventory works in Git-free staged snapshots. Local instructions, environment
files, local evidence, build/dependency directories, the exact naming regression
module and its shared rejection-data JSON are excluded; binary and non-UTF-8
content is skipped.
Retained storage and SP-001 identities have path-and-context exceptions, and the
exact migration rows in [the naming policy](BRANDING.md) are allowed. Exceptions
do not exempt whole files. Factual third-party references and attribution remain
permitted. This bounded scan does not establish legal clearance or validate all
semantic naming and attribution claims; those still require manual review.

Individual commands:

```text
python -m unittest discover -s tests -v
python tests/integration/nats_probe.py
cargo test --locked --workspace
python scripts/go_test.py
```

## Coverage

Unit tests cover the equality grammar, schema-version decisions, and the
pure B-003 status/workflow command rules. The latter test configuration
references, phase derivation, migration restrictions, archival targets, stale
versions, and replay order; they do not establish persistence or concurrency.
Pure project-rule tests cover archive access effects, project-local readable ID
allocation, exact estimate parsing and the monotonic unit-lock flag, and
the populated-field kind-change rule. Their cross-record storage effects are
still unverified.
Project metadata command tests cover one coordinated revision, issued IDs across
prefix changes, name and unit validation, estimate-unit locking, stale and
archived state, and replay precedence. Atomic persistence
is B-005 work.
Pure field-rule tests cover hidden value preservation, optional/required
validation, typed values and WorkItem type ownership, choice-option archival,
and calendar boundaries. Required-field changes under concurrent writes still
need authoritative storage checks.
Pure knowledge-value tests cover typed entry kinds, labeled file references,
UTF-8 limits, identity/version validation, and stable error codes. They do not
establish durable retention. Pure knowledge-command tests cover create/edit
revision append, author retention, current grants before replay, stale entry
versions, incomplete history, wrong value kind, and archived targets. Atomic
persistence remains unverified.
Pure follow-up tests cover cross-project Link grants, single-origin and cycle
rejection, and replay precedence. Concurrent ancestry and endpoint checks
remain B-005 integration work.
Pure relationship and current-work tests cover type ownership, type-use and
two-endpoint Link inputs, canonical duplicate/self-link rejection, stable record
ID in directed inverse and symmetric views, archival
eligibility, selection versions, and selection independent of lifecycle.
Relationship command tests cover both endpoint project revisions and item
versions, current authorization before replay, and changed operation content.
Atomic multi-record storage checks, uniqueness, and authorization enforcement
remain unverified.
Pure configuration-change tests cover active defaults, replacement before
archival, dependency-preserving edits, phase mapping preservation, and revision
and authorization gates. They also cover required-field values, historical
kind/removal protection, and value preservation under rename, hide, and archive.
The complete-revision command checks replay, changed operation content, and
revision-only no-ops.
Relationship-type configuration tests cover owner scope, duplicate IDs,
rename/archive compatibility, and rejection of direction changes or removal
after historical use.
Complete-configuration tests also reject removal of an option from a retained
single-choice field, even when no current item uses it; rename, archive, and
reordering keep the stable option identities. An unused choice field may change
kind or be removed. Display-name tests reject control characters in field,
option, and relationship-type names.
The complete configuration bound is checked for excessive entry counts and
aggregate UTF-8 string bytes. Encoded HTTP-body limits remain adapter work.
Concurrent item/configuration writes remain B-005
integration checks.
Pure conversion tests cover complete destination validation, migration
authorization, both workflows' phase permissions, Done-to-New rejection,
reopening, source-value history output, and successful replay precedence.
Durable history, knowledge, and provenance retention remain B-005 checks.
Pure boundary tests also cover the accepted M1 estimate, prefix, UTF-8 text,
numeric-field, and Gregorian-date limits, project-scoped readable IDs, and
the 128-entry/1-MiB logical field payload bound.
Pure item-mutation tests cover untitled creation and title display, initial
status, required/hidden fields, payload rejection, estimate locking,
item/configuration versions,
and replay. Durable sequence allocation and coordinated project/item writes
remain B-005 checks.
Pure archival tests cover item/project version conflicts, restore eligibility,
replay, matching project/configuration state, and versioned selection clears
without lifecycle changes. Coordinated cross-user selection clearing remains
a B-005 integration check.
Pure option-administration tests cover stable IDs through rename and archival,
retaining old assignments while blocking new ones, restoration, invalid names,
duplicate IDs, revision conflicts, and replay. Concurrent configuration and
item writes remain a B-005 integration check.
Pure error-code tests cover shared conflicts across commands, nested field and
project failures, distinct selection/relationship codes, and every stable wire
string. API status and
message mappings remain unverified.

Critical B-003 scenarios have direct pure test anchors:

| Contract scenario | Pure test file | Downstream evidence still required |
| --- | --- | --- |
| BC-19, BC-37 estimate-unit lock and metadata revision | `src/pure/task_contract/tests/project_admin.rs`, `project_rules.rs` | B-005 concurrent first estimate versus unit change |
| BC-23, BC-28 field kind and historical use | `src/pure/task_contract/tests/configuration_change.rs`, `configuration_fields.rs` | B-005 authoritative usage and item snapshots |
| BC-32 configuration replay and revision | `src/pure/task_contract/tests/configuration_change.rs` | B-005 durable history and atomic commit |
| BC-34 follow-up origin | `src/pure/task_contract/tests/follow_up.rs` | B-005 atomic ancestry and B-007 grants |
| BC-36 configuration and option limits | `src/pure/task_contract/tests/configuration_change.rs`, `option_admin.rs` | B-004/B-006 encoded-body limits |

This mapping identifies pure checks; it does not claim service-level acceptance.

Integration tests use the development NATS server to check its greeting, connection handshake,
and request/response behavior. The Python probe also checks the JetStream account
API using a temporary subscription. It creates no streams or application data.
Connections use three-second deadlines and bounded frame sizes and counts.

The native container fixture runs authenticated Project/Task/Milestone paths
against isolated PostgreSQL TLS databases and checks independent durable-state
observers. Its selected `item-read` case includes socket-body timeout with a
concurrent authoritative read, and an observed serving-network stall followed by
owned shutdown and an authenticated successor read. A complete request validates
the body harness before the incomplete request; actual held bytes validate the
network fault before its original deadline expires. These controls retain the
native protocol and single-active authority boundary.

The body reader uses the earlier of the original connection deadline and two
seconds after body reading starts; the socket-body fixture has a five-second
total budget. Prompt admission leaves time for the 408 response and closure.
Late admission or an exhausted total budget can prevent response delivery; no
additional response grace period or production deadline extension is implied.

The serving-response relay enters explicit cleanup before releasing held bytes.
Release is not a delivery acknowledgment. Only a downstream serving-client write
that encounters `BrokenPipe` or `ConnectionReset` after the observed gate is
released in cleanup may terminate normally. Implicit gate release from join or
drop does not establish that phase. Database reads, upstream traffic, control
failures and other write errors remain strict, as do task panics and join
deadlines. Existing owner/relay joins, independent database release and successor
read checks remain required. Paired injected-error and closed in-memory duplex
peer tests cover classification; they do not reproduce a TCP reset or replace
the native PostgreSQL/TLS composition fixture.

The removed worker/API, their live scripts and controller-specific tests have
explicit keep/adapt/retire dispositions in the
[retirement record](implementation/ferretdb-retirement.md). They are not optional
legacy execution lanes. Historical source and observations remain available.

The Go runner and frozen oracles retain owned temporary compiler outputs, visible
cleanup errors and bounded diagnostics. Directory cleanup tests retain canonical
path refusal and real symlink coverage where the host permits it. Windows hosts
without symlink privilege skip that filesystem case; the portable redirected-path
control still requires refusal before deletion.

The pure command includes `kehila_query`, `task_contract`, `project_create_codec`
and `work_item_codec`. Maintain this explicit list when adding a pure crate;
selection tests require crate failures to propagate. The shared host suite tests
all remaining Rust workspace crates, with the live NATS round trip in its broker
phase. Native database qualification remains the separate fixture described above.

### Required retesting after retirement or native-path changes

During implementation, run the affected pure and boundary tests. After changes
to native storage, dependencies, service ownership or verification, complete
applicable native fixture/observer checks and the full shared service matrix on
the final revision. The full exported-source container gate composes these
checks. Do not substitute `--pure`, an unrelated focused case or a previous
revision's CI for affected final evidence. Run `cargo fmt --all -- --check` and check
changed Go files with `gofmt`. Required GitHub CI must pass on the final revision.
A failed or unavailable live dependency is a failed check, not a skip.

Repeat SP-001 C01-C06 when its Python adapters, physical storage mapping, CDC
configuration, or projection behavior change, or when new evidence contradicts
its conclusion. Changes confined to the native application boundary require fresh
native contract evidence rather than a repeat of an unchanged historical
Python experiment. Native event/search delivery, supported populated upgrades
and real-data recovery remain separately scoped work.

Integration results apply to the behavior exercised. A successful handshake does
not establish durable publication, replay, or recovery. New adapters need tests
against their actual services for the contracts they introduce. Unit tests and
test doubles may complement these checks.

## Local hooks and CI

The POSIX real-Git hook tests require executable external temporary storage.
Before initializing each test repository, a bounded, directly invoked `/bin/sh`
script checks that execution is permitted in the selected scratch directory.
It is removed before fixture staging. Failure reports how to configure `TMPDIR`,
`TEMP` or `TMP` before starting Python; the tests do not change mount settings,
fall back to the checkout or silently skip real dispatch. Windows launcher
behavior is unchanged. The probe establishes a prerequisite; actual Git commit
tests still prove that Git invokes the hook and obeys its verification result.

The retirement guard scans executable tests as well as application and verifier
inputs. Its explicit retained-oracle inventory includes the shared SP-002/SP-003
checkers and their imported/compiled source inputs; a missing inventoried input
fails the check. Historical experiment observations and frozen JSON goldens are
preserved. Deliberate negative-test bindings live in the single non-executable
`tests/fixtures/retired-bindings.json` file. That data exception does not exclude
test modules, helpers or other fixture files. The naming checker uses the same
rejection data. Neither check is a semantic proof against dynamically constructed
or externally supplied dependencies.

Active inputs must decode as UTF-8. A malformed input produces a path-specific
violation while scanning continues to collect other violations. The guard leaves
the input unchanged and returns failure; it does not replace undecodable bytes
or silently omit the file.

Test counts and verification elapsed time describe functional evidence, not
production throughput, latency or memory benchmarks. These checks do not
establish a performance-regression baseline; this retirement follow-up introduces
no new benchmark campaign.

`scripts/setup_env.py` installs the repository's commit hook. To configure only
the hook, run `python scripts/setup_env.py --hooks-only`.

The hook exports staged files into a temporary directory and tests that snapshot.
Unstaged edits cannot fix failing staged code. Changes limited to pure code use
the pure matrix. Other non-prose changes use the development backstop, including
adapter, dependency, tool and CI changes. Documentation-only changes do not run
tests locally. Run `python .githooks/pre_commit.py --qualification` for the full
staged-tree gate at goal completion; this explicit request also tests an empty
or prose-only diff. Documentation updates are expected when behavior changes,
but are not required for every code edit.

The snapshot verifier rejects symlinks and submodules and checks that the index
has not changed during execution. It cannot judge test completeness or relevance;
these remain part of review.

CI selects development or full scope as described above, then exercises the same
snapshot gate against the event base. Full scope propagates `--qualification`;
an initial main push uses a disposable clone with its staged tree preserved and
full qualification retained. Invalid event metadata never silently selects a
lighter gate. Draft development keeps formatting/lint, standalone transport and
Windows client checks separately visible; service preparation/native journeys
are deferred. Whole-goal readiness still requires full applicable qualification.
Required CI settings on the hosting service provide the shared automated merge
gate; local hooks alone cannot enforce it. Independent review is optional during
the sole-contributor phase described in the contribution guide. Consult the
[procedure index](procedures/README.md) for planned and implemented SOP checks.

## Native PostgreSQL creation experiment

[SP-002](spikes/SP-002-postgresql-project-create.md) records bounded native
PostgreSQL structure/protocol evidence. Reproduce with the documented
[Podman-only runner](../experiments/SP-002/README.md) and its isolated Python
environment. It uses fresh containers/volumes and the frozen proposal baseline.
Expected exit 2 preserves a known privileged integrity gap and missing product
checks; it must not be counted as a complete application QA pass. Local Windows
execution is observed; no Linux CI result or integrated native route is claimed.

## Native PostgreSQL foundation

The executable fresh baseline has a separate opt-in
[Podman verification guide](../storage/postgresql/README.md#disposable-native-verification).
`scripts/check_postgresql.py` requires an explicitly named, running, networkless,
labelled container with the documented image and resource limits. It checks
atomic installation failure/rollback, reinstallation rejection, actual scalar
and structural constraints, permanent operation/payload shape, creation history,
serving-role privileges and a payload-identity mutation regression. Additional
checks reject accepted-payload updates and DELETE/INSERT replacement: separate guard
mutations must be detected, and rollback must restore the guards. Serving
connections can read the installed schema version but cannot modify migration
records. These checks do not implement startup compatibility or compaction. It never
resets an existing schema or manages container lifecycle. Clean up the disposable
container after success or failure.

The shared verification matrix runs the verifier's safety-gate unit tests, but
does not execute its native SQL suite. Run that command separately after changes
to storage/postgresql/, its SQL tests or verifier. Native evidence currently
covers a Windows host and PostgreSQL 16.15 in a Linux container; native Linux-host
execution and hosted CI remain unverified. Structural fixtures do not establish
trusted M1 seed equality, application codecs, authorization, compaction or a
supported native application route.

The native command also builds the host-side `project_snapshot_check` helper
from the production codec crate, in an owned temporary target directory, then
runs actual Rust-to-JSONB-to-Rust checks. The pure crate retains no I/O driver or
process dependencies. The bridge bounds process output/time, terminates owned
process trees on timeout/overflow, validates hexadecimal SQL inputs, and verifies
exact row framing and typed equality. A shared orchestration regression detects
omission of this native phase without pretending to execute the database suite.

Snapshot cases cover standalone JSONB u64 extremes, ordered/empty collections,
valid attributed revision rows, negative relational/coherence cases, NUL and
exact 1 MiB text output/one-byte overflow. Raw duplicate-key rejection remains
original-byte codec evidence, independent of JSONB conversion. The
[W1 implementation report](implementation/m1-01-project-snapshots.md) records
Windows/Podman PostgreSQL 16.15 results, including measured formatting expansion.
The [verification plan](implementation/m1-01-snapshot-verification.md) defines
acceptance. These native checks remain separate required PR evidence; hosted
shared verification does not execute them or qualify native Linux-host behavior.

## Verification time budgets

Shared verification commands have a 300-second deadline. Cargo verification on
Windows has a 450-second deadline: observed workspace runs exceeded 300 seconds
while individual tests passed. Linux retains the 300-second Cargo deadline. The
native staged hook retains its 600-second overall deadline. The container lane
uses the validated configuration described above, and clamps each command by the
remaining gate allowance. A timeout fails verification.
These are tooling execution limits, not product performance requirements.

## SP-003 replay codec checks

Shared full/pure verification runs experiments/SP-003/check.py: eight Rust tests
and five grouped Python checks against the actual domain types and frozen
version-one fixtures. This builds `project_create_codec`, which directly owns
the locked `task_contract`, `serde_json` and `sha2` inputs. Cargo metadata binds
the producer manifest and direct package identities; only exact library artifacts
are accepted. Metadata and the producer build explicitly select the compiler's
native host target for offline cache compatibility. No obsolete adapter is built
and no frozen codec fixture is rewritten.

Native PostgreSQL BYTEA/JSONB persistence and ID-domain checks are a separate
Podman-only experiment, not part of those pure CI checks. See the
[SP-003 reproduction guide](../experiments/SP-003/README.md) for resources,
explicit connection selection and cleanup, and the
[spike report](spikes/SP-003-replay-codecs.md) for observations and limitations.

## Production project-creation codec checks

The pure/full Rust workspace checks run project_create_codec's request, raw
JSON, configuration-snapshot and saved-result tests. Run them independently:

```text
cargo test --locked -p project_create_codec
```

Coverage includes frozen version-one byte/digest/JSON fixtures, every request
truncation, malformed UTF8/framing/versions/kinds, decimal and enum corruption,
ordered rich typed round trips, duplicate decoded keys, exact resource admission
boundaries, typed encoding preflight and escaped-output overflow. Historical
reconstruction is separate from new native request admission. Shared verification
also checks that a codec test failure propagates through the pure command.

Object-key regressions cover 64 KiB decoded UTF8 keys and one-byte overflow,
including escaped spellings and multibyte characters. Oversized keys are rejected
before their values are parsed; maximum-length escaped duplicates remain rejected.

The [crate guide](../src/pure/project_create_codec/README.md) documents the
accepted architectural resource ceilings and remaining adapter obligations. This pure
suite makes no database, authentication, native Linux-host PostgreSQL, throughput
or supported-route claim. SP-003's frozen sources/evidence remain separate.
