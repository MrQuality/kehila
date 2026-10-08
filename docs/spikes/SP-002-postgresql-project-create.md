# SP-002 — PostgreSQL project-creation feasibility

| Field | Value |
| --- | --- |
| Status / outcome | Concluded / bounded feasibility supported; original integrity gap retained and proposed guard separately verified |
| Owner / assessor | Project maintainer; observations recorded for maintainer assessment; recommendations are not new accepted decisions |
| Created / last updated | 2026-10-08 / 2026-10-08 |
| Related work | [#26](https://github.com/MrQuality/kehila/issues/26), [B-005/#9](https://github.com/MrQuality/kehila/issues/9), [#27](https://github.com/MrQuality/kehila/issues/27), [#11](https://github.com/MrQuality/kehila/issues/11), [#32](https://github.com/MrQuality/kehila/issues/32) |
| Accepted direction | [D-032 through D-034](../product/decisions.md#d-032); [ADR-102](../architecture/ADR-102-postgresql-transactions.md) |
| Effort limit / stopping condition | Initial investigation had no declared timebox and was registered retrospectively. Do not infer full procedure compliance. R02 stops on failed preflight, an unexpected case failure or its finite process/statement/lock budgets. |
| Prior evidence | [SP-001](SP-001-task-path.md) tested an earlier stack; it does not establish native PostgreSQL behavior. |

## Question and scope

Can the proposed native PostgreSQL 16 structure support atomic trusted project
creation, permanently scoped request identity, current creation authorization,
and replay payload expiry/retirement under the agreed 90-day policy?

Test the unchanged storage/seed SQL first, then separately declared role and
protocol helpers. Include rollback, concurrency, controlled response loss,
process recovery and clean logical restore. Exclude production route readiness,
real authentication, final codecs, target allocation and grant administration.
A supported prototype does not complete #26 or approve every physical choice.

## Pre-Execution Plan

1. **Goal & Context:** Establish concrete database feasibility and expose integrity,
   privilege and recovery failures before installing a native migration/adapter.
2. **Definition of Done:** Case outcomes and reproducible code retained; failing
   baseline preserved; experimental helpers and missing product checks identified.
   Production E2E completion is excluded because those components are absent.
3. **Dependencies & Prerequisites:** PostgreSQL 16/UTF8 in isolated Podman,
   independent connections, restricted roles, Rust pure oracle and Python driver.
4. **Risks & Mitigations:** Fresh containers/volumes; no durability weakening;
   actual serving login; observed lock barriers; no host-clock changes; explicit
   early and COMMIT-time deferred failures. Owner probes are separate.
5. **Spikes & Open Questions:** Identity representation, codecs, trusted actor
   binding, owner-right mapping and target allocation. Remaining implementation
   checks are not converted into fictitious successful helper tests.
6. **High-Level Architecture / File Changes:** Reproduction in
   [experiments/SP-002](../../experiments/SP-002/README.md); source schema remains
   frozen at its tested proposal revision. No application adapter is delivered.
7. **Verification & Testing Plan:** A01–A12 schema groups, B01–B15 experimental
   protocol groups and C01–C04 fault/restore/measurement groups. Retain errors,
   final states and reached schedules; nonzero overall verdict for gaps/failure.

## Cases and observations

| Cases | Expected observation | Observed boundary |
| --- | --- | --- |
| A01–A04 | SQL loads; complete trusted creation commits; every failure rolls back; readers see atomic state | Supported for 26 rows across 13 tables, all 26 individual write failures and deferred failures |
| A05–A09 | Membership/ownership/attribution, scoped uniqueness and exact scalars reject invalid values | Supported; all 60 family/target pairs tested in separate modified schema; byte/NUL bounds remain a contract choice |
| A10–A11 | Permanent core, coherent full/tombstone shape and mutation/TRUNCATE restrictions | Supported within tested role/owner boundaries |
| A12 | Payload UPDATE denied to serving; examine privileged identity move | Serving denied; privileged move leaves old core orphaned despite forced deferred checks; experiment rolled back |
| B01–B07, B09 | Restricted locking, same-key retry, independent actors, original-result replay and current authority ordering | Supported by QA helpers; 20 trials per selected ordering with observed waits/completion |
| B08 | Current grant/event agreement; management and owner rights | QA validation catches mismatch; actual grant administration/owner semantics remain blocked |
| B10–B13 | Recorded timestamp, exact expiry, safe compaction and races | Supported by trusted-time QA helpers; no grace or commit adjustment; production adapter/compactor absent |
| B14 | Exact scalar/string/order and historical codecs | JSONB/driver exactness and QA digest tested; final canonical/historical codec remains blocked |
| B15 | Failed transactions rolled back before bounded retry | Timeout 55P03, cancellation 57014 and deadlock 40P01 observed |
| C01–C03 | Lost response reconciles once; durable process recovery; full/tombstone restore | Supported within the stated isolated fault/restore scope |
| C04 | Descriptive bounded observations without invented capacity threshold | Plans/index/vacuum/WAL and small warm workloads captured; no production envelope established |

## Environment and reproduction

Baseline `9f433f99b31d0a3e7378a6009fd319a6788640fe`; five original SQL blocks,
SHA-256 `004e3fe0a9d104df656f9b137fe6c608820b3ea5bfafe809fe48d2f9e3aad197`.
PostgreSQL 16.15, UTF8, C collation, 8192-byte pages, READ COMMITTED; fsync,
synchronous_commit and full_page_writes enabled. PostgreSQL runs as a non-root
container user. Podman/Windows/WSL2 execution; Linux CI has not been executed.

Use the [experiment README](../../experiments/SP-002/README.md) and
[runner](../../experiments/SP-002/manage.py). The immutable image, prerequisites,
synthetic fixture exporter, role/helper definitions, finite timeouts, isolation,
cleanup and expected nonzero verdict are versioned. A current source-contract
change requires recording a new baseline rather than reusing this claim.

## Run summaries

### SP-002-R01 — Native schema and protocol investigation

Executed 2026-10-08, UTC timestamps. Untouched schema loaded. Complete trusted
seed, rollback, constraint and role tests ran. Initial experimental compactor
failed at COMMIT with 42501: deferred invoker checks ran after the definer helper
returned. A separate SELECT grant on the two operation tables resolved it without
adding direct writes. A faulty ACL assertion and incomplete stderr capture were
corrected; affected experiments were rerun and original failures retained.
Recovery-log capture initially failed despite recovered data; a fresh crash trial
with both streams retained confirmed automatic recovery. The payload identity
move failure was reproduced and rolled back, not fixed.

Final bounded case records: 403 pass, one known privileged failure, six product
checks blocked. Required operation/project-create pure tests and broader
`python scripts/verify.py --pure` completed successfully. Four supplemental pure
probes also passed. Prototype code and SQL placeholders do not certify codecs.

### SP-002-R02 — Sanitized reproduction runner

Executed 2026-10-08, fresh isolated resources on the same platform. The public
runner completed end to end, producing the same 403 pass / one fail / six blocked
case counts and expected exit 2. See the stable
[observed summary](../../experiments/SP-002/results.observed.json).
Two hundred selected concurrency schedules were reached: same-key commit/abort,
same-actor/independent-actor, revocation-first/command-first for creation and replay,
and replay-first/compaction-first; each ordering ran 20 trials.

The response-loss proxy forwarded COMMIT, observed server CommandComplete and idle
ReadyForQuery, withheld those responses and closed the client. Independent lookup
proved one complete committed action and original-result replay. A separate
pre-COMMIT disconnect left none. SIGKILL/restart preserved acknowledged state and
excluded an open transaction. Fresh-cluster pg_restore loaded full operations and
tombstones before post-data triggers, then passed shape, identity, replay, mutation,
sequence and exact content/role/ACL/function comparisons.

### SP-002-R03 and R04 — Review corrections and separate identity guard

Executed 2026-10-08 on Windows/WSL2 with Podman and PostgreSQL 16.15. Both full
native reproductions retained the original 403 pass / one fail / six blocked
records, then passed five additional cases in a separate corrected-schema database:
complete creation, same-identity UPDATE, reassignment rejection with complete
rollback, serving-role UPDATE denial, and full-to-tombstone retirement. Combined
counts are 408 pass / one intentional frozen-subject fail / six blocked. The old
failure is not relabeled successful. See the separate
[review summary](../../experiments/SP-002/results.review.json); R01/R02 evidence
and its original schema hash remain unchanged.

R04 used a custom `CARGO_TARGET_DIR`, selected the exact Cargo-reported library,
completed with expected runner exit 2, and recorded no cleanup errors. R03 also
completed the bounded cases with no cleanup errors. The proposed schema now
rejects payload identity changes using a dedicated guard and named 23514 error;
no production migration has been installed.

Host preflight explicitly supports Windows/Linux under D-036 and rejects other
platforms and optimized Python. Case modules use explicit imports and entry
points; runtime fixtures load on execution and uninitialized connections are rejected. Invariant checks execute independently
of Python assertion optimization, with lazy failure diagnostics. Phase timeout
output is retained and cleanup inspection/stop calls are bounded and checked.
Focused host regressions exercise orchestration failure paths without claiming
simulated database evidence. Native Linux execution remains unperformed; ordinary
Linux CI is a separate verification boundary.

## Conclusion and reuse boundary

Keep the native PostgreSQL direction and useful deferred membership/attribution
constraints. The database can support the tested bounded creation protocol.
The revised proposal rejects payload identity moves, with separate G01–G05
regressions. Install that protection and explicit deferred-trigger privileges in
the eventual executable migration. Align identifier acceptance and final codecs
before exposing the native route.

Full records may replay again after a backward wall-clock jump into their valid
window; retired tombstones cannot revive. No durable clock high-water mark was
introduced. Logical expiry ignores cleanup lag and post-sample commit delay.

Evidence is bounded to the frozen schema/pure baseline, experimental roles and
helpers, tested PostgreSQL image, small synthetic workloads and specified faults.
It does not establish production authentication, API/codec compatibility, physical
power-loss resilience, production capacity or supported installation/restore
orchestration. Schema, clock, ACL, codec or runtime changes require targeted reruns.

## Decision and follow-up

- [SP-003/#39](SP-003-replay-codecs.md): codec/identifier representation under #26.
- [SP-004/#40](SP-004-actor-authority.md): trusted actor binding and owner authority under #11/#32.
- [SP-005/#41](SP-005-project-target-allocation.md): stable creation allocation under #27.
- #26 retains installation of the verified payload guard, executable migration/role
  provisioning, native clock sampling and production compactor integration. Those are delivery
  follow-ups to observed mechanics, not three additional engine investigations.

D-032–D-034 remain accepted choices; recommendations above are not new D-decisions.
Prototypes are retained for reproduction. Promote applicable cases into normal
regressions when the native implementation exists; replay helper passes must not
be substituted for verification of delivered authenticated routes.

## Handoff

Latest completed run: R04, with the frozen failure preserved, separate corrected
guard cases passing, and six missing-product checks. Await maintainer review and
merge of PR #42. Next, start SP-003/#39 on a separate branch from updated `main`,
then deliver the native migration/role slice and applicable access/allocation
integration. No linked product issue is closed.
