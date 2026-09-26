# SP-001 — First task create/update/read path

| Field | Value |
| --- | --- |
| Status / outcome | Concluded / bounded Python-adapter path supported; product adoption pending maintainer assessment |
| Owner / assessor | Codex executed the maintainer-authorized local experiment; maintainer assessment pending |
| Created / last updated | 2026-09-25 / 2026-09-26 |
| Related work | [B-002](../product/backlog.md#b-002), [R-001](../product/requirements.md#r-001), [D-015](../product/decisions.md#d-015) |
| Questions | [Q-001](../product/open-questions.md#q-001), [Q-002](../product/open-questions.md#q-002), [Q-019](../product/open-questions.md#q-019) |
| Effort limit / stopping condition | 90-minute investigation limit; service cases completed within that limit; stop for failed prerequisites, <1 GiB host available memory, OOM, or sufficient bounded evidence |
| Prior evidence | [Current implementation and limitations](../IMPLEMENTATION.md); connection checks do not establish durable delivery |

Use the approved [spike procedure](README.md). B-002 remains the source for the
task's acceptance criteria; this record will hold its cases, evidence, and handoff.

## Question and scope

Can the reference stack support one locally usable task create/update/read path,
with explicit save, delivery, visibility, error, and recovery contracts?

Start with PostgreSQL through FerretDB, NATS, OpenSearch, and the reference CDC
approach. Validate database mapping and event connections with actual services.
An engine comparison, production capacity benchmark, full UI, complete resource
lifecycle, and Kubernetes are outside this spike. Record prerequisites for later
consistent resource releases and cost adjustments without implementing that scope.

The experiment retains the reference storage/event stack. Its provisional HTTP
contract is 200 for an acknowledged database save, with the persisted version and
`sync_token: null`; search visibility is observed separately. A stalled indexer
does not invalidate the saved write. This is an experimental recommendation,
not an accepted product decision or replacement architecture.

## Cases and latest observations

The [experiment definition](../../experiments/SP-001/README.md) freezes fixture
shape, memory ceilings, timing limits, interruption barriers, and the provisional
response contract. R04 uses six synthetic case tasks and a fresh setup seed;
each task has at most 16 operations and titles of at most 128 characters.

| Case | Variable / condition | Expected observation | Current state |
| --- | --- | --- | --- |
| C01 | Create, update, read one synthetic task | Defined fields and versions survive the round trip; save and query visibility are recorded separately | R04 pass |
| C02 | Worker stops immediately after acknowledged save | Independent change capture eventually delivers the saved change | R04 pass |
| C03 | CDC stops and restarts with its durable state retained | Resume from checkpoint; saved changes are not lost | R04 pass |
| C04 | Duplicate delivery or retry of the same operation | No duplicated logical effect; operation identity and acknowledgment rules are explicit | R04 pass |
| C05 | Indexer stalled, then resumed | Defined pending/error response, followed by correct search visibility; no write authorization dependency on search | R04 pass |
| C06 | Competing updates to one task | Defined conflict/version behavior, without silent loss of an accepted update | R04 pass |

For C02-C05, define a deterministic interruption point and observable event
identity/version before testing. Record expected ordering and any time bound
instead of relying on arbitrary sleeps. Add narrowly scoped cases only when
needed to settle the linked questions.

## Environment and reproduction

All application and supporting services run in Podman containers. Test commands
run on the host/CI runner. Verify `podman-compose` as the selected provider.
The experimental Compose stack adds Debezium and Python API/worker/indexer
adapters to the four supporting services. These are boundary probes, not the
planned Go/Rust application. The current Docker CI job does not establish Podman
compatibility; R04 provides local Windows/Podman evidence only.

Preflight must check current resources, software versions, Podman/Compose,
WSL/Linux runtime where applicable, ports, image access, and OpenSearch settings.
The historical workstation observations in B-002 are not a fresh passing check.
Size the expanded stack before starting its workload. Identify any missing host
tools according to where builds and tests will actually run.

The [runner and reproduction commands](../../experiments/SP-001/README.md#runner)
use isolated project `yaja-sp001`, loopback ports, pinned image versions,
explicit timeouts, and named volumes retained across restart cases. The helper
checks capacity and limits, stops containers on setup/test failure, and keeps
raw local evidence under ignored `.yaja/spikes/SP-001/`. Retain the prototype
for reproduction; it is not promoted into supported application code.

## Run summaries

### SP-001-R01 — 2026-09-25, static preflight blocked

Inspection at 10:02 +03:00 on a Windows workstation, at source revision
`9774664055b37228dd916a634534403ce3550e7f`. The checkout was clean before
inspection. Only this record and the spike register were subsequently updated;
no experimental implementation was tested. This is a preflight attempt, not
execution of C01-C06. Owner/assessor assignment and the experiment effort limit
remain unset; no workload execution is authorized by this record.

| Check | Observation | Assessment |
| --- | --- | --- |
| Host memory / CPU | 15.71 GiB total, 4.85 GiB available; 20 logical processors | Snapshot only. The documented roughly 4 GiB development guidance does not establish capacity for CDC plus API/worker/indexer containers. Expanded workload budget remains unverified. |
| Workspace disk | 131.89 GiB free | Host observation only; free space inside container storage not measured. |
| Required existing service ports | No listening TCP endpoints reported for 5432, 27017, 4222, 8222, 9200; candidate API port 8080 also had none | Does not establish future bind success or host-to-container connectivity. |
| Host tools | Podman 4.9.2, Python 3.10.6, Git 2.55.0.windows.5, rustc/cargo 1.93.0, Go 1.27.0 windows/amd64 | Executable version output observed; build compatibility not tested. |
| Compose provider | `podman-compose` not found on PATH; `python -m pip show podman-compose` reported no package in the selected Python installation | Required provider unavailable in the inspected host environment. No claim about all other Python/WSL installations. |
| Runtime | WSL 2.7.14.0, kernel 6.18.33.2-2; Ubuntu 22.04 and the Podman WSL machine both stopped | No machine started. `podman compose --help` attempted a connection and reported the local Podman socket unavailable. |
| Container prerequisites | Container-storage capacity, engine connection, forwarded connectivity, image access/digests, and `vm.max_map_count` not verified | Blocked pending static capacity planning and provider setup. |
| CI | Checked-in job targets Ubuntu 24.04 and starts services with Docker | Configuration inspection only. No CI runner was inspected or run; no Podman CI evidence. |

The first sandboxed resource/port queries returned access-denied errors. Their
derived zero values were invalid and discarded. The table uses the successful
host inspection, not those failed values. No containers or tests were started,
no packages were installed, and no runtime/security settings were changed.

Read-only reproduction commands used for the static checks (PowerShell; these
are inspection commands, not a complete experiment runner):

```powershell
git rev-parse HEAD
git status --short
Get-CimInstance Win32_OperatingSystem | Select-Object TotalVisibleMemorySize, FreePhysicalMemory
Get-CimInstance Win32_Processor | Select-Object NumberOfLogicalProcessors
Get-PSDrive -PSProvider FileSystem | Select-Object Name, Free
Get-NetTCPConnection -State Listen | Where-Object LocalPort -in 5432,27017,4222,8222,9200,8080 | Select-Object LocalAddress, LocalPort
Get-Command podman,podman-compose,python,git,rustc,cargo,go,wsl -ErrorAction SilentlyContinue
podman --version
podman machine list --format json
podman-compose --version
python -m pip show podman-compose
podman compose --help
python --version
git --version
rustc --version
cargo --version
go version
wsl --version
wsl --list --verbose
```

Memory values from WMI are KiB; divide by 1,048,576 for GiB. Drive free space
is bytes; divide by 1,073,741,824 for GiB. Failed queries must remain unknown,
not be converted into numeric capacity. Raw machine identifiers and local paths
are omitted from this summary.

| Cases | Observed | Outcome |
| --- | --- | --- |
| C01-C06 | No service workload executed; preflight and experiment plan incomplete | Blocked |

The evaluation also confirms that the repository has no task API/worker/CDC
implementation and retains the contradictory stalled-pipeline response. B-001's
planning records and the approved SOP exist, but they do not establish B-002's
runtime prerequisites. There is no evidence here for accepting or rejecting the
reference architecture.

### SP-001-R02 — 2026-09-25, provider installation and static verification

At approximately 10:10 +03:00, installed `podman-compose==1.6.0` from PyPI
into the existing per-user Python 3.10 installation. Its dependencies
`python-dotenv==1.2.2` and `PyYAML==6.0` were already installed and unchanged.
The source revision remains R01's revision, with uncommitted documentation
updates to this record and the register only.

The [provider documentation](https://pypi.org/project/podman-compose/1.6.0/)
lists Python >=3.9 and the stable 1.x branch for Podman >=3.4. Local checks
returned exit code zero for both version output (provider 1.6.0, Podman 4.9.2)
and parsing all four services in the existing Compose file. This establishes
installation and configuration parsing, not live runtime compatibility.

```powershell
python -m pip install --index-url https://pypi.org/simple podman-compose==1.6.0
podman-compose --version
podman-compose -f docker-compose.yml config --services
$provider = (Get-Command podman-compose -ErrorAction Stop).Source
[Environment]::SetEnvironmentVariable('PODMAN_COMPOSE_PROVIDER', $provider, 'User')
$env:PODMAN_COMPOSE_PROVIDER = $provider
```

The user-level provider setting was read back and matched the installed
executable. New processes must inherit the updated environment; existing
terminals can use the final assignment above. The two verification commands
were also rerun with checked exit status and 20-second subprocess deadlines.
The provider returned `postgres`, `ferretdb`, `nats`, and `opensearch`.

`python -m pip check` reported six dependency conflicts in unrelated installed
packages; it did not report a provider dependency conflict. Those packages were
not changed. This is not a clean bill of health for the shared Python environment.

No machine, containers, or service cases were started. Fresh resource checks,
container preflight, and C01-C06 remain unexecuted in this attempt. Installation
removes the missing-provider blocker only. No CDC image or application container
was selected or installed: their experimental definitions remain to be written.

### SP-001-R03 — 2026-09-26, runtime preparation and replica-identity failure

Started static preflight at approximately 11:10 +03:00, source revision
`9774664055b37228dd916a634534403ce3550e7f`, with the pre-existing R01/R02
documentation edits retained and new uncommitted `experiments/SP-001/` artifacts.
The maintainer authorized local execution. The experiment README records its
execution owner, pending assessment, 90-minute investigation limit, fixture
bounds, and provisional contracts before service execution.

Fresh host inspection found 15.71 GiB usable physical RAM, 6.19 GiB available,
20 logical CPUs, and approximately 161.56 GiB free host disk. Required loopback
ports were available. Observed tools: Python 3.10.11, Git 2.55.0.windows.5,
rustc 1.93.0, Go 1.27.0, Podman client 5.8.3, and podman-compose 1.6.0.

The existing machine metadata could not be read by the newer client. A separate
WSL machine was initialized, preserving the old distribution/data. The tested
new engine is Podman 5.8.7 on Fedora 44, kernel 6.18.33.2-microsoft-standard-WSL2,
with cgroup v2 memory control and `vm.max_map_count=1048576`. WSL reported about
7.61 GiB memory rather than the requested machine setting; container ceilings
were therefore verified separately. No firewall/antivirus settings were changed.

Preparation exposed and corrected these issues:

- The initially selected Debezium `3.2.7.Final` image tag was absent; registry
  inspection identified the available pinned `3.2.4.Final` release.
- podman-compose 1.6.0 applied RAM ceilings but ignored `memswap_limit`.
  `podman update` did not change the inspected swap ceiling on these stopped
  containers. The runner now supplies `--memory-swap` at container creation and
  verifies both values before starting each stage. The initial failed gate was
  retained as a failed attempt, not treated as passing configuration parsing.
- Debezium required `/debezium/config/application.properties`; its first startup
  failed with the configuration in a different directory. The image was corrected.
- Some retries were blocked by the host headroom gate after builds/startup.
  Stopped VM restarts reclaimed cache; no workload was allowed through a failed
  capacity gate. Early service observations also supported reducing the small
  services' ceilings for R04 without changing any service-case assertion.

After setup passed, C01 ran at approximately 11:56 +03:00. Creating a task saved
version 1 and made it searchable, but updating it returned 503. PostgreSQL
reported SQLSTATE 55000: the task table lacked a replica identity while its
publication included updates. The runner stopped the containers. C02-C06 were
not run in this attempt. Minimum sampled host available RAM was 2792 MiB; this
failure was a replication prerequisite, not an observed memory shortage.

The physical mapping probe found `sp001.tasks_ad2e48cd` with a `_jsonb` document
and FerretDB `$s` type metadata. The bounded string/int fields can be extracted
from that document, but this does not establish arbitrary BSON compatibility.
The fix configures `REPLICA IDENTITY FULL` on the discovered experiment table
and a publication containing only that table, excluding FerretDB metadata.
This follows PostgreSQL's documented
[replica-identity requirement](https://www.postgresql.org/docs/16/logical-replication-publication.html).
Future collection creation/migration needs an explicit provisioning contract.

### SP-001-R04 — 2026-09-26, constrained end-to-end cases passed

Setup and cases ran approximately 11:59–12:01 +03:00 with the R03 replication
fix, fresh synthetic seed, and tightened memory profile. Static preflight found
4722 MiB host available RAM after VM startup (required: 4224 MiB) and 951.74 GiB
available in the sparse container filesystem, with host disk checked separately.
The provider created all eight services with verified memory/swap ceilings.
Seed delivery established live host/API, database, broker, CDC, and search
connectivity before running C01-C06. Image versions and observed digests are
retained in [images.observed.json](../../experiments/SP-001/images.observed.json).

| Case | Observed result | Elapsed seconds |
| --- | --- | ---: |
| C01 | Saved versions 1 and 2 survived FerretDB reads and became searchable | 1.828 |
| C02 | CDC was paused before save, worker was killed after acknowledgment, and CDC delivered the change while worker remained down | 7.156 |
| C03 | CDC was killed with an offset file present; a version saved during the outage became searchable after restart with the same volume | 6.735 |
| C04 | Identical retry returned the original result; changed-content ID reuse returned 409; duplicate version 2 and older version 1 were acknowledged without replacing version 2 | 5.547 |
| C05 | Save returned 200 while indexer was stopped, search returned pending/404, and restart made the saved version searchable | 12.609 |
| C06 | Two concurrent updates against version 1 returned one 200 and one 409; saved/search version was 2 | 1.891 |

All cases passed in 35.766 seconds total case time. Delivery waits used a
30-second deadline; setup readiness used 120 seconds. C02/C03 used explicit
interruption barriers, and C04 verified acknowledgment through each republished
stream sequence. C06 is one concurrent pair, not a stress or linearizability test.

| Service | Enforced ceiling (MiB) | Highest sampled usage (decimal MB) |
| --- | ---: | ---: |
| PostgreSQL | 256 | 60.09 |
| FerretDB | 128 | 10.01 |
| NATS | 128 | 6.537 |
| OpenSearch | 1536 | 1257 |
| Debezium | 768 | 304 |
| Worker | 128 | 32.03 |
| API | 128 | 30.42 |
| Indexer | 128 | 27.54 |

Total container ceilings: **3200 MiB (3.125 GiB)**. Highest aggregate observed
container usage: **1.597 GiB**, from seven samples approximately 5.3 seconds apart.
Host available RAM was sampled about once per second and never fell below
**2691 MiB (2.628 GiB)** during cases. No monitor errors or container OOM flags
were reported. Samples can miss short peaks and exclude VM overhead; these are
not minimum hardware requirements or a production capacity benchmark.

Post-run NATS inspection confirmed file storage, a durable consumer with explicit
acknowledgments, 15 retained messages including prior fixtures/replays, acknowledgment
through stream sequence 15, and zero pending/unacknowledged messages. This does
not establish power-loss durability, broker crash recovery, or a backup guarantee.

Validation: `python scripts/verify.py --pure` passed during preparation. The full
`python scripts/verify.py` subsequently passed against this live Podman stack:
14 Python tests, service readiness, the Python JetStream probe, Rust parser tests,
the live Rust NATS test, and Go tests. Final verification/recording resumed later
the same day; the intervening idle interval was not a monitored soak test. All
experiment containers and their dedicated VM were stopped afterward; named
volumes and images remain for reproduction. No CI environment was run.

## Conclusion and reuse boundary

The reference storage/event stack supported this small task create/update/read
path on the available 16 GB workstation with constrained containers. Continue
with the reference stack for the next implementation design, subject to maintainer
assessment of the provisional save/conflict contracts and replication provisioning.
The concrete component roles, retry rules, and resource/cost consistency
prerequisites are in the [design handoff](../../experiments/SP-001/README.md#design-handoff).

Evidence applies only to the pinned versions, local Windows/Podman pairing,
Python boundary adapters, small string/int fixture, and cases above. It does not
validate the planned Go/Rust adapters, UI, authentication, schema evolution,
deletion, collection migration, multi-document sagas, WAL-retention policy,
poison-event recovery, production concurrency, or power-loss/backup behavior.
Database mapping, CDC, delivery, or projection changes require targeted reruns.

## Decision and follow-up

No new product/architecture decision is accepted by this result. Q-001 now has
bounded integration evidence and a proposed contract; Q-002 has local-only
experimental runtime evidence, not an accepted authentication/deployment model.
Q-019 remains open for cross-document consistency. Record accepted choices in
D-records after maintainer assessment; preserve the experiment's limitations.

## Handoff

- Latest attempt: SP-001-R04; preflight, C01-C06, and the full repository suite
  passed. R01-R03 are retained, including the replica-identity failure.
- Resource result: this bounded prototype fits the observed workstation; no
  memory upgrade was demonstrated necessary. Runtime and containers are stopped.
- Exact next action: maintainer assessment of the proposed save/conflict contract,
  table provisioning requirement, and local access scope; then design the Go/Rust
  implementation against the retained cases. No maintainer approval is inferred.
- Open implementation work: collection lifecycle/schema handling, supported
  authentication/recovery objectives, cross-document consistency, and promotion
  of relevant cases into regression coverage alongside actual product adapters.
