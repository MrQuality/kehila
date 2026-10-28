# Disposable Linux PostgreSQL bridge

Status: native transport subset observed on 2026-10-28; complete owner-login
integration remains pending under [the delivery contract](local-owner-authentication-delivery.md).
This is part of [A1](local-owner-authentication-a1.md), within the existing
AUTH-001 experiment. No production schema, worker dependency or supported
installation is changed.

## Ownership and authentication

`src/database.rs` is Linux-only. It owns one dedicated authority connection,
one exclusive serving connection and both independently polled driver tasks.
There is no pool, queue, reconnect or raw serving-client escape. Admission uses
one irreversible runtime-liveness latch. Driver exit, panic or unfinished read
closes it. The application coordinator must eventually share that same latch
with the session and pre-session registries.

The endpoint admits one private/loopback IPv4 address, an explicit root and the
fixed TLS name `auth-db`. TLS 1.3 uses ordinary rustls chain/name verification.
The synthetic server profile requires SCRAM-SHA-256 for the narrowly privileged
runtime login; this is a server admission rule, not a client API that rejects
every other PostgreSQL authentication method. No system trust store is used.

The existing `prepare` example accepts `--database` for a separate disposable
CA and `auth-db` server certificate, with explicit non-CA/server-authentication
markers. Its output directory must already exist outside the checkout with
restricted access; existing files are never overwritten. It does not install
trust. The signing key stays outside the server fixture; delete generated private
keys during fixture cleanup. This helper is not a supported certificate installer.

The dedicated connection acquires the exact session advisory lock once and
commits `start_runtime` at READ COMMITTED before admission. Startup IDs are
prepared before connection/lock work. Unknown commit closes the runtime without
automatic retry. Generation values are read as canonical decimal strings and
checked as full-width `u64`, without signed narrowing.

A credential read requires exclusive database ownership. Dropping or timing out
that read stops admission; it does not make the connection reusable. Normal
shutdown drops clients and joins both owned drivers, with a five-second fallback
that aborts and joins remaining tasks. Exceptional destruction requests abort;
it does not claim observed joins or completed server rollback. Driver buffers
and configuration may copy secrets; zeroizing owned inputs is not a universal
memory-erasure guarantee.

## Native observations and limits

The Linux Rust 1.93.1 build passed after correcting a fixture decoder API error:
the dependency's selected features require `decode_mut` with a caller-owned
buffer. The failed compile is retained in the verification record. An earlier
image-download attempt was stopped for insufficient host RAM before compilation.

Two opt-in native tests passed against fresh PostgreSQL 16.15 state:

- `native_lifetime` read credentials with authentication generation above signed
  64-bit range and verifier revision `u64::MAX`, observed administrator-role
  denial, then stopped admission after the exact advisory-holding backend was
  terminated. The remaining driver joined without abort.
- `native_cancelled_read` waited until PostgreSQL reported the actual credential
  query blocked on a relation lock. Abandoning that future closed admission and
  denied another read. After releasing the fixture lock, both drivers joined
  without abort. Coordination files alone were not accepted as proof of a wait.

A targeted mutation removing the unfinished-read stop failed at the intended
`!database.is_live()` assertion in the second case. The mutation used a separate
source copy; repository source remained intact.

Restored Linux verification passed Clippy with warnings denied and the serial
`cargo test --locked --all-targets -- --test-threads=1` run: 114 unit tests and
20 transport tests passed. Three unit tests were ignored in that ordinary run:
the two opt-in native fixtures executed separately above, and the externally
provisioned blocklist corpus test whose earlier selected-asset result remains
recorded in [A2](local-owner-authentication-a2.md). A Clippy assertion-style
finding was corrected; initial launcher/component setup failures are retained
without counting them as successful checks.

Both cases observed two TLS 1.3 connections and advisory-lock release after
shutdown. The application test binary ran as UID/GID 65532, with a clean
environment, without PostgreSQL administrator credentials or access to the
database data directory/server private key. Local administration used separate
peer-authenticated OS identities. The container had no network or published
ports and used a 256 MiB ephemeral data filesystem. Per-container CPU/memory
limit enforcement remains unqualified.

These are protocol/ownership observations inside a disposable database container,
not qualification of separate application-container topology, secret mounts,
Windows forwarding or browser login. Ordinary HTTP cancellation must leave owned
application jobs alive until actual completion; exposing the low-level read's
stop-on-drop behavior directly to HTTP callers would let cancellation force a
runtime restart. The bounded coordinator, fenced writes, secret publication,
browser/CLI integration and full T01–T16 cases remain unfinished.

[Dependency review](../../experiments/AUTH-001/DEPENDENCIES.md#linux-postgresql-bridge-selection-2026-10-28)
records the pins, grants and advisory scope. Actual Linux compiler artifacts
contain all 37 selected additions and none of the 21 inactive additions. This
does not clear those inactive packages for other platforms or source
redistribution. Rust 1.88 execution remains unqualified.
