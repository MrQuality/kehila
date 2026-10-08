# M1 project creation: command and verification proposal

Status: Proposed, 2026-10-08. Scope: #26/B-005 and its project-creation slice.
Read with the [operation storage](M1-project-create-storage.md) and
[seed/history tables](M1-project-create-seed.md). These are design documents,
not executable migrations or evidence of PostgreSQL behavior.

## Outcome and prerequisites

One accepted ProjectCreateResult supplies the Project, ordered trusted M1V1
configuration, initial access assignment, immutable revision-one snapshots, and
saved successful response. All become visible together at commit. A caller that
loses the response retries the original intent with the original identity/token.

Before implementing a route, settle actor/session provisioning and the initial
owner's effective permissions (#11/#32), stable creation-target allocation (#27),
and the replay time policy (P-07). A lost creation response must not cause a new
target ID or token to be allocated. A client-supplied ID must not let one actor
claim another actor's target. This draft does not solve those identity rules by
adding an unexplained reservation table.

## Proposed transaction protocol (P-06)

Use READ COMMITTED for this command. Each authorization read below occurs after
its protecting lock is acquired; an earlier unlocked read is not authoritative.

1. Authenticate the actor, validate the request envelope, and decode exact typed
   intent. Preserve original strings and collection order. Validate storage
   bounds explicitly; do not silently truncate or normalize contract input.
2. Begin the transaction and lock the actor row FOR SHARE. Read current active
   state and project-create capability. Reject inactive/unauthorized actors before
   exposing saved content or intent-conflict details, including on replay.
3. For a previously created target, lock its Project FOR NO KEY UPDATE, after
   verifying the target belongs to this actor's creation scope. For a new target
   there is no row to lock: the project primary key arbitrates concurrent inserts.
4. Look up the complete operation key (actor, family, target, token). Lock an
   existing core FOR SHARE, then read its payload while retaining that lock.
   Check its stored grant requirements against current authority before replay.
   Return the saved response only for equal typed intent within the full-replay
   period. Different intent conflicts. Expired equal intent returns the defined
   expiry outcome; it does not execute the command again. Use the permanent core
   even when payload has been retired.
5. For a fresh key, run the trusted pure creation operation. Insert the permanent
   core, Project, ordered seed rows, revision-one snapshots, attributed initial
   access assignment, and saved response payload. Only this trusted result is
   eligible for persistence; callers cannot submit arbitrary seed definitions.
6. Reload the persisted Project and ordered configuration in the transaction.
   Compare typed equality with the trusted result and snapshots. Force deferred
   constraints with SET CONSTRAINTS ALL IMMEDIATE after all writes. Any failure
   rolls back the complete action; it is not a partially successful creation.
7. Commit, then return the saved result. A failure received from PostgreSQL while
   committing deferred constraints is a known rollback. A lost connection or
   response around COMMIT can leave the outcome unknown; reconcile with the same
   operation key instead of claiming rollback or issuing a new creation.

Lock hierarchy is actor, existing target Project, then operation core. Commands
that need several actors/projects must specify sorted identity order before they
are added. Permission revocation updates the same actor row and conflicts with
FOR SHARE. Compaction takes the core FOR NO KEY UPDATE and must never acquire
actor/Project locks afterward. Concurrent creations by one actor can share the
actor lock; they need not serialize on an authority singleton. Test the actual
lock waits and deadlocks rather than assuming this hierarchy proves every future
command safe.

The actor lock protects the proposed P-03 capability only. It is not a proof that
future project/group grants are safe: those routes need explicit shared lock
ownership with their revocation procedures. PostgreSQL row-lock compatibility
and foreign-key lock acquisition must be verified with real connections.

## Uniqueness races and bounded retries

Two transactions can both observe an absent key. A matching unique-key collision
must roll back the losing transaction, then start a fresh transaction and repeat
authorization/key lookup. Reuse the same request; never continue a failed
PostgreSQL transaction. Distinguish the named operation-key constraint from
project-ID collisions and other constraints. A different token cannot adopt an
existing Project or overwrite its saved result.

Propose at most three complete attempts, all inside one application request
deadline. Final deadline and per-statement/lock/idle timeout values require
measurement before release; server statement_timeout and
idle_in_transaction_session_timeout do not bound an entire active transaction.
Serialization failures (40001) and deadlocks (40P01) can retry the complete intent
within that budget. Lock timeout/cancellation needs an explicit error mapping;
connection loss during commit requires reconciliation, not a blind assumption.

Do not retry changed-intent conflicts, forbidden access, invalid input, or
unexpected integrity failures as transient success candidates. For canceled or
failed statements, finish rollback before reusing a connection. A retry limit is
not an idempotency expiry rule and cannot erase the permanent operation key.

## Codecs, expiry, and reconstruction

Propose a versioned canonical binary request encoding with command/version tags,
length-delimited UTF8 fields, and explicit scalar tags. The codec must distinguish
all accepted typed values and preserve exact bytes/order. Compute SHA-256 from
that encoding, never from incidental JSON formatting. Retain request bytes for
exact typed comparison during full replay and verify digest coherence. Before
payload retirement, define how permanent versioned digests support comparisons
for expired requests, including requests from older codec versions.

The saved result and snapshots need explicit codec versions, complete fields,
and retained historical decoders. JSONB object-key order is irrelevant; ordered
arrays and exact scalar values are not. Replay returns the recorded result, not
a reconstruction from subsequently edited current tables. Test large u64 values
without a lossy floating-point JSON bridge.

P-07 remains an implementation blocker: the accepted full-replay duration is
90 days after authoritative commit. A pre-commit timestamp can shorten it and
does not satisfy that promise. Specify the origin/clock/commit relationship,
overflow handling, equality-at-deadline behavior, and permitted compactor clock
before wiring timestamps. The presence/retirement constraints enforce shape;
they cannot prove the compactor ran after expiry. Irreversible retirement must
prevent clock rollback from reviving full replay.

## Database roles and mutation ownership

Use a migration owner distinct from the serving login. The serving login cannot
own schema objects, replace functions/triggers, disable constraints, or run DDL.
Grant only the operations this creation slice needs: SELECT on required tables
and INSERT on its persistence tables. It must not UPDATE/DELETE operation cores,
payloads, initial assignments, or immutable history. Provision generated-identity
sequence privileges only as required by the executable migration and role tests.

Actor capability provisioning/revocation needs its own restricted, audited path;
the serving creation route cannot grant itself permission. A restricted compactor
entry point must validate expiry and atomically retire the core/delete payload.
Do not give a general serving role direct payload deletion or retirement rights.
Review privileged function ownership and fixed trusted search_path together with
role grants. Trigger shape checks alone do not establish authorization.

## Invariants and evidence required

| Invariant | Enforcement owner | Required verification |
| --- | --- | --- |
| Atomic creation, seed, access, history, response | One transaction plus attributed/deferred foreign keys | Fail after each write boundary; no committed partial rows |
| Unique scoped request identity | Permanent composite unique key | Concurrent same-key requests; independent actors/targets/tokens |
| Exact request equality | Versioned typed codec and retained request bytes | Golden vectors for strings, order, scalar boundaries, codec upgrades |
| Current permission before replay | Actor lock plus current grant evaluation | Replay after revocation; concurrent revoke/create/replay |
| Historical creation attribution | Composite operation/project/actor foreign keys | Reject a valid operation attributed to the wrong target or actor |
| Core permanence and coherent payload shape | Immutable core trigger and deferred shape triggers | Reject deletion, mutation, missing/unexpected payload, retirement reversal |
| Safe expiry/compaction | Reviewed clock policy and restricted compactor | Before/at/after deadline, clock rollback, compaction/replay races |
| Initial New status belongs to workflow | Phase and membership foreign keys | Reject missing membership and wrong initial phase |
| Default workflow belongs to type | Deferred membership foreign key | Reject default outside permitted workflows |
| Title field belongs to type and is text | Ownership foreign key and creation-only text constraint | Reject foreign-type or unsupported field |
| Ordered configuration matches trusted seed | Position uniqueness plus ordered adapter reconstruction | Compare every ordered collection, empty collections, and snapshots |
| Exact unsigned scalar persistence | Integral numeric domain and exact adapter decode | Fractions, negative values, u64 maximum, overflow |
| History remains readable after upgrades | Versioned immutable snapshot/result decoders | Fixtures from every supported stored version |
| Least database privilege | Ownership and explicit role grants | Attempts to bypass history, access, core, and compaction restrictions |
| Unknown commit outcome reconciles safely | Original key reuse and saved result | Disconnect around commit; retry returns one creation/result |

Run those integration cases against supported PostgreSQL with separate serving,
migration, and compactor roles and independent concurrent connections. Include
restore/reconnect checks and migration up/down policy in the implementation PR.
CI must fail when required database cases are skipped or a service is unavailable.
Pure tests cover trusted construction and codec equality; they do not prove SQL
constraints, privileges, isolation, durability, or lock behavior.

## Definition of done for the design and next implementation gate

- [ ] Review P-01 through P-07 and record acceptance or changes individually.
- [ ] Resolve creation-target identity and effective initial-access behavior.
- [ ] Specify canonical request/result/snapshot codecs and historical support.
- [ ] Turn reviewed SQL into one ordered migration with explicit role grants.
- [ ] Implement the bounded creation transaction and error/reconciliation mapping.
- [ ] Demonstrate the invariant matrix on real PostgreSQL locally and in CI.
- [ ] Update implementation status with evidence, not proposal claims.

No native adapter, executable migration, database test, or new public route is
delivered by these documents. Implement the approved first slice in small commits
once its unresolved behavioral and physical choices are settled.
