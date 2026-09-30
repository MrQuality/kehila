# Testing

The [test strategy](TEST_STRATEGY.md) describes the proposed long-term quality
approach. This guide records executable commands, current coverage, and limits.

For bounded technical investigations, see the approved [spike procedure and
register](spikes/README.md). Spike records distinguish planned cases from observed
results and link experiments to decisions and reusable regression coverage.

## Commands

Run the full suite after starting the development services:

```text
python scripts/setup_env.py --start
python scripts/verify.py
```

Run unit tests without external services:

```text
python scripts/verify.py --pure
```

The full suite runs Python tests for the development tools, service readiness
checks, the NATS account API check, Rust workspace tests, and Go tests. Missing
tools, unavailable services, and test failures produce a nonzero exit status.

Both modes also run `python scripts/check_branding.py` to reject retired project
names and identifiers in current source and documentation. The check permits
factual third-party references and attribution; it is not a legal clearance
check. Its regression fixture is excluded from the naming scan.

Individual commands:

```text
python -m unittest discover -s tests -v
python tests/integration/nats_probe.py
cargo test --locked --workspace
python scripts/go_test.py
```

## Coverage

Unit tests cover the equality grammar, schema-version decisions, and the first
pure B-003 status/workflow command rules. The latter test configuration
references, phase derivation, migration restrictions, archival targets, stale
versions, and replay order; they do not establish persistence or concurrency.
Pure project-rule tests cover archive access effects, project-local readable ID
allocation across prefix changes, exact estimate parsing and unit locking, and
the populated-field kind-change rule. Their cross-record storage effects are
still unverified.
Pure field-rule tests cover hidden value preservation, optional/required
validation, typed values and WorkItem type ownership, choice-option archival,
and calendar boundaries. Required-field changes under concurrent writes still
need authoritative storage checks.
Pure relationship and current-work tests cover cross-project access decisions,
canonical duplicate/self-link rejection, symmetric identity, archival
eligibility, selection versions, and selection independent of lifecycle.
Storage uniqueness and authorization enforcement remain unverified.
Pure configuration-change tests cover active defaults, replacement before
archival, dependency-preserving edits, phase mapping preservation, and revision
and authorization gates. Concurrent item/configuration writes remain B-005
integration checks.
Pure conversion tests cover complete destination validation, migration
authorization, both workflows' phase permissions, Done-to-New rejection,
reopening, source-value history output, and successful replay precedence.
Durable history, knowledge, and provenance retention remain B-005 checks.
Pure boundary tests also cover the accepted M1 estimate, prefix, UTF-8 text,
numeric-field, and Gregorian-date limits and project-scoped readable IDs.
Pure item-mutation tests cover untitled creation and title display, initial
status, required/hidden fields, estimate locking, item/configuration versions,
and replay. Durable sequence allocation and coordinated project/item writes
remain B-005 checks.
Pure archival tests cover item/project version conflicts, restore eligibility,
replay, and selection-clear effects without lifecycle changes. Coordinated
cross-user selection clearing remains a B-005 integration check.

Integration tests use the development NATS server to check its greeting, connection handshake,
and request/response behavior. The Python probe also checks the JetStream account
API using a temporary subscription. It creates no streams or application data.
Connections use three-second deadlines and bounded frame sizes and counts.

The full suite also runs `python tests/integration/task_path.py`. This builds the
Rust worker and Go API, installs indexes in a unique `yaja_test_` collection,
starts two workers sharing that collection and an API on temporary loopback
ports, and removes its processes and collection on success or failure. It needs
FerretDB at `mongodb://127.0.0.1:27017`; override `YAJA_TEST_MONGO_URL` with a
single-host MongoDB URI for a different disposable development instance. It uses
the `yaja` database. Never point regression tests at a supported user installation.
No host Python packages or production CDC/indexer are required.

The runner tests 48 competing create/update/identical-retry/changed-content pairs
across two processes, replay after later updates, direct-worker request guards,
a stalled request body while other reads proceed, a stalled database connection
with bounded failure and recovery, and Go-to-Rust saves with search unavailable.
Concurrent clients reach independent worker processes; the test does not rely on
a single serial HTTP handler to establish storage exclusion. Finite race tests
are regression evidence, not exhaustive linearizability proof.

The pure command includes both `yaja_query` and `task_contract`; maintain this
explicit list when adding a pure crate. Its selection test verifies that a
failure in `task_contract` propagates. The full suite tests all Rust workspace
crates, including adapter policy tests.

The older `live_task_worker.py` and `live_go_rust.py` remain manual probes for
already-running processes. `live_task_api.py` specifically targets the original
Python experiment adapters with search running; it is not a Rust CDC test.

### Required retesting after task-path changes

During implementation, run the affected pure and boundary tests. After changes
to the worker, API, storage decisions, dependencies, or verification runner, run
`python scripts/verify.py` once against running disposable services on the final
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
