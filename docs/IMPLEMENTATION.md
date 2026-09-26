# Implementation status

YAJA is in early development. The current components are:

| Component | Implemented behavior | Verification |
| --- | --- | --- |
| Rust query parser | Single `identifier = 'nonempty value'` expression; rejects trailing clauses | Unit tests |
| Rust NATS connection | Bounded TCP connection, INFO/CONNECT handshake, PING/PONG | Live integration test |
| Python NATS check | JetStream availability and account API response | Live integration check |
| Development services | Four TCP ports and OpenSearch HTTP health | Readiness checks |
| Go synchronization rules | Provisional evaluation requires matching schema version and a pure query | Table-driven unit test |
| TypeScript contracts | Shared declarations | No runtime implementation |
| Development tools | Staged-source verification and CI base selection | Temporary Git repository tests |

The isolated [SP-001 experiment](spikes/SP-001-task-path.md) also exercised a
small task save/CDC/search path with Python boundary adapters on Windows/Podman.
Its six cases passed with constrained memory after configuring PostgreSQL replica
identity and a task-only publication. This prototype is retained for reproduction;
it is not the supported Go/Rust application or an accepted production contract.

## Current limitations

The Rust NATS component is a synchronous connection probe. Publishing,
acknowledgment, and replay are not implemented. Rust currently has no external
crate dependencies.

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
- Typed database mutations and authoritative schema state.
- Sagas, durable CDC delivery, and idempotent search projections.
- Go authentication, API routes, and SSE delivery.
- React UI, optimistic state, and reconciliation.
- Recovery and crash-isolation integration tests.

The [v0.2 design](reference/YAJA-v0.2.md) defines proposed contracts. Its
`CDC_PIPELINE_STALLED` entry lists both HTTP 503 and a 200 response with a null
token; this remains an open API decision.
