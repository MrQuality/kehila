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
| Rust task mutation worker | Stores each accepted mutation and its replay result as one immutable FerretDB operation record; optimistic version and operation-ID indexes were checked against real services | Pure/policy tests and automated live checks across two worker processes |
| Rust work-item rules | Pure typed status/workflow decisions validate project configuration references, phase changes, migration restrictions, archival targets, version conflicts, and replay order; not yet used by the worker | Focused pure tests; no storage or API integration claim |
| Rust project rules | Pure project archival/access, project-local readable ID allocation, exact estimate and unit-lock rules, and populated-field kind-change decisions; not yet used by the worker | Focused pure tests; storage coordination and lookup unverified |
| Rust field rules | Pure typed value, usage, owner, hidden/required, and choice-option validation; not yet used by the worker | Focused pure tests; configuration/item contention unverified |
| Rust relationship and current-work rules | Pure canonical relationship identity, duplicate/self-link eligibility, two-endpoint access input, and user-scoped selection decisions; not yet used by the worker | Focused pure tests; storage uniqueness and access enforcement unverified |
| Rust configuration-change rules | Pure revision, default, and in-use item compatibility decisions; not yet used by the worker | Focused pure tests; authoritative item set and commit-time serialization unverified |
| Rust type-conversion rules | Pure complete destination, phase, migration, history-snapshot, and replay decisions; not yet used by the worker | Focused pure tests; atomic persistence and non-field history preservation unverified |
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
Each attempt reads the current version and then looks up the operation record;
only then does it decide replay, admission, validation, or version conflict. This
ordering recognizes an identical concurrent success before rejecting its version;
unique insert conflicts reload both observations. The pure task-contract function
owns replay comparison, task validation, version conflict, and next-version rules.
It compares decoded typed fields rather than JSON byte order, returns
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

## Worker boundary and execution limits

The internal Rust HTTP listener rejects non-loopback bind addresses, unexpected
Host headers, and requests carrying Origin or Sec-Fetch-Site. Mutations require
`application/json`. Go constructs fresh upstream requests; it does not forward
browser context headers. Direct CLI probes may omit browser headers. This is a
transport boundary for disposable development, not owner authentication.

The worker uses the asynchronous MongoDB driver and Axum on two runtime threads,
with at most 16 active requests, eight in-flight storage operations, and eight
database pool connections. Excess HTTP requests receive 503 `worker_busy`; request bodies are limited to 4096 bytes and
three seconds. Waiting for a storage result has a four-second deadline; all
admitted HTTP handlers, including reads and health, have a five-second deadline. Database
connection and server-selection timeouts are two seconds. Timeout responses do
not prove that a write was rejected: reconcile an uncertain outcome using the
same operation ID. MongoDB 2.x driver futures run to completion in separate tasks;
a timed-out caller releases its HTTP handler but the storage task retains its capacity permit
until completion. This avoids unsafe driver cancellation and unbounded abandoned
work. No further mutation command is started after its deadline, but a command
already sent may still commit. A fully stalled pool rejects further storage work
promptly until the dependency recovers; deadlines do not forcibly cancel server
work. These are development limits, not measured product SLOs.

The default full verification suite now launches independent workers against
real FerretDB and checks concurrency, direct access rejection, stalled bodies,
database transport stalls/recovery, and Go-to-Rust behavior with search down.
See [required retesting](TESTING.md#required-retesting-after-task-path-changes).

Local regression evidence (2026-09-29): the full verification suite and pure
suite passed on Windows/Podman with PostgreSQL 16.13, FerretDB 1.24.2, NATS
2.10.26, and OpenSearch 2.19.1. The full run included all 48 cross-process
contention pairs, worker boundary checks, stalled body and database recovery,
and the Go-to-Rust search-outage case. SP-001 C01-C06 were not rerun: their
Python adapters, mapping, CDC configuration, and projection were unchanged.
