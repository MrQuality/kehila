# Testing

The [test strategy](TEST_STRATEGY.md) describes the proposed long-term quality
approach. This guide records executable commands, current coverage, and limits.

For bounded technical investigations, see the approved [spike procedure and
register](spikes/README.md). Spike records distinguish planned cases from observed
results and link experiments to decisions and reusable regression coverage.

## Commands

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

Run the full suite with a ready container engine:

```text
python scripts/setup_env.py --hooks-only
python scripts/verify.py
```

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

### Owned test services

Full verification first runs tooling/static checks, precompiles the complete
Rust workspace test executables, executes the offline workspace tests with
`--workspace --exclude event_dispatcher`, runs frozen oracles and Go tests, and
prepares both live task-path binaries. Child commands use `CARGO_BUILD_JOBS=1` and append
`-p=1` to existing `GOFLAGS`; compiler parallelism is bounded without reducing
test counts or changing application concurrency.

Only then does it start unique, disposable Compose projects, one phase at a time:

| Phase | Running services | Required checks |
| --- | --- | --- |
| Broker execution | NATS | TCP readiness, account API probe and all `event_dispatcher` tests, including the live broker round trip |
| Search readiness | OpenSearch | TCP readiness and yellow/green HTTP cluster health |
| Task path | PostgreSQL, FerretDB | Both TCP probes and complete two-worker contention, request/dependency deadline, recovery and Go-to-Rust/search-unavailable checks |

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
Task-path resources use an explicit cleanup owner: the body exception remains
primary and each callback failure is attached with its resource label, even if
the callback suppresses its Python context or supplies its own cause. All
registered callbacks run. On a successful body, the first cleanup failure is
primary. The reporter follows unsuppressed Python contexts with cycle/duplicate
protection; it does not undo deliberate context suppression.
The verifier owns a 600-second work budget (`--timeout`, positive and at most
600). Every work command receives the smaller of its own timeout and the
remaining budget. Resource-owning Python checks (Go tests, SP-002/SP-003 check
drivers and task-path preparation/execution) receive a same-host monotonic
deadline through `KEHILA_VERIFICATION_DEADLINE`. Their parent waits while they
time out leaf work and unwind their own directories, processes and collection
cleanup. Task-path readiness/network operations also check the deadline, including
requests running in helper threads. The parent does not kill these Python owners
at the work deadline. Standalone commands keep their existing local timeouts.
Diagnostics and teardown use their separate bounded command
limits after work expires. The staged hook passes this budget and waits for the
owner to finish cleanup; it does not kill the owner at 600 seconds. Cleanup can
extend total elapsed time beyond the work budget. Normal interruption unwinds
these contexts. Forced process termination or host
failure can prevent cleanup; inspect resources carrying the printed unique
`kehila-test-...` project label before removing any leftover test project.

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
`KEHILA_TEST_MONGO_URL=mongodb://127.0.0.1:27017`,
`KEHILA_NATS_ADDRESS=127.0.0.1:4222`, `KEHILA_NATS_HOST=127.0.0.1` and
`KEHILA_NATS_PORT=4222` (unset variables use these defaults). Overrides are rejected
before builds so readiness, the Python account probe, Rust broker tests and live
task-path checks agree about their targets. `--external-services` changes lifecycle
ownership; it does not add custom endpoint support. Use standalone `task_path.py`
or `nats_probe.py` for custom endpoints. Native PostgreSQL investigations retain
their separate documented container/resource preflight and qualification scope.

Generated SP-002 check executables now use the shared owned temporary-directory
cleanup policy on success and failure. The frozen oracle source/fixtures and
historical evidence remain unchanged. Current SP-002/SP-003 Python support
scripts participate in deadline handling; their behavior is separate
from those recorded native runs.

### Lifecycle verification record

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
always-triggered `verify` job and selects this path only when every changed file
is a known ordinary prose document. The shared hook policy excludes engineering
register/preamble/generated-standard inputs from that shortcut. All other CI
changes retain full verification; an empty diff, unavailable event base, unknown
input or code deletion/rename into documentation selects full verification.
The hook retains its pure/full source distinction; CI is deliberately more
conservative for every non-prose change. Workflow changes themselves select full
verification. No workflow-level path ignore or commit-message bypass is added.
Lightweight results establish only their documented scope; release qualification
still requires full verification of its subject source and maintainer assessment.

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
files, local evidence, build/dependency directories and the exact naming regression
fixture are excluded; binary and non-UTF-8 content is skipped.
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

The full suite also runs `python tests/integration/task_path.py`. This builds the
Rust worker and Go API, installs indexes in a unique `kehila_test_` collection,
starts two workers sharing that collection and an API on temporary loopback
ports, and removes its processes and collection on success or failure. It needs
FerretDB at `mongodb://127.0.0.1:27017`; override `KEHILA_TEST_MONGO_URL` with a
single-host MongoDB URI for a different disposable development instance. It uses
the `yaja` database. Never point regression tests at a supported user installation.
No host Python packages or production CDC/indexer are required.

Each child binds port zero and reports `KEHILA_BOUND_ADDR` after binding. The
runner discovers that owned loopback address from at most 8 KiB of startup output
within 25 seconds, then requires HTTP health readiness. It never releases a
probed port for another process to claim before startup. Missing, malformed,
duplicate or incomplete address reports cannot satisfy discovery.
If a child exits, discovery reports its exit status and up to 8 KiB of output,
marking truncation explicitly. A live child exceeding the output limit still
fails discovery; exit diagnostics do not relax the admission limits.

The Go test runner and live runner own their temporary build directories. After
child processes exit, deletion retries only Windows access/sharing/lock errors
(5/32/33), for at most 30 seconds. Other errors and an exhausted retry deadline
still fail verification. Cleanup never targets a path outside the allocated
temporary directory; an earlier test failure remains visible if cleanup also
fails.

Directory cleanup tests exercise canonical-path refusal before deletion, with
real directory-symlink coverage where the host permits creation. Windows hosts
without symlink creation privilege skip that filesystem case; the portable
redirected-path test still checks refusal and that deletion is never invoked.

The runner tests 48 competing create/update/identical-retry/changed-content pairs
across two processes, replay after later updates, direct-worker request guards,
a stalled request body while other reads proceed, a stalled database connection
with bounded failure and recovery, and Go-to-Rust saves with search unavailable.
Concurrent clients reach independent worker processes; the test does not rely on
a single serial HTTP handler to establish storage exclusion. Finite race tests
are regression evidence, not exhaustive linearizability proof.

The pure command includes `kehila_query`, `task_contract` and
`project_create_codec`; maintain this
explicit list when adding a pure crate. Its selection tests verify that failures
in `task_contract` and `project_create_codec` propagate. The full suite tests all
Rust workspace crates, including adapter policy tests.

The older `live_task_worker.py` and `live_go_rust.py` remain manual probes for
already-running processes. `live_task_api.py` specifically targets the original
Python experiment adapters with search running; it is not a Rust CDC test.

### Required retesting after task-path changes

During implementation, run the affected pure and boundary tests. After changes
to the worker, API, storage decisions, dependencies, or verification runner, run
`python scripts/verify.py` once with its owned disposable services on the final
revision. This includes the live task-path runner; do not substitute `--pure` or
a previous revision's passing CI. Run `cargo fmt --all -- --check` and check
changed Go files with `gofmt`. Required GitHub CI must pass on the final revision.
A failed or unavailable live dependency is a failed check, not a skip.

Repeat SP-001 C01-C06 when its Python adapters, physical storage mapping, CDC
configuration, or projection behavior change, or when new evidence contradicts
its conclusion. Changes confined to the Go/Rust mutation boundary require fresh
Go/Rust regression evidence, not a repeat of the unchanged Python experiment.
The new Rust operation collection still needs separate production CDC and
recovery verification before a supported installation is claimed.

Integration results apply to the behavior exercised. A successful handshake does
not establish durable publication, replay, or recovery. New adapters need tests
against their actual services for the contracts they introduce. Unit tests and
test doubles may complement these checks.

## Local hooks and CI

`scripts/setup_env.py` installs the repository's commit hook. To configure only
the hook, run `python scripts/setup_env.py --hooks-only`.

The hook exports staged files into a temporary directory and tests that snapshot.
Unstaged edits cannot fix failing staged code. Changes to adapters, dependencies,
test tools, or CI run the full suite; changes limited to pure code use unit tests.
Documentation-only changes do not run tests locally. Documentation updates are
expected when behavior changes, but are not required for every code edit.

The snapshot verifier rejects symlinks and submodules and checks that the index
has not changed during execution. It cannot judge test completeness or relevance;
these remain part of review.

CI runs the full suite, then verifies changes relative to the event's base commit.
Initial pushes are checked in a disposable clone with the staged tree preserved.
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
staged hook still has its 600-second overall deadline; a timeout fails verification.
These are tooling execution limits, not product performance requirements.

## SP-003 replay codec checks

Shared full/pure verification runs experiments/SP-003/check.py: eight Rust tests
and five grouped Python checks against the actual domain types and frozen
version-one fixtures. This builds exact artifacts using the existing locked
workspace dependencies, including the task_worker library, without running its
I/O. A pure build therefore requires Rust 1.88+ and may need the existing adapter
dependencies cached. The prototype currently receives sha2 transitively through
mongodb; production hashing must declare its own direct dependency.

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
