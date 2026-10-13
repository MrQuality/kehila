# Native PostgreSQL foundation: local implementation plan

Status: Foundation prepared for PR review, 2026-10-11. Related: #26/B-005, #39, #40, #41.
Base: main at 12871228bae3a1b6a45b08f188741a18631ac5d7.
The foundation is a bounded storage slice, not completion of M1-01. GitHub #26
is closed despite outstanding production deliverables; its closure is not
implementation evidence. #39 remains open. Remaining work is listed below.

## Pre-Execution Plan

1. **Goal & Context:** Turn the reviewed project-creation schema into a versioned,
   atomic native PostgreSQL foundation with explicit ownership and service-role
   privileges. Install the corrected payload identity guard. Record selected
   representation choices without claiming an authenticated creation route.
2. **Definition of Done:**
   - [x] Record native version-one codec/identifier choices and remaining alignment obligations.
   - [x] Provide executable installation SQL for operations, creation/configuration/history and restricted service roles.
   - [x] Installation is transactional and rejects unsupported servers, encoding, existing schemas or colliding roles rather than silently adopting them.
   - [x] Real database checks cover install/reinstall/failure rollback, scalar/target constraints, payload identity/shape, irreversible retirement, history protection and serving-role privileges.
   - [x] Required staged checks and an evidence-based review pass; documentation distinguishes completed foundation from outstanding adapters/routes and native Linux verification.
3. **Dependencies & Prerequisites:** PostgreSQL 16 UTF8, a fresh dedicated database
   and deployment administrator, Python 3.10+, existing toolchain and Podman only
   for local services. No new Python driver or external package is required.
   Native checks retain 3 GiB available host memory and bounded container resources.
4. **Risks & Mitigations:** Separate NOLOGIN ownership/service roles; no credentials
   or actor fixtures in installation. Fix SECURITY DEFINER search_path and revoke
   PUBLIC execution. Unique SQL ownership and transactional installation prevent
   partial state. Never migrate/delete legacy FerretDB data or rewrite spike evidence.
   Test actual SQLSTATEs and committed state, including privileged reassignment.
5. **Spikes & Open Questions:** No further engine spike. #40/#41 remain route
   dependencies; trusted clock, compaction, duplicate-key ingress, production
   codecs/adapter and coordinated native domain/interface/import validation remain
   delivery obligations. The identifier choice applies to new native persistence;
   legacy acceptance and historical reconstruction must not be silently narrowed.
6. **High-Level Architecture / File Changes:** storage/postgresql/ owns install SQL
   and ordered schema fragments. tests/integration/ owns native acceptance SQL;
   a Podman verification command uses isolated labelled resources and no networking.
   Architecture/implementation/testing docs record installation and scope.
7. **Verification & Testing Plan:** Write native cases before SQL and retain their
   expected failure. Run real PostgreSQL install/negative/role tests, including a
   mutation that removes the payload guard and must fail the regression. Verify
   rollback leaves no schema/roles after injected installation failure. Run staged
   verification with development services, inspect final changes and preserve local
   evidence. Stop the runtime when finished. No hosted CI is claimed for local work.

## Selected representation and enforcement boundary

The physical foundation is recorded in [ADR-103](../architecture/ADR-103-native-creation-foundation.md),
proposed for production adoption pending review of this local implementation.
The maintainer authorized this local progression on 2026-10-10 after SP-003 review.
Use version-one domain-separated binary request bytes with SHA-256; use separate
versioned JSONB saved-result/configuration envelopes and canonical decimal strings
for u64 fields. Stored-version dispatch and direct hashing dependency ownership
must be implemented before promoting the prototype into the production adapter.

New native opaque identifiers are nonempty UTF8, at most 128 bytes, without NUL;
identity uses C collation with no normalization or trimming. Operation tokens keep
the existing ASCII-graphic rule. This is a native storage contract, not permission
to silently change public String-backed wrappers or import existing values.
Authoritative native domain/interface/import enforcement must be aligned before
any supported native command path is exposed. Do not validate historical decoding
against today's mutable domain rules. Preserve the exact 90-day recorded-time
replay policy with no grace or post-commit adjustment.

## Delivery boundaries

Foundation installation provides structural integrity and database capability
separation. It does not prove a trusted seed, permission mapping or exact replay
reconstruction by an application. Those require the production codec/adapter and
#40/#41 integrations. Creation-only guards remain explicit until later migrations
supply replacement invariants. No serving compaction/revocation path is granted.

## Local result and next slice

The [executable baseline and reproduction guide](../../storage/postgresql/README.md)
install schema version one atomically with separate NOLOGIN ownership and
creation-capability roles. Native verification passed on PostgreSQL 16.15 in a
bounded networkless Linux container on Windows. An injected late installation
failure left no schema or capability roles. Reinstallation was rejected. Tests
checked scalar/UTF8 limits, maximum supported creation-key widths, replay-window
shape, operation attribution, deferred membership, immutable history, restricted
serving DML and ownership separation. Disabling the payload identity guard caused
the regression to fail; rollback restored the guard and the regression passed.
Required staged full-suite checks passed for the implementation commits.

One verification attempt failed in the existing Go/Rust task-path test when a
process exited before API readiness. Its standalone diagnostic rerun passed;
the cause was not established. No hook or integration assertion was bypassed.
Treat this as an existing-suite diagnostic/reliability concern for follow-up,
separate from the passing native database cases.

Self-review covered correctness, readability, architecture, security and bounded
resource behavior. No new runtime dependency or public route was added. Evidence
is local; no independent approval, hosted CI or native Linux-host execution is
claimed. Fixed cluster-wide role names deliberately restrict this fresh installer
to an unused role namespace; upgrading populated databases remains separate work.

Next implement the production typed storage codec and internal project-creation
adapter, including domain/interface storage-bound alignment and reconstruction
from the single trusted result. Then integrate trusted actor/authority (#40),
stable target allocation (#41), replay clock/retry handling and native end-to-end
tests before exposing a route. The legacy task path remains separate. This PR
publishes only the foundation; it does not complete #26/#39 or change their
tracking status.

## CR-01 verifier follow-up

Expected verifier rejections now require psql script-error exit status 3 and an
exact SQLSTATE in the verbose primary error diagnostic. Optional message checks
cannot match context text; the client diagnostic locale is fixed to C. Nine
verifier unit tests passed, and in-memory inversions of the exit-status and
SQLSTATE comparisons were detected. Fresh native PostgreSQL verification passed
with the stricter parser, including installation rollback, reinstallation,
permission denial and the identity-guard mutation. A real missing-container error
whose name contained 42501 was correctly rejected as infrastructure failure.
The follow-up review found no additional actionable issue. Changes do not alter
the database schema or application behavior.

## Publication verification (2026-10-11)

Fresh PostgreSQL 16.15 verification passed on the existing rootful Podman
connection with the documented network isolation and resource ceilings. It
repeated installation failure/rollback, reinstall rejection, scalar/core/creation
integrity, separate serving-role checks and payload-identity mutation detection.
All nine verifier unit tests passed. The first rootless container startup failed
before PostgreSQL ran because the runtime could not apply its memory cgroup
limit; no verifier safety gate or resource limit was relaxed.

Full `python scripts/verify.py` verification passed on the publication branch:
Python tooling regressions, naming/engineering checks, Go formatting/vet,
service readiness/NATS checks, workspace Rust and Go tests, SP-002/SP-003 pure
oracles, and the live two-worker task-path contention/deadline/recovery cases.
Two POSIX-only Python fixtures were skipped on Windows. This shared result is
separate from the native PostgreSQL suite above; neither establishes a supported
native application route or production qualification.

## Payload and schema-version review follow-ups (2026-10-12)

All payload updates are rejected, and deletion requires setting the irreversible
core retirement flag first. Deferred final-state checks continue to reject
payload reinsertion after retirement. Owner-level regressions cover content,
codec-version and no-op updates, DELETE/INSERT replacement, retirement rollback
and reinsertion. The creation capability has read-only schema-version access;
migration INSERT/UPDATE/DELETE/TRUNCATE remain denied. No startup compatibility
check or compactor is introduced.

The PostgreSQL 16 native suite passed against the documented pinned image in an
isolated, resource-bounded disposable container on Windows through Podman. The
new regressions first failed against the earlier baseline: a payload content
update was accepted, and schema-version reads were denied. With the changes,
separate update/deletion guard-removal mutations were detected, and the suite
passed again after rollback restored each guard. Installation rollback,
reinstallation rejection, scalar/core/creation integrity and serving-role
separation also passed. The historical SP-002 guard and experiment remain intact.

Full shared verification passed independently for both staged code commits:
105 Python tests run (103 passed, two POSIX-only fixtures skipped on Windows),
naming/engineering checks, Go formatting/vet, service readiness/NATS, workspace
Rust/Go tests, SP-002/SP-003 pure oracles and live task-path integration. The live
checks included 48 competing request pairs, deadline and stalled-database
recovery, plus save/replay/read with search unavailable.

These results do not establish a supported native application route, native
Linux-host PostgreSQL execution or production qualification. Adapter,
trusted-clock, compaction and #40/#41 prerequisites remain as described above.

## Codec follow-up (#47)

The next bounded slice now provides pure version-one request/fingerprint and
saved-result/configuration codecs plus checked native request admission. See the
[codec implementation handoff](project-create-production-codecs.md) for tests,
resource ceilings and limitations. ADR-104 was Accepted by explicit maintainer
approval on 2026-10-13; its downstream contracts remain mandatory. The native database
adapter, project-only snapshot envelope, authority/allocation, clock/retry and
compaction obligations above remain open; no supported route is established.
