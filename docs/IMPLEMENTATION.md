# Implementation status

YAJA is in early development. The current components are:

| Component | Implemented behavior | Verification |
| --- | --- | --- |
| Rust query parser | Single `identifier = 'nonempty value'` expression; rejects trailing clauses | Unit tests |
| Rust NATS connection | Bounded TCP connection, INFO/CONNECT handshake, PING/PONG | Live integration test |
| Python NATS check | JetStream availability and account API response | Live integration check |
| Development services | Four TCP ports and OpenSearch HTTP health | Readiness checks |
| Go synchronization rules | Provisional evaluation requires matching schema version and a pure query | Table-driven unit test |
| Go local task API boundary | Proxies authoritative task writes/reads to a worker; keeps version-gated search reads separate; checks loopback origin on mutations | Unit tests and a live boundary check |
| Rust task mutation worker | Stores each accepted mutation and its replay result as one immutable FerretDB operation record; optimistic version and operation-ID indexes were checked against real services | Unit tests and a bounded live Go/Rust write/read/conflict check |
| TypeScript contracts | Shared declarations | No runtime implementation |
| Development tools | Staged-source verification and CI base selection | Temporary Git repository tests |

The isolated [SP-001 experiment](spikes/SP-001-task-path.md) also exercised a
small task save/CDC/search path with Python boundary adapters on Windows/Podman.
Its six cases passed with constrained memory after configuring PostgreSQL replica
identity and a task-only publication. This prototype is retained for reproduction.
The newer Go/Rust work covers only the API-to-worker mutation/read boundary so far;
it does not yet provide the production CDC/search path or supported installation.

## Current limitations

The Rust NATS component is a synchronous connection probe. Publishing,
acknowledgment, and replay are not implemented in that component. The new task
worker uses external crates for its FerretDB adapter and HTTP boundary.

The Compose services are for local development. PostgreSQL enables logical WAL,
but that development stack has no replication slot or CDC connector. FerretDB runs as a standalone
proxy. TCP readiness does not establish query correctness.

The Go test helper preserves test exit status and retries temporary-directory
cleanup for up to 30 seconds to handle Windows executable file locks. Failed
tests are not retried.

## Planned work

The [product plan](product/README.md) adds the agreed product
requirements, design rationale, open questions, and a proposed delivery backlog.
Use [that backlog](product/backlog.md) to trace product work to requirements;
the technical work below remains part of the existing architecture reference.
Documented requirements and planned backlog items are not implemented features.

- Full query grammar, OpenSearch and Rhai emitters, and Wasm bindings.
- Complete typed domain mutations and authoritative schema state.
- Sagas, the production CDC-to-Rust-indexer path, and idempotent search projections.
- Go authentication, full API routes, and SSE delivery.
- React UI, optimistic state, and reconciliation.
- Recovery and crash-isolation integration tests.

The [v0.2 design](reference/YAJA-v0.2.md) now reflects the accepted
[first-increment save contract](product/decisions.md#d-016): an acknowledged task
action returns 200 and a persisted version independently of search visibility.
The first Go/Rust mutation/read slice is implemented, but its operation-record
retention/compaction, installation and replication readiness, authenticated access,
and production indexing remain incomplete.

## Task-path implementation in progress

The [accepted direction](product/decisions.md#d-016) requires atomic mutation and
successful-operation recording. FerretDB v1.24 does not support multi-document
transactions, so the current Rust worker stores each successful task mutation and
its result as one immutable operation document. Its `_id` is the task/version pair;
a unique task/operation-ID index rejects reuse. A bounded local service check
verified both uniqueness constraints and latest-version lookup. This is a new
mapping, separate from the SP-001 Python fixture, and its physical PostgreSQL
table has not yet been provisioned for CDC.

The worker uses provisional, internally configurable 90-day limits for admitting
an unseen UUIDv7 operation ID and replaying a recorded success after server commit.
It looks up recorded successes before testing admission age or the current task
version. It compares decoded typed fields rather than JSON byte order, returns
the original successful version for an identical replay, rejects changed-content
reuse, and rejects expired IDs rather than executing them again. Rejected attempts
are not retained. Old operation records are not yet compacted, so physical
retention and the separate product-history policy remain unfinished.
The worker also lacks replication-readiness verification and separate supported
installation credentials. Its current index-install command is a development
setup aid; it does not establish production provisioning.

The Go API defaults to loopback and enforces same-origin browser mutations. It
keeps database-backed reads available when search is down. Its projection route
now checks an actual OpenSearch query and refuses a result older than the
requested saved version. A live Go-to-Python experiment check passed with the
search projection running; a separate live Go-to-Rust check passed with search
deliberately stopped. These checks do not validate the production indexer, CDC, browser UI,
authenticated access, or post-crash durability. The current missing-Origin policy
is not a supported browser-session CSRF contract.
