# SP-002 ? PostgreSQL project-creation experiment

See the [spike record](../../docs/spikes/SP-002-postgresql-project-create.md) for
observations and limitations. This prototype tests the frozen proposed schema
and explicitly experimental roles/protocols. It is not an application adapter,
authentication system, final storage codec or production compactor.

## Prerequisites and execution

Use PostgreSQL 16 through Podman only. Start your selected Podman machine first
on Windows; the runner checks connectivity but does not choose or start a machine.
Inspect free space in the Podman VM as well as the host. Budget 3 GiB available
host RAM and 10 GiB artifact-disk headroom for this bounded workload. These are
preflight limits, not measured production requirements. Containers have no
verified per-container CPU/memory ceiling; monitor the runtime's actual resource
usage. The initial rootless environment rejected resource-limit flags.

Host tools: Python 3.10+, Rust/Cargo 1.75+, Go, Git and Podman. Build the actual
pure contract on the host. Create an isolated Python environment and install:

```text
python -m pip install -r experiments/SP-002/requirements.txt
python experiments/SP-002/manage.py
```

The runner freezes the original schema and pure-contract baseline at
`9f433f99b31d0a3e7378a6009fd319a6788640fe` and pulls an immutable PostgreSQL image.
It refuses a changed pure contract instead of silently testing a different seed.
No Docker executable or Compose provider is used.

Each execution creates fresh names, loopback ports, synthetic credentials,
containers and persistent volumes. Artifacts default to an ignored run directory
under `.kehila/spikes/SP-002/`; `--output` accepts another NEW directory. Existing
artifact directories are never overwritten. Copies of the executable cases run
there; generated credentials, raw logs and dumps must not be committed.

Expected exit **2** means the bounded run completed with the known privileged
payload-move failure and six declared product checks blocked. It is not an overall
QA pass. Exit **1** means setup or another experiment failed: retain the evidence
and inspect it. The runner returns no complete-pass exit for this frozen subject.

The runner stops its verified, labelled containers in `finally`, preserving data
volumes for review. It does not remove existing resources. Inspect exact generated
names in `runtime.json` before any manual cleanup; volume removal loses test data.
Connection files contain synthetic passwords; do not publish them. Port allocation
is checked before container start; a competing process taking that port causes
setup failure, not fallback to another database.

## Cases and ownership

- `schema_cases.py`: untouched SQL, trusted 26-row seed, every-write/deferred
  rollback, independent-reader visibility, typed target constraints, scalar and
  lifecycle boundaries. Family expansion is tested only in a separate database.
- `protocol_cases.py`: restricted ACLs/helpers, observed lock schedules, current
  creation authority, original result replay, trusted test clock and compaction.
  Apply `compactor-read-grant.sql` separately: deferred invoker triggers need
  caller read privileges at COMMIT after a SECURITY DEFINER helper returns.
- `recovery_cases.py`: before-COMMIT disconnect, protocol-aware withheld COMMIT
  response, isolated SIGKILL/restart, fresh-cluster logical restore and bounded
  descriptive measurements. Capture both log streams; binary dump uses stdout.
- `additional_cases.py`: case-sensitive keys, maximum keys, QA digest coherence,
  exact restored table contents, roles, table/sequence ACLs and function ownership.
- `concurrent_workload.py`, `maximum_key.py`: bounded two-actor observations and
  random maximum-key insertion without relying on repeating input compression.
- `seed_oracle.rs`: emits fixture values from the actual pure ProjectCreate result,
  including complete ordered configuration; its JSON format is QA-only.

The experiment's actor/time inputs are trusted test inputs. Helpers that accept
arbitrary actor or time parameters must not be installed as public application
interfaces. The baseline serving login has INSERT privileges for the experiment;
a real application must bind authenticated identity and validate pure decisions.

`results.observed.json` is a compact sanitized result summary. Per-case generated
records include error diagnostics, final-state assertions and observed lock waits.
Initial observations and corrected reruns are described in the spike record.
Publicly share only reviewed synthetic summaries. Local execution establishes
neither unexecuted Linux CI behavior nor production capacity/power-loss resilience.
