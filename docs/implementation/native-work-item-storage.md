# Native WorkItem storage packet

Status: owner-only structural checkpoint passed under Accepted ADR-111 revision 3;
product capabilities, installation and runtime adoption remain incomplete.
Baseline: `b81b64c`, with the codec checkpoint published and verified separately.
The initial RAM hold was lifted on 2026-11-04 for bounded codec verification.
The SQL overlay has separate PostgreSQL evidence; codec verification does not
qualify these records or their future serving capabilities.

This is one coupled storage checkpoint: record keys, transition/history guards,
fixed entries and their owner-only qualification must agree before runtime
adoption. Keeping that invariant set together justifies its size; it does not
expand the PR's accepted outcome. The review map is records/retention/closure
for permanent identity, heads/history for accepted-state agreement,
authority/pending/discovery for scoped intent handling, reconciliation/save for
historical versus fresh execution, and the opt-in checker/fixtures for bounded
evidence. Production SQL, test harnesses and documentation are separate review
surfaces; there are no generated files. Installer/profile and Rust/API/interface
integration remain the next components. The source hold ends after this packet's
required gate and exact-head review; no full-goal readiness is implied.

## Scope and prerequisite corrections

Implement fresh product schema version three for pending ItemCreate, direct
ItemEdit, permanent item revisions and operational heads. Preserve the shared
version-one foundation and ProjectCreate representations. This is not an upgrade
of a populated version-two database and does not adopt ADR-108 or ADR-109.

The baseline has an unconditional `operations.operation_success_mapping` foreign
key into project creation intents. It must become family-exact deferred closure,
not simply disappear when the operation-family guard is extended. The existing
`projects.project_success_mapping` and creation snapshot/grant protections stay.
Allow exactly ProjectCreate, ItemCreate and ItemEdit with their original grant
labels and policy version one. Preserve all other operation identity/retirement
guards. Seed/configuration/optional-text restrictions stay until their own scope
is implemented.

The codec's pre-admitted history JSON must not contain its own newly allocated
operation-row ID: that ID is generated inside the write transaction. Keep it in
the relational revision row and verify full attribution on reload. Actor,
configuration revision, successor state version, previous operation and accepted
result are known from authorized preparation. No guessed sequence ID, read-side
allocation or post-admission JSON rewriting is allowed.

## Physical records and integrity closure

| Record | Required stored facts and keys | Closure |
| --- | --- | --- |
| `kehila_auth.pending_items` | Internal noncycling positive request number for pagination; installation, actor, project, ItemCreate tag; opaque 128-bit locator; operation token; trusted random item target; versioned canonical request bytes and fingerprint; registration/expiry and prepared/completed/cancelled/expired state; nullable successful core reference | Unique scoped locator and unique reserved item target. Immutable identity/content until terminal content removal. Full core actor/family/project/item/token agreement at completion. Successful metadata remains permanent. |
| `kehila.items` | Stable item/project identity, issued prefix and project-local sequence, immutable creation request/core, current item-version pointer | Item must reference its originating pending target. Unique `(project, issued_sequence)`; issued prefix/number never change or recycle. Deferred current-version FK requires a matching immutable revision. |
| `kehila.item_revisions` | Item version, configuration revision, actor, family/token and database operation ID; type/workflow/status/phase/archive, exact optional estimate/unit; versioned admitted history JSON; project operational version, predecessor operation and resulting sequence/estimate flag | Full operation target/actor/family/token FK, not the old five-column attribution key alone. Unique operation and unique `(project, operational_version)`. Item/configuration references and current/head closure are checked before commit. |
| `kehila.item_field_uses` | Project, owning type, field ID, first attributed revision/operation where a value was accepted | Append-only retained use survives field clearing and replay retirement. FK definition ownership cannot change or disappear. No current-value-only history inference. |
| Project operational head | Version zero points to creation; later head points to the exact item revision operation/version | Indexed latest-boundary check, predecessor transition and live Project values agree. Every accepted item mutation advances this version, including edits with unchanged counters. |

Use a six-component item attribution key plus token on operations where required:
operation row, actor, family, target kind, project, item and token. Existing
ProjectCreate foreign keys continue using their unchanged checked project key.
Positive database row IDs must fit PostgreSQL bigint even when a codec represents
an external metadata value as u64.

The reserved target belongs to a prospective item. Do not require an item to exist
before registration. Instead, each actual item references its reserved pending
target, and completed pending metadata remains permanent. This closes uniqueness
across pending and existing items without a second target catalogue. Deletion of
never-created cancelled/expired entries cannot authorize delayed execution.

The family closure guard must reject these final states independently of the
adapter: orphan core/payload, project core without creation intent, item-create
core without matching completed request/item/revision, item-edit core without
matching revision, item without current revision, newly accepted revision without
its atomic item/project head transition, skipped/regressed project state, and
mismatched predecessor. Retained older revisions remain valid after later
commands advance the heads; closure must not require every historical revision
to equal the current head.
Maintain logical expiry and no-gap retirement separately from product history.

## Write entry and narrow capabilities

Capability preparation starts with a private item write fence, followed by fixed
registration/cancellation entries. Source-first independent challenge confirmed
that the existing project-creation fence cannot be reused: it requires the
separate permission to create another project. The item fence must independently
validate complete expected authority, acquire installation then actor then project
locks, and read coherent current grant/latest-event evidence after those waits.
It must not reject archived/currently changed item state before resolving old
success. Schema-owner fixtures can exercise these checks but cannot prove actual
runtime privileges, session release, observed waits or revocation composition.
Registration capacity/TTL and cleanup ownership require the explicit bounded
contract below; legacy project quotas are not implicit item policy. Read-only
discovery must not copy legacy mutating expiry behavior.

Registration/cancellation preparation uses explicit containment defaults: 32
physically prepared requests per installed owner across projects, at most 64
never-completed metadata rows per owner/project, a 16-row discovery page and a
24-hour lifetime. Each canonical request remains at most 1 MiB. Counting elapsed
but still physically prepared rows bounds prepared request bytes to 32 MiB; this
does not bound database overhead, terminal metadata across arbitrarily many
projects, or retained successful request content. These are containment choices,
not measured capacity. Actor fencing precedes quota checks across projects.

Registration expires/prunes only a bounded selected set of never-completed rows
in its protected project. It neither scans successful history nor acquires locks
in other projects. Cancellation's transition guard samples trusted time after
the pending-row wait and clears content as cancelled or expired; completed
identity is unchanged.
Both return bounded identity metadata rather than full request content. Expired
rows in another project still consume prepared capacity until explicitly cleared
there; discovery must report effective expiry and quota errors must explain that
recovery path. Independent challenge required prepared-only owner and
project-scoped never-completed indexes so bounded matching-row counts do not
conceal scans of unrelated terminal or successful history.

Locator, operation token and item target are generated by the trusted backend
before canonical encoding, then supplied to the fixed registration entry. They
are never client-chosen authority. Collision handling belongs to the bounded
caller under the original deadline; SQL does not retry or deduplicate equal
user forms. Registration stores intent only, without allocating an issued number,
item or operation success. Typed request/scope agreement remains an application
admission/reload obligation; digest equality alone cannot establish it.

The tests-first `work_item_pending.sql` candidate and its separate-backend state
observer now cover registration/cancellation, exact retained bytes, invalid
version/empty/oversized requests, duplicate identity, current authority/grant
checks on both entries, completed/missing cancellation, constructed expiry and
prepared capacity. A rollback-only second project isolates owner-wide capacity,
project-scoped cancellation and maintenance, and the fact that elapsed prepared
bytes in another project continue consuming quota until explicitly cleared.
The checker selects both files and has failure-propagation controls for each.
Independent source review found no fixture reachability or relational-closure
blocker to the expected missing-entry failure. Python source syntax
passed at that preparation checkpoint. These tests were then
unexecuted and fixed entries absent; the subsequent execution record below
supersedes that status. Earlier memory admission failures remain failures.

The private `authority.sql` candidate now implements that fence without a serving
EXECUTE grant or independent SECURITY DEFINER boundary. It requires every
expected-authority component, binds the freshly fenced owner, locks the project
and then verifies the current active grant against its latest event and immutable
creation origin. Its version-one restriction matches the current one-owner grant
schema; future grant administration must deliberately evolve that contract.
Independent source review found no substantive blocker to native verification.

The tests-first candidate at tree `8ff150b5d9bfa915f878c7f2a5c59f607dd19829`
passed thirteen checker controls and earlier structural/history checks, then
failed at the first new positive authority call with missing-function SQLSTATE
`42883`. This establishes fixture reachability, not mutation sensitivity for all
authority cases. Cleanup completed and provider shutdown was confirmed.
The implementation candidate at tree
`0955096adfc9ae9f6ac78aaf11f2532377d9588d` passed thirteen checker controls,
but its next campaign failed the existing 3 GiB host-memory admission after
database startup, before SQL installation. The attempt took 41.000 seconds
through cleanup; owned resources were removed, the provider stopped and the
controller joined without termination. No authority case passed in that attempt.
The minimum sampled available RAM was 2.973 GiB and aggregate peak 1.581 GiB.
Do not relax admission or treat this resource rejection as SQL qualification.
After cleanup an independent observation showed 4.481 GiB available, justifying
one fresh attempt at the unchanged tree and limits. It again passed thirteen
checker controls but failed the 3 GiB database-check admission before SQL
installation. It took 42.297 seconds through cleanup, with owned resources
removed, provider stopped and a normal joined controller. Sampled minimum
available RAM was 2.844 GiB and aggregate peak 1.543 GiB. The pre-start recovery
did not predict sufficient post-start reserve; do not keep retrying merely on a
similar pre-start observation. Retain both resource failures and the earlier
expected missing-function failure. At that point authority qualification was
still unexecuted; the subsequent checkpoint below records the resumed campaign.

Own installation/actor/project/pending/core/item acquisition in one fixed command
entry path. Installation and actor locks precede the project lock, then pending
creation row, existing core SHARE and item where applicable. No helper invoked
after a later-class lock may acquire a new earlier-class lock. Avoid assembling
the protocol from independently callable helpers that re-fence unpredictably.

Proposed function packet for exact SQL review:

| Capability | Contract |
| --- | --- |
| Creation registration | Current installed-owner/session generations and active coherent project mutation grant; bound request bytes; server-generated locator/token/target; no item allocation or sequence increment. Register only new immutable content under bounded pending policy. |
| Pending discovery/open | Authorized read-only bounded snapshots. Return bounded summaries for lists; fetch canonical request bytes only for individual open/preparation. An elapsed prepared request is effectively expired even before physical maintenance. |
| Creation cancellation | Same write fence and pending lock as execution. Cancel only prepared/unexpired work; committed creation is reported completed and never compensated. |
| Command resolution/persistence | Fixed ItemCreate/ItemEdit modes, current original-grant authorization and full scoped lookup before mutable checks. Historical-only resolution cannot fall through to creation. Fresh persistence requires completely admitted result/history and protected preparation agreement. |
| Item read/list | Fresh consistent authority/current-grant snapshot, bounded item data/summary projection, no mutation or execution side effect. Final live-session release check stays in Rust. |
| Owner retirement | Not granted to runtime/admin-enrollment capabilities. Retire only eligible core/payload; preserve permanent request binding, item revisions, field use and operational history. Successful pending-content pruning is a separate maintenance transaction. |

Serving receives new SELECT and only explicitly enumerated EXECUTE capabilities,
not broad table UPDATE/DELETE/TRUNCATE/TRIGGER, retirement or DDL. State/head
changes occur within fixed owner-defined functions. Function ownership, trusted
search path, PUBLIC revocation and effective-role checks are reviewed together.
New APIs retain the combined trusted Rust authority boundary; these functions
are not independent end-user authentication endpoints.

Item replay retirement acquires only core/payload locks. It must neither update
pending content nor invoke a deferred guard that acquires pending, item, project
or actor locks. Execution can already hold a pending lock while waiting for the
core; a retirement path holding the core and waiting for pending would form a
cycle. Prune eligible successful pending content in a separate transaction after
retirement releases its locks. That path protects the pending row, reads the
irreversible retired-core fact without locking the core, and preserves permanent
locator/token/target/completion metadata. Concurrent pruning cannot change command
identity or authorize execution. Keep existing ProjectCreate maintenance behavior
separate: its earlier installation/actor fence is not evidence for this item path.
Readers must accept retired replay payload with temporarily retained successful
pending request content between these transactions. Content may be absent only
after eligible retirement, but retirement does not require immediate pruning.

Item authorization uses active coherent `initial_owner` project grants. Revoking
the actor's separate permission to create another project does not by itself
remove existing item rights. Verify current grant/latest-event agreement after
the protecting waits. Mutable project/item eligibility cannot mask old success:
archive/version/configuration validation follows recorded-operation resolution.

## Prepared and protected facts

An independent lock-composition challenge after the sequential save campaign
identified an ordering gap: `item_revisions.previous_project_operation`
references an external predecessor core. PostgreSQL 16's
[RI checks](https://raw.githubusercontent.com/postgres/postgres/REL_16_STABLE/src/backend/utils/adt/ri_triggers.c)
take a referenced-row KEY SHARE lock; the candidate could first acquire it after
the mutable item fence. The resolver's requested-token lookup does not prehold
that predecessor for an unseen command. This violated the accepted core-before-
item protocol; it was not an observed deadlock. The correction fences the
actual protected project-head predecessor FOR SHARE before item acquisition,
then retain the existing fresh post-wait dependency comparisons. SHARE matches
the accepted boundary and excludes maintenance NO KEY UPDATE; KEY SHARE alone
does not, per the [row-lock conflict table](https://www.postgresql.org/docs/16/explicit-locking.html).
Later immutable revision/configuration/definition references and new rows owned
by the same transaction are separate parts of the physical lock graph, not a
claim that all later SQL is lock-free.

A bounded container control was prepared before that correction.
It positively holds the actual predecessor core in maintenance NO KEY UPDATE,
observes a writer blocked by that backend, and independently probes the item
NOWAIT before releasing the blocker. The writer must accept then roll back;
scoped state counts and backend cleanup are checked. The old-source expected
failure requires accepted rollback without any predecessor wait, not a crash,
timeout, replay or absence alone. This is a core-contention ordering control,
not actual payload-retirement qualification. The sequential save pass does not
settle this material lock-composition gate before dependent integration. Actual
results and the corrected malformed-history oracle are recorded below.

The relational predecessor is derived from that fenced head, rather than the
supplied history. The immediate history-agreement guard checks the unchanged
envelope against it before foreign-key checks can route a malformed predecessor
to another external core. New operation references are owned by the transaction;
immutable revision/configuration references remain separate physical FK locks.

This ordering analysis assumes exactly one admitted command per writable
transaction, against previously committed project/item rows, with no earlier
writes to those rows. PostgreSQL 16's
[`RI_FKey_fk_upd_check_required`](https://github.com/postgres/postgres/blob/REL_16_STABLE/src/backend/utils/adt/ri_triggers.c#L1194-L1285)
skips unchanged FK keys on those old tuples; it can require a recheck when their
`xmin` belongs to the current transaction. The allocation/estimate update leaves
the project's creation-core key unchanged, as does an item's version-pointer
update for its creation binding. Head zero's creation core is already the fenced
predecessor. This does not establish lock ordering for multiple saves, preceding
project/item writes or project-create-plus-item batching in one transaction.
Future Rust integration must own one command through constraint forcing and
commit, or full rollback before retry/repreparation. The uninstalled SQL packet
does not enforce transaction ownership for arbitrary owner-written SQL.

Fresh-save preparation uses a private typed SQL packet containing the current
Project snapshot, configuration revision/operation and immutable configuration
snapshots, operational head version/operation, and the current item
version/operation/history for editing. Creation leaves item dependencies null.
Each JSON field is capped at 1 MiB; the aggregate packet is larger than an
individual record and must fit the application's existing shared capacity.
History is the sole admitted accepted-state envelope; SQL projects its fixed
result fields for persistence rather than applying sparse edits or prospective
business rules. Recorded resolution precedes packet inspection. PostgreSQL
still evaluates argument casts/domains before function entry, so this bypass
applies to well-typed or null packets, not malformed SQL casts.

Private constant read/write resolver modes share scoped historical checks.
They never acquire installation/actor/project fences themselves. Read mode
requires RR read-only and takes no locks; write mode requires RC and takes
creation-pending then existing-core SHARE locks after its caller's project fence.
Write create echo disagreement is a request conflict after stored binding
integrity is established; read-side stored disagreement remains corruption.
Fresh saves compare configuration/item dependencies before deciding operational
re-preparation, perform no automatic retry and require full rollback/reentry on
stale operational state. Parent Rust owns typed reload, forced constraints,
COMMIT and final release.

Creation completion's transition trigger owns its final clock sample. Once
immutable identity, exact content and completion shape agree, elapsed completion
raises only `P0002 / pending_item_expired`; the entire write transaction must roll
back. Other invalid transitions remain integrity failures. Deferred operation
attribution may not yet have run at that expiry point; an earlier expiry error
does not prove all other supplied attribution was valid. The adapter must match
the narrow expiry signal and never catch all constraint errors or continue an
aborted transaction. Guard-level constructed-time controls do not qualify an
observed real-time deadline crossing or lock wait.

Historical item reconciliation is a separate read-only RR capability before
fresh persistence. Create resolves its immutable scoped pending binding; direct
edit supplies its retained exact request/token and has no pending record. A
private core inspector accepts only a trusted scoped database row, takes no
locks/fences and is reusable by the later fixed writer after core SHARE. It
checks retirement/payload presence even for expired records, distinguishes
changed permanent identity from expiry, and performs exact bounded request-byte
comparison for live replay. A missing live pending request is corruption even
after logical expiry; retired content may be absent. Completed creation must
agree with its actual item/creation revision/core tuple before reporting history.
The bounded resolution includes fresh authority and core identity, never an
unverified saved result. Internal replay availability requires subsequent Rust
typed reload/attribution checks in the owned snapshot and final session/expiry
eligibility before client success. Reconcile never calls a write fence or
reconstructs missing work. Independent source-first challenge confirmed this
component boundary; fresh write entry remains required afterward.

Pending discovery/open preparation requires a read-only REPEATABLE READ
transaction and the existing nonlocking authority path. Complete expected owner
and verifier revision are additionally bound, with coherent current CreateItem
grant evidence in the same snapshot. Shared grant validation must preserve the
write fence's fresh post-wait statement. Lists return at most 16 summaries plus
one keyset sentinel, never request bytes. Individual open uses a SQL byte sentinel
before returning at most 1 MiB. Effective expiry is projected from one clock
sample per call; expired/cancelled content is omitted, and no physical row is
changed. Completed metadata accepts either retained or pruned request content.
Final Rust live-session release and typed scope/decoding remain separate gates.
Independent source-first review confirmed these prerequisites before candidate
implementation; neither read calls nor login/discovery may execute saved intent.
The clock-dependent public reads use VOLATILE classification, while the shared
grant predicate is STABLE. This follows PostgreSQL 16's
[volatility and snapshot rules](https://www.postgresql.org/docs/16/xfunc-volatility.html):
the writer calls grant validation in a new statement after its project wait;
readers retain their required repeatable read snapshot. Native post-wait and
revocation controls remain necessary before application qualification.

The immutable request is identity; a preparation snapshot is not. Compare the
complete preparation dependency set at write entry: Project stable/configuration
facts and operational head version/operation/sequence/ever_estimated, current item
version/content for editing, and pending identity/content/state for creation.
Another item's edit can invalidate the head even when sequence/estimate values
are unchanged. Re-prepare only operational-state changes under the same command,
deadline and capacity permit; item/configuration conflicts do not silently rebase.

The SQL persistence path must return current fresh authority plus its outcome/core
identity to the private Rust transaction owner. The Rust owner performs bounded
reload, typed comparison to the single accepted result/history, deferred-constraint
validation and COMMIT before successful delivery. The SQL function itself cannot
commit. Known rollback and uncertain COMMIT/response remain distinct outcomes.
No client-facing request chooses an actor, trusted creation target or grant.

## Operational transition and snapshot rules

Item revisions are unique by `(project_id, item_id, item_version)`. Creation
establishes exactly version one. Editing advances exactly from the protected item
head by one with checked overflow, retaining a verifiable predecessor. The
current pointer cannot skip or regress; a matching current-version FK alone is
insufficient. Insert the new revision and advance the item head atomically with
the corresponding project transition and operation. Historical revisions remain
immutable after either head advances again.

Creation supplies state version zero and immutable creation snapshot. Every new
item command appends exactly one successor, checking u64 increment overflow.
Creation increments readable sequence exactly once; editing leaves it unchanged.
New `ever_estimated` equals previous value OR an estimate present in the accepted
item revision, including zero. A caller boolean is not an estimate witness.
Ordinary edits preserve type/workflow/status/phase and do not archive an item.

The current Project read verifies stable metadata/configuration against retained
snapshots, then compares its operational head and values with attributed permanent
revision evidence and the indexed latest project-state boundary. Creation replay
still returns the original creation result. Do not rewrite creation snapshots or
use the current Project as a replacement replay result.

This bounded read relies on enforced append-only transitions. It does not freshly
audit every old row or defend against arbitrary privileged DDL/tampering. Review
each claimed corruption oracle against actual constraints and query predicates.

## Materialization and interface bounds

Raw bytes enter through checked parsing before shared scheduling/database work.
Preparation is read-only, authorized and shares the existing permit/deadline and
serving connection. Admit canonical request, result and history in their complete
envelopes, including native JSONB expansion, before write entry.

Individual reads use SQL octet-length/count sentinels before returning stored
JSON/text/bytea. List only bounded summaries, never one full history/result payload
per row. Use an explicit bounded page policy and keyset cursor over issued sequence;
each page reauthorizes and does not promise a cross-request stable snapshot.
Title fallback is computed from the configured title field for display, preserving
truthful missing/blank stored data and Rust's established whitespace semantics.

Do not duplicate a full pending request inside an execution reply containing a
full saved result. Return terminal identity metadata with the result; individual
open supplies immutable content separately. Verify the aggregate reply fits the
existing bounded client response reader, not merely each nested storage envelope.

## Version-three adoption and source-only verification matrix

Wire the module into the fresh product installer only when its capability set
and corresponding runtime profile are implemented together. Keep installation
transactional, fail on populated/unknown state, and never repair schema at runtime.
Readiness must require exactly versions `[1,2,3]` and detect extra rows with a
four-row sentinel, rather than accepting the old three-row prefix. Enumerate all
new table/sequence/function privileges and retain unexpected-privilege negatives.

| Case | Required eventual oracle |
| --- | --- |
| Family closure | Actual serving role cannot commit missing/wrong project or item completion/revision; legacy project constraints remain effective. |
| Identity | Wrong actor/project/item/token/locator/core cannot bind; independent registrations cannot adopt another target; cancelled/missing execution cannot recreate it. |
| Head/current state | Current/version/predecessor mismatch, duplicate issuance, skipped/regressed head and false unlock reject atomically. Unchanged-counter edit still advances state version. |
| History | Zero then clear-last-estimate retains flag; cleared field use/type/workflow/status history survives payload retirement and pending content pruning. |
| Authority | After observed lock waits, revoked/current generation/grant is rechecked. Original mutation grants govern replay; read alone is insufficient. |
| Admission | Corrupt/oversized stored records fail before materialization; generated overflow causes no write entry. Historical replay bypasses current domain preparation. |
| Contention/uncertainty | Held-lock same command/distinct creates/two edits; write rollback, post-COMMIT response loss and original-identity reconciliation with independent SQL totals. |
| Privileges/readiness | Extra schema version, unexpected function execution, broad UPDATE or missing required capabilities fail startup; runtime cannot enroll, retire or migrate. |
| Cleanup | Required native campaign observes joined owned work and released runtime connections/locks; source inspection alone is never a pass. |

The SQL overlay is qualified only by the focused checkpoint below. The codec
candidate was captured after its history-attribution correction and qualified
separately. These SQL sources were not included in that earlier full gate.

## Prepared source and review record

The records-only candidate is
[`work_items/records.sql`](../../storage/postgresql/work_items/records.sql).
It defines reserved creation identities, full operation attribution keys,
item/revision/configuration references, exact item-version predecessor shape,
retained field-use references and project operational-head storage. It is not
included by an installer and grants no new serving capability. Existing
creation-only operation guards and the project-intent reverse FK remain intact.
This candidate alone cannot persist item operations or establish complete
transition integrity. The subsequent transition/closure functions must enforce
the contracts above before coordinated installer/profile/adapter adoption.

Independent source review of this packet identified and resolved two issues:
core-first retirement cannot also prune pending content, and a current-version
FK cannot substitute for exact item-version transitions. The revised plan
separates maintenance transactions and preserves the valid interim retained
content state; it explicitly requires item version one, checked successors,
no head skipping/regression and valid older history after head advancement.
The reviewer found no remaining substantive planning blocker to preparing DDL.
Exact DDL review was separate from that document verdict. At that review no SQL
installation, role/constraint check, query plan or contention observation had run.

The independent records-only review found a further invariant gap: identity
generation alone permits explicit zero/negative overrides. The candidate now
adds a named positive operation-ID CHECK, so referenced operation identifiers
use the required positive signed-bigint vocabulary. The review found the
composite-key/null behavior structurally coherent. It also distinguished the
remaining guard obligations from FK guarantees: same-project predecessor/head,
history-to-relational agreement, first field-use content/type evidence, creation
completion agreement and append-only/reverse closure are still to implement.
This disposition is based on inspected source, not observed PostgreSQL behavior.

The separately prepared
[`work_items/retention.sql`](../../storage/postgresql/work_items/retention.sql)
adds immediate item identity/version, append-only history and pending lifecycle
guards. Source review confirmed that its direct scalar comparison covers all 13
immutable pending columns without copying a full request into JSON. Successful
content pruning reads an irreversible retired-core fact without taking a core
lock. Separate retirement/pruning transactions remain an entry-owner/test
obligation, not something that trigger proves. These guards are also outside the
installer and have no runtime grants. Deferred completion/head/history closure,
fixed write entries, authorization and actual-role qualification remain missing.

The uninstalled [`work_items/heads.sql`](../../storage/postgresql/work_items/heads.sql)
candidate adds exact operational transitions and deferred current-boundary checks.
The write entry must insert the successor revision before advancing its head and
must already protect the project/item. Each head step validates predecessor,
sequence and estimate history; deferred checks compare final heads with indexed
latest boundaries, allowing valid older revisions after subsequent steps. Queries
project scalar facts and do not load history JSON or acquire explicit row locks.
Review found no transition/batch-history counterexample under that entry contract.
It did find that strict missing-row lookups could leak `P0002`, which the existing
project coordinator treats as eligibility/reconciliation control flow. Named
`23514` integrity errors now preserve that distinction; the reviewer confirmed
the correction in source. PostgreSQL installation/execution was still pending at
that review. Family/completion closure was then prepared separately; JSON
agreement and capability entry implementation remain dependent source work.

The uninstalled [`work_items/closure.sql`](../../storage/postgresql/work_items/closure.sql)
replaces the project-only reverse FK with family-exact deferred completion checks.
It preserves the existing ProjectCreate orphan SQLSTATE/constraint identity and
links ItemCreate to its revision, actual item and completed immutable pending
binding. Live item replay bytes must match the retained digest and replay results
must match permanent history. A narrowly owned trigger supplies the execution
context for its private helper; review caught and resolved the earlier invoker
permission gap without exposing helper execution to serving. The head initializer
also seeds version zero atomically for existing ProjectCreate INSERTs. Pending
request fingerprints use PostgreSQL 16's built-in
[`sha256(bytea)`](https://www.postgresql.org/docs/16/functions-binarystring.html).

## Focused structural qualification

[`check_work_item_storage.py`](../../scripts/check_work_item_storage.py) is an
opt-in control-plane checker reusing the existing pinned, networkless PostgreSQL
container admission and strict psql error handling. It refuses existing state,
installs a fresh product baseline plus transactional test overlay, probes private
helper/head-write denials with `SET ROLE`, commits an owner-authored create and
zero-to-clear edit, then retires replay and prunes successful content separately.
The independent [observer](../../tests/integration/work_item_storage_state.sql)
uses a new backend and checks exact retained core/revision/head/pending linkage,
payload absence, fixed request digests and estimate history. Source review caught
and resolved a weaker earlier observer. It also checks that name resolution is
bound to one full container ID before any write. Controller failure-propagation
tests were prepared. Their first containerized run passed all ten controls;
the SQL fixture was still pending at that point.

This is owner-only structural evidence preparation, with limited `SET ROLE`
negative privileges. It does not qualify a runtime login/session profile, full
typed-history agreement, all attribution/predecessor corruption cases, concurrent
serving transactions or the API/interface. Product installation remains unchanged.
The execution wrapper must capture an immutable reviewed public-source snapshot,
retain the established memory/ownership/deadline controls and confirm owned
container/provider cleanup before reporting a passed campaign. Actual resource
observations and test outcomes were unknown at that planning checkpoint.

The first focused attempt subsequently failed at startup-descendant joining,
before the new controller tests or SQL fixture ran. The startup command returned
zero, but its owned tree still contained a descendant when the join allowance
expired. The tree was then terminated/joined and provider shutdown was confirmed.
The surviving process was not identified by the retained observations. Memory
was not sampled during that join interval; its sampled minimum/peak must not be
presented as continuous bounds. The revised execution composition keeps one
controller alive through startup, work and shutdown before its ordinary job join.
The failed attempt is retained. The revised composition passed eleven focused
lifecycle command-double controls and then ran the ten checker controls inside
the toolchain container. It started PostgreSQL and reached overlay installation,
where PostgreSQL rejected an unparenthesized CASE expression in the PL/pgSQL head
condition with SQLSTATE `42601`. The overlay failed before the structural fixture
ran. Owned resources were removed, the controller tree joined and provider
shutdown was confirmed. The expression is now grouped explicitly, preserving the
same sequence rule; a fresh native run was then required. The failed SQL attempt
is not a passed constraint check or a completed structural campaign.

### Passed structural checkpoint, 2026-11-04

The corrected snapshot `ee49b22a0f8a40143cfd57df1f800cd1f85aae10`, based on
`b81b64c`, passed the focused campaign from 20:53:35 UTC in 50.594 seconds through
cleanup. Ten checker controls passed in the isolated toolchain container. Actual
PostgreSQL overlay installation then passed, with committed creation and direct
editing, 12 selected SQLSTATE/constraint rejection controls, replay retirement,
separate successful-content pruning and a new-backend committed-state observer.
The observer checked exact core/head/revision/pending linkage, request digests,
retired payload absence and zero-to-clear monotonic history. This is structural
owner evidence plus the selected `SET ROLE` denials, not runtime-login or API
qualification. Both earlier failures remain failed evidence.

Owned resources were removed; the provider was confirmed stopped and the
controller job joined without termination. The configured database used 256 MiB,
one CPU, 128 PIDs, a 256 MiB data tmpfs, eight connections and 16 MiB shared
buffers. Sampled aggregate working sets peaked at 1.607 GiB; minimum sampled
available RAM was 3.221 GiB, without a warning or stop event. No memory saving,
continuous peak, latency guarantee or broader concurrency claim follows.

Measured nested spans were 20.407 seconds for controller tests through cleanup,
3.531 seconds for database creation/readiness and 7.109 seconds for combined SQL
installation/overlay/fixture/observation. SQL subdivisions and other elapsed time
were not isolated; nested spans are not added to the parent duration. Earlier
source/review time remains unknown.

Independent read-only qualification review inspected the retained parent/child
results, execution log, prior SQL failure and source delta. It found no material
evidence-validity blocker to capability source work. Reuse this evidence while
its SQL/fixture/observer/environment premises remain valid, and revalidate
affected cases after material changes. Current residual obligations are exact
serving privileges and startup profile; fixed-entry authority/protected-state
agreement; full typed history/field-use correctness; observed contention and
rechecks; coordinated version-three installation and ProjectCreate regression.
The full authenticated WorkItem goal remains incomplete.

### Fixed registration/cancellation checkpoint, 2026-11-05

After additional RAM became available, tree
`cce824d140086dbee88a0bd87f28e077dfb4fdd5` passed fifteen in-container checker
controls and the existing structural/history/private-authority fixtures, then
failed at the intended missing registration entry with SQLSTATE `42883`. That
attempt remains failed overall, despite establishing the reached private-fence
controls. It completed owned cleanup and provider shutdown in 49.547 seconds.

The new `pending.sql` implements fixed owner-defined registration/cancellation,
bounded identity summaries, explicit containment policy and supporting partial
indexes. It remains outside the product installer and grants no serving EXECUTE.
Source review found a double-clock cancellation race: the entry could classify
unexpired cancellation before the trigger sampled an elapsed deadline. A new
constructed-expiry regression failed with named `pending_item_transition`
SQLSTATE `23514` at tree `df4b979a63115255459f92940393e0bf27ec9a33`. It took
48.453 seconds through owned cleanup and stopped-provider confirmation. The
trigger now owns classification and converts elapsed cancellation intent to
expired; the entry returns persisted state. Identity/content/null-operation
checks, completed permanence and expired-completion rejection remain intact.

Corrected snapshot `94d43d8cd9060b5643fa0b619124bcb5965d8983`, based on
`b81b64c`, passed the complete focused campaign from 01:40:00 UTC on 2026-11-05
in 51.156 seconds through cleanup. Fifteen checker controls passed in-container.
PostgreSQL passed the structural/history/private-authority fixtures plus fixed
registration/cancellation, stale authority and inactive grants, invalid admission,
duplicate identity, missing/completed/wrong-project cancellation, constructed
expiry, owner-wide prepared capacity across projects and maintenance isolation.
A separate backend verified exact committed request bytes and fingerprint,
prepared/cancelled/completed identities, retained history/core cardinalities and
unchanged item count/issued sequence/operational head. Rollback-only quota and
second-project fixtures left no additional committed product state.

Owned resources were removed, the provider stopped and the controller joined
without termination. Sampled aggregate working sets peaked at 1.681 GiB; minimum
sampled available RAM was 4.047 GiB, with no warning or stop event. Nested spans
were 19.110 seconds for checker controls through cleanup, 3.187 seconds for
database creation/readiness and 11.156 seconds for combined SQL work. SQL
subdivisions and other elapsed time remain unisolated; nested durations are not
added to the parent. Both reached expected failures remain failed evidence.

This proves selected owner-level relational behavior and private-fence checks,
not actual serving login/session privileges, observed lock waits/revocation,
typed request/history reload, discovery, item execution/reconciliation or API/UI.
Those contracts, coordinated installation/profile adoption and required final
commit/CI/composition gates remain outstanding.
Independent read-only review inspected the exact source, retained failures,
passing parent/child records and observer selection. It found no material
evidence-validity blocker to the next source component within the accepted scope.
Quota controls in rollback scopes establish SQL decisions, not concurrent or
durable saturation qualification; constructed timestamps do not prove observed
lock-wait or real-time boundary behavior.

### History agreement checkpoint, 2026-11-04

The additional history fixture first failed against the structural-only candidate:
an otherwise valid edit with inconsistent history actor attribution was accepted,
so the expected named rejection was absent. That attempt remains failed evidence.
The subsequent `history.sql` guards compare immutable result/history envelopes
with relational actor, configuration, item and project facts, preserve ordinary
edit lifecycle, and require bidirectional retained first valued-field use.
They distinguish numeric JSON codec tokens rendered as `1.0` from the frozen
integer token `1`; PostgreSQL JSONB numeric equality alone does not do so.
Integral SQL u64 values are normalized to canonical decimal strings independently
of their physical numeric scale. Prospective domain validation remains in Rust.

Snapshot `333b63deb3d0f9bb7040752260c764b022b22398`, based on `b81b64c`,
passed the corrected focused campaign from 21:32:25 UTC in 50.828 seconds through
cleanup. Twelve checker controls passed in-container. PostgreSQL passed both
the existing structural fixture/observer and the new history fixture/observer:
attribution and representation rejection, lifecycle preservation, empty text
recording first use, clearing without losing that use, 129 retained values,
integral numeric-scale normalization and retained history after replay retirement.
The collection and scale cases roll back; they are representation controls,
not evidence of prospective command acceptance. The snapshot-corruption probe
temporarily disables a user trigger only inside a rolled-back rejection scope;
it does not establish protection against privileged DDL.

Owned resources were removed, the provider stopped and the controller job joined
without termination. Sampled aggregate working sets peaked at 1.651 GiB; minimum
sampled available host RAM was 2.884 GiB, with no warning or stop event. Initial
and database admission retain their separate 3 GiB requirement; later samples
are governed by the approved warning/stop thresholds. Nested measured spans were
20.234 seconds for controller tests through cleanup, 2.609 seconds for database
creation/readiness and 9.329 seconds for combined SQL work. SQL subdivisions and
other elapsed time were not isolated. These spans are not added to the parent.

This remains an uninstalled owner-only SQL overlay with selected role denials.
Fixed serving entries, effective runtime privileges, startup profile, typed
application reload, authorization, contention and API/interface qualification
remain outstanding. The earlier codec gate and this focused campaign do not
substitute for the required gate on the eventual committed storage packet.

### Read-only pending discovery checkpoint, 2026-11-05

Tree `076190064f475ade2a1be7188e8cebf8039ec5f9` passed seventeen checker
controls and all earlier SQL fixtures, then reached the intended missing-list
failure with SQLSTATE `42883`. The new discovery implementation shares private
grant validation with the write fence while preserving its post-wait statement.
Reads require a read-only REPEATABLE READ snapshot, complete expected authority
and coherent current project grants. Lists project bounded metadata only;
individual open uses length sentinels and a CASE-bounded byte projection before
returning content. Effective expiry omits content without changing physical rows.
An inclusive numeric upper cursor bound removes the nullable OR plan risk while
preserving positive-bigint exclusive keyset semantics, including the first page.
No actual query-plan cost or concurrent wait guarantee is established here.

The first implementation attempt at tree
`0545b2383aa358a9a3546e13d91d5b92e6b341ad` passed normal read, pagination,
authority and unchanged-row-version controls, but its corruption harness failed
with SQLSTATE `55006` when re-enabling a trigger with pending deferred events.
The overall attempt remains failed. Only fixture timing changed: remaining
constraints run immediately in corruption/restoration transactions before DML
and later ALTER operations. No additional protection was disabled. Owned
cleanup/provider shutdown completed in 50.297 seconds; sampled minimum available
RAM was 4.139 GiB and aggregate peak 1.688 GiB. Those observations do not turn the
incomplete corruption/observer checks into passes.

Corrected tree `8152ad470cee19dcca9523b002bd89e9ac1a6223`, based on `b81b64c`,
passed the full focused campaign from 01:58:31 UTC in 52.625 seconds through
cleanup. Seventeen checker controls passed in-container. PostgreSQL passed all
prior fixtures plus scoped read-only discovery/open, 16-row keyset pages with a
sentinel, all null authority components on both entries, nonnull stale/spoofed
authority, grant denial/incoherence, missing/cross-project locators, physical
prepared/effectively expired state and both legitimate retired-content states.
Corruption probes reached missing mandatory and oversized byte rejection; the
independent observer verified 26 pending rows, exact product cardinalities and
restored validated constraints/enabled trigger. These privileged fixture probes
qualify reader classification, not protection against arbitrary owner DDL.
Snapshot row-version comparison separately established that successful normal
reads changed no pending rows. Summary projection is inspected source evidence;
availability with oversized content alone does not prove internal allocation.

Owned resources were removed, the provider stopped and the controller joined
without termination. Sampled aggregate working sets peaked at 1.710 GiB; minimum
sampled available RAM was 4.127 GiB, with no warning/stop event. Nested spans were
19.109 seconds for checker controls through cleanup, 3.063 seconds for database
creation/readiness and 13.390 seconds for combined SQL work. SQL subdivisions
and remaining elapsed time were not isolated; nested spans are not added.
The expected missing-function failure and failed corruption harness are retained.

This is owner-authored SQL evidence. The entries remain uninstalled and have no
serving grants. Final application session release, typed canonical decoding,
observed concurrency/revocation, actual query plans, item execution/reconciliation,
version-three startup/permissions and API/interface qualification remain open.
Independent read-only review inspected the exact production SQL, fixture delta,
observer and retained results/failures. It found no material evidence-validity
blocker to the next accepted source component. Snapshot authority is not final
response eligibility, and query shape is not an executed plan or scaling result.

### Historical reconciliation checkpoint, 2026-11-05

The tests-first snapshot `8c4743c74e7c5cc6c418a783fe3fa6a464b73be1` passed
nineteen checker controls and prior SQL fixtures, seeded three coherent accepted
records, then failed at the intended missing create-reconciliation entry with
SQLSTATE `42883`. It remains failed overall; cleanup and provider shutdown
completed in 54.203 seconds. This establishes reachability of the first new call,
not mutation sensitivity for every subsequent control.

The uninstalled `reconciliation.sql` provides read-only RR create/edit entries
and a private core inspector. Trusted callers supply fully scoped database cores;
future writers must hold core SHARE before inspection. The inspector acquires no
locks, checks payload/retirement coherence, distinguishes permanent identity
conflict before expiry and compares available request bytes exactly. Live missing
pending content is integrity failure even after logical expiry. Completed creation
checks its immutable pending/item/creation-revision/core binding. All outcomes
carry bounded authority/identity metadata, never a decoded saved result.
An internal `replay` classification still requires Rust stored-codec decoding,
permanent-attribution checks and final session/expiry response eligibility.

Snapshot `8fb9bf7ba4bbeb17927088db67af6bbe35da0321`, based on `b81b64c`,
passed the full focused campaign from 02:22:21 UTC in 55.969 seconds through
cleanup. Nineteen checker controls passed in-container. PostgreSQL passed prior
fixtures and the new historical controls: recorded create/edit after later edits,
changed identity before/after retirement, logical expiry with retained payload,
equal retired identity, prepared/cancelled/expired/missing creation, unseen edit,
scoped current authority, archive precedence and unchanged pending row versions.
Owner corruption probes reached missing-payload, retained-retired-payload and
missing-elapsed-prepared-content rejection, plus constructed backward-time
classification. All affected protections were restored. These are disposable
reader-classification controls, not a claim of resistance to administrative DDL
or observed clock movement.

The separate observer verified 27 pending rows, two items, seven revisions, nine
cores and five payloads; the selected issued identity, current version, project
head and estimate witness; restored grants, five enabled triggers and validated
pending-state shape. `unseen` remains an observation at its read snapshot, not
proof that another in-flight execution cannot commit or permission to omit
fresh execution's protected lookup.

Owned resources were removed, the provider stopped and the controller joined
without termination. Sampled aggregate working sets peaked at 1.720 GiB; minimum
sampled available RAM was 3.981 GiB, with no warning/stop event. Nested spans were
18.860 seconds for checker controls through cleanup, 3.093 seconds for database
creation/readiness and 15.485 seconds for combined SQL work. SQL subdivisions
and remaining elapsed time were not isolated; nested spans are not added.
Independent read-only evidence review found no material blocker to beginning
fresh-save source work. The expected missing-function failure remains retained.

Fresh atomic save entries, protected preparation comparison/reentry, serving
privileges/profile, typed Rust reload/commit/release, native concurrent waits,
retirement/revocation races, installation and API/interface qualification remain
outstanding. These read-only entries cannot execute a command and are not a
substitute for the full authenticated WorkItem capability.

### Atomic saves and predecessor ordering checkpoint, 2026-11-05

The first save snapshot `a63b18a0e51680dbf086b40dbab032e241c8271d` passed
twenty-one checker controls and earlier SQL fixtures, then reached missing SQL
preparation type `42704`. The first implementation snapshot
`8ad1c52b905bcab5737592788c27c164c79f5aac` passed the owner-level sequential
save campaign in 56.719 seconds through cleanup. It covered fresh Milestone
creation, direct edit/clear, unchanged issued identity, permanent field use and
estimate witness, configuration/item/head conflict precedence, recorded replay
before fresh preparation, scoped authority, read-mode denial and injected write
rollback boundaries. Its observer found 26 pending rows, three items, nine
revisions, eleven cores and seven payloads, with operational head nine and
next sequence four. That pass did not qualify core-before-item acquisition.

Independent source challenge found the implicit predecessor-FK ordering defect
described above. Snapshot `c8c778494155b773a9102b4c3b555af1a2fb0740` passed
twenty-five checker controls and the sequential fixtures, then failed the new
native contention control: the fresh edit accepted, forced all deferred
constraints and rolled back while the predecessor's maintenance fence remained
held. The failure was absence of the required predecessor wait with positive
writer completion, not a timeout or crash. The blocker was intentionally
cancelled, state remained unchanged and cleanup/provider shutdown completed in
60.562 seconds. Minimum sampled available RAM was 3.698 GiB and aggregate peak
1.738 GiB. This is an observed protocol violation, not an observed deadlock.

The first correction snapshot `cbf47952d502db9e8a872170a5f50be70fffbb0e`
passed native contention and the full focused campaign in 59.360 seconds, with
minimum available RAM 3.622 GiB and aggregate peak 1.687 GiB. Its new malformed-
history test used a JSON number instead of the required string, so that test
could pass on representation mismatch alone. It does not establish rejection
of an alternate canonical predecessor identity. Corrected test snapshot
`dd775e948b0b6489d4d03ab9fe728004502cd96e` exposed that remaining gap: the
canonical alternate predecessor reached `project_operational_transition`
instead of the expected immediate `item_history_agreement`. That overall
attempt remains failed, with normal cleanup/provider shutdown in 55.250 seconds,
minimum available RAM 3.660 GiB and aggregate peak 1.730 GiB.

Corrected snapshot `5a722f2c2e31bd13f18c3bb4ff45e17b3f4482e1`, based on
`b81b64c`, passed the full focused campaign from 03:59:21 UTC in 57.844 seconds
through cleanup. All twenty-five checker controls passed in-container, followed
by all prior SQL fixtures, atomic saves, the corrected canonical alternate-
predecessor rejection and the native contention control. The control positively
observed the writer blocked by the exact predecessor backend, independently
acquired the item NOWAIT, intentionally cancelled the blocker, then required
accepted execution, deferred-constraint forcing and rollback. It found unchanged
accepted-state counts and no owned backend remnants; the separate final observer
also verified the saved version, issued identity, operational head, permanent
field uses and removal of fixture-only fault/packet objects.

Owned resources were removed, the provider stopped and the controller joined
without termination. Minimum sampled available RAM was 3.669 GiB and aggregate
peak 1.734 GiB, with no warning/stop event. Nested spans were 18.953 seconds for
checker controls through cleanup, 3.031 seconds for database creation/readiness
and 19.235 seconds for combined SQL work. SQL subdivisions and other elapsed
time were not isolated; nested spans are not added to the parent.

The native wait case forces constraints and rolls back; it does not commit a
concurrent command or actually retire payloads. The completed-state observer
separately checks sequential committed saves. Current source review closes the
identified lock-routing gaps only under the documented transaction boundary.
Head-zero runtime execution, typed reload/attribution, transaction ownership,
serving privileges/profile, actual maintenance/revocation races, final response
eligibility, HTTP and interface recovery remain integration gates. This remains
an uninstalled owner-only SQL packet; the required eventual commit gate and
final exact-head review have not been replaced by these focused checks.

### Required-gate hold and checkpoint retrospective, 2026-11-05

The normal-hook full Podman gate on tree
`f5baacf2a39ea061821796db06916587d35d80ed` started at 04:04:34 UTC with
5.483 GiB available RAM. It passed 451 Python tooling tests with two platform
skips, the interface/codec/workspace/oracle/Go checks, and the existing native
authority fixture and durable-state observer. It then reached OpenSearch
readiness and was stopped when sampled available RAM fell to 1.383 GiB, below
the approved 1.5 GiB floor. Aggregate sampled working sets peaked at 4.555 GiB.
The OpenSearch readiness command logged a successful response, but its service
phase did not complete cleanup. Subsequent PostgreSQL/FerretDB task-path phases
were not reached; the overall gate failed and created no commit.

The interrupted process tree was terminated and joined. Initial cleanup removed
the runner but missed the service-phase OpenSearch container/network, so provider
shutdown correctly refused while that owned service remained active. Separate
recovery verified the exact resource ownership, removed only those resources
and confirmed exclusive provider shutdown; recovery started at 04:14:21 UTC.
Pre-existing stopped
containers were retained. The original failed result and cleanup failure remain
separate from the successful recovery. The attempt's 484.734-second measured
span ends before that separate recovery; it is not total time to recovered idle
state. No unchanged retry or lowered memory threshold is justified.

This remains a local storage checkpoint under PR #96, owned by the delivery
author, based on `b81b64c`. The original full WorkItem outcome and accepted
architecture are unchanged. The required gate, checkpoint commit/publication
and final committed-head review remain open; the earlier focused SQL evidence
remains valid for its unchanged executable source and stated environment.

| Observation / consequence | Evidence and causal limit | Follow-up / owner / state |
| --- | --- | --- |
| Sequential save success did not establish lock composition. | Source challenge found the implicit predecessor FK; native contention exposed the missing fence. No observed deadlock is claimed. | Author retains the corrected fence and focused contention control; exercised. Future Rust ownership remains a gate. |
| The first malformed-history oracle could pass for the wrong reason. | It varied both identity and JSON representation; a canonical string exposed the later rejection. | Author preserves representation while varying identity; corrected control exercised. Broader prevention effectiveness is unmeasured. |
| Available initial memory did not ensure full-gate completion. | The focused SQL campaign passed, but this full run crossed the floor during OpenSearch startup. Neither later peak demand nor an exact sufficient reserve was measured. | Author requires materially greater headroom before another full run; retain thresholds and avoid unchanged retries. |
| Interruption recovery covered the runner but missed an owned service phase. | Initial cleanup failure and separate ownership-checked recovery are retained. Runner-only recovery had a known scope limit; normal-path cleanup success did not establish interrupted service cleanup. | The later ownership repair and scoped interruption evidence are recorded below. The required full-gate retry remains pending. |

Source preparation/review spans overlap and were not timed end to end; do not
sum the many focused campaigns into a development-time estimate. The current
residual risks are serving activation, typed bounded state proof, one-command
transaction ownership and end-to-end recovery. The next coherent component is
coordinated fresh-v3 installation/profile plus the internal typed read/preparation/
save/reload bridge with an actual serving-role fixture; HTTP/interface delivery
follows. A separate source-first review found that scope consistent with ADR-111,
while exact projections, aggregate retention and finite attempt accounting still
need concrete review before dependent implementation. No new product policy or
resource authority follows from that direction review.

### Verification ownership repair checkpoint, 2026-11-05

The full-gate interruption made a narrow test-lifecycle repair necessary before
another attempt. Its scope is the existing optional host ownership record and
the verifier's three serial Compose groups; it changes no product installer,
serving privilege, WorkItem API, provider or memory threshold. The original
delivery goal is unchanged. The important premise was contradicted by the failed
gate: a runner owner alone cannot authorize cleanup of unrelated Compose project
identities. A separate source-first review inspected the ownership writer,
service-group lifecycle and recovery requirements before receiving this repair.

The repaired record is versioned and retains the caller-known run owner,
original routing/source binding and finite group reservations. Empty-namespace
preflight precedes durable creation authorization; a reserved or occupied
namespace never authorizes deletion. Full resource identities are persisted
before destructive cleanup, including failed partial startup. Tracked cleanup
and post-join recovery use exact-ID inspections/removal under a shared bounded
service deadline, refuse replacement/foreign resources and retain the original
failure with cleanup refusals. The existing untracked lifecycle remains separate.
This matrix has no declared named volumes; recovery refuses them rather than
extending its scope to caches or arbitrary volumes.

Retained missing-component controls preceded implementation. Independent review
then found two concrete gaps: failed-start cleanup could remove the container
before recording its anonymous-volume binding, and cleanup's deliberate
`ValueError` refusals could replace a primary failure. Each new causal regression
failed before correction. The corrected service controls passed all 44 cases;
15 container controls passed, including explicit caller-owner recording before
executor entry and invalid-owner rejection before record creation. Ten receipt
and seven resource controls passed; the combined ownership/budget/settings
selection passed 37 cases. These are host control-plane checks with command
doubles, not service or application qualification.

The first tiny runtime attempt at source
`fa60fb8569cd6f1d051228e0e5c1c359f805674c` failed before fixture/container
creation because its image admission incorrectly required exactly one matching
repository digest. It completed normal cleanup/provider shutdown in 17.406
seconds, with minimum sampled available RAM 3.208 GiB and aggregate peak
1.425 GiB. A separate metadata-only observation established that the cached
image has two repository digests; that observation passed in 17.187 seconds,
minimum available RAM 4.347 GiB and aggregate peak 1.381 GiB. Neither attempt
qualifies interruption recovery. The corrected harness selects the reported
primary digest, requires exact repository membership and local resolution to
the same content ID, then verifies both reference and content ID on creation.
It retains no-pull behavior. Pre-execution review also corrected an atomic
barrier-publication race and added actual internal-network inspection.

The fresh tiny NATS control on the same source tree passed in 43.594 seconds
through cleanup. It used the cached immutable image with a verified 64 MiB,
one-CPU, 128-PID container, no published ports or mounts, and an inspected
internal network. It positively observed the live container before saved IDs,
intentionally terminated and joined the lifecycle child while its receipt was
still armed, then required discovery and persistence of exact IDs before
container/network deletion. Empty inventory, idempotent second recovery and
unchanged pre-existing container/network identities passed. The provider stopped
and the outer controller joined normally without termination. Minimum sampled
available RAM was 4.460 GiB and aggregate peak 1.453 GiB; no warning/stop event
occurred. Inner work through its cleanup measured 40.343 seconds and is nested
inside the parent span; finer subdivisions were not measured.

This is actual evidence for one NATS interruption path, not anonymous-volume
recovery, arbitrary host failure, the required full gate or WorkItem runtime
behavior. The required full-gate retry and final committed-head review remain
open at that checkpoint. Source-bound passed SQL evidence is retained; full-goal
review readiness is not claimed.

### Passed full storage checkpoint gate, 2026-11-05

The normal-hook full Podman gate passed on exact tree
`d067298285b6c456d53e8b1c2a9493de75338bad`, creating commit
`0046be0a9b1966dc6e0c13fe7e0976b7bca8dde2` from parent `b81b64c`. It ran
474 Python tooling tests with two platform skips, workspace/interface/codec,
oracle and Go checks, the selected native authority fixture cases and independent
durable-state observer, NATS/OpenSearch readiness and the PostgreSQL/FerretDB
task path. The latter passed 48 competing pairs, stalled-body/database guards
and Go-to-Rust save/replay/read with search unavailable. Externally provisioned
password-corpus and protected-startup assets remain outside this qualification.

The attempt started at 16:03:55 UTC with 5.773 GiB available host RAM and took
542.296 seconds through cleanup. The nested command/hook span was 540.687
seconds and the container phase reported 537.516 seconds; these spans are not
added. Sampled aggregate working sets peaked at 4.647 GiB and minimum available
RAM was 1.585 GiB. Reserve warnings occurred, but no stop-floor or aggregate-
budget event occurred. This was tight observed headroom, not a measured safe
launch minimum or a continuous peak guarantee.

All three tracked service groups reached verified completion, including normal
removal of the PostgreSQL/FerretDB anonymous volumes. The provider stopped and
the controller joined without termination. This qualifies normal tracked
cleanup in that full run; interrupted anonymous-volume recovery remains outside
the actual tiny-control evidence. Earlier failed gate, image-admission failure,
invalid routing oracle and separately verified recoveries are retained as
distinct results; this successful run does not change their outcomes.

The product installer and runtime profile still target version two: the WorkItem SQL is still
an uninstalled owner-only overlay. Its executable SQL, checker and fixture files
are unchanged from the scoped PostgreSQL pass at `5a722f2`; source comparison
confirmed that relationship. The full gate adds existing application regression
and verification-lifecycle evidence, not authenticated WorkItem runtime proof.
Typed bounded preparation, one-command transaction consumption/reload, fresh-v3
installer/profile adoption, routes/interface and the end-to-end failure cases
remain required. Publication must retain exact-head review and actual remote CI
status; no merge or full PR readiness follows from this checkpoint.
