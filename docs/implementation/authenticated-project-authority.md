# Product authority: authenticated project creation

Status: implementation in progress. The maintainer authorized this one-PR outcome
on 2026-11-03 after accepting server-owned pending creations as an ADR-107 amendment.
Scoped runtime observations are recorded in the
[verification record](project-authority-verification.md); browser qualification
is not claimed by this plan.
Owners: #49, authentication #40 and allocation #41.

## Outcome and boundaries

An owner signs in, saves a pending project creation, explicitly executes it and
reads the project/configuration through a fresh authorized database read. Another
browser can discover the same pending entry without client persistent storage.
Cancellation, retry and response loss cannot silently produce another project.

One PR includes product source, fresh schema/capabilities, application adapter,
browser interface, failure tests and evidence. Commits remain focused. Review map:
domain/codecs; schema/capabilities; authentication/startup; transactions and pending
lifecycle; transport/browser; tests/evidence. Extraction and integration are coupled:
neither an experiment rename nor a database fixture is the delivered application.
Separate review is required at the exact final head.

Excluded: existing-data adoption, supported installer/real-data use, generic drafts,
background execution, lifecycle consumers, online upgrades and replicated authority.
ADR-108/109 remain Proposed. Applicable expiry, retirement and restored-identity
route-exposure checks remain required. Keep broader #40/#49 obligations open.
No merge follows from completion.

## Product ownership and adoption

The Linux `kehila_authority` package owns a product composition root and the needed
bounded credential/session, coordinator, protected startup, database and HTTP units.
Do not import experimental source, invoke its installer or expose diagnostic probe
routes. Carry relevant regressions and identify extracted units. Pure task contracts
and creation codecs retain domain/representation ownership. Package-specific Rust
1.88 is the initial adoption candidate; the root workspace declaration is unchanged.
Review and qualify the actual locked graph, features, licenses and advisories;
experimental checks do not clear product dependencies or #40 advisory obligations.

A fresh product installation composes the foundation and product auth/pending/
mapping SQL in one transaction. No populated upgrade or runtime schema repair.
Ownership stays NOLOGIN; serving gets narrow capabilities, not DDL or actor/credential
administration. Enrollment stays a separate local operator operation. A process
holding auth and project capabilities is one combined trusted authority.

## Pending lifecycle and API contract

Strict original authority/origin and bounded raw-byte admission apply. Mutations
require independent CSRF. Actor/installation come from current authentication;
request IDs and binding metadata are not credentials.

| Operation | Contract |
| --- | --- |
| Register | Bounded name/prefix/unit; server generates independent identity/token and commits immutable values without allocating a project. Reply only after commit. |
| List/open | Fresh current authorization, actor-scoped pagination and bounded materialization. Login/GET/reload never execute. Lost registration response may leave a discoverable entry; equal forms do not merge entries. |
| Execute | Entry ID and exact values disclosed by open are untrusted preparation input. Admit request/result/snapshot envelopes before execution DB/worker admission; fenced exact server-content comparison remains authoritative. |
| Reconcile | Historical-only lookup/replay using the server entry. No current seed/admission requirement or fall-through to creation. |
| Cancel | Serialize against execution. Committed cancellation prevents future execution; a committed project is reported completed, never undone. Lost acknowledgement remains uncertain. |
| Read | Fresh coherent PostgreSQL snapshot of authority, current grant/latest event and project/configuration; recheck live session/expiry before releasing data. |

Candidate states: prepared/completed/cancelled/expired. Transaction locks avoid
orphaned durable running claims. Editing registers a separate entry; it does not
mutate or cancel the original. Missing/cancelled/expired entries cannot be rebuilt
by delayed execution. Successful mapping/core identity remains permanent.

Initial limits are configurable containment policy, not calibrated capacity/SLOs.
Retain the bounded 16-slot transport/coordinator and single owned serving connection;
do not multiply independent queues/pools. Retain finite auth deadlines. State and
test pending quotas, page bounds and expiry before exposure. Cleanup cannot erase
successful identity or race execution. No real-data privacy/archival policy follows.

Wire outcomes distinguish success, conflict, forbidden, cancelled/expired/absent,
contention and unknown commit. Equal completed retries return saved results only
with current authority and replay eligibility. Do not tell clients to replace
identity merely to resolve uncertainty.

## Transactions and owned work

Use READ COMMITTED and installation-before-actor fencing, then fresh authority and
pending lookup after waits. Same-actor execution/cancel share this boundary; target
uniqueness arbitrates cross-actor collisions. Classify named constraints; rollback
completely before re-entry and re-admit changed candidates before another attempt.

Commit pending completion, mapping, operation/replay, project, ordered configuration,
grants/history and snapshots together. Reload/compare typed state with the single
trusted result and force deferred constraints before commit. Lost COMMIT/response
is unknown; retain identity and reconcile. Corrupt references fail closed.

Own database drivers, blocking password/randomness jobs, permits and reply eligibility
through actual completion and cleanup. Result delivery does not establish OS thread
exit. Do not move OS randomness onto the async executor. No exclusive actor fence
spans response delivery. Cancellation must join work or stop an affected authority
whose completion cannot be proved.

## Evidence and delivery gates

Review controlled native schedules: equal/changed races, independent actors,
target collisions and a winner during rollback, registration/create/cancel response
loss, cancellation versus commit, expiry/retirement/restored coherence, revocation,
deep/duplicate/oversized JSON and stored-record bounds. Observe final state and
released connections/locks rather than infer correctness from HTTP.

Backend/service workloads use bounded Podman with active configured host supervision,
one compiler job and external generated secrets/logs/build outputs. Required hooks,
CI and exact-head review remain. Preserve failures, diagnose ordinary defects and
repeat affected cases/controls without weakening checks.

On 2026-11-03 the maintainer explicitly removed Chrome/Edge execution from this
goal's completion criteria and deferred discussion of Playwright/Selenium
automation to future work. The interface and server-owned recovery contracts
remain; actual browser behavior is unqualified. This is a scope deferral, not a
passed browser check. It authorizes no browser launch, automation dependency,
deployment or TLS trust change. Hosted/separate-machine browser investigation is
also outside this delivery's finish line.

Required native/API evidence, mandatory commit/CI gates and exact-head independent
review remain. Keep draft until those remaining gates pass; then the PR may become
ready for review under the maintainer's standing delivery instruction. Do not merge.
