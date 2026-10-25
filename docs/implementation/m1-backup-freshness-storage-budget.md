# M1 backup freshness and storage budget

Status: **Proposed design clarification**, 2026-10-25. No backup mechanism,
destination, numerical operating limit or automated disk-pressure policy is
selected. This proposal refines the existing real-data readiness obligations in
[D-017](../product/decisions.md#d-017), [B-005](../product/backlog.md#b-005) and
[Q-020](../product/open-questions.md#q-020). Implementation belongs to
[#49](https://github.com/MrQuality/kehila/issues/49), including W7 qualification.

## Outcome and boundary

The owner needs to understand how much recent work a recoverable backup contains
and whether storage pressure threatens saves or recovery. A daily job schedule
and database settings alone cannot establish either fact.

D-017 already requires visible backup age/failure, clean-instance restore of
configuration and replay records, search reconstruction, and safe handling of
stale browser versions and retry IDs before real data. Machine-loss recovery
requires a copy outside the machine's failure boundary. Broader disaster recovery
remains post-MVP under [D-037](../product/decisions.md#d-037) and
[B-024/#60](../product/backlog.md#b-024).

[ADR-105's accepted restore contract](../architecture/ADR-105-local-owner-authentication.md#consequences-and-impacts)
also applies: preserve actor/operation/tombstone identities, invalidate issued
credentials, remove the restored password verifier, and keep the API
authentication-blocked until local owner recovery sets a new password.

This document proposes definitions and future acceptance cases. It supplies no
new execution evidence, supported installation, recovery-point objective (RPO,
acceptable data loss) or recovery-time objective (RTO, time to restore service).
It does not block independently justified authentication/file-admission work.
ADR-103 production adoption remains Proposed; SP-006 remains a bounded proof.

## Recovery-point age

Proposed primary status: **Recoverable data is X old**, qualified by whether the
verified copy is local or off-machine. Details show the latest attempt, delivery
and verification separately. A failed new attempt must remain visible even when
an older usable backup exists. Unknown protection must not appear healthy.

For a given failure scope, select the newest consistent data point supported by
an eligible backup set; calculate `age = observation time - recovery-point time`.
For machine-loss status, only a verified recoverable off-machine set is eligible.
A newer local copy can be shown separately but cannot improve that status.

The proposed manifest records:

| Information | Purpose |
| --- | --- |
| Backup identity, source lineage, format and schema versions | Bind verification and restore to the intended data and compatible procedure. |
| Consistent recovery point and its time basis | Identify the data represented, independently of later file activity. Preserve a conservative bound if the mechanism cannot supply an exact time. |
| Required components and integrity identities | Identify configuration, authoritative data, replay records and any mechanism-specific dependencies needed for that point. |
| Destination/failure scope and delivery completion | Distinguish a local copy from protection against machine loss. |
| Verification result, time and procedure version | State what was actually checked, tied to the same backup set. |
| Latest attempt and failure state | Explain interrupted, failed or pending work without hiding an older usable set. |

Neither a checksum nor upload success alone establishes recoverability. The
future implementation must define eligibility through a reviewed restore
procedure and checks of the complete required set, with clean-instance restore
evidence for the supported format. The frequency of restore drills and ongoing
verification policy remain open; do not claim every archive was restored unless
it was. Required restore secrets/configuration must survive the failure scope
without exposing them in status or public manifests. Their storage/access design
is a separate review decision.

A copy or later verification preserves the original recovery point. Example:
data captured Monday at 09:00 and uploaded Tuesday at 09:00 is 25 hours old on
Tuesday at 10:00, even if verification just completed. A newer set still in
transfer does not advance the machine-loss recovery point. If none qualifies,
show **No verified off-machine recovery point**. Missing, future or inconsistent
timestamps make age unknown; define clock-error handling before implementation.
The stale threshold and notification policy remain for maintainer review.

The recovery point is mechanism-specific. PostgreSQL's
[SQL dump documentation](https://www.postgresql.org/docs/16/backup-dump.html)
describes a consistent snapshot at the start of the dump. A physical backup with
WAL recovery instead depends on a complete usable backup/WAL chain and a supported
replay target. PostgreSQL documents those requirements in
[continuous archiving](https://www.postgresql.org/docs/16/continuous-archiving.html).
This distinction selects neither method. It also prevents treating a logical
dump as a base for physical WAL replay.

## WAL and whole-installation disk budget

PostgreSQL 16's
[replication settings](https://www.postgresql.org/docs/16/runtime-config-replication.html#GUC-MAX-SLOT-WAL-KEEP-SIZE)
apply `max_slot_wal_keep_size` to slot-retained WAL at checkpoint time. It is
neither a hard cap on all WAL nor a volume quota. Setting it can allow required
WAL to be removed and leave a consumer unable to resume. The default unlimited
setting also does not establish an acceptable disk budget.

Likewise, [`max_wal_size`](https://www.postgresql.org/docs/16/runtime-config-wal.html#GUC-MAX-WAL-SIZE)
is a soft checkpoint-related limit. Load, archiving failures and other retention
requirements can keep WAL above it. Neither setting replaces measurement of
actual `pg_wal` usage and free space. PostgreSQL warns that a full WAL disk can
cause a server shutdown in its [disk-full guidance](https://www.postgresql.org/docs/16/disk-full.html).

Proposed accounting covers the installation's actual storage layers: Linux
container filesystems/volumes, the Podman provider's backing storage and the
Windows host volume. Measure each constrained layer; apparent free space inside
a container alone cannot establish host headroom. Do not double-count nested
storage as independent physical capacity. The supported provider's capacity and
space-reclamation behavior need direct qualification.

| Budget component | What future measurements must include |
| --- | --- |
| Authoritative data | Tables, indexes, configuration, replay/history retention, growth and maintenance overhead. |
| WAL | Observed generation rate, checkpoint overshoot, slot lag and archive backlog where used; retained WAL is shared across requirements, not one additive copy per slot. |
| Search and rebuild | Current index, simultaneous replacement targets, delivery backlog and failed-build cleanup. |
| Backup/restore | Staging, retained local sets, transfer retries and temporary restore working space on each affected volume. |
| Other consumers and reserve | Logs, temporary files, upgrades, provider growth, shared-host use and a measured safety margin. |

A useful planning estimate is observed WAL bytes/second multiplied by the
candidate tolerated consumer outage, plus measured bursts/overshoot and reserve.
It is workload-dependent, not a guaranteed outage duration or a formula for
total disk capacity. A paused consumer, long transaction, checkpoint boundary or
failing archive can change the result. Measure retained tuples/catalog pressure
as well as WAL if slots are used.

Proposed monitoring combines actual volume use/free space and growth with
checkpoint/archive health, oldest required WAL, consumer lag and delivery
backlog. Where slots are used, PostgreSQL exposes `wal_status` and
`safe_wal_size` in
[`pg_replication_slots`](https://www.postgresql.org/docs/16/view-pg-replication-slots.html).
The latter is a byte allowance, not a time guarantee; NULL must not be reported
as safe/unlimited headroom without interpreting the slot/configuration state.
A `lost` slot is unusable. Project recovery must not silently skip a delivery
gap or report search caught up merely because a new slot exists.

Candidate pressure responses are an actionable warning, stopping optional
rebuild/backup staging, and controlled write admission or shutdown before reserve
is exhausted. The exact thresholds, ordering, ownership and save-outcome handling
need review and failure tests; no automatic action is authorized here. Backup
failure must remain visible if staging is suspended. Never use automatic removal
of PostgreSQL WAL files, authoritative records or the last eligible backup as an
unreviewed space-recovery policy. Slot loss requires a reviewed recovery path
such as a new consistent snapshot/rebuild, preserving authority and replay
guarantees. SP-006's bounded rebuild does not qualify that full recovery path.

## Proposed acceptance cases

These are future checks, **not passed tests**:

1. Upload and reverify an old set: its data age does not reset. A failed newer
   attempt stays visible beside the older recovery point.
2. Sleep through a scheduled run, interrupt delivery, and lose destination access:
   status continues aging, reports the failure/pending state, and makes no
   unconditional 24-hour RPO claim.
3. Remove a required component or make its integrity/lineage incompatible:
   refuse eligibility; show an older valid point or unknown/unavailable status.
4. Restore a complete set to a clean instance; check configuration, authoritative
   data and replay identities, reconstruct search, and enforce ADR-105's restore
   security contract above. Verify access remains blocked through restore and
   re-enrollment, and old credentials/verifiers cannot regain authority. Exercise
   both retained and lost-ledger browser requests:

   - A retained success/tombstone follows D-032/D-033/D-034 under current original
     mutation grants: exact retries replay or return `replay_expired`; changed
     content conflicts, and the operation never executes again.
   - A tab/journal newer than the recovery point may hold a version ahead of the
     restored server or a token whose committed record is absent after rollback.
     After reauthentication, including from a cloned journal, it must enter an
     explicit reconciliation path rather than silently execute as a fresh
     intention. Preserve the draft/request and communicate the rollback-related
     uncertainty; do not silently replace its token or expected version.

   Review the restore/reconciliation design before implementation and prove its
   fence before supported use. This specifies the safety outcome without choosing
   an installation/restore epoch or new error code. See the
   [uncertain-save proposal](m1-uncertain-save-reconciliation.md).
   Record the actual restored point and elapsed time.
5. For machine-loss claims, repeat with only the off-machine set and independently
   available recovery prerequisites. A local synchronized folder or second
   container on the same machine does not establish that failure isolation.
6. Exercise missing timestamps and clock changes; never turn an uncertain or
   negative age into a fresh/healthy status.
7. Measure a declared workload through normal writes, paused delivery, catch-up,
   checkpoint activity, rebuild overlap and backup staging. Include archive
   failure only if archiving is selected. Record peak WAL, disk growth, reserve,
   save latency and recovery duration rather than deriving a hard cap from a
   configuration value.
8. Under a separately reviewed disposable profile, exhaust a slot's retention
   allowance and approach each storage layer's pressure threshold. Verify visible
   failure, reviewed admission behavior, and complete recovery without silent
   data/delivery gaps. Never fill a shared development disk to run this case.

## Decisions before implementation

The maintainer still needs to review the backup method, destination and failure
scope, protected recovery prerequisites, format compatibility, retention and
deletion policy, verification/drill frequency, age/notification thresholds and
RPO/RTO targets. Qualification must then measure the actual supported workload
and failure cases. WAL/slot settings, volume sizing/reserve, pressure actions and
lost-WAL recovery also need reviewed values and implementation evidence. Merging
this proposal does not accept those choices or close Q-020, B-005 or #49.
