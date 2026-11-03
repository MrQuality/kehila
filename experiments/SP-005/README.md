# SP-005: physical creation-intent mapping subset

This executable candidate follows Accepted ADR-103/107 and the owning
[SP-005 plan](../../docs/spikes/SP-005-project-target-allocation.md) under #41.
It is an isolated structural subset, not a product migration or complete allocation
adapter. The native structural subset, fixture-actor transaction schedules and
shared containerized gate have passed within their separately recorded scopes.

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

## Fixture-actor transaction schedules

The opt-in `--concurrency` extension adds three database-layer schedules after
the structural checks. All three passed on 2026-11-03:

- Keep actor A's creation transaction open while actor B commits a distinct target.
  Both use the same token and handle text in their separate actor scopes.
- Observe B blocked by A's uncommitted target. Commit A, require B's named
  `23505/projects_pkey` rejection, confirm complete rollback and actor-fence
  release, then repeat B's fence/lookup with a fresh target.
- Observe the same target wait but roll A back. B must commit its own correctly
  attributed creation; A must leave no operation core.

Two independent `foundation_serving` logins execute the transactions. A separate
observer checks actual PostgreSQL blocking and final actor/core/project/mapping/
token tuples. Backend PID, start time and unique application identity distinguish
participants. Fixed fixture SQL creates the same deliberately small relational
seed as the existing structural fixture; its placeholder snapshot/payload bytes
do not qualify production codecs or complete typed M1V1 creation.

Participant commands share the runner's finite work deadline. Process cleanup
uses the configured runner-stop allowance, releases the lock holder first, joins
owned clients and preserves a primary failure if cleanup also fails. The caller
still owns final container removal, including after an uncertain client timeout.
The default structural-only invocation and its claims remain unchanged.

This extension follows the maintainer-selected
[qualification split](../../docs/engineering/QUALIFICATION.md#native-project-creation-separate-database-and-http-evidence):
fixture actors qualify database arbitration, while ADR-105 retains one HTTP
owner. Chrome/Edge same-owner recovery and later multi-actor HTTP under #65 are
separate obligations. No second HTTP user or product pending-owner exception is
introduced.

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
final JSON distinguishes structural-only execution from the opt-in concurrency
extension and records source fingerprints and unqualified work. Keep raw
execution evidence locally outside the checkout.

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

The concurrency extension passed 18 protocol/runner checks and actual PostgreSQL
schedules on 2026-11-03. Independent observations found B's committed distinct
target while A remained uncommitted, the actual target blocker, complete rollback
and released actor fence after B's typed uniqueness rejection, fresh re-entry,
and the expected winner after A rolled back. Final checks required five new
projects with matching cores, payloads and mappings, exact actor/target/token/
handle attribution, expected child rows and no unmatched mapping.

The full phase, including provider startup and owned cleanup, took 124.407 seconds.
Sampled aggregate memory peaked at 1.647 GiB; available RAM stayed at or above
4.804 GiB. No resource warning/stop occurred. The owned database was removed and
the initially stopped provider was stopped. These are observations, not capacity
targets. Database limits remained 256 MiB, one CPU and 128 PIDs.

Three earlier attempts failed and remain recorded. The first observer mixed psql
format feedback with its Boolean result. The second diagnostic expected feedback
that psql did not emit. The third reached the expected server uniqueness rejection
but failed client classification. Its exact consumed packet was not retained.
A subsequent six-trial diagnostic reproduced an error arriving after the client
acknowledgement marker on the original remote exec channels. Merging stderr into
stdout inside the container before remote transport preserved ordered diagnostics
in all three corresponding controls. The harness retains strict SQLSTATE and
schema/table/constraint checks; a regression rejects a late error packet. Those
diagnostic controls are not allocation qualification. Cleanup completed after
each attempt; the later pass does not rewrite the earlier failures.

Fixture actor-row fences are exercised by this extension. It supplies no product
installation fence, current end-user authorization, production codec adapter,
complete typed application creation, uncertain commit, historical replay/expiry/
retirement, supported restore, browser route or aggregate capacity qualification.
Product observations are recorded separately in the
[authority verification record](../../docs/implementation/project-authority-verification.md).
Do not close SP-005 or finalize dependent application interfaces from this subset
alone; #41/#40/#49 retain their applicable obligations.
