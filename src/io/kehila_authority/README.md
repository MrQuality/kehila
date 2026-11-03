# Kehila product authority

Implementation in progress for the
[authenticated creation contract](../../../docs/implementation/authenticated-project-authority.md).
This Linux backend owns authentication and native project operations; Windows
is the intended client platform. Source presence is not integrated qualification.

## Ownership and adoption map

The initial authentication ownership units derive from the reviewed AUTH-001
foundation at `3ad275d`, with their relevant regressions. The experiment remains
unchanged. This package has its own composition root and product SQL namespace;
it imports no experiment source and exposes no diagnostic probe server.

| Product unit | Responsibility |
| --- | --- |
| `password`, `secret`, `session`, `pre_session` | Bounded credential work, secret handling and volatile session ownership |
| `coordinator`, `lifecycle` | Owned admission, blocking jobs, registry/database lifetimes and irreversible stop |
| `postgres` | Private serving connection and dedicated authority lock; fixed fenced operations |
| `protected_file`, `bundle`, `startup` | Protected Linux assets, canonical certificate/key admission and fail-closed readiness |
| `http_auth`, `listener` | Original-byte HTTP/cookie/origin/CSRF admission and retained outbound eligibility |
| `transport` | Bounded content-free connection outcomes |
| `test_exchange` | Test-only TLS clients and exchange/shutdown regression diagnostics |
| `native_ingress` | Closed, bounded project request shapes before application/database admission |

The binary owns signal handling and awaits normal listener/coordinator shutdown.
The pure task/codec crates retain domain acceptance and versioned representation.
Runtime schema installation, actor enrollment and administrative recovery are
separate operator capabilities, not HTTP actions or automatic startup repairs.

The product package declares Rust 1.88 independently of the root workspace.
The [dependency assessment](../../../docs/implementation/project-authority-dependencies.md)
records the selected graph, licenses, notices and advisory-query limits.
No minimum-version, native database, browser, installer or real-data result follows
from the extracted unit tests. Tests marked as requiring a provisioned PostgreSQL
fixture must run with the actual product fixture before their gates can pass.

The [verification record](../../../docs/implementation/project-authority-verification.md)
separates component results, actual database/HTTP/TLS observations and remaining
delivery gates. Node page tests use a simulated DOM; they establish neither
Windows browser behavior nor native database persistence.

The interface saves server-owned creation requests before offering explicit
execution. It discovers requests after sign-in and keeps interrupted creation or
cancellation identity through discovery/reopening. It uses no browser persistent
storage for recovery. See the [pending/API contract](../../../docs/implementation/project-pending-contract.md).
