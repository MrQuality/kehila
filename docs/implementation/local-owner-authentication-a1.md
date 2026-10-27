# A1: disposable owner storage and authority fencing

Status: reviewed disposable implementation contract, 2026-10-27. The
[enrollment subset](../../experiments/AUTH-001/storage/README.md) now has SQL,
separate capability roles and native enrollment/rights/constraint observations.
Runtime startup and post-wait takeover fencing also have disposable SQL/native
oracles. Recovery, credential-generation CAS and session-management SQL and their
native proof remain pending. This is an
internal task of the [owner-login delivery](local-owner-authentication-delivery.md),
not a separate production migration or completed authentication capability.
[ADR-105](../architecture/ADR-105-local-owner-authentication.md) already accepts
the owner, generation, authority and administrative boundaries. This contract
makes their PostgreSQL realization reviewable before stateful implementation.

## Outcome and boundary

An operator provisions one disposable owner, a single runtime starts, and narrow
serving functions support credential/session management while stale runtimes and
stale password proofs fail. Recovering a password preserves the actor, grants and
history. Browser clients receive no database capability or actor-selection input.

Implement an `auth001` overlay in a fresh, dedicated disposable PostgreSQL 16/UTF8
database after the existing native foundation installer. Preserve that installer,
its roles, codecs and migrations. Do not apply the overlay to the anonymous
Go/FerretDB prototype or an existing installation. Its SQL is not a supported
upgrade path, ADR-103 adoption or A5 restore implementation.

## Durable records

| Record | Exact role and constraints |
| --- | --- |
| `auth001.installation` | Exactly one pre-created row, keyed by a boolean constrained to true. Installation/owner/enrollment binding fields are either all null or all non-null; runtime generation starts at zero. Enrollment sets all three bindings atomically; they cannot subsequently change. The owner uniquely references `kehila.actors`. Runtime generation uses checked `kehila.u64`; takeover increments it and records a fresh non-secret startup-attempt ID. |
| `auth001.owner_credentials` | One row with a boolean singleton key and a foreign key to the unique installed owner, preventing a second credential row or different actor binding. Version-one policy, bounded ASCII PHC verifier (at most 512 bytes), positive checked auth generation and verifier revision. A null verifier means credentials blocked; it never enables anonymous or empty-password login. Runtime code must validate PHC algorithm/parameters before hashing; SQL bounds are not cryptographic validation. |
| `auth001.management_events` | Append-only non-secret events for enrollment, runtime start, issuance, logout, individual-session revocation, password change, global revocation and recovery. Stable identity is `(runtime_generation, public_id, event_kind)`; for issuance/logout/individual revocation, `public_id` is the affected public session ID. Individual revocation records its initiating public session ID separately. Record installation, subject owner, auth generation/revision, browser/CLI kind where applicable, and a function-defined initiator kind (`local_admin`, `runtime`, `owner_session`). Timestamps describe history, not credential validity. |

Credential state is separate from the installation row. Normal management holds
the installation row in shared mode, then updates credentials under the actor
fence; it must never upgrade its installation lock while holding an actor lock.
The blocked-verifier representation leaves room for reviewed restore invalidation,
but no restore coordinator or supported backup/reset operation is implemented here.

Use `kehila.ident` for stable IDs and explicitly bounded ASCII public event IDs.
IDs are not passwords, credentials, digests or proof of authorization. Every
generation/revision increment rejects the maximum value instead of wrapping.
Replacing a verifier increments both auth generation and verifier revision;
generation-wide revocation increments auth generation only. Non-session events
use a fresh, stable per-attempt public ID rather than reusing a session ID.

## Database capabilities

The existing `kehila_owner` NOLOGIN object owner owns the overlay. Two new
NOLOGIN, non-superuser capability roles have no object ownership, role creation,
database creation, replication or row-security bypass:

| Capability | Permitted functions | Explicitly excluded |
| --- | --- | --- |
| `auth001_runtime` | Load the installed credential snapshot; start a runtime on the dedicated lock connection; acquire owner fences; append/confirm issuance or logout audit; change the verifier or revoke generations after the required proof/CAS. | Enrollment, recovery, arbitrary actor mutation, direct auth-table DML, direct business operation writes, object ownership and membership in the administrative capability. |
| `auth001_admin` | Initial enrollment/reconciliation and actor-preserving credential recovery, with attributable local-administration events. | Issuing browser/CLI sessions, returning bearer/CSRF secrets, public HTTP recovery and implicit grant/actor reactivation. |

Fixture bootstrap attaches these capabilities to different restricted LOGIN roles;
credentials and deployment transport are separately protected test inputs. The
serving process never loads an administrator login. The runtime SQL principal is
a trusted backend capability, not an end-user identity; SQL cannot replace Rust
password/session verification. Backend or database administrator compromise is
outside this password-containment boundary.

Revoke PUBLIC schema/function rights in the installation transaction. Definer
functions use a fixed trusted search path (`pg_catalog`, trusted schemas, then
`pg_temp`), fully qualified objects and no dynamic SQL. No caller has CREATE in
those schemas. Do not add the runtime to `kehila_creator` merely to reuse an actor
lock: the definer fence helper locks only the installed owner itself. A4 will
review additional command/read capabilities independently.

## Lock and snapshot protocol

1. Require READ COMMITTED for the serving transaction.
2. Acquire the singleton installation row `FOR SHARE` for normal issuance,
   logout and credential management, or `FOR UPDATE` for enrollment/takeover.
3. Acquire the installed owner actor `FOR NO KEY UPDATE`. Future multi-actor
   operations retain ADR-105's complete `COLLATE "C"` ordering; this single-owner
   overlay does not add an arbitrary-actor serving API.
4. After waiting, read credential generation/revision, active actor and runtime
   generation using a fresh statement. Definer helpers are VOLATILE; they are
   not stable-snapshot authority caches. The Rust transaction also rechecks its
   live registry reference/monotonic expiry after acquiring the fences.
5. Keep the SQL locks until commit/rollback. Hashing and network search occur
   outside them. Retrying starts a fresh transaction and repeats all checks.

An early verifier read permits bounded password work only. It cannot authorize
session issuance or password replacement. Those paths must compare the exact
verified revision and auth/runtime generations after the fences.

## Enrollment, startup and uncertain commits

**Enrollment:** lock the existing singleton row exclusively before creating the
actor. If unbound, create one new active actor with initial creation permission,
bind the installation/owner/enrollment-attempt IDs, insert initial credentials
and one event in the same transaction. Never adopt an arbitrary existing actor
as a new owner. Competing enrollment cannot leave a second actor or overwrite
the first verifier. Reconciliation with the same three IDs returns non-secret
committed metadata; different IDs conflict. Reconciliation does not replace a
verifier or treat a newly supplied password as already installed.

**Startup:** the runtime owns one dedicated PostgreSQL connection for its entire
lifetime, separate from serving transactions. Acquire the session advisory key
`(1262831689, 1)` once (the experimental `KEHI` namespace, one installation per
dedicated database). No actor lock is held while acquiring it. The startup
function verifies that the calling backend holds that exact advisory lock,
then takes the exclusive installation fence and owner actor fence, increments
the durable runtime generation and appends a fresh startup-attempt event.
Admission begins only after confirmed commit and a still-owned lock connection.
Repeated calls cannot silently reuse a completed startup attempt.

**Uncertain startup commit:** never guess whether takeover rolled back. Stop and
close the owned connection; retry with a fresh connection, advisory acquisition,
attempt ID and generation advance. A committed but unused generation may be
skipped. If the old backend still retains its session lock, the replacement
cannot start; it waits for verified cleanup rather than stealing authority.
No old sessions are reconstructed. Observed lock loss or uncertain ownership
stops admission/output; reacquisition is a new runtime, not a continuation.

**Recovery:** the separate administrator takes shared installation then owner
actor/credential locks, changes only credential state and increments auth
generation/revision with a distinct administrative event. Preserve runtime
generation, actor ID, actor activity, grants and business history. A disabled
actor does not become active merely because its password was recovered.

## Serving transactions and audit

| Operation | Required post-fence action |
| --- | --- |
| Session issuance | Compare verified revision/auth/runtime generations; reserve a bounded unpublished registry entry and append issuance audit. Reveal the secret only after confirmed commit. On error/unknown commit, revoke/remove the unpublished entry without delivering it; a committed unused event is permissible. |
| Current-session logout | Revalidate the eligible registry reference and generations. Mark that same entry revoked under the actor fence before attempting the audit. Insert or confirm the fixed `(runtime_generation, public_session_id, logout)` event. A failed/unknown audit never restores the entry; return 503 and allow only eligible same-logout retries. |
| Password change | Complete verification/replacement hashing under one permit outside SQL fences. Compare the exact old verifier revision/generations, increment auth generation/revision, replace the verifier and append audit atomically. No automatic stale-proof rehash/retry. |
| Revoke other sessions | Verify the current password outside fences, then compare its exact verifier revision and auth/runtime generations after the owner fence. Apply individual revocation to each other live registry entry; retain the current entry unchanged. This is distinct from generation-wide revocation: do not change the durable auth generation, verifier or current entry's generation binding. Each newly revoked entry has a fixed per-session revocation audit identity and the initiating public session ID. |

Audit duplicate identities must match all fixed non-secret attributes, excluding
the original event timestamp. Mismatch is integrity failure, never deduplication
success. Audit is separate from business replay; never store passwords, PHC
verifiers, bearer values/digests or CSRF values in events. There is no SQL
token-to-audit lookup available to an unknown or reclaimed credential.

The registry remains volatile. Existing ADR-105 bounds govern retained revoked
entries, reclamation and audit loss. SQL persistence does not create a durable
logout retry queue or extend the retiring credential's deadline.

For revoke-other management, local revocation remains irreversible on audit
failure, as for individual revocation in ADR-105. Return success only after the
affected audits are confirmed; otherwise return 503 with the signed-out/audit-
unconfirmed distinction for the other sessions; the current entry remains active.
A retained current credential may retry one bounded
pending batch only after renewed password proof and fresh fence checks. Retain
at most 63 public target IDs and fixed non-secret audit attributes under the
initiating entry's original lifetime. Do not retain target bearer/CSRF material
or extend target lookup/retry eligibility. Generation change, expiry or replacement
cannot resurrect an entry. No durable queue,
unbounded batch cache or current-entry generation rebinding is introduced. The
exact registry implementation and failure oracles require A2 review before use.
Freeze the pending batch deadline to the initiating entry's idle/absolute limit
at batch creation; later activity cannot renew that batch. A retry confirms only
its original fixed targets/attributes, not a new target selection. Already-revoked
entries retain their first transition/event identity and initiator; another batch
cannot overwrite that provenance.

## Proof and implementation exit

Before HTTP integration, run actual isolated PostgreSQL oracles for competing
enrollment and lost enrollment responses; serving/admin rights; immutable owner
binding; generation/revision overflow; takeover waiting behind shared fences;
old-runtime denial after takeover; fresh post-wait credential checks; recovery
preserving identity/activity/grants/history; audit identity conflicts and uncertain
commits; rollback/failed startup and advisory-lock cleanup. Test paused/failed
participants with owned cleanup, not unbounded sleep-based timing assumptions.

The dedicated lock lifecycle and serving transaction lifecycle are separate
owned resources. The executable proof must record barriers and SQLSTATE outcomes,
and detect mutations that remove installation fencing, reuse a pre-wait authority
snapshot or overwrite an existing enrollment. Pure mocks do not qualify these.
Native A4 command/replay and full T01-T16 cases remain mapped to their later work.

Review this contract before stateful implementation; record exact source and
findings/disposition in the owning PR. The accepted authenticated-test profile
permits synthetic fixtures only. Heavy execution requires fresh permission/RAM
preflight, and trust changes require their exact separate approval.

Primary PostgreSQL 16 references: [row/advisory locking](https://www.postgresql.org/docs/16/explicit-locking.html),
[definer function security](https://www.postgresql.org/docs/16/sql-createfunction.html),
and [advisory lock inspection](https://www.postgresql.org/docs/16/view-pg-locks.html).
Function-level privileges and allowed/denied native tests are evidence requirements,
not a claim that schema text alone proves caller authenticity or deployment safety.
