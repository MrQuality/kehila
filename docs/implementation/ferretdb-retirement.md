# FerretDB prototype retirement

Implemented in [PR #97](https://github.com/MrQuality/kehila/pull/97).
Local final qualification of the changed build, setup and verification composition
passed at `2ec9794`. The PR tracks exact-head hosted checks and review readiness.
Earlier results below retain their own source and scope.

## Application and preservation boundary

[ADR-102](../architecture/ADR-102-postgresql-transactions.md) selects native
PostgreSQL and preserves experimental records pending an explicit compatibility
or import decision. [ADR-105](../architecture/ADR-105-local-owner-authentication.md)
and [ADR-111](../architecture/ADR-111-native-work-items.md) define the authenticated
Rust authority and bounded native WorkItem capability. The old anonymous
`/tasks` worker and forwarding Go API are superseded prototypes. Their HTTP
payloads, startup environment and projection endpoints are retired contracts;
this change does not implement an anonymous compatibility interface.

Retirement removes those components from application builds, normal setup and
required checks. It preserves unrelated Go/domain code, frozen codec fixtures,
source history and factual historical documentation. It does not import, drop,
reset or prune existing databases or retained volumes. Disposable synthetic
verification resources remain separately owned and cleaned up by exact identity.

## Test dispositions before removal

The former sources are identified at baseline
`30a8fe5372959d99c2855af3b602961cc1aec034`; their history remains available after
deletion. The native contracts retain integrity and bounded ownership while
using a different authenticated protocol and one active authority.

| Former assertion | Disposition and retained evidence |
| --- | --- |
| `live_task_worker.py::main` and `task_path.py::concurrency`: competing create/update, equal retry, changed-content token reuse, stale version, historical replay after a later edit | **Adapt.** Native item-write, pending-creation and mutation fixtures exercise scoped identity, exact typed requests, conflicts, history and replay. `project/competing.rs` covers controlled native ordering. Two legacy worker processes and anonymous title/status JSON are not a supported native topology or wire contract. |
| `task_path.py::boundary`: Host, Origin, fetch-site and content-type rejection without a write | **Adapt.** Native HTTP admission, original-byte ingress and item route tests retain their security and no-work oracles. Browser requests require the native cookie, Origin and CSRF proofs; the prototype's blanket Origin rejection is retired. |
| `task_path.py::boundary`: incomplete body with concurrent health/read | **Adapt.** `project/body.rs::exercise` validates a complete request through the same custom TLS/native-adapter service, then withholds one byte. An actual pending Hyper body poll precedes a concurrent authoritative read through the production listener helper; the adapter must still be pending when that read completes, then return its own 408. Admission accounting rejects premature database work. This qualifies the bounded adapter/socket composition, not protected startup or the production accept loop. The anonymous health response and prototype latency assertions are retired. |
| `task_path.py::database_outage`: paused database traffic, bounded failure and recovery | **Adapt.** `project/network.rs::exercise` first completes a native read, then holds observed serving-response bytes while leaving the dedicated connection unpaused. The original caller deadline produces raw unavailability and retires the authority. Explicit cleanup releases the held traffic without acknowledging delivery; only the gated serving-client downstream write may classify a terminal disconnect as expected cleanup. Owner/relay joins and independent backend/lock release precede a successor's authenticated read of unchanged state. The [testing guide](../TESTING.md) records strict error boundaries and the distinction between injected, in-memory and native composition controls. The one-second caller budget is a test containment bound, not a product latency guarantee. No same-runtime recovery or row-query-specific stall is claimed. |
| Worker `cancelled_callers_do_not_cancel_or_unbound_storage_work` | **Adapt.** Native admission receiver-drop/capacity and stop/abort controls retain bounded work ownership. Native mutation response-loss and pending-command controls cover uncertain outcomes using actual PostgreSQL state. Cancellation does not establish that a write was rolled back. |
| Worker `admission_and_replay_have_independent_clocks` | **Adapt.** Native clock-authority, pending-expiry, mutation replay and tombstone checks retain distinct work/pending deadlines and server-origin replay identity. The prototype's UUIDv7 issuance-age admission policy retires with its wire contract; identical old admission windows are not promised. Frozen typed codec checks remain unchanged. |
| Go `TestSaveAndAuthoritativeReadDoNotDependOnSearch`, `live_go_rust.py::main`, and `live_task_api.py::main` save/replay/read assertions | **Adapt.** The native fixture provisions an isolated PostgreSQL path without a search service and checks authoritative operations and durable state. The Go-to-worker forwarding observation is retired with those components. The standalone `live_task_api.py` example is outside the default suite and uses an obsolete operation-ID format; it is not retained as a supported execution lane. |
| Go `TestCrossOriginMutationRejected`, `TestDistinctWorkerConflictIsPreserved` | **Adapt.** Native browser admission and native command conflicts are retained. Forwarding an anonymous worker status/error response is retired. |
| Go `TestOlderSearchProjectionRemainsPending`, `TestSearchVisibilityAcceptsNewerProjection`, and the `live_task_api.py::main` / `live_go_rust.py::main` `/search?min_version` assertions | **Retire** the obsolete projection-router protocol. Native search/event delivery is outside ADR-111's implemented slice. Keep the unrelated `go/pure/sync_contract` module, its tests and historical projection records. No native search implementation is introduced to preserve an obsolete endpoint. |
| Three Rust worker startup-configuration tests and six Go command startup/helper tests | **Retire** the removed binaries' environment, address-reporting and startup ABI. Retain native closed profile/asset validation, loopback/TLS boundaries and verifier configuration rejection tests. These are current contracts, not an emulation of the removed environment. |
| All six `test_task_path_build.py` tests: default owned preparation, build failure, run-only, missing binaries, build-only, expired-child cleanup | **Retire** that binary-pair controller with its producer. Keep the current container command/source, service ownership, partial-start and cleanup tests, plus SP-003 compiler/output lifetime tests. No obsolete binaries are built merely to preserve a controller test. |
| All six `test_task_path_diagnostics.py` tests: reset/status context, bounded tails, unreadable output, broken diagnostics, primary-error preservation | **Retire** that controller's diagnostic format. Keep bounded current command/native-fixture diagnostics and primary/cleanup failure bookkeeping. Historical failed evidence remains historical evidence. |
| All seven `test_task_path_readiness.py` tests: LF/CRLF addresses, invalid addresses, duplicate reports, exited children, oversized diagnostics, exact limit/overflow, partial line | **Retire** the obsolete process-discovery protocol. Remaining provider startup, copied-source/cgroup identity, native profile rejection and exact resource-ownership checks stay required. |
| Pure Task/domain and Go synchronization contracts, SP-002 seed oracle, SP-003 eight Rust tests and five grouped golden/rejection controls | **Keep.** SP-003 now builds the retained `project_create_codec` producer, resolves its direct Cargo package identities and accepts exact native-target libraries. Rust oracle sources and frozen fixtures are unchanged. |
| Service startup, interruption, joining and cleanup | **Adapt** the remaining verifier groups without interpreting an older receipt as a new resource grant. Preserve exact source/Compose/owner/resource bindings and reject incompatible records before destructive recovery. The changed composition still needs final runtime evidence. |

The Python controller rows cover every test in those three removed test files.
The Rust startup row covers all tests in `startup_configuration.rs`; the Go rows
cover all server and command tests, including the startup helper process. Their
dispositions remove obsolete contracts while retaining applicable native or
shared ownership obligations.

## Evidence and validity

The baseline [native CI run](https://github.com/MrQuality/kehila/actions/runs/42389987900)
at `30a8fe5` completed all fifteen native modes, independent durable-state
observations and owned cleanup. The application runtime implementation is
unchanged by the subsequent oracle and test-only preservation checkpoints.
That evidence remains relevant to unchanged native contracts; it does not
qualify the forthcoming removal and verifier composition.

The direct-oracle checkpoint `0e6bad9` passed its required containerized
development hook, including 510 Python tests with two platform skips, workspace,
page, Go, seed and frozen-codec checks. An earlier lifecycle-double mismatch and
an offline metadata request for an uncached target dependency were repaired;
the actual oracle then passed with explicit compiler-host filtering/building.

The body control passed the focused native `item-read` fixture and its independent
SQL observer before checkpoint `99cdba6`; its required development hook also
passed. Its initial compile-time closure assertion was corrected without changing
the production rejection type. The serving-network control also passed the
focused native fixture and independent observer. Failed attempts are distinct
from these successful results and do not establish qualification.

The removal's tooling suite passed 499 Python tests with two platform skips,
including six isolated guard cases, and the naming, engineering-register and
active-path checks. The guard runs in the shared verification matrix, checks
existing Go/service/ownership inventories and known retired active bindings,
and preserves historical records and frozen fixtures. An earlier tooling run
found stale verifier assertions and engineering-register links to retired
files; those were corrected before the successful rerun. The register's
requirement statements and implementation statuses remain unchanged.

Cargo and Go regenerated the reduced workspace through their package managers.
Retained package versions and checksums did not change; the new manifest-scoped
dependency cache was prepared separately. These are development observations,
not final runtime qualification.

The full local qualification at `2ec9794` passed all fifteen native modes with
independent database observers, the retained seed/frozen-codec checks, shared
workspace/page/Go checks and PostgreSQL, NATS and OpenSearch service groups.
The normal PostgreSQL-only group passed readiness and exact owned cleanup.
Separate real PostgreSQL-only controls passed a controller failure after resource
creation but before normal ownership observation, and forced owner interruption
at that armed boundary. Both captured actual container, network and anonymous
volume identities before removal; interruption recovery followed the writer's
confirmed termination/join. These observations cover the selected fault stages,
not every possible internal Compose startup failure. All owned groups completed,
the runner/network were removed, and the initially stopped provider was stopped.

Local qualification used the existing pinned toolchain image. The revised
Containerfile only removes an unused prototype output directory; hosted native
CI builds and uses that current Containerfile. Documentation-only historical-link
and result updates after `2ec9794` preserve the runtime evidence's source premises.
Readiness also requires exact-head hosted checks and independent final composition
review, tracked on the PR. These synthetic fresh-database checks do
not establish real-data import, populated upgrade, general restore, browser-loss
recovery or a multi-active authority capability.
