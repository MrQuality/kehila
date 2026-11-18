# ADR-112 - Persistent owner current-work selection

Status: Accepted, revision 1. Owner: project maintainer. Created: 2026-11-17.
Acceptance: on 2026-11-17 the maintainer explicitly selected the reviewed
self-selection authority, permanent success-event storage and same-profile
browser recovery options for implementation. Exact SQL, codecs, capability
composition and harnesses retain technical review and qualification gates.
This decision grants no populated migration, trust/provider change or merge.
Related: [B-003](../product/B-003-contract.md),
[ADR-102](ADR-102-postgresql-transactions.md),
[ADR-104](ADR-104-project-create-codecs.md),
[ADR-105](ADR-105-local-owner-authentication.md),
[ADR-111](ADR-111-native-work-items.md),
[#27](https://github.com/MrQuality/kehila/issues/27),
[#32](https://github.com/MrQuality/kehila/issues/32),
[#49](https://github.com/MrQuality/kehila/issues/49).

## Context

An authenticated owner chooses an existing WorkItem as their current work,
switches to another item or explicitly clears the choice. The choice survives
reload and fresh login. It changes personal context only: no item status, phase,
estimate, timer, resource or scheduling mutation follows from selection.

The pure current-work rules already specify version conflicts, unchanged
successful selections and original-result replay. They do not establish durable
native authority, storage, transport or browser recovery. Selection identity is
user-scoped; the selected item is command content, not the operation target.

## Decision

### Self authority and target disclosure

For this one-owner profile, derive the subject UserId from the installed owner
actor. Accept no caller actor/user assertion. Active installed-owner authority
confers self ReadCurrentSelection and SelectCurrentWork independently of project
creation permission. A non-null new selection also requires the current coherent
`initial_owner` project grant, extended to confer selection, and current item Read
and live project/item eligibility. Login alone does not authorize a target.

A self-read always returns the selection version. If current target access is
absent, disclose only `unavailable`, without project/item identifiers. Self-clear
requires no access to the former target, but still compares the exact expected
selection version. Revocation therefore cannot trap the owner with an uncleared
choice. A recorded non-null selection rechecks current authority for its ORIGINAL
target before disclosing any outcome, even on payload expiry. A changed retry
target cannot supply that authority. Historical replay does not reapply current
archive/version eligibility or require access to an unrelated current selection.

### Fresh native schema and permanent success evidence

Adopt fresh product schema version four, preserving the baseline installer and
version-one through version-three historical codecs. Require exact runtime
schema/capability compatibility; serving cannot migrate or repair a database.
No populated upgrade, database replacement or import is provided.

Store one current selection per user and append one immutable event per
successful operation, including an initial empty-clear or unchanged set. Each
event retains before/after selection, selection versions, its own positive event
ordinal, predecessor, original grant scope and exact attributed operation.
Selection version advances only when the selected target changes. The event
ordinal advances on every success. The head references the latest event.

Commit head, event, permanent operation and replay payload together. Enforce
attribution, predecessor/current-boundary and result agreement at commit. Native
serving gets narrow functions, not broad table mutation. Permanent operation
identity is actor + selection family + user target + token. Immutable event grant
scope remains available independently of expiring payloads. Existing trusted-origin
90-day replay and permanent retired identities apply; expiry never permits reuse.

### Admission, coordination and replay

Freeze separate version-one selection request/result/event codecs. Requests retain
derived user, exact token, expected version and nullable project/item target.
Canonical requests use versioned length-delimited binary fields. IDs preserve
exact UTF8 bytes, are nonempty and at most 128 bytes, and exclude NUL. Native wire
tokens are 128 random bits encoded as 32 lowercase hexadecimal characters.
Versions are canonical decimal u64 strings. Request/response limits are 4 KiB;
generated persisted selection envelopes are at most 16 KiB. Reject overflow,
duplicate keys, unknown fields and malformed shapes without truncation.

Extend ADR-104 read preparation ONLY to Selection: after raw edge admission, a
bounded authorized read-only repeatable-read snapshot may resolve historical
identity/authority, or prepare current state for an unseen command. It creates no
operation or reservation. Admit generated envelopes before a fresh write
transaction, then reauthorize and revalidate. Other command families retain their
existing admission boundaries. No new independent queue/pool is introduced.

Write lock order is installation, actor, original/requested project, existing
operation core, item, selection transition. Immutable undisclosed metadata lookup
under the actor fence can discover original grant scope before project locking.
After waits, recheck current grants before outcome disclosure. Replay precedes
mutable eligibility and does not acquire mutable item/selection locks. Null clear
requires no unrelated project lock. Final live-session/deadline eligibility remains.

An actual changed selection version returns a version conflict. An intervening
no-op can change the event head without changing that version: permit at most two
same-identity re-preparations under the original deadline, then return preparation
unavailable rather than invent a version conflict. Unknown commit requires
reconciliation of the original identity, never silent rebasing or a new token.

Expose authenticated POST read/set/reconcile routes. Reconcile is lookup-only;
absence describes its ordering point and never proves another request cannot
commit. Accepted/replayed responses contain the ORIGINAL result; separately read
current selection for display. Unexpected integrity failures are not conflicts.

### Browser recovery

Before first transmission, commit one exact unresolved selection to strict
IndexedDB in the same profile and HTTPS origin. Bound it to installation, owner
and family, with at most 4 KiB exact wire and 2 KiB metadata, no credentials, and
seven-day eligibility from preparation. Retries do not extend eligibility.
Current server selection has no corresponding seven-day expiry.

Storage failure before transmission prevents send; failure after transmission
does not establish rollback. Cross-tab comparison/generation prevents overwriting
unresolved work. Reload/login restores a view, never automatically executes it.
Explicit check/retry retains the exact identity and expected version. Signout,
binding change and abandon attempt logical deletion with visible failures and
callback invalidation. Late replies cannot clear a replacement journal. Edit and
selection journals have independent identities but coordinated session cleanup.

Cross-browser/profile discovery of UNKNOWN operations is excluded. Committed
current selection remains available after any authorized fresh login. Actual
browser durability and native restart/response-loss behavior require evidence.

## Alternatives

Separately administered selection grants and general delegation add policy and
administration beyond the selected owner profile. Compact current state plus
minimal operation bindings, or transitions only on actual changes, reduce history
but require different integrity designs. Full success events were selected.
Server-owned pending selection supports cross-browser recovery but introduces
registration, discovery, quota, retention and cancellation contracts. Volatile
tracking alone does not provide the selected browser-loss recovery. Neither was
selected. These dispositions do not rule out a later separately accepted design.

## Consequences and impacts

- Security/privacy: current original-scope authority precedes replay disclosure.
  Permanent events retain prior target identifiers, including no-ops, after browser
  cleanup or payload expiry. Browser metadata uses normal OS/profile isolation,
  without application encryption or a forensic-erasure guarantee.
- Performance: permanent rows grow per success; event-only contention needs bounded
  re-preparation. No throughput, storage saving or concurrency improvement is claimed.
- Archive: inspected native routes do not expose archive/restore. Before any such
  exposure, implement the existing atomic affected-user selection-clear contract,
  including contention. This decision adds no archive route or cross-user admin.
- Recovery: current selection/events/operations are authoritative and must be
  preserved coherently in future supported backup/restore. No cross-restore
  exactly-once or disaster-recovery qualification follows.
- Compatibility: v4 is fresh-install-only; old runtimes cannot serve it. Preserve
  stored historical versions. Populated migration remains separate work.
- Languages: stable IDs, operation identity and wire values are locale independent;
  translated labels must not change them.
- Future actors/replication: preserve performer/subject attribution. No machine
  credential, delegation, multi-owner or replicated coordination is added. Those
  profiles must revisit identity, fences and authority explicitly.

## Verification and remaining evidence

Verify actual-role installation/capabilities, exact codecs and attributed SQL
closure; set/clear/no-op, original replay after later changes, changed payload
reuse, conflicts, revoked/absent/archived targets, no unrelated writes and version
exhaustion. Observe two-no-op and no-op-versus-set contention. Inject and observe
precommit rollback and postcommit response loss, then reconcile the same identity
after runtime restart with independent SQL head/event/success oracles.

Review harness authority, fault activation, deadlines and owned cleanup before
costly qualification or user participation. The visible browser journey selects
A, retains A after reload/fresh login, selects B, clears, shows a stale-tab conflict
and reconciles an uncertain original request. Pure tests or source review alone
do not qualify this native/browser outcome. Required gates and exact-head review
remain; no implementation or qualification is claimed by this accepted record.
