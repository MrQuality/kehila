# Container execution for the shared verification gate

Status: the original pinned-image/dependency/container lane passed at the sources
recorded below. The current native-only application direction and prototype
retirement are tracked in the [retirement record](ferretdb-retirement.md); changed
composition still needs its own final qualification. Earlier failures remain
recorded. Committed-head review and CI remain separate delivery gates.

## Outcome and scope

Run the Python, Rust, retained Go, frozen-oracle, native authority and readiness matrix
inside an owned Linux toolchain container controlled by Podman. This lane has no
Docker fallback. The Windows local hook selects this lane. Existing Linux CI verification and the
separate Windows-client CI job remain mandatory and unchanged. Host code selects/copies the exact
source snapshot, controls provider/container/network lifetimes and records output.
Application listeners, fault relays and compiled test workloads run inside the
owned container. Host lifecycle coordination does not qualify product behavior.

The native fixture checks authenticated project/WorkItem behavior, durable state
and owned recovery against isolated PostgreSQL TLS databases. The retirement
record gives explicit dispositions for the old anonymous worker/API checks.
There is no host execution fallback. Windows-specific observations remain separately scoped;
Linux results do not qualify Windows account APIs or client behavior.

## Ownership and execution

The hook captures one immutable Git tree for changed-path scope selection,
supported-mode validation and export. An external temporary index populated from
that tree keeps concurrent real-index changes from altering the payload. The
final real-index and HEAD checks remain mandatory. A host
coordinator copies that snapshot with the engine's copy operation into an owned
container; no Windows bind-mount behavior is assumed. The runner receives no
engine socket, host devices, host PID namespace or privileged flag. Source and
build/output directories are distinct. Build and temporary outputs stay in the
engine's external container storage and are removed with the owned runner.

Prepare the toolchain image separately, using explicit immutable input images,
and select its immutable image ID for a gate attempt. The gate never builds or
pulls an image implicitly. Toolchain provisioning is a separate resource-approved
prerequisite, not test evidence. Record image identity and effective resource
settings. This lane requires effective cgroup-v2 memory/CPU/PID controls and
checks their files inside the runner before executing the matrix; unsupported or
mismatched controls fail closed. Upstream defaults and external local overrides remain distinct.

The host starts the runner before compiling. Native qualification owns its
separate PostgreSQL TLS fixture and independent observers; the remaining serial
Compose groups are NATS, OpenSearch and PostgreSQL infrastructure. The runner initially uses
an owned build network for dependency access. Before live checks it detaches from
that network and attaches only to the verified group's network. Service addresses
come from inspecting the exact owned container IDs and network identity, rather
than Windows host forwarding. No worker/API ports are published by the runner.

The runner retains native fixture binaries between build and execution. Application
loopback listeners and the serving-network fault relay run inside it. Readiness probes
and NATS tests use the actual service IPs from the owned network. Compose source
context and provider/shared-machine ownership rules remain unchanged.

Recovery binds records to their exact source, Compose content, service scope and
resource identities. Removed service groups are rejected rather than interpreted
as a new group; this change does not rewrite existing records or grant authority
to remove retained resources.

## Deadlines, failure and resources

The parent retains the finite configured `gate_seconds` work budget (600 seconds
upstream); each command is clamped by `command_seconds` (300 seconds upstream)
and the remaining parent budget. Explicit external local overrides can extend
those operational allowances. Native Linux CI retains its existing 600-second
default. Dependency preparation has its own `dependency_prepare_seconds` budget
(1800 seconds upstream), and does not extend an already running gate.
Pass remaining durations across the host/container boundary and establish a fresh
container-local monotonic deadline; never copy an absolute host monotonic value.
Compiler concurrency remains one. Runner memory/CPU/PID limits are explicit policy
values, not calibrated performance claims. Validate actual limits and retain the
host memory floor and resource approval requirements before execution.

A failed or timed-out runner command fails the gate. Remove the runner and its
processes before tearing down its service network; preserve the primary error
alongside cleanup errors. No automatic retry, missing-check success, extra
privilege, or unbounded cleanup is introduced. Successful command checks alone
do not establish a successful full gate: runner, service and snapshot cleanup
must also complete.

An external host controller may set `KEHILA_VERIFICATION_OWNERSHIP_FILE` to request
the same new, absolute external ownership record as `--ownership-file`. The
container workload cannot access that record. Existing-file and checkout-local
paths are rejected; this enables recovery of the exact owned runner without
adopting unrelated resources.

## Completion and remaining gates

Implementation is complete only when offline ownership/routing/deadline/failure
checks pass, the exact proposed source is independently reviewed, the image is
prepared under approved resources, and a real containerized full gate including
cleanup passes. The PR requires exact-head review and CI; merging remains a
maintainer decision. Image preparation subsequently succeeded using retained
cached layers. The initial Python preflight ran 338 tests with two platform-specific skips,
then passed the naming and engineering-metadata checks. Its first container run
failed on a stale command expectation; the corrected rerun passed, including
runner/network cleanup and provider shutdown. The full gate had not run at that
point. The prior host gate failure remains a separate recorded outcome.

The first full container gate subsequently stopped at the Rust workspace
compilation command's 300-second deadline. Index updates and dependency downloads
preceded compilation; their individual durations were not isolated. The runner,
network and provider cleanup completed. This is a failed gate, not a compiler
diagnostic or qualification of the unexecuted service matrix.

After adding explicit dependency preparation and configurable allowances, the
revised preflight passed 348 tests with two Windows-specific skips, plus naming
and engineering-metadata checks. Dependency preparation passed in 340.30 seconds
through cleanup. A fresh runner reused that named cache and passed the full gate
in 246.72 seconds through cleanup. Cargo reported 1 minute 35 seconds for fresh
workspace test compilation. This is an offline dependency-cache run, with fresh
compilation outputs; it is not a cold dependency-download benchmark.

The full gate included Rust workspace compilation/tests, SP-002/SP-003 oracles,
Go static/tests, task-path binary preparation, live NATS, OpenSearch readiness,
and PostgreSQL/FerretDB task-path execution. The task path passed 48 competing
pairs across two workers, direct-worker/stalled-body guards, database
outage/recovery, and Go-to-Rust save/replay/read with search unavailable. Runner,
service, network, provider and source-snapshot cleanup completed. Sampled
VM/helper working sets peaked at 4.80 GiB; minimum sampled available host RAM was
2.17 GiB. These are bounded observations, not a guaranteed peak or product
capacity qualification. The retained dependency volume is intentional disk state.

Broader machine calibration remains deferred until after this delivery. Retain the
prior native structural pass and failed Windows gate evidence with their limits;
do not combine them into a fabricated container qualification total.

## Image preparation and invocation

The Containerfile pins official Rust 1.99.1 slim Bookworm, Go 1.27.3 Alpine
copy-source images and a Python 3.12.15 Bookworm final runner on 2026-11-02.
This profile targets Linux amd64; other architectures remain unqualified. Registry index bodies were checked
against their returned SHA-256 digests and included Linux amd64 manifests. This
establishes input identity, not an executed build or minimum-version qualification.
The current Linux CI toolchain remains a separate required observation.

The product page-state suite additionally requires Node 24.22.2. The image and
Linux CI use `tools/verification/install_node.py`, which downloads the fixed
official Linux-amd64 archive over verified HTTPS and verifies its pinned SHA-256
before copying only the runtime and bundled license to a new external prefix.
No npm packages or browsers are installed. Full verification runs the deterministic
page tests before Rust compilation; documentation-only checks do not require Node.
These simulated-DOM tests do not establish Windows browser or TLS behavior.
An older retained image without this runtime must be explicitly prepared again;
adding the command to the matrix does not make an existing image ready.
The image also installs Clippy explicitly for the pinned Rust toolchain and checks
its archive identity against that toolchain's retained manifest; the official
minimal profile does not include it. Tool-version probes establish availability,
not a successful project lint result.

The container matrix runs the native product PostgreSQL/listener fixture serially
after compilation and build-network disconnection, before the broker/search/task
service groups. Its database shares only the runner's loopback namespace, uses a
pinned PostgreSQL 16 image and bounded tmpfs/memory/CPU/PIDs, and publishes no ports.
The host lifecycle owner removes that database before its namespace-owning runner.
Independent queries check durable cardinalities/identity and released authority
connections/locks. Test secrets and failure diagnostics stay in external storage;
the serial retention/authority extension uses an explicitly marked disposable
database and a privileged test connection, separate from the runtime capability.
It temporarily replaces the database clock with a fixed, owner-controlled test
function, verifies runtime denial of its controls, restores the original function
identity/owner/ACL/body and destroys the database during owned cleanup. This tests
expiry/replay/retirement paths; it does not qualify the production clock body,
machine clock changes or backup/restore behavior.
After restoring the production clock, a quiescent synthetic copy is restored into
a fresh database in the same disposable cluster. Records and captured schema/
authority properties compare before fresh-authority startup; CHECK expressions
use PostgreSQL scratch-object reparse with semantic-change negative controls.
Raw snapshots and the dump contain verifier/audit data and must remain outside
the checkout and public output. This is a coherence test, not supported recovery
or exclusion between databases. Selected authentication regressions run serially
afterward, with the password-changing case last.
local callers must provide private temporary storage and resource supervision.

`--native-authority` selects just this compiled product fixture after explicit
`--prepare-dependencies`. The `native-authority` Linux CI job uses the same mode
with an exact exported source tree and Podman. Ordinary documentation changes
skip that job's heavy steps. This is a required backend observation, not Windows
browser, protected-startup, supported installation or external-corpus qualification;
it does not make the remaining ignored fixtures pass.

After resource approval, build with the chosen engine using
`tools/verification/Containerfile` and `tools/verification` as its context. Record
the resulting immutable image ID. Set `runner_image` in the external local settings to that ID for the
Windows staged hook, or use an explicit `KEHILA_VERIFICATION_IMAGE`. Missing images, mutable references and missing execution
prerequisites fail closed; the gate never pulls or builds implicitly.

The current default runner policy is 2048 MiB, one CPU, 512 PIDs and 120 seconds
for engine cleanup. These are configurable containment allowances, not measured
machine requirements. Provider startup allowances are configured separately
from the runner's limits. Image preparation and a first cold build can
have different costs from a warm gate; measure and report them separately before
calling any local values calibrated.

The separate cold image-preparation allowance defaults to 1800 seconds through
`image_prepare_seconds`. It is a provisional operational ceiling, not measured
machine performance, and it does not extend the shared gate. The original input metadata contained 1,114,761,485 unique compressed base-layer
bytes for Linux amd64. The revised copy-source inputs contain 776,401,209 bytes
before the required formatter component, about 30% fewer. Expanded storage,
transfer duration and peak RAM remain separate observations.

## Deferred efficiency work

After the first qualified observations, compare smaller official image variants,
duplicated toolchain layers, dependency/cache reuse and provider/runner overhead.
Use measured peak RAM, expanded image storage and phase duration when selecting
changes. Image minimization and a runtime-provider migration are not additional
completion gates for this delivery. CRI-O is not adopted by this container lane.

## Preparation correction and scoped execution

The original image pull stopped at the configured available-memory guard during
its first Go source stage; no project compilation or Python preflight ran. The
provider was stopped after the engine completion event and resource inventory
were reconciled. This failed attempt is retained, not converted into a pass.
The event alone is not proof that all image-pull work has joined. In the inspected
[Podman 5.8.7 Buildah executor](https://github.com/containers/podman/blob/v5.8.7/vendor/github.com/containers/buildah/imagebuildah/executor.go),
a stage error can return before the stage wait group completes; its
[pull implementation](https://github.com/containers/podman/blob/v5.8.7/vendor/github.com/containers/buildah/pull.go)
uses a background context. Preserve uncertainty after terminating the local build
client, and establish provider shutdown separately. These source observations do
not establish that a historical pull survived the completion event.

Only the Go installation tree and the Rust cargo/rustup trees are copied into the
final Bookworm runner. The official [Bookworm Go source](https://github.com/docker-library/golang/blob/41d9b4da70324ae7b6461ac2cc401b06719ff57d/1.27/bookworm/Dockerfile)
and [Alpine Go source](https://github.com/docker-library/golang/blob/41d9b4da70324ae7b6461ac2cc401b06719ff57d/1.27/alpine3.24/Dockerfile)
use the same Linux-amd64 Go distribution archive and checksum. The [Rust slim source](https://github.com/rust-lang/docker-rust/blob/de3b3171ec576fd91eb8e28d3d37fa117b135fcb/stable/bookworm/slim/Dockerfile)
retains the same Rust and rustup versions and the minimal installation profile.
These identities support the source-variant change; actual final-image execution
compatibility is still a required preflight result.

The minimal Rust profile omits `rustfmt`, which SP-003 requires. Preparation now
adds that component explicitly to toolchain 1.99.1, checks its official archive URL
and checksum in the pinned toolchain manifest, records the installed formatter
version and binary digest, and checks tools again as the final non-root user.
The component archive checksum is
`e3b36c4c7ec3486c059d8067ff48787676612007123aad7a4e8ec12c48ee9ba2`.
The component's extra transfer is excluded from the base-layer comparison.

This correction requires an isolated networked image-preparation stage for the
official Rust distribution download. The earlier network-disabled preparation
command is no longer sufficient. It does not require host networking, an engine
socket or broader runtime privilege. A future attempt must explicitly use the
revised network policy and existing memory/cleanup controls. No proportional RAM
saving is claimed. The revised image was subsequently built and used for the
Python/metadata preflight and later full gate. This cache-assisted result does not
qualify clean cold-cache preparation or Windows-specific behavior.

## Resource-control boundaries

Container memory limits do not establish a bound for the engine's image-transfer
and extraction work. Measure cold preparation separately from runner execution;
reducing the runner limit alone does not address an engine-side preparation peak.
The JSON settings are inputs to lifecycle controllers, not global Podman/WSL
configuration. A controller must explicitly apply a preparation allowance.

For WSL-backed Podman, displayed machine resource metadata must not be treated as
proof of the effective guest memory envelope. Inspect the running guest and host
observations. [Podman's machine-setting documentation](https://docs.podman.io/en/latest/markdown/podman-machine-set.1.html)
limits its memory setter to QEMU; [Microsoft's WSL documentation](https://learn.microsoft.com/en-us/windows/wsl/wsl-config)
describes the separate global WSL memory configuration. Changing that policy can
affect other distributions and is outside this delivery's automatic setup.

Possible later work includes lower layer-copy concurrency and a smaller final
image with an explicit compiler/Git/runtime-library closure. Neither is adopted
here. Global daemon/WSL settings and cache flushing are not silently changed to
make a resource guard pass. The revised source-stage pins do not establish a
proportional reduction in cold provisioning memory; the successful preparation
reused retained cached layers.

Host reserve and aggregate-memory budgets are machine-local policy, represented by
`host_available_memory_floor_gib` and `host_aggregate_memory_budget_gib`. Upstream
leaves both unset rather than inventing a universal machine reserve. Local
lifecycle controllers must require and enforce the chosen values when operating
under an approved machine budget. Configuration does not grant resource approval
or establish a measured peak, and it does not modify WSL settings.

`host_available_memory_warning_gib` is an optional observation threshold. A
warning does not fail verification or authorize changing a separate stop
condition. Controllers may report it while enforcing an explicitly approved
aggregate budget and operating-system low-memory signal; they must distinguish
warnings from stop events. These observations do not establish a hard memory cap.

The committed coordinator does not monitor host RAM or act on these host guard
settings itself. A supervising lifecycle controller must implement the selected
warning/stop policy; configuring a value alone does not activate enforcement.
The runner's memory limit excludes the provider, host helpers and service
containers. The current Compose services have no individual container memory
ceilings; OpenSearch's configured Java heap is not a total-process ceiling.
Measure each serial service group's footprint and provider overhead before
choosing service limits and a host reserve with headroom. Those measurements and
limits remain verification-tooling follow-up, not established capacity evidence.
If a supervising controller stops a run for resource pressure, preserve the
failure and perform owned cleanup; skipped phases cannot qualify the gate.
Required local and hosted checks remain mandatory. Moving checks from a local
workstation to CI requires an explicit policy change; lack of safe local headroom
does not authorize skipping them or using a host-workload fallback.

## Recovery after abrupt termination

Ordinary failure cleanup cannot guarantee release after a forced process kill,
host crash or reboot. No automatic orphan reconciler is implemented. Before the
next run after such an interruption, inventory containers and networks in the
same Podman connection. Inspect runner/build-network ownership through
`org.kehila.verification-owner` and service ownership through the exact unique
Compose project and `com.docker.compose.project` labels. A prefix or age alone is
not proof of abandonment. Establish that the owning verifier has exited and its
leases are free; if ownership cannot be established, report the ambiguity rather
than removing resources. Do not delete lease files to force access.

For a proven abandoned run, remove only inspected owned containers by their full
IDs, then remove its exact networks after checking attached containers. Preserve
intentionally persistent dependency volumes and unrelated workloads. A provider
may be shared; abandoned resources do not authorize stopping it. Engine/Compose
errors currently surface port and subnet allocation failures; there is no
specialized collision recovery or global network-pool repair. The development
Compose services publish their configured ports even though the runner publishes
no worker/API ports. An explicit ownership-aware recovery command and preflight
inventory remain tooling follow-up; blanket pruning is not the recovery policy.

## Prepared dependency cache

Run the coordinator's explicit `--prepare-dependencies` mode before a pure/full
container gate. Preparation fetches the locked root Cargo workspace for Linux
amd64 and downloads the Go workspace's modules. It performs no compilation or
test execution, verifies that copied source remains unchanged, and only then
publishes an atomic readiness witness. SP-002 and SP-003 use libraries from that
root Rust workspace; the isolated AUTH-001 workspace remains outside this matrix.
The witness records dependency preparation, not a verification result.

A labelled Podman local volume retains the dependency cache in engine storage,
outside the checkout. Its identity includes the actual immutable toolchain image,
Cargo manifests/lockfile and Go workspace/module manifests and sums. Volume
ownership, driver, absence of host redirection and the effective runner mount are
checked. A host lease serializes preparation and verification using the same
cache. The cache is writable, trusted local input for this verifier; it is not a
shared cache for untrusted jobs, credentials or arbitrary Cargo configuration.
The host does not cryptographically attest the volume's contents before mounting
it. Labels establish expected local ownership, and the readiness witness records
completed preparation; neither proves dependency provenance or absence of local
tampering. Do not reuse a cache suspected of corruption or compromise. After all
users release the relevant lease, inspect and remove only that exact volume using
the non-forced procedure below, then explicitly prepare it again from reviewed
inputs. Rebuilding is not remediation for a compromised host or poisoned upstream
input. Cache integrity checks and independent CI remain separate assurances.

Verification requires an existing, prepared cache. Cargo and Go dependency
resolution are set offline, and the build network is disconnected before the
matrix executes. Missing dependencies or build scripts that require an external
download fail; there is no online fallback or implicit fetch. Real offline
compilation must establish whether preparation covers the actual graph. Source
is freshly exported/copied, compilation outputs remain in the disposable runner,
and all required tests still execute. Test-result or binary caches do not replace
checks.

The named dependency volume intentionally survives successful and failed runs;
runner, process, service, network and source-snapshot cleanup remain mandatory.
`rm --volumes` removes anonymous runner volumes, not this explicitly named volume.
No automatic prune or unrelated-volume removal is performed. After the relevant
verifier lease is free, inspect volumes with the
`org.kehila.dependency-cache` label and remove only the exact reviewed name with
`podman volume rm NAME`, without `--force`. Normal engine refusal for an in-use
volume remains effective. Different image/manifest identities can accumulate disk
usage; this cache has no disk-capacity guarantee and requires explicit reclamation.

Preparation and verification timings must be reported separately, including
cleanup. Time allowances are provisional operating policy until measured on the
chosen machine; extending one does not convert a previous timeout into a pass.

## Review follow-up verification, 2026-11-02

Commit `343d5cc` clarified hook status reporting, image setup diagnostics and
generic resource-lease naming while preserving the existing lock-file identity.
Its mandatory containerized hook passed 348 Python tests with two platform skips,
the shared Rust/Go/oracle and live-service phases, and owned cleanup. The measured
process took 271.14 seconds through supervisor cleanup. Sampled VM/helper working
sets peaked at 4.70 GiB; minimum available host RAM was 1.01 GiB. The reserve warning
was observed, with no aggregate-budget or OS low-memory stop. The provider was
confirmed stopped. These measurements describe this run, not a safe universal
reserve or a worst-case memory guarantee. No SP-005 allocation, browser, restore
or abrupt-termination recovery qualification was added by this gate.

## Native CI cache compatibility, 2026-11-03

The first native-authority CI attempt failed during dependency preparation on
Podman 4.9.3: `volume create` rejected the standalone `--uid` option. Image
preparation passed, but no native test executed. The failed
[CI job](https://github.com/MrQuality/kehila/actions/runs/41620517059/job/125864736107)
remains part of the record; the local application gate does not replace it.

Cache creation now uses the documented
[ownership-only volume option](https://docs.podman.io/en/v4.9.3/markdown/podman-volume-create.1.html#opt-o-option)
`--opt o=uid=10001,gid=10001`. Admission retains exact local driver/name/label
checks and accepts only empty legacy options, the exact ownership-only `o`
dictionary, or that dictionary with matching explicit `UID`/`GID` strings.
Wrong or partial ownership, extra mount options, device/type redirects and
alternative drivers remain rejected. No fallback creation retry is introduced.

The first local probe failed closed because Podman 5.8.7 serialized matching
explicit `UID`/`GID` fields alongside `o`; it stopped before creating the probe
container. Evidence was preserved, the temporary volume removed and the provider
stopped. The corrected admission accepts that exact observed representation.

The affected fresh-volume probe then passed. A network-isolated container with
128 MiB, one CPU, 64 PIDs, a read-only root and one exact owned-volume mount ran
as `10001:10001`, observed no effective capabilities and enabled no-new-privileges,
and checked directory ownership before writing. Its sentinel write/read and file
ownership checks passed. It used no `chown` or `:U` repair. This establishes the
normal mounted-volume behavior, not the isolated causal role of one engine option.
The owned container and temporary volume were removed and the initially stopped
provider stopped.

The successful phase took 59.063 seconds through cleanup, peaked at 1.353 GiB
sampled aggregate memory and retained at least 5.227 GiB available RAM, without
resource warnings/stops. This is local-provider evidence. The affected Podman
4.9.3 CI execution and required commit/final-head checks remain separate gates.

## Native fixture failure reporting, 2026-11-03

Run [41633004402](https://github.com/MrQuality/kehila/actions/runs/41633004402)
at `aef8f21` passed five native modes, then failed `auth-unknown`. The wrapper's
generic completed-Cargo error conflated exit status, success-summary detection
and required marker detection. Detailed Rust output was not retained as a hosted
artifact; the underlying assertion/cause remains unknown. The console record is
retained. Owned CI resources were removed; runner teardown used forced termination,
which is not evidence of graceful application shutdown or the failure's cause.

After explicit authorization, the reporting-only extension adds a closed failure
record: mode/stage, Cargo exit, summary/marker Booleans, bounded known-operation
timings and a panic location only if mapped to known public product source.
It emits no raw log/assertion text, credentials, environment values or arbitrary
paths. Metadata is best-effort: construction or stderr failure cannot replace the
original journey error. Successful tests do not construct or emit this metadata.
Timeout, decoding and oversized-log exceptions retain their existing separate
paths; product assertions, deadlines and resource limits are unchanged.

The preimplementation focused check observed the expected missing helper errors,
but its supervisor incorrectly expected eight tests/errors rather than seven
methods with nine subcase errors. That supervisory phase remains failed with its
cleanup record; it was not rerun or relabelled as a pass. Later checks and the
single authorized diagnostic CI attempt are recorded against their actual heads
in the PR. A nonreproduction cannot establish a cause or fix the original failure.

### Maintainer disposition, 2026-11-03

The maintainer explicitly waived the earlier `auth-unknown` CI failure as a
merge-blocking uncertainty after subsequent runs did not reproduce it. No further
investigation or reproduction campaign is required for PR #92. The failed run and
its unknown cause remain historical evidence; this waiver neither converts it
into a pass nor establishes a root-cause fix. A new failure must be assessed on
its own evidence rather than covered by this disposition. Required checks on
subsequent changes remain mandatory. This waiver does not authorize merging.

### Native operating-deadline profile repair, 2026-11-03

The separate [CI attempt at `11407ac`](https://github.com/MrQuality/kehila/actions/runs/41657253544)
passed five native modes and then failed `auth-unknown`. Its bounded metadata
located the panic at the request helper's five-second observer, after completed
login operations of approximately 2.8 seconds each. Password change is next in
the inspected fixture. This supports timing pressure as a hypothesis; it does
not establish either hash duration, database/commit progress or the original
historical failure's cause. The earlier waiver does not cover this new attempt.

The harness was running unoptimized Cargo test binaries against an operating
deadline whose recorded qualification used optimized code. ADR-105 originally
selected five seconds as a provisional design bound, not a measured universal
latency limit. Later [complete release-profile requests](local-owner-authentication-listener.md#complete-request-workload-and-active-work-subsets-2026-10-31)
took 0.375, 0.391 and 0.390 seconds for verification, replacement hashing,
database commit and response closure. Those small samples support their recorded
profile, not arbitrary hardware, unoptimized code or sustained contention.

Native fixture preparation and execution now share an explicit
`cargo test --release --offline --locked -p kehila_authority --lib` prefix.
Both native-only and full verification prebuild those tests before starting the
native database; compilation is not counted as a runtime journey. Exact ignored
test selection, fault hooks, state/cleanup observers, Argon2 cost, admission and
five-second product/observer deadlines remain unchanged. Ordinary debug tests
remain in the shared matrix. Release builds change optimization and debug/overflow
checking defaults, so they complement those debug tests rather than replace them.

Extending only the observer would not extend the product's capped deadline.
Extending the product deadline would change request latency and how long admitted
work retains capacity and reply/database eligibility; transport/client deadlines
would also need coherent review. Neither change is justified by this evidence.
Optimized compilation adds time and separate build artifacts; existing resource
and enclosing verification limits still apply. Subprocess-boundary regression
checks cover the profile, preparation ordering and retained exact test selection.
Actual follow-up results remain bound to their recorded commit in PR #92; the
failed attempts above are not retrospectively converted into passes.
