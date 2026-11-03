# Pending project creation: physical and API candidate

Status: technically reviewed engineering contract; implementation verification
is in progress. Product behavior is accepted under ADR-107 revision 4; this
document supplies delegated engineering choices, not rollout or real-data acceptance.

## Fresh storage and capabilities

The product installer composes the existing foundation and product authentication,
pending lifecycle and successful mapping in one fresh-install transaction. The
standalone foundation installer retains its behavior through a shared SQL body.
Product schema version 2 requires the exact foundation/profile at startup.

Keep NOLOGIN schema ownership and separate NOLOGIN enrollment/recovery capability.
The NOLOGIN auth runtime inherits the narrow existing creator capability: one
process is deliberately trusted for both. The login may SET ROLE to that runtime,
not enroll/recover or assume schema ownership. Runtime receives pending SELECT
and reviewed transition functions, not direct pending UPDATE/DELETE or owner DDL.
PUBLIC execution is revoked and SECURITY DEFINER search paths are fixed.

Pending rows bind installation/actor to the installed owner, family ProjectCreate,
32-lowercase-hex handle/token, version-one exact name/prefix/unit and immutable
registration/expiry timestamps. A positive noncycling bigint identity supplies a
request number, encoded as a canonical decimal JSON string. Runtime cannot supply
or reset that identity. This prevents a deleted never-created request number from
being reissued during the installation lifetime; gaps are allowed. It is a locator,
not a credential. Restoring an older database still has the retained cross-restore
uncertainty boundary; no global exactly-once guarantee is added.

States are prepared, completed, cancelled and expired. Only prepared may transition
to another state. Terminal cancellation/expiry clear saved form content. Completion
has a permanent core/project reference and retains original request content only
while its replay payload is retained. Retirement must clear duplicated content
coherently and cannot erase successful identity or revive execution.

The successful mapping retains actor/family/handle and target uniqueness, existing
five-column attribution and the actual project-creation-operation reference.
Deferred matching references require completed pending/core/project/mapping tuples
to agree in both directions. A core-ID/token unique reference enforces token
agreement: pending tokens exist before the core, so deriving them only after
success cannot replace this constraint. No separate mutable mapping-token copy.

Owner-only pending policy initially selects 32 prepared entries, 16 entries/page
and a 24-hour unexecuted lifetime. The selected ranges are 1–64 prepared entries,
1–32 entries/page and 60–604800 seconds of lifetime. These configurable development
containment defaults are not throughput or retention promises. Clients cannot
set policy. A separate hard ceiling of 64 never-created rows per actor bounds
prepared plus cancelled/expired metadata, even after policy is reduced. Under
the actor fence, registration visits that indexed set, expires eligible prepared
entries and deletes terminal never-created entries before checking the configured
prepared quota. It never scans completed history for quota cleanup. Each such
maintenance pass inspects at most 64 rows; the SQL transition enforces the hard
ceiling before insertion. Completed pending/core/mapping references are never
pruned by this path. Listing uses an actor/request-number index and a keyset bound,
fetching at most the configured page size plus one, without loading total history.
No background draft executor or general retention/compaction service is introduced.

All pending transitions acquire the existing installation/actor fence and require
current active authority plus project-create capability. Recheck after waits.
The trusted clock is the database's `clock_timestamp()`, sampled after obtaining
the fences and any row wait; use checked millisecond conversion and expiry
arithmetic. Transaction-start `now()` cannot order a request that waited for a
fence. Expiration is an irreversible transition at that ordering point; a clock
rollback cannot restore prepared state. A wall-clock rollback before an expiry
transition commits can extend eligibility, an explicit wall-clock limitation. Executions already
ordered before expiry may commit later; no commit-wall-clock guarantee is claimed.
Cancellation and expiry cannot overtake an executing transaction under that fence.
List/open may perform bounded expiry maintenance, so their API uses POST and CSRF.
Retained logical 90-day operation replay is distinct from unexecuted pending expiry.
Its origin remains a separate fresh trusted sample after operation lookup and
acceptance, immediately before persistence. Pending registration time is not the
operation replay origin.

Completed entries permanently expose their completion/project identity under
current authority even after exact result replay expires. Reconciliation reports
`replay_expired` rather than recreating payloads; cancellation still reports
already completed. An owner-only retirement helper locks the operation in the
same order as replay, checks its trusted replay deadline, and atomically retires
the core payload and clears the duplicated completed form. Deferred constraints
enforce content presence exactly when the core payload remains retained. Runtime
has no retirement capability. Missing content never permits seed reconstruction.

## Wire candidate

Use fixed POST endpoints with closed JSON objects, no query/forwarded assertions:

| Endpoint | Exact input responsibility |
| --- | --- |
| `/projects/binding` | Empty object; current authenticated installation/actor comparison metadata |
| `/projects/register` | `name`, `prefix`, `estimate_unit`; current input validation, no caller target/actor/grant |
| `/projects/list` | `before`: null or canonical request number; descending identity pagination, policy page limit |
| `/projects/open` | Request number; return bounded saved state/content under current authority |
| `/projects/execute` | Request number, server-disclosed token and exact form values; untrusted preparation copy, never an update |
| `/projects/reconcile` | Request number; historical-only lookup/replay, no new domain/seed preparation |
| `/projects/cancel` | Request number; confirmed cancellation or already-completed outcome |
| `/projects/read` | Project ID; fresh independently authorized project/configuration read |

All require browser credentials, independent CSRF and exact origin/authority.
Names/strings are preserved, never trimmed or normalized. Reject unknown/missing/
duplicate decoded keys and wrong JSON types through original-byte bounded parsing.
Retain the 4096-byte HTTP body ceiling; native form bounds fit within it. Canonical
decimal identifiers avoid JavaScript numeric rounding. Embedded versioned storage
records retain their codec ceilings. The outer project API envelope has a separate
finite admission policy: 1 MiB + 8192 encoded bytes and aggregate decoded key/string
bytes, depth 18, 16640 values, 1024 entries/collection and 64 KiB per decoded
key/string. Traverse the complete envelope before bounded in-memory serialization;
the writer counts actual encoded punctuation and escaping too. A storage-record
admission alone does not admit its wrapper. Paginated discovery never materializes
complete actor history.

Use 201 for confirmed registration/new creation, 200 for reads/equal replay and
confirmed/idempotent cancellation (whose returned state may already be completed),
400 for malformed/admission input, 401 for unknown/stale credentials, 403 for
current authority/CSRF denial, 404 for absent scoped references, 409 for exact
token/content/state conflict, 410 for execute/reconcile terminal expiry/cancellation
or replay expiry (open remains a 200 state read),
429 for bounded admission/quota contention, and 503 for unavailable/unknown commit.
Machine-readable `outcome_unknown` is distinct from known rejection; preserve
identity and reconcile. Unexpected integrity faults are unavailable/corrupt state,
never implicit fresh creation or blind transient retries. Named target collision
and supported transient errors alone may re-enter after complete rollback, within
the original deadline and a reviewed attempt bound.

Unknown registration commit is a distinct recovery state: the client may have
received no request number. It refreshes authenticated pending discovery and asks
the owner to identify the saved request. Equal form content is not an idempotency
identity; neither the interface nor server silently registers it again as recovery.
Unknown execution commit preserves the known request number for reconciliation.

## Admission, transaction and response ownership

Raw input checks precede any application queue or per-request SQL. Bounded owned
entropy preparation runs off the async executor; generated request/result/snapshot
admission precedes persistence-queue/database work. The listener retains its bounded
connection ownership throughout preparation; existing CPU-lane ownership and actual
thread joining remain. No unbounded preparation queue or extra hash pool.

Preparation registers synchronously in an owned, bounded task set and consumes
the same aggregate admission slot as the eventual execution/reply. An HTTP
disconnect drops only the result receiver. The owner settles preparation tasks
and their engine-reference barrier before unwrapping the engine, joining any
remaining OS worker thread and releasing database ownership. Publication of a
worker result is not proof of thread exit; that slot remains held until joining.

Execution preparation seals exactly three prospective target candidates before
execution admission. Duplicate random targets reject the bounded batch; there is
no generate-until-distinct loop. A supported collision/transient retry consumes
the next immutable proof only after confirmed full rollback and serving-lock
release, then rechecks liveness/deadline and reacquires current authority. It
never generates entropy or new envelopes while holding database fences.

Fenced execution compares the immutable server token/content before accepting a
prepared candidate. All writes, completion references, typed relational reload and
deferred constraint checks precede COMMIT. Lost confirmation remains unknown.
Historical reconciliation is a separate path independent of prospective validation.
The final `complete_project` check, after the installation/actor and pending-row
locks, is execution's expiry ordering point. Earlier opening or preparation does
not reserve eligibility. If that check reports expiry, roll back every creation
write before reconciling the pending entry; never commit a partial project.

Project read uses a read-only repeatable snapshot of public authority, current
grant/latest-event agreement, project and current configuration/history. It does
not hold the exclusive creation actor fence through response delivery. Every data
response retains session/live-generation/monotonic-expiry eligibility through writes.
Actual-role constraints, state races, cancellation/unknown commit, expiry/pruning,
admission counters and independent relational read-back must verify these claims.

## Interface recovery

The interface discovers server-owned saved requests after sign-in. Discovery,
opening an entry and page reload never execute it. Saving and creating are
separate explicit actions; execution echoes the immutable opened form/token,
not editable registration inputs. No browser database or storage is authoritative.

Keep an unconfirmed execution/cancellation locator separately from the displayed
selection. Discovery or reopening a prepared entry cannot erase its uncertainty.
Explicit reconciliation may report only that it was not completed at that lookup;
an earlier attempt can still complete. A later explicit retry uses the same saved
identity. Completed cancellation responses preserve the existing project. Replay
expiry preserves project identity and permits a separate fresh authorized read.
Failed read-back cannot turn a confirmed creation into a new creation attempt.

Known registration validation/capacity rejection restores the exact editable form.
Lost, malformed or unavailable acknowledgement instead requires discovery and
owner identification, never automatic equal-form registration. Installation/actor
comparison precedes project actions; changed binding clears old actionable state,
and unavailable binding disables project controls until session recovery. Response
reading enforces the wire byte limit on actual streamed bytes before JSON parsing.
Deterministic page tests exercise these state transitions without claiming real
Windows browser, cookie, TLS or native-storage qualification.

## Certificate and execution windows

The product listener enforces the certificate's validity interval, with an initial
monotonic deadline that a UTC rewind cannot extend and periodic fail-closed UTC
checks. The experiment's three-minute process cap is not a product availability
requirement. Request, session and owned-shutdown deadlines remain separate. Each
development workload still requires explicit finite external supervision and
owned cleanup; this separation authorizes no longer test run, supported resume,
installation or production-availability claim.
