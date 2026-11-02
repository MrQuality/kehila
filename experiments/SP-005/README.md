# SP-005: physical creation-intent mapping subset

This executable candidate follows Accepted ADR-103/107 and the owning
[SP-005 plan](../../docs/spikes/SP-005-project-target-allocation.md) under #41.
It is an isolated structural subset, not a product migration or complete allocation
adapter. The native structural subset and the shared containerized gate passed;
broader allocation qualification remains pending.

## Schema and trust boundary

`overlay.sql` adds an experiment-owned `sp005.creation_intents` table after the
fresh native foundation. All six columns are non-null. The primary key binds
actor/family/handle; target uniqueness and the existing five-column operation
attribution key bind it to one immutable core. A second foreign key references
the project's unique recorded creation-operation ID. Together they require this
project's actual creation core/actor, not just another core for an existing target.
Family/kind and 32-character hexadecimal handle
checks restrict this candidate to project creation. No mapping token is stored:
trusted backend queries derive it from the permanent core.

The creator capability receives schema usage and mapping SELECT/INSERT only.
Statement-level guards reject UPDATE, DELETE and TRUNCATE, including owner DML.
Schema owners can disable guards; this does not contain administrators or replace
actor/session authentication. Database TEMP privileges used by fixture assertion
functions are distinct from protected-schema DDL privileges.

`serving.sql` uses a genuinely separate `foundation_serving` connection to verify
attribution, actual creation identity, shape, uniqueness, rollback, token derivation
and denied privileges. `fixtures.sql` prepares well-formed extra cores for the same
target so wrong-operation and wrong-creator rejection are not masked by other checks.
Its pre-existing `created` project is the baseline's deliberately small structural
fixture, not a newly accepted typed M1V1 result. Attaching that existing fixture
does not prove mapping and application creation are accepted atomically.
`owner.sql` checks immutable DML guards, non-null/key representation and default
function-execution denial. Untrusted access is checked through a separate login.
Token equality is a fixture join predicate, not a complete authenticated resolver.

## Harness and bounded execution

The runner validates its container/connection names and timeout before resource
preflight or database work. It reuses the native verifier's pinned-image,
networkless, mount-free, resource-capped disposable-container validation and fresh
database/role checks. It neither creates/removes services nor resets existing data.
The caller owns the container and must clean it up on success, failure and timeout.

The native verifier now exposes `prepare_structure()` for installation/structural
fixtures without compiling the snapshot bridge. Its existing public `run()` still
performs the complete original checks, including that bridge. This experiment
does not omit a phase while claiming the full baseline result.

The default total subprocess-work budget is 180 seconds, configurable in (0,600].
Each process wait is capped by remaining work time and at most 60 seconds; SQL
statements/idle transactions use finite limits. Timeouts end this run rather than
producing success. Host-process timeout alone is not evidence of immediate server
query cancellation: the server limits and caller-owned container teardown remain
part of cleanup, to be observed during native qualification.

After the reviewed resource approval, use the pinned disposable profile in the
[native guide](../../storage/postgresql/README.md#disposable-native-verification),
then invoke the explicit already-running disposable container:

```text
python experiments/SP-005/run.py --disposable-container <disposable-name> --timeout 180
```

An explicit `--podman-connection` is supported. No trust change is required for
container-local psql. Database error assertions require actual SQLSTATE/constraint
metadata. The harness requires each role oracle's success marker exactly once;
subprocess failure or missing/duplicated markers cannot establish success. The
final JSON describes only the structural subset and records source fingerprints
and unqualified work. Keep raw execution evidence locally outside the checkout.

## Evidence limits and next cases

Offline runner/control tests do not execute SQL. They cover fail-closed target
validation, work-budget propagation, prerequisite/oracle failures and false-success
prevention. The native structural run passed on 2026-11-02. Reconciliation found
11 source fingerprints identical to the staged bytes and only Git line-ending
normalization in `serving.sql`; its SQL contracts are unchanged. The original
runtime fingerprints remain recorded without rewriting them. The documented pinned PostgreSQL image ran in a
network-isolated disposable Linux container with verified limits of 256 MiB,
one CPU and 128 PIDs. Owned-container removal completed. This evidence qualifies
only the structural role/schema/guard subset. A later full-gate attempt passed
its checks but failed final snapshot cleanup; no commit was created. The required
shared commit gate remains mandatory, including successful cleanup. A later
containerized gate passed, including all shared Rust/Go/oracle/live-service
phases and owned cleanup; see the [container verification record](../../docs/implementation/container-verification.md).
This does not extend the structural SQL evidence to the unimplemented allocation
contracts below.

This subset supplies no installation/actor fence, current end-user authorization,
production codec adapter, atomic application creation, concurrent retry/collision
re-entry, uncertain commit, historical replay/expiry/retirement, supported restore,
browser route or aggregate capacity evidence. Those remain in #41/#40/#49. Do not
close SP-005 or finalize dependent application interfaces from this subset alone.
