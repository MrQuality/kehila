# Qualification, operability and release evidence

This guide explains required evidence for the [standard](ENGINEERING-STANDARD.md).
Everything described as a target below is pending implementation unless linked
current evidence explicitly establishes its scope. The existing
[test strategy](../TEST_STRATEGY.md) remains the risk/test-method reference and
[testing guide](../TESTING.md) remains the executable matrix. B-023–B-028 own these
campaigns; approved spike procedures remain appropriate for uncertain boundaries.

## Qualification evidence records

**Status: record protocol defined; release qualification pending.** The
[baseline assessment](ASSESSMENT.md) records dated source evidence and limitations.
It does not approve a release or the maintainer's final PR disposition.

A verification/qualification record MUST pair the immutable subject source SHA
with check/run URLs, results, scope, runtime/service versions where relevant and
known gaps. Identify the person assessing applicability and the disposition;
author verification is not independent approval. Repository settings need their
own dated inspection because they can change without a source commit.

The subject commit and the document recording its result are different things.
Record final-commit CI in the PR conversation or a separate qualification record
after it finishes. A later documentation commit can cite an earlier immutable
subject, but cannot claim that subject's run qualifies its own changed source.
If merge-source verification is required, record that merge SHA/run separately.
Do not repeatedly amend source just to insert its own future SHA or CI result.

Update requirement status when its scoped obligations are actually evidenced and
assessed, not because its policy file or PR was merged. A material change reviews
affected evidence and requirements; it need not rewrite every register status.
Passing metadata or CI checks alone is not qualification. Resolve SHOULD
departures and applicability explicitly in the assessment before claiming a gate.

## Contract and invariant qualification

**Status: partially evidenced.** Pure rules, bounded Go/Rust task-path tests and
the Python spike support the current invariant table. Supported OpenAPI, event
contracts and complete framework-control assessments remain pending.

The standard's source references map to implementation obligations, not badges.
ISO/IEC 25010:2023 guides measured product qualities: functional suitability
(FUNC/API/DATA), performance efficiency (PERF/LIMIT), compatibility (COMPAT/EVENT),
interaction capability (ACCESS/FUNC), reliability (REL/REC), security
(SEC/DEP/SUPPLY/INC), maintainability (ARC/GOV/TEST), flexibility (OPS/MIG/support
matrix), and safety-risk review where downstream consequences warrant it.
This is a Kehila mapping, not a reproduction of ISO clauses or certification.

SSDF 1.1 organizes follow-up work: PO.1/PO.2 requirements/roles (GOV), PO.3/PO.4
tooling/security criteria (TEST/SEC); PS.1/PS.2/PS.3 source/release/archive
protection (GOV/SUPPLY); PW.1/PW.2 secure design (ARC/threat model), PW.4 reusable
components (DEP), PW.7/PW.8 analysis/testing (SEC/TEST), PW.9 safe defaults (OPS);
RV.1/RV.2/RV.3 vulnerability discovery, response and root-cause learning (DEP/INC).
B-020 owns detailed practice assessment. Framework revision/mapping changes
require review. ASVS and OSPS require complete applicable-control matrices;
the threat model's chapter references are only initial work destinations.

Public API work in B-004/B-022 MUST select a supported OpenAPI revision and retain
validated specification/examples before exposing supported endpoints. Every
endpoint must declare auth/grants, size/resource limits, status codes, stable
error code/envelope, pagination (bounded size, cursor stability/expiry), allowed
filters/sorts and tie-breaking order, and conflict/retry behavior where applicable.
Avoid converting the experimental `{error: code}` and `{pending: true}` shapes
into a supported contract by documentation alone. `operation_id_reused` and
`version_conflict` remain distinct errors; define current-version disclosure and
correlation identifiers without leaking protected data. Document unsupported
pagination/filter/sort rather than pretending absent features exist. Conformance
tests must cover both Go and Rust, future TS/client contracts, malformed input,
denied access, errors and old-client behavior.

Future event metadata MUST include event ID, type, schema version, aggregate ID,
aggregate version, timestamp and correlation ID, plus causation ID where useful.
Do not confuse aggregate version, event schema version, project configuration
revision or stream offset. Duplicate events must not duplicate effects. Older or
reordered events cannot lower projection/client version. Delayed events remain
observable as freshness lag. Missing sequence/version ranges require detection
and a declared repair/rebuild strategy, with care for events that are full-state
versus deltas. Unknown future schemas must not be silently decoded as current;
reject/quarantine with diagnostics and upgrade/replay instructions. Poison events
need bounded retry, durable quarantine, operator disposition and idempotent replay
after correction; do not block unrelated aggregates forever or silently discard
an integrity gap. None of these production consumer controls exists yet.

| Invariant | Current enforcing layer / evidence | Future required enforcement |
| --- | --- | --- |
| Task versions strictly increase on accepted new mutation | Pure decision and immutable task/version record identity; finite live races | Crash/restart, sustained/model testing and supported storage qualification |
| One operation ID cannot represent multiple intents | Pure replay checks, task/operation unique index and live tests | Actor/family/target scope, versioned exact-request fingerprint, grant checks, atomic tombstones and restore |
| Identical replay returns original accepted result | Worker ledger and pure tests, bounded retention policy | Full API mappings, authorization/expiry/compaction/recovery tests |
| Search/client projection version never decreases | Go minimum-version search check; Python spike indexer | Production consumer and UI merge gate, schema/gap/rebuild tests |
| Acknowledged mutation does not depend on search | Separate task/search paths and live search-outage case | Supported E2E and failure/recovery qualification |
| Configuration governing acceptance remains valid | B-003 pure complete-revision/snapshot rules | B-005 commit-time coordination, Q-019 physical protocol |
| Allocation cannot confirm incompatible capacity | Product rules and Q-012 only | Authoritative ledger/constraint or demonstrated coordination; competing/resume tests |
| Completion releases future reservations; reopening does not reclaim others' bookings | R-017/R-018 and D-013 | B-011 integrated domain/storage/recovery tests under Q-019 |
| Historical costs/rates and released reservations retain their meaning | R-025–R-027; B-012/Q-015 | Exact cost rules, immutable/versioned history and atomic/recoverable lifecycle effects |

Per-task optimistic acceptance and search eventual consistency are the current
bounded model. Do not describe the system as linearizable: finite races do not
prove that property. Cross-record serialization/atomicity is unresolved. Define
state-machine or model tests before claiming stronger consistency.

## Failure hypotheses and dependency behavior

**Status: partially evidenced.** Current tests cover finite worker contention,
worker guards, database stalls/recovery and search-down authoritative operations.
The full supported dependency matrix and restart/termination/resource campaigns
remain pending; spike evidence applies only to its recorded adapters and cases.

This matrix is the required supported behavior, not a claim that main implements
the complete pipeline. Current Go/Rust evidence covers search-down saves/reads,
worker boundaries, database stalls/recovery and finite contention. Other pipeline
interruptions have only SP-001 Python-adapter evidence or remain untested.

| Unavailable subsystem | Authoritative capability | Search / synchronization | Required health and recovery behavior |
| --- | --- | --- | --- |
| PostgreSQL or FerretDB | New reads/writes unavailable; uncertain writes reconcile by same ID | Existing projection may be stale; cannot establish current authority | Process may be live but authoritative readiness fails; bound work, reconnect and verify indexes/mapping |
| NATS | Existing direct worker DB path can operate; supported pipeline must verify no synchronous broker prerequisite | Event delivery stops; retained change capture and backlog must be bounded | Degraded propagation, visible lag/WAL risk; checkpoint-safe resume or rebuild |
| OpenSearch | Direct saves/reads remain valid | Search unavailable; pending hit is not authoritative absence | Search capability degraded; rebuild/reconnect without regressing saved client state |
| Indexer | Direct saves/reads remain valid | Projection stalls while old results may exist | Freshness degraded; visible backlog, bounded retention, catch-up/rebuild |
| Rust worker | Mutations/authoritative reads through API unavailable | Projection may remain queryable as stale data | Authoritative readiness fails, live API may diagnose; same-ID reconciliation after recovery |
| Go API | User/API operations unavailable | Downstream services may continue | User-facing readiness fails; worker/pipeline continuity never implies API availability |

Liveness means the process can make progress and respond; readiness identifies
capabilities safe to serve; degraded service identifies capabilities unavailable
or stale while independent ones remain valid. Avoid one global dependency check
that disables valid authoritative work during search outage. Existing `/health`
is not a complete supported readiness contract. Configuration schema, safe defaults,
secret delivery, override precedence, startup rejection and compatibility need
B-027 design. A future `kehila config validate` is a candidate, not a shipped CLI.

Fault campaigns MUST state an invariant, controlled interruption point, expected
outcome, workload, deadline, observation and cleanup. Cover DB/NATS/search/indexer
outage; API/worker/DB/machine/container restart; disk/resource exhaustion; slow DB
and search; malformed, duplicate, reordered and delayed events; and termination
immediately before/after mutation commit. A timeout is not proof of rejection.
Observe independent authoritative state and reconcile IDs after an unknown result.
Disk-full tests must not consume the maintainer's real disk; use a bounded isolated
failure domain. Ordinary PR runs use deterministic bounded regressions; sustained
contention and failure campaigns run separately with frozen parameters and no
retry-until-pass policy. Retain failed cases and convert fuzz discoveries into
regression fixtures. No random disruption without a hypothesis and oracle.

## Reliability, telemetry and performance targets

**Status: targets and qualification pending.** No application SLO, performance
envelope or OpenTelemetry instrumentation has been qualified. Existing test
deadlines and spike timings are bounded test observations, not service targets.

No final numeric SLO, RPO/RTO or capacity targets are set here. Q-020 and Q-022
remain decision destinations. B-026 must decide populations, measurement windows,
error budgets/exclusions and targets before Beta; B-025 must measure the envelope.

| SLI | Required measurement definition | Evidence gap |
| --- | --- | --- |
| API availability | Successful eligible requests / all eligible requests over declared window; distinguish deliberate client rejection from server/dependency failure | Eligibility/window/target undecided |
| Mutation durability | Acknowledged mutations retained with accepted intent/version across supported fault/recovery class / acknowledged cohort | Accepted direction is no ordinary-restart loss; supported proof missing |
| Authoritative read latency | p50/p95/p99 end-to-end latency with error rate and workload | No application benchmark |
| Search latency | p50/p95/p99 query-visible response latency with errors and dataset | No supported search envelope |
| Search freshness | Source commit → query-visible projection delay; version-gap and oldest-unprojected age, including stalled pipeline | No production pipeline/instrumentation |
| Recovery success | Successful integrity-checked restore exercises / scheduled exercises, with time and realized data-loss measurement | No clean restore qualification |

OTel-compatible target: Go API extracts/propagates trace context to worker;
worker spans authority calls; CDC/event consumers preserve or link causation and
correlation across async boundaries. Avoid assuming CDC can reconstruct missing
trace context; persist/link metadata deliberately through the event contract.
Each service records structured severity/service/version/correlation logs,
metrics and trace context using supported SDK/exporter conventions. Include
request rate/duration/errors/active work, DB pool utilization, mutation failures,
retry counts, CDC lag, NATS backlog, projection lag, indexing failures and poison
events. Use bounded-cardinality dimensions; never task/operation/user IDs as
metric labels. Redact secrets and protected data; define telemetry access and
retention. Test trace propagation, metric meaning and failure alerts. Collector,
backend and instrumentation choices remain B-026 work.

Performance qualification measures create/update, authority reads, search, CDC
propagation, index rebuild, startup, storage growth, CPU/memory and concurrency
saturation. Publish dataset shape/distribution (including history), tasks per
project/installation, fields, clients, mutation/search throughput, runtime/hardware
and service versions. Report p50/p95/p99 plus failures, throughput and resource
ceilings. Measure rebuild/compaction during writes, not only an idle small fixture.
Repeat controlled trials; isolate host noise and never gate routine PRs on an
unqualified timing threshold. Capacity numbers come from data, not the design's
unmeasured millions-of-tasks aspiration.

## Backup, recovery and upgrade qualification

**Status: supported recovery and upgrade qualification pending.** Existing
database stall/recovery tests do not establish backups, clean-instance restore,
machine-loss recovery or supported storage migrations.

B-024 must preserve authoritative tasks, configuration/history, operation success
records/tombstones and necessary identity/configuration state. Classify what can
be reconstructed (search) versus what must be backed up. Automate daily backups,
show last success/age/failure, test sleeping/offline hosts and destination failure,
protect confidentiality/integrity and keep a copy off-machine/failure-domain.
Choose retention and backup destination through Q-020; do not infer RPO from the
schedule. Document the exact recovered point and last acknowledged lost mutation.

Clean-instance exercises must restore without relying on the old instance,
validate domain/replay invariants, reconstruct search, resume/reconcile CDC state
and verify stale clients and old operation IDs cannot create duplicate/new intent.
Reset/epoch handling after restore is an explicit design question, not an
invented implementation. Test machine-loss RPO/RTO separately from restart survival.

Version migrations explicitly, test supported previous releases and realistic
volumes, and record deterministic progress, failure/resume or rollback semantics.
Back up before destructive migration. Verify retained history and replay state,
CDC mapping/replica identity/publication/indexes after relevant upgrades, and old
client/event/config compatibility. Do not introduce a new migration framework
until B-005's durable model and Q-019 are resolved.

## Release and compatibility qualification

**Status: policy defined; supported release pipeline pending.** No supported
release artifact, SBOM, signing/provenance qualification or tested support matrix
is established by this guide. Existing CI verifies source, not release readiness.

The future release pipeline is immutable source commit → shared verification →
security checks → controlled hosted build → artifact/system tests → per-artifact
SBOM → signed provenance → signing → packaging → publication. It must retain
checksums, source/build inputs, artifact digests, licenses/notices, support matrix,
install/upgrade/recovery docs and consumer verification instructions. OCI images
use standard image/runtime/distribution formats where applicable. Evaluate
SLSA 1.2 Build L2 against actual hosted builder/provenance controls; signed JSON
alone does not attain it. Build L3 is later threat-model/hardening work. There is
no public release creation in this change.

0.x changes may break compatibility but MUST be documented. From 1.0 define the
public surfaces (HTTP, events, config, storage upgrades, CLI and public packages)
and enforce SemVer. A minor/patch release cannot silently break a supported client
or abandon promised data/replay semantics. Event evolution must specify required
fields, unknown-field/schema behavior and producer/consumer rolling upgrade order.

Future changelog entries use version/date with Added, Changed, Fixed, Security,
Deprecated and Removed sections as applicable. Mark **BREAKING** changes with
migration instructions even in 0.x. Security entries follow coordinated disclosure.
After 1.0 deprecations name replacement, affected surface, first deprecated version
and removal major/window; decide the actual minimum support/deprecation window
before 1.0 rather than inventing a duration now. Support matrix includes runtime,
browser, DB/broker/search versions, upgrade paths and support/security end dates.

Required runbooks are install, upgrade, backup, restore, health, DB failure,
search rebuild, CDC backlog, poison/dead-letter recovery, disk full, secret
rotation and crash recovery. Use [the runbook template](RUNBOOK-TEMPLATE.md);
each must be executed against its supported environment before claiming readiness.
