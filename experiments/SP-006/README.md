# SP-006: projection rebuild handoff proof

Status: bounded standalone prototype; four live cases passed on 2026-10-24.
Two targeted mutations were detected and the restored source passed all four cases
again. Final publication gates remain pending. Owner: project maintainer.
Related: [spike plan](../../docs/spikes/SP-006-projection-rebuild.md),
[#49](https://github.com/MrQuality/kehila/issues/49), B-005/B-023.
No application schema, runtime dependency or production indexer is adopted.

## Protocol

All projecting writers acquire a shared transaction-scoped maintenance gate before
any installation, actor or domain lock and retain it through COMMIT/ROLLBACK.
The fixture's single `put` function demonstrates this; production privileges
would have to make bypass impossible. No one-owner serialization assumption or
sequence-number commit frontier is used.

One coordinator owns a separate session lock for its entire operation. It:

1. Takes the exclusive maintenance gate and waits for prior writers to finish.
2. Commits registration of the new building target. While still fenced, it opens
   and actually establishes a repeatable-read source snapshot. It then releases
   the writer gate. Later writes atomically queue durable per-target deliveries
   alongside each immutable complete outbox event, including the building target.
3. Bulk-copies the pinned snapshot. Strict OpenSearch external versions prevent
   an older snapshot value from replacing an already delivered newer value.
   Snapshot loss fails the epoch; a new snapshot uses a new target.
4. Takes the exclusive gate for cutover. This drains writers, leaving a finite
   pending set. It acknowledges every building-target delivery individually,
   verifies every fixture document, and refreshes the target for search visibility.
   Delivery/ack workers never acquire the writer gate.
5. Commits switching intent, changes the read alias atomically, verifies the
   actual alias, then commits the new active target. Both targets remain enrolled
   until that confirmation. A coordinator interruption after the alias call
   recovers from durable intent and actual alias state, fences/drains again and
   completes the transition. The old index remains retained in this proof.

External HTTP work holds no writer transaction or actor lock. The read-only
source snapshot transaction spans bulk-copy HTTP; its bounded lifetime can still
pin MVCC history, so production snapshot age/cancellation needs qualification.
A session advisory
maintenance gate deliberately pauses projecting writes during short setup/cutover
windows. Its production acquisition ordering, duration/availability limits,
capabilities and cancellation ownership require their own reviewed adapter plan.

A delivery is acknowledged only after confirmed indexing, same-version exact
payload equality, or a verified strictly newer stored revision. Missing/ambiguous
acknowledgment stays pending. Same-version differing content is an integrity error.
Physical deletion is excluded; a production deletion/archive design must retain
versioned projection state to prevent stale resurrection. This is a correctness
prototype using trusted synthetic SQL, not an authentication/privilege proof.

## Cases and execution

`run.py` owns PostgreSQL/OpenSearch Compose services and uses the repository's
owned psql participant helper from SP-004 without changing its experiments.
The four cases exercise actual SQL locks and actual OpenSearch indexing/alias APIs:

- Sequence allocation before COMMIT demonstrates the rejected maximum-ID frontier.
- A registration fence blocked by an existing writer, pinned snapshot/live deliveries,
  out-of-order versions, missing delivery ACK, corruption rejection, a false
  maximum-revision completeness check, and correct refreshed cutover.
- Coordinator interruption after alias application but before metadata commit;
  subsequent writes retain both targets and recovery completes under a new owner.
- Actual source-backend termination rejects an incomplete snapshot; the old alias
  survives and a fresh target can rebuild successfully.

Use a dedicated initially stopped disposable machine. Set TEMP/TMP outside the
checkout; no secrets or real records are admitted. No Python dependency is added:
Python 3.10+ standard library and psql in the owned PostgreSQL container suffice.
The runner checks the existing 3-GiB RAM/2-GiB free-disk minimum before startup,
after VM startup, after service startup and between cases. The minimum is not a
capacity guarantee. No unrelated machine, service or container may be stopped.

```text
python experiments/SP-006/run.py --podman-machine NAME --exclusive-podman-machine
```

Work budget: 600 seconds plus bounded owned cleanup. SQL participants have a
60-second lifetime/64-KiB output ceiling; fixture commands have a 32-KiB ceiling
and SQL timeouts. HTTP uses a five-second timeout, a 64-KiB response ceiling,
no proxy and no redirect. Fixtures contain fewer than 32 documents/deliveries;
snapshot and drain loops fail when their bound is exceeded. These are test
bounds, not a supported operating envelope. Raw logs stay in external storage.

Passing this proof would establish only the exercised protocol schedules on the
recorded versions. It would not qualify full crash durability, native application
roles, infinite load, multi-coordinator deployment, real-data readiness or the
complete search pipeline. Production adoption remains W3/W5/W7 work under ADR-103.

## Resource-profile disposition

The first service-start attempt with per-container cgroup memory/CPU/PID limits
failed before protocol cases because the selected rootless provider could not
open the memory controller. Owned cleanup completed. No protocol pass/failure or
memory-limit qualification follows from that startup error.

The maintainer explicitly allowed a revised small-workload profile on 2026-10-24:
no per-container cgroup caps, the same 600-second work budget, tiny fixed fixtures,
OpenSearch's fixed 512-MiB JVM heap, loopback-only publication, available-RAM checks
between phases and owned cleanup. This changes the test profile, not host settings;
the runner never silently strips failed flags or changes privileges/delegation.
It does not qualify container memory-limit enforcement or a production resource
budget. If RAM preflight fails, stop rather than overlap heavy jobs or weaken it.

## Recorded result, 2026-10-24

All four live cases passed on Windows against the configured PostgreSQL 16.13 and
OpenSearch 2.19.1 images. The [sanitized result](results.observed.json) records
observations, failed preparation attempts and LF-normalized source hashes. These
hashes bind the observed code to later publication without exposing raw logs or
local metadata. They do not turn source review into production qualification.
The manifest separates executed hashes from publication hashes: after execution,
a Rebuild docstring was corrected to disclose the pinned read-only transaction.
AST comparison after removing docstrings found no executable change. Publication
hashes must match the final committed implementation.

All owned SQL participants, containers/networks and the exclusively owned VM
were stopped/removed by their lifecycle owners. The test injects abandonment at
the alias/metadata boundary; it does not claim an arbitrary host crash or network
fault campaign. The actual backend-termination case is distinct from that injected
control-flow boundary. No certificate, trust store or real user data is involved.

The writer-gate omission failed the expected-wait assertion. Bypassing the
same-version payload comparison failed the corruption-rejection assertion. Both
mutations were restored before the successful final four-case run. These checks
establish sensitivity to those two regressions, not a general mutation score.
