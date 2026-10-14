# Native PostgreSQL foundation

This is the fresh version-one storage baseline for #26/B-005, following
[ADR-102](../../docs/architecture/ADR-102-postgresql-transactions.md).
It provides permanent operation identity, expiring replay payloads, attributed
project/configuration/history rows and a restricted creation capability.
It does not provide an authenticated native application route or migrate the
existing FerretDB task path. See the [implementation plan](../../docs/implementation/issue-26-postgresql-foundation.md).

## Installation and ownership

Use PostgreSQL 16 with UTF8 encoding in a fresh dedicated database, with normal
durability settings enabled. From the repository root, run
`psql -X -v ON_ERROR_STOP=1 -f storage/postgresql/install.sql`
as a deployment administrator able to create roles and assume the new owner role.
The installer and all three SQL fragments must remain together. It is tested
using the database superuser; the serving application must never use that account.

Installation is one transaction, including cluster-wide role creation. It fails
on an existing `kehila` schema or either capability role name, unsupported major
version or encoding. Failure rolls back all changes. Reinstallation is rejected;
this is not an upgrade or repair command. Do not drop populated schemas to retry.
Future upgrades require new reviewed migrations and real-data backup/restore
qualification. The two fixed role names require a dedicated role namespace in
the PostgreSQL cluster; this installer cannot provision multiple independent
Kehila databases in the same cluster without a later role-provisioning design.

`kehila_owner` is NOLOGIN and owns the schema, tables and functions.
`kehila_creator` is a separate NOLOGIN capability role with creation INSERT/SELECT,
sequence USAGE, actor/schema-version SELECT and two restricted locking functions. An administrator
must provision a backend login separately and grant only this capability.
The installer creates no login, credentials or actor records. The creator cannot
administer actors, update/delete grants or replay storage, retire operations,
change history or create objects in kehila. PUBLIC has no schema usage or function
execution in kehila. Database-level CONNECT/TEMP privileges and other schemas
remain deployment configuration. SECURITY DEFINER functions use a fixed trusted
search path.

The capability can read `schema_migrations` for future startup compatibility
checks. It cannot INSERT, UPDATE, DELETE or TRUNCATE migration records. No
application startup check or migration runner is introduced by this grant.

The backend capability is trusted to persist accepted results. It is not an
end-user permission boundary: actor authentication, permission evaluation and
stable creation-target allocation remain #40/#41 prerequisites. No compactor
capability is granted. An owner can retire payloads structurally, but expiry
authorization, trusted clock behavior and bounded compaction remain adapter work.

## Integrity and representations

`operations.sql` owns scalar domains, typed targets, scoped uniqueness and the
permanent-core/payload guards. A new operation must have one payload at commit;
retirement atomically changes the core flag and removes that payload, in that
order. Identity and retirement are irreversible through normal DML. Every
payload UPDATE is rejected, including content changes and no-op updates. DELETE
requires a retired core; reinsertion after retirement fails the deferred shape
check. These guards prevent privileged DML from replacing accepted payloads
without disabling a guard. TRUNCATE is rejected.

`project_create.sql` owns current relational configuration, checked operation
attribution and deferred membership/current-revision foreign keys.
`access.sql` owns capability grants, locking functions and append-only history
guards. Owners can alter or disable guards; these protect normal DML and mistakes,
not an administrator with schema-changing privileges.

Opaque IDs use C collation, no normalization/trimming, and a 128-byte UTF8 bound.
Tokens use ASCII graphic characters. `u64` is checked integral numeric, avoiding
fractional rounding. These are new native storage rules: align domain/interface
validation and inspect legacy values before exposing native commands or importing
data. Request/result/snapshot columns carry version tags, but SQL does not prove
codec contents, exact M1 seed equality or current/history consistency. The
production adapter must use the single trusted result, reconstruct it inside
the transaction, dispatch stored codecs and preserve the 90-day replay policy.

Primary/unique indexes and the actor-first grant/expiry indexes are installed.
Additional referencing-side indexes listed in the [seed specification](../../docs/architecture/M1-project-create-seed.md)
are deferred until the later mutation/deletion/query paths establish their need.
No load, recovery, backup/restore or production throughput qualification is claimed.

## Disposable native verification

The verifier uses Podman, Python 3.10+, Cargo/Rust and the container's `psql`. It supports
Windows and Linux hosts. Ensure at least 3 GiB available host memory after Podman
starts and 2 GiB free disk. Pull the pinned image if it is not already available.
Use a unique container name; the example name is explicitly disposable.
In PowerShell or a Linux shell, run these as individual commands:

```text
podman run -d --name kehila-foundation-test --label purpose=kehila-postgresql-test --network=none --memory=256m --cpus=1 --pids-limit=128 --tmpfs /var/lib/postgresql/data:rw,size=256m -e POSTGRES_HOST_AUTH_METHOD=trust docker.io/library/postgres@sha256:0ea6700a3b4f0ae6ce746519073558aed4d88a79d8d07622a9a644946c7319c4 postgres -c listen_addresses=
podman exec kehila-foundation-test pg_isready -U postgres
python scripts/check_postgresql.py --disposable-container kehila-foundation-test
podman rm -f kehila-foundation-test
```

Wait for `pg_isready` to succeed before running the verifier. When choosing an
explicit Podman connection, add `--connection CONNECTION` to each Podman command
and `--podman-connection CONNECTION` to the Python command. Do not mix connections.
Socket trust is restricted to this networkless disposable container; it is not a
deployment authentication configuration. Never point the verifier at a real database.

The verifier checks isolation/resource limits before writing, rejects existing
schemas/roles, injects a pre-COMMIT installation failure and checks full rollback,
installs the baseline, rejects reinstallation, then runs scalar/core/creation and
serving-permission cases. A separate backend connection checks role separation.
It temporarily disables the update and retirement-deletion guards in separate
transactions: content-update, payload-replacement and identity regressions must
detect their corresponding missing guard, then pass after rollback restores it. SQL failures
are fatal; expected failures check SQLSTATE and named constraints where applicable.
Expected verifier-level rejections require psql's script-error exit status 3 and
the actual verbose ERROR SQLSTATE. Optional message checks use the primary error
line. Container/runtime/connection failures and matching text in error context
cannot substitute for a PostgreSQL rejection. The client diagnostic locale is C.
The caller owns container cleanup even when verification fails. Retest with a
fresh container; the verifier never resets an existing schema or drops roles.

The same command builds an owned host-side production codec helper and verifies
recorded project/configuration snapshots through real JSONB. Standalone fixtures
cover zero through u64::MAX, exact ordered reconstruction, escapes and empty
collections. Real positive revision rows exercise project/operation attribution;
negative cases reject zero revisions, wrong attribution and pair incoherence.
Boundary cases verify exact 1 MiB JSONB text output, one-byte formatting overflow
and NUL incompatibility. Process I/O remains outside the pure codec crate; its
new-write admission reserves JSONB formatting space and rejects NUL while
historical reconstruction retains its existing representation contract. See the
[W1 report](../../docs/implementation/m1-01-project-snapshots.md) for evidence and
limits. The narrow relational fixtures do not implement the trusted seed adapter.

This opt-in native check complements `python scripts/verify.py`; the shared
matrix does not start this container or execute the native SQL cases. Frozen
SP-002/SP-003 experiments remain separately reproducible and unchanged.

## Invariant and evidence map

| Invariant | Enforcement | Native verification |
| --- | --- | --- |
| No partial installation | Single transaction in install.sql | Inject failure after DDL; assert schema and roles absent |
| Exact scalar/key bounds | ident, u64 and operation_token domains | Empty/oversized/multibyte IDs, NUL, fractional/overflow u64, maximum creation key |
| Permanent scoped request identity | operations unique key and core guards | Duplicate key, UPDATE/DELETE/TRUNCATE, fresh retired core rejection |
| One full payload or none when retired | Deferred shape triggers and identity guard | Missing/moved/deleted payload, atomic retirement, guard-removal mutation |
| Accepted payload content cannot be replaced | UPDATE rejection and retirement-only DELETE | Owner content/version/no-op updates, DELETE/INSERT replacement, retirement rollback, reinsertion after retirement, guard-removal mutations |
| Exact 90-day stored interval | operations_replay_window | Accept valid interval; reject altered interval; clock enforcement remains adapter work |
| Attributed configuration and ownership | Composite FKs and deferred membership/revision FKs | Wrong operation attribution, missing membership/revision, wrong title owner |
| Append-only creation history | History DML/TRUNCATE triggers and creator ACLs | Owner mutation rejection and serving denial |
| Separate serving authority | Role attributes, ACLs, restricted lock functions | Permitted creation/locks, forbidden administration/DDL, separate login and owner escalation denial |
