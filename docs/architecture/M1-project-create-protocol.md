# M1 project creation: command and verification proposal

Status: Proposed, 2026-10-08. Scope: #26/B-005 and its project-creation slice.
Read with the [operation storage](M1-project-create-storage.md) and
[seed/history tables](M1-project-create-seed.md). These are design documents,
not executable migrations or evidence of PostgreSQL behavior.

The maintainer agreed these corrections on 2026-10-08: restricted locking
functions, exclusive actor coordination for creation, no Project lock on creation
replay, typed permanent operation targets, and current grants separate from grant
history. Detailed physical choices still require execution and review. P-07 now
records the agreed 90-day operation-timestamp policy without additional grace.

## Outcome and prerequisites

One accepted ProjectCreateResult supplies the Project, ordered trusted M1V1
configuration, current access grant and immutable grant event, revision-one
snapshots, and saved successful response. All become visible together at commit. A caller that
loses the response retries the original intent with the original identity/token.

Before implementing a route, settle actor/session provisioning and the initial
owner's effective permissions (#11/#32), stable creation-target allocation (#27),
and the replay clock implementation (P-07). A lost creation response must not cause a new
target ID or token to be allocated. A client-supplied ID must not let one actor
claim another actor's target. This draft does not solve those identity rules by
adding an unexplained reservation table.

## Proposed transaction protocol (P-06)

Use READ COMMITTED for this command. Each authorization read below occurs after
its protecting lock is acquired; an earlier unlocked read is not authoritative.

1. Authenticate the actor, validate the request envelope, and decode exact typed
   intent. Preserve original strings and collection order. Validate storage
   bounds explicitly; do not silently truncate or normalize contract input.
2. Begin the transaction and call a restricted locking function that locks the
   actor row FOR NO KEY UPDATE. Read current active
   state and project-create capability. Reject inactive/unauthorized actors before
   exposing saved content or intent-conflict details, including on replay.
3. Verify the trusted target allocation belongs to this actor's creation scope.
   Do not lock the Project for creation replay: the saved result does not depend
   on its mutable state. For fresh creation, the project primary key arbitrates
   target collisions; the actor lock does not protect other actors.
4. Look up the complete operation key (actor, family, target, token). Lock an
   existing core FOR SHARE through a restricted locking function, then read its
   payload while retaining that lock.
   Check its stored grant requirements against current authority before replay.
   Return the saved response only for equal typed intent within the full-replay
   period. Different intent conflicts. Expired equal intent returns the defined
   expiry outcome; it does not execute the command again. Use the permanent core
   even when payload has been retired.
5. For a fresh key, run the trusted pure creation operation. After locks and
   acceptance, sample the server operation timestamp immediately before the
   persistence batch. Store that origin and its checked 90-day deadline once.
   Insert the permanent
   core, Project, ordered seed rows, revision-one snapshots, active version-one
   grant and attributed grant event, and saved response payload. Only the trusted
   result is eligible for persistence; callers cannot submit arbitrary seed definitions.
6. Reload the persisted Project and ordered configuration in the transaction.
   Compare typed equality with the trusted result and snapshots. Check that the
   current grant and event agree in actor, project, version, profile and active
   state; foreign keys alone do not enforce this equality. After all writes/checks,
   force deferred constraints with SET CONSTRAINTS ALL IMMEDIATE. Any failure
   rolls back the complete action; it is not a partially successful creation.
7. Commit, then return the saved result. A failure received from PostgreSQL while
   committing deferred constraints is a known rollback. A lost connection or
   response around COMMIT can leave the outcome unknown; reconcile with the same
   operation key instead of claiming rollback or issuing a new creation.

Creation's lock hierarchy is actor, then operation core. Concurrent creation and
creation replay by the same actor serialize; different actors share no authority
singleton. Permission revocation updates the actor row and conflicts with its
FOR NO KEY UPDATE lock. Compaction takes the core FOR NO KEY UPDATE and must never
acquire actor/Project locks afterward. Commands needing several actors/Projects
must specify their own sorted lock order. This does not mandate serialization
of every future item command by actor. Test actual waits, foreign-key locks, and
deadlocks rather than generalizing from creation.

The actor lock protects the proposed P-03 capability only. It is not a proof that
future project/group grants are safe: those routes need explicit shared lock
ownership with their revocation procedures. PostgreSQL row-lock compatibility
and foreign-key lock acquisition must be verified with real connections.

## Uniqueness races and bounded retries

Two cooperating same-actor creation requests cannot both observe an absent key:
the second waits for the actor lock and repeats its lookup under READ COMMITTED.
The permanent unique key remains a backstop. A named operation-key collision
requires rollback and reconciliation in a fresh authorized transaction; examine
whether a writer bypassed the protocol. Distinguish it from project-ID collisions
and other constraints. A different token cannot adopt an existing Project or
overwrite its saved result. Never continue a failed PostgreSQL transaction.

Remove the earlier arbitrary three-attempt proposal. Select a small complete-
transaction retry budget from real deadlock/cancellation tests, inside one
application request deadline. Deadline and per-statement/lock/idle values require
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

P-07 is settled as a product policy: E = recorded_at_ms + 7,776,000,000, with
checked arithmetic. Equal authorized intent replays while recorded_at_ms <= now
< E; it expires at/after E even if payload still exists. No added grace or
post-commit deadline adjustment applies. The pre-commit sample is intentional:
remaining persistence/commit delay slightly shortens availability after commit.
Only committed successful operations are replayable, and retries never move E.

Implement server-controlled sampling after coordination/pure acceptance, just
before the persistence batch. Prefer PostgreSQL clock_timestamp() converted to
integer UTC epoch milliseconds for origin and expiry decisions; unlike now(), it
does not retain the transaction-start time. The concrete clock conversion is a
proposal to verify. Do not accept origin/deadline from a client. Test late cleanup,
overflow, equality at E, rollback, and a delayed commit without deadline extension.
Full-record time before the origin is incoherent. Irreversible retirement prevents
clock rollback from reviving a tombstone. The accepted policy does not introduce
a durable clock high-water mark: before retirement, backward movement to a valid
pre-expiry time can affect full-record replay under the existing pure decision.
Document that behavior and test it rather than claiming all wall-clock rollback
is eliminated. Shape constraints cannot prove the compactor ran after expiry.

See [PostgreSQL clock semantics](https://www.postgresql.org/docs/16/functions-datetime.html#FUNCTIONS-DATETIME-CURRENT).

## Database roles and mutation ownership

Use a migration owner distinct from the serving login. The serving login cannot
own schema objects, replace functions/triggers, disable constraints, or run DDL.
Grant SELECT on required tables, INSERT on creation persistence tables, and
EXECUTE on reviewed locking functions. The serving login must not directly
UPDATE/DELETE operation cores, payloads, current grants, or immutable history.
Locking SELECT clauses require UPDATE privilege: do not grant permission-column
UPDATE merely to make locking legal. The restricted function owner holds that
privilege and cannot be assumed by the serving login. See
[SELECT privileges](https://www.postgresql.org/docs/16/sql-select.html).
Provision generated-identity
sequence privileges only as required by the executable migration and role tests.

Actor capability provisioning/revocation needs its own restricted, audited path;
the serving creation route cannot grant itself permission. A restricted compactor
entry point must validate expiry and atomically retire the core/delete payload.
Do not give a general serving role direct payload deletion or retirement rights.
Review privileged function ownership and fixed trusted search_path together with
role grants. Trigger shape checks alone do not establish authorization.

Locking functions use SECURITY DEFINER, schema-qualified objects, and a fixed
trusted search_path with pg_temp last. Revoke PUBLIC EXECUTE and grant only the
serving role atomically with creation. Functions lock/read protected state, never
mutate capabilities or accept arbitrary SQL, and stay in the caller transaction.
Authentication and trusted actor binding remain application prerequisites, not
implied by a shared database login. Test successful locks and rejected direct
UPDATE as the actual role. See
[function security](https://www.postgresql.org/docs/16/sql-createfunction.html).

## Invariants and evidence required

| Invariant | Enforcement owner | Required verification |
| --- | --- | --- |
| Atomic creation, seed, access, history, response | One transaction plus attributed/deferred foreign keys | Fail after each write boundary; no committed partial rows |
| Unique scoped request identity | Permanent composite unique key | Concurrent same-key requests; independent actors/targets/tokens |
| Exact request equality | Versioned typed codec and retained request bytes | Golden vectors for strings, order, scalar boundaries, codec upgrades |
| Current permission before replay | Actor lock plus current grant evaluation | Replay after revocation; concurrent revoke/create/replay |
| Historical creation attribution | Composite operation/family/target-kind/project/actor foreign keys | Reject a valid operation attributed to the wrong family, target kind, target or actor |
| Typed target scope | Target-shape/family checks and NULLS NOT DISTINCT scoped key | Test pure target variants, absent components, duplicate keys; future families require deliberately expanded slice guards |
| Current access and immutable grant history | Restricted mutation ownership and transactional reconstruction | Creation installs matching active version-one rows; historical events alone cannot authorize; future revocation updates current state with an event |
| Core permanence and coherent payload shape | Fresh-insert guard, immutable core and deferred shape triggers | Reject initially retired inserts, deletion, mutation, missing/unexpected payload, retirement reversal; verify error metadata |
| Safe expiry/compaction | Reviewed clock policy and restricted compactor | Before/at/after deadline, clock rollback, compaction/replay races |
| Initial New status belongs to workflow | Phase and membership foreign keys | Reject missing membership and wrong initial phase |
| Default workflow belongs to type | Deferred membership foreign key | Reject default outside permitted workflows |
| Title field belongs to type and is text | Ownership foreign key and creation-only text constraint | Reject foreign-type or unsupported field |
| Ordered configuration matches trusted seed | Position uniqueness plus ordered adapter reconstruction | Compare every ordered collection, empty collections, and snapshots |
| Exact unsigned scalar persistence | Integral numeric domain and exact adapter decode | Fractions, negative values, u64 maximum, overflow |
| History remains readable after upgrades | Versioned immutable snapshot/result decoders | Fixtures from every supported stored version |
| Least database privilege | Restricted locking functions, ownership, explicit grants | Successful locks as serving role; rejected direct UPDATE/DELETE/TRUNCATE and unauthorized function calls |
| Unknown commit outcome reconciles safely | Original key reuse and saved result | Disconnect around commit; retry returns one creation/result |

Run those integration cases against supported PostgreSQL with separate serving,
migration, and compactor roles and independent concurrent connections. Include
restore/reconnect checks and migration up/down policy in the implementation PR.
CI must fail when required database cases are skipped or a service is unavailable.
Pure tests cover trusted construction and codec equality; they do not prove SQL
constraints, privileges, isolation, durability, or lock behavior.

## Definition of done for the design and next implementation gate

- [ ] Settle remaining physical/behavioral choices in P-01 through P-07; preserve the agreed corrections recorded above.
- [ ] Resolve creation-target identity and effective initial-access behavior.
- [ ] Specify canonical request/result/snapshot codecs and historical support.
- [ ] Turn reviewed SQL into one ordered migration with explicit role grants.
- [ ] Implement the bounded creation transaction and error/reconciliation mapping.
- [ ] Demonstrate the invariant matrix on real PostgreSQL locally and in CI.
- [ ] Update implementation status with evidence, not proposal claims.

No native adapter, executable migration, database test, or new public route is
delivered by these documents. Implement the approved first slice in small commits
once its unresolved behavioral and physical choices are settled.

The next execution step is an isolated PostgreSQL verification harness for the
corrected SQL and actual roles, before building the route/adapter. Prioritize
locking privileges, same-actor serialization, cross-actor target collisions,
deferred rollback, structured errors, fresh-core/payload enforcement, and
compaction/replay races and the agreed recorded-time expiry boundary. Remaining
role/codec/clock implementation gaps must be reported explicitly. Keep SQL
execution evidence separate from documentation-only checks.
