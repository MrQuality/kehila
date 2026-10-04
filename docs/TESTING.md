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

Run the full suite after starting the development services:

```text
python scripts/setup_env.py --start
python scripts/verify.py
```

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

The runner tests 48 competing create/update/identical-retry/changed-content pairs
across two processes, replay after later updates, direct-worker request guards,
a stalled request body while other reads proceed, a stalled database connection
with bounded failure and recovery, and Go-to-Rust saves with search unavailable.
Concurrent clients reach independent worker processes; the test does not rely on
a single serial HTTP handler to establish storage exclusion. Finite race tests
are regression evidence, not exhaustive linearizability proof.

The pure command includes both `kehila_query` and `task_contract`; maintain this
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
