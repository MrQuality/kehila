# SP-001 bounded task-path experiment

Experimental boundary probe, not a supported application. See
[the spike record](../../docs/spikes/SP-001-task-path.md) for observations.

Execution owner: Codex, under the maintainer's local execution request.
Assessment: maintainer; automated results do not constitute maintainer approval.
This attempt has a 90-minute investigation limit, starting with runtime preflight;
stop earlier for an unresolved prerequisite, host memory pressure, or sufficient
evidence. Retain an inconclusive result when appropriate.

## Fixed scope and budget

Use an isolated Compose project `yaja-sp001`, synthetic tasks only, and loopback
host ports. Run host tests sequentially. Use Python boundary adapters for the
prototype; the intended Go API and Rust worker are not implemented by this
experiment. Do not infer their performance, memory consumption, or correctness.

| Service | Container memory ceiling (MiB) |
| --- | ---: |
| PostgreSQL | 256 |
| FerretDB | 128 |
| NATS JetStream | 128 |
| OpenSearch (512 MiB Java heap) | 1536 |
| Debezium Server (256 MiB Java heap) | 768 |
| API | 128 |
| Worker | 128 |
| Indexer | 128 |
| Total | 3200 |

Reserve a further 768 MiB for Linux/runtime overhead and at least 1 GiB of host
available memory. These are experimental limits, not measured requirements.
R04 passed under these limits; the result and measured usage are recorded in the
spike, with observed base-image identities in [images.observed.json](images.observed.json).
Build/pull before full-stack execution. Verify the engine enforces each limit.
Stop workloads if host available RAM falls below 1 GiB or any container is OOM
killed. Record sampling intervals; sampled peaks are lower bounds on true peaks.

## Proposed contracts to investigate

The database is authoritative. A successful write returns HTTP 200 with the
persisted task version and `sync_token: null`; search visibility is pending until
that version is observed. A stalled indexer does not turn an acknowledged write
into a failed write. This result led to the accepted first-increment contract in
[D-016](../../docs/product/decisions.md#d-016); the evidence remains bounded to
the experimental adapters.

Each mutation supplies an operation ID and expected version. Retry of identical
content returns the original operation result; reuse of an ID with changed
content returns 409. Competing writes against the same version yield one success
and one 409. A bounded operation ledger lives in the same document so the
mutation and its deduplication record are atomic. This is not a general solution
for retention or arbitrary multi-document transactions.

Independent Debezium PostgreSQL WAL capture publishes to file-backed JetStream.
The indexer acknowledges only after OpenSearch accepts an externally versioned
projection. CDC offsets and service data survive container restart. Duplicate
or older events cannot overwrite a newer projection.

Freeze at most 12 tasks, at most 16 operations per task, titles <=128 characters,
and 30-second delivery deadlines. Readiness has a 120-second deadline. Interruption
cases use stopped containers and acknowledged writes as barriers, not arbitrary
sleep durations. All service cases require a passing runtime preflight.

## Cases

1. C01: create/update/read; inspect saved version separately from search visibility.
2. C02: stop CDC, save, stop worker, resume CDC; search must receive the saved change.
3. C03: stop CDC after checkpoint, save another change, resume with its data retained.
4. C04: identical retry, changed-content retry, duplicate event projection.
5. C05: stop indexer, save, observe pending search, resume indexer and observe version.
6. C06: concurrent mutations with the same expected version; one success and one conflict.

## Runner

From the repository root, with an initialized and running Podman machine:

```powershell
# Windows only: select the isolated machine; the helper defaults to this name.
$env:SP001_CONNECTION = 'yaja-sp001'
python experiments/SP-001/manage.py config
python experiments/SP-001/manage.py pull
python experiments/SP-001/manage.py build
python experiments/SP-001/manage.py setup
python experiments/SP-001/manage.py test
python scripts/verify.py
python experiments/SP-001/manage.py status
python experiments/SP-001/manage.py stop
```

On Linux, omit the environment assignment and use the same Python commands with
a local Podman engine. Linux execution has not yet been verified. On Windows,
use `podman machine init --memory 5120 --cpus 4 --disk-size 25 yaja-sp001` and
`podman machine start yaja-sp001` if a dedicated machine does not already exist.
WSL may not apply the machine memory/CPU/disk options as limits; the helper checks
actual resources and enforces container memory ceilings separately.

`setup` requires free host ports and at least 4224 MiB host available RAM after
VM startup (3200 MiB container ceilings plus 1024 MiB host headroom). It checks
container storage, `vm.max_map_count`, image availability, real service readiness,
FerretDB's physical row mapping, and delivery of a fresh synthetic seed to search.
It configures `REPLICA IDENTITY FULL` on the discovered task table and a publication
containing only that table. The prototype needs this explicit provisioning step
before updates can be replicated; future collection lifecycle handling is open.
The provider ignores `memswap_limit`, so the runner passes the swap ceiling as a
Podman creation argument for each service and checks the result before starting it.
`test` fails without all eight containers, checks memory/swap ceilings, monitors
host available RAM once per second, and executes C01-C06. Each host test command
has a timeout. A setup/test failure stops the project's containers while retaining
named volumes. Successful cases leave services running for the repository's full
verification suite; explicitly stop them with the final command.

Raw local preflight, mapping, case, and memory evidence is stored under ignored
`.yaja/spikes/SP-001/`. Shared conclusions and relevant image digests belong in
the spike record. Re-running cases creates a new synthetic task prefix. Named
volumes retain prior fixtures; do not interpret this as testing an empty database.
This script does not delete volumes. Resource cleanup is scoped to this Compose
project. Stop the dedicated VM when finished to release its memory.

The runner recreates only selected project containers during setup and retains
named volumes. Use `podman machine stop yaja-sp001` / `podman machine start yaja-sp001`
on Windows to reclaim an idle experiment VM's cache before a fresh setup if the
headroom gate fails; stop its containers first. Do not lower the gate to bypass a
resource failure. `podman machine` settings do not replace measured host headroom.

## Design handoff

The browser would submit operation IDs and expected versions through the API,
display database acknowledgment, and retain a pending indicator until the desired
version appears in the search/update channel. The browser has no direct database,
broker, or search access. The experimental API proxies mutation requests to the
worker; only that worker writes task documents through FerretDB. Database reads
and search reads are distinct endpoints so the experiment does not confuse save
success with projection visibility. UI, authentication, query compilation, SSE,
and the future Go/Rust adapters are outside this prototype.

Debezium owns database-log capture and its durable checkpoint, independent of the
worker. File-backed JetStream owns durable event retention. The indexer consumes
with explicit acknowledgments and an external document version; only successful
indexing or a verified version conflict permits acknowledging the event. A crash
between indexing and acknowledgment causes replay, which the version rule makes
harmless for the fixture. Deletions, collection migration, schema evolution,
arbitrary BSON values, and poison-event recovery are not covered.

Future resource releases and cost adjustments need durable operation identity,
an authoritative reservation/cost ledger, explicit transaction or saga boundaries,
and compensating operations for partial failure. Search must not authorize writes
or decide resource availability. A single-task version check does not establish
cross-document consistency. These remain prerequisites for B-011/B-012 rather
than claims made by this experiment.

Configuration references:
[Debezium Server 3.2](https://debezium.io/documentation/reference/3.2/operations/debezium-server.html),
[Podman machine initialization](https://docs.podman.io/en/v5.8.3/markdown/podman-machine-init.1.html).
Only the versions and cases recorded in the spike establish local observations.
