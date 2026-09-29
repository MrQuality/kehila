# YAJA — Project and task management

![License: Apache 2.0](https://img.shields.io/badge/license-Apache%202.0-blue)
![Stage: early development](https://img.shields.io/badge/stage-early%20development-orange)

YAJA is an open-source project management system in early development, designed
around custom fields and real-time updates. This repository currently contains
a Rust query parser, NATS connection checks, and shared synchronization types.
There is no runnable web application yet.

## Current components

| Component | Available today |
| --- | --- |
| Rust query parser | Parses a single equality filter, such as `status = 'Open'` |
| Rust NATS connection | Connects to a broker and checks protocol round trips |
| Go synchronization rules | Checks whether a query can run at the current schema version |
| TypeScript contracts | Shared type declarations; no JavaScript runtime |
| Development services | PostgreSQL, FerretDB, OpenSearch, and NATS in Compose |

See [implementation status](docs/IMPLEMENTATION.md) for limitations and planned work.

## Product planning

The [product plan](docs/product/README.md) records the agreed
requirements for local task tracking, shared resources, scheduling, and costs.
It includes decisions, open questions, and a requirement-linked implementation
backlog. Planned capabilities are not claims of currently available features.

## Development setup

Install Python 3.10+, Git, Rust stable, Go 1.22+, and Podman or Docker with a
Compose provider. Allow about 4 GiB of memory and ports 5432, 27017, 4222, 8222,
and 9200 for the development services. Initial image pulls need Internet access.
No third-party Python packages are required. npm/pnpm is optional for the
declarations-only JavaScript workspace.

From a checkout:

```text
python scripts/setup_env.py --start
python scripts/verify.py
```

On Windows with Podman, start the VM first using `podman machine start`.
OpenSearch requires `vm.max_map_count` of at least 262144 on the Linux host or
VM. CI configures this in its disposable runner.

For unit tests without containers:

```text
python scripts/verify.py --pure
```

Inspect or stop the services with your chosen container engine:

```text
podman compose -f docker-compose.yml logs
podman compose -f docker-compose.yml down
```

Use `docker compose` if you started the stack with Docker. Development data is
ephemeral and is lost when containers are removed. Read [SECURITY.md](SECURITY.md)
before changing network exposure or using real data.

## Parser example

The `yaja_query` crate accepts a single equality expression:

```rust
use yaja_query::parse_filter;

let filter = parse_filter("status = 'Open'").unwrap();
assert_eq!(filter.field, "status");
assert_eq!(filter.value, "Open");
```

Identifiers use ASCII letters, digits, and underscores and cannot start with a
digit. Values are nonempty single-quoted strings. Compound expressions and
escaped quotes are not supported yet.

This is YAJA's own filter grammar. Jira Query Language (JQL) compatibility is
not a current feature or requirement. See the [naming and third-party reference
policy](docs/BRANDING.md) for terminology and compatibility claims.

## Planned architecture

The planned application uses a Go API, Rust workers, PostgreSQL through FerretDB,
NATS for events, and OpenSearch for search. A shared Rust compiler will support
server queries and browser-side evaluation. The API, UI, change-data-capture
pipeline, and full query compiler remain to be implemented.

The [v0.2 design](docs/reference/YAJA-v0.2.md) describes the target architecture
and proposed contracts. It is a design reference, not a list of shipped features.
The current development stack uses FerretDB 1.24.2 with PostgreSQL 16; a deployment
requires a separate review of dependencies, authentication, and storage choices.

## Contributing

Use the [project procedure index](docs/procedures/README.md) to find SOPs,
their approval status, and their enforcement coverage.

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution requirements and
[docs/TESTING.md](docs/TESTING.md) for test commands and the
[test strategy](docs/TEST_STRATEGY.md) for the longer-term quality approach.
Report vulnerabilities as
described in [SECURITY.md](SECURITY.md).

Code is licensed under Apache-2.0. The Code of Conduct retains its upstream
Contributor Covenant attribution.

YAJA is an independent project and is not affiliated with, sponsored by, or
endorsed by Atlassian. Jira is a trademark of Atlassian.
