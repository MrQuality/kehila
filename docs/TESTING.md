# Testing

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

Unit tests cover the equality grammar and schema-version decisions. Integration
tests use the development NATS server to check its greeting, connection handshake,
and request/response behavior. The Python probe also checks the JetStream account
API using a temporary subscription. It creates no streams or application data.
Connections use three-second deadlines and bounded frame sizes and counts.

The in-progress Go/Rust task path has unit coverage in the default suite. Its
additional live checks are in `tests/integration/live_task_worker.py` and
`tests/integration/live_go_rust.py`. They require FerretDB from the disposable
SP-001 stack, a Rust worker on loopback port 8082 using an installed development
operation collection, and, for the second check, the Go API on port 8081. The
second check deliberately expects search to be stopped. They exercise actual
save, replay, conflict, authoritative read, and search-unavailable behavior, but
they are not yet an automated production pipeline test. The separate
`live_task_api.py` checks the Go boundary through the original Python experiment
adapters while search is running.

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
