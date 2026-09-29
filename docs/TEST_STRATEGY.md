# YAJA Test Strategy

- **Document:** `docs/TEST_STRATEGY.md`
- **Status:** Proposed
- **Applies to:** YAJA repository and all supported YAJA distributions
- **Strategy owner:** Project maintainer / Quality owner
**Review trigger:** Material architecture change, supported-runtime change, release-scope change, or evidence that this strategy no longer provides adequate risk coverage

---

# 1. Purpose

This document defines the long-term testing and quality strategy for YAJA.

It is the north-star for:

- feature-level test planning;
- architecture validation;
- functional verification;
- security testing;
- performance engineering;
- reliability and recovery testing;
- chaos engineering;
- CI/CD quality gates;
- release qualification;
- test evidence and traceability.

This document defines **what must be demonstrated, at what stage, and to what level of confidence**.

It intentionally does not contain volatile command-line instructions. Execution commands, current tooling, environment setup, and the exact composition of the automated suite belong in `docs/TESTING.md`.

The strategy is risk-based. YAJA does not attempt to run every conceivable test for every change. Tests are selected according to the behavior changed, the architecture affected, the potential impact of failure, and the evidence needed to support the resulting claim.

Passing tests is not equivalent to proving correctness. Tests provide evidence within a defined scope.

---

# 2. Quality Objectives

YAJA testing shall primarily protect the following qualities:

1. **Functional correctness**
   The system performs the behavior defined by product requirements and decisions.

2. **Data integrity**
   Accepted user actions are not silently lost, duplicated, corrupted, reordered incorrectly, or applied to the wrong state.

3. **Consistency and concurrency correctness**
   Concurrent activity produces outcomes allowed by the defined consistency model.

4. **Idempotency and retry safety**
   Network retries and uncertain outcomes cannot silently create a second logical action.

5. **Security**
   Untrusted input, users, processes, dependencies, and network peers cannot obtain capabilities they were not explicitly granted.

6. **Availability and graceful degradation**
   Failure of one subsystem has explicitly defined effects rather than arbitrary cascading failure.

7. **Recoverability**
   Durable state can survive and recover from process, service, container, host, and storage failures within approved recovery objectives.

8. **Performance and scalability**
   Response latency, throughput, resource consumption, and recovery behavior remain within agreed budgets for supported workloads.

9. **Compatibility**
   Supported upgrades, schema changes, browsers, operating environments, and persisted data remain usable according to the compatibility contract.

10. **Operability and diagnosability**
    Failures can be detected, attributed, investigated, and recovered without relying on guesswork.

---

# 3. Current YAJA Quality Baseline

At the time this strategy is introduced, YAJA already contains several useful quality foundations.

The current implementation separates pure logic from I/O boundaries:

- `src/pure/yaja_query`
- `src/pure/task_contract`
- `src/io/task_worker`
- `src/io/event_dispatcher`
- `go/pure/sync_contract`
- `go/io/task_api`

The Rust workspace forbids unsafe Rust.

The task worker currently provides:

- bounded HTTP concurrency;
- bounded database operations;
- bounded request bodies;
- database connection and selection timeouts;
- operation deadlines;
- UUIDv7 operation validation;
- replay/admission expiry policy;
- optimistic version checking;
- operation-ID replay;
- changed-content operation-ID reuse rejection;
- loopback-only worker binding;
- Host and browser-context boundary checks;
- required-index readiness checks.

The current integration suite exercises:

- competing mutations through independent worker processes;
- identical request replay;
- changed-content operation-ID reuse;
- stale-version conflict;
- worker boundary rejection;
- stalled HTTP request bodies;
- database transport stalls;
- dependency recovery;
- Go-to-Rust mutation and read behavior;
- search unavailability.

SP-001 additionally provides bounded evidence for:

- FerretDB/PostgreSQL persistence;
- logical replication;
- worker crash after acknowledged persistence;
- Debezium restart;
- duplicate/older event handling;
- indexer outage and recovery;
- basic concurrent update behavior.

These are useful foundations, but none of them should be interpreted as complete production qualification.

---

# 4. Testing Principles

## 4.1 Test behavior, not implementation trivia

Tests should protect externally meaningful contracts, domain invariants, security boundaries, and architectural guarantees.

Tests should not unnecessarily freeze internal implementation details.

Refactoring should normally break tests only when observable behavior or required architecture changes.

---

## 4.2 Shift important testing left

Quality work starts during requirement and architecture design.

Before implementation, significant changes must identify:

- required behavior;
- invariants;
- invalid behavior;
- concurrency assumptions;
- trust boundaries;
- failure modes;
- performance expectations;
- recovery expectations;
- observability required to prove those properties.

A design that cannot be tested deterministically is incomplete.

---

## 4.3 Prefer deterministic tests

Arbitrary sleeps are not synchronization.

Tests involving asynchronous systems should wait for observable conditions with bounded deadlines.

Fault tests should inject faults at identifiable boundaries whenever possible.

---

## 4.4 Failure is evidence

Failed tests must not automatically be rerun until they happen to pass.

Retries may be part of the workload being tested, but must not be used to disguise test instability.

A failed repetition means the test campaign failed.

---

## 4.5 Test real boundaries with real dependencies

Mocks are useful for pure decision logic and controlled component behavior.

Mocks cannot prove:

- database semantics;
- indexes;
- transaction behavior;
- protocol compatibility;
- logical replication;
- message redelivery;
- OpenSearch behavior;
- browser security behavior;
- recovery;
- performance.

Those properties require the real dependency or a deliberately qualified substitute.

---

## 4.6 Negative testing is mandatory

Every meaningful feature shall consider not only what valid users can do, but also:

- invalid inputs;
- incomplete inputs;
- excessive inputs;
- stale inputs;
- conflicting inputs;
- duplicated inputs;
- unauthorized inputs;
- malformed protocol behavior;
- dependency failures;
- resource exhaustion.

---

## 4.7 Repetition is a distinct test technique

Running the same scenario repeatedly is not equivalent to retrying a failed test.

Repetitive testing is deliberately used to reveal:

- races;
- timing sensitivity;
- state leakage;
- cumulative resource leaks;
- probabilistic failures;
- idempotency defects;
- flaky behavior.

The repetition count or duration must be part of the test definition.

---

## 4.8 Non-functional qualities are release behavior

Security, performance, recovery, and resilience are not optional test categories performed after functional testing is “finished.”

A system that returns the right answer but leaks authorization, loses acknowledged writes, collapses under normal load, or cannot be restored is not functionally acceptable.

---

# 5. Risk Classification

Every significant change shall receive a test-impact classification.

### Risk A: Critical integrity/security

Examples:

- persistence;
- authentication;
- authorization;
- operation replay;
- version/concurrency logic;
- migrations;
- CDC;
- event ordering;
- resource allocation;
- accounting/cost calculations;
- backup/restore;
- cryptographic or secret handling.

Requires the strongest functional, negative, concurrency, security, recovery, and where relevant performance evidence.

### Risk B: Important service behavior

Examples:

- API endpoints;
- query compilation;
- search projection;
- workflow processing;
- scheduling logic;
- custom-field handling;
- browser synchronization;
- lifecycle transitions.

Requires automated positive and negative coverage, integration coverage where boundaries are involved, and impact assessment for security/performance/resilience.

### Risk C: Low-risk isolated behavior

Examples:

- presentation behavior;
- static declarations;
- internal tooling with limited impact.

Usually requires focused automated tests and static verification.

### Risk D: Documentation-only or non-behavioral

May require no runtime execution when genuinely unable to affect executable behavior.

A documentation label must not be used to avoid testing when configuration, executable examples, generated artifacts, deployment behavior, or operational instructions changed.

---

# 6. Test Levels

YAJA uses complementary test levels rather than one giant regression suite.

## Level 0: Static Verification

Executed before runtime testing.

Includes, as applicable:

- formatting;
- compiler warnings;
- Rust Clippy;
- Go vet/static analysis;
- TypeScript type checking;
- linting;
- architecture rules;
- forbidden API/pattern checks;
- secret scanning;
- dependency policy checks;
- SAST;
- infrastructure/configuration scanning.

Static checks should be fast enough to run on every pull request.

---

## Level 1: Pure Unit Tests

Tests deterministic business or transformation logic without external services.

Primary targets include:

- task mutation decisions;
- query parsing;
- schema-epoch decisions;
- future scheduling algorithms;
- cost calculations;
- state-machine transitions;
- custom-field validation;
- permission decisions.

Pure code should normally have:

- positive cases;
- negative cases;
- boundary cases;
- property-based cases where useful.

---

## Level 2: Component and Contract Tests

Tests one component through its real public boundary while controlling surrounding systems.

Examples:

- Go HTTP API using controlled upstream services;
- Rust worker with a disposable database;
- query compiler producing OpenSearch DSL;
- event consumer processing controlled messages;
- browser/Wasm compiler contract.

Contract tests shall verify both sides of important cross-language boundaries.

YAJA is polyglot. Rust, Go, TypeScript/Wasm and external service contracts must not drift independently.

---

## Level 3: Integration Tests

Exercise real service combinations.

Examples:

- Rust worker plus FerretDB/PostgreSQL;
- NATS JetStream behavior;
- Go API plus worker;
- PostgreSQL → CDC → NATS;
- NATS → indexer → OpenSearch;
- schema provisioning and readiness.

Integration tests verify behavior that mocks cannot establish.

---

## Level 4: System / End-to-End Tests

Exercise the supported user path through all relevant production components.

Eventually this includes:

Browser → Go API → Rust worker → authoritative storage → CDC → NATS → indexer → OpenSearch → synchronization → browser.

End-to-end tests shall remain selective.

A thousand slow browser scenarios are not a substitute for good lower-level testing.

---

## Level 5: Non-Functional Qualification

Dedicated environments and campaigns for:

- performance;
- scalability;
- security;
- concurrency;
- longevity;
- volume;
- recovery;
- chaos.

These tests often require workloads or conditions that would make ordinary PR testing impractical.

---

## Level 6: Release and Operational Qualification

Confirms:

- supported installation;
- upgrades;
- persisted-data compatibility;
- backup/restore;
- supported runtime environments;
- release artifact integrity;
- documented operating procedures;
- recovery procedures.

---

# 7. Functional Test Strategy

Functional testing shall cover more than happy paths.

## 7.1 Positive testing

Verify valid behavior at:

- normal values;
- minimum valid values;
- maximum valid values;
- meaningful combinations;
- allowed state transitions.

Acceptance scenarios from product requirements should become automated tests at the lowest appropriate level.

---

## 7.2 Negative testing

Every external boundary shall be challenged with:

- omitted required fields;
- unknown fields;
- invalid types;
- malformed encoding;
- invalid identifiers;
- invalid state transitions;
- stale versions;
- unsupported methods;
- unsupported media types;
- oversized payloads;
- truncated payloads;
- invalid query syntax;
- unexpected duplicate requests;
- unauthorized access;
- dependency failures.

Errors must be:

- intentional;
- stable enough for clients to handle;
- non-destructive;
- appropriately classified;
- free of sensitive implementation information.

---

## 7.3 Boundary-value testing

Particular attention shall be paid to:

- integer boundaries;
- time boundaries;
- replay/admission expiration;
- string limits;
- zero/one transitions;
- collection limits;
- scheduling start/end boundaries;
- daylight-saving/calendar behavior when calendars are introduced;
- financial rounding boundaries;
- resource-capacity boundaries.

The existing `i64::MAX`, title-length, body-size, and operation-age tests are examples of this principle.

---

## 7.4 Repetitive testing

Critical behaviors shall be exercised repeatedly.

Examples include:

- identical operation retries;
- concurrent updates;
- repeated create/update/delete cycles;
- repeated service restart/recovery;
- repeated schema migration;
- repeated browser reconnect;
- repeated event redelivery.

PR testing may use a small deterministic repetition budget.

Nightly testing should use larger time-bounded or iteration-bounded campaigns with recorded random seeds.

---

## 7.5 Property-based testing

Property-based testing should be preferred where the state space is much larger than useful hand-written examples.

Priority YAJA targets:

### Query compiler

Properties include:

- parser never panics for arbitrary input;
- successful parse always produces valid AST;
- rejected syntax does not produce partial executable output;
- serialization/compilation preserves semantic meaning;
- generated database/search queries cannot escape the AST model;
- complexity limits remain bounded.

### Mutation contract

Properties include:

- version never decreases;
- a fresh accepted operation increments exactly once;
- identical recorded operation always replays its original result;
- changed-content reuse never applies;
- stale version never silently applies as a new operation.

### Scheduling

Future scheduling logic should use model/property testing extensively because examples alone will not cover combinations of:

- dependencies;
- resource calendars;
- fixed milestones;
- multiple resource types;
- release/reopening;
- manual and automatic scheduling.

### Cost calculations

Financial properties should include conservation and history rules, not merely examples.

---

# 8. Fuzz Testing

Fuzzing shall be used for untrusted parsers and protocol boundaries.

Priority targets:

1. Rust query parser.
2. Future full query/compiler pipeline.
3. JSON mutation deserialization.
4. Task and custom-field identifiers.
5. Go HTTP boundary.
6. Wasm compiler interface.
7. CDC/event payload decoders.
8. Any import/export format.

Fuzzing should test for:

- crashes;
- panics;
- hangs;
- excessive CPU;
- excessive memory;
- invalid state acceptance;
- security boundary bypasses.

Short fuzz smoke tests may run on PRs.

Longer fuzz campaigns should run nightly or periodically.

Every reproducible fuzz defect must become a permanent regression case.

---

# 9. Model-Based and State-Machine Testing

YAJA contains stateful behavior for which isolated examples will eventually be insufficient.

State-machine testing should be used for:

- task lifecycle;
- status/phase mappings;
- workflow transitions;
- resource reservation/release;
- task completion/reopening;
- time tracking;
- scheduling;
- saga execution;
- authentication/session lifecycle.

The model defines legal transitions and invariants.

Random command sequences are executed against both the model and the system.

Differences are failures.

This will become particularly important as configurable WorkItem types, custom fields, workflows, and project-specific status models expand the state space.

---

# 10. Concurrency Correctness Strategy

Concurrency receives a dedicated strategy because occasional race testing is not sufficient evidence.

The existing 48 competing mutation pairs should remain as fast regression coverage.

They must not be treated as proof of linearizability.

Concurrency qualification shall progress through four layers.

## Layer 1: Deterministic unit-level concurrency rules

Pure decision functions verify version and retry semantics.

## Layer 2: Controlled race regression

Small concurrent cases exercise known problematic interleavings.

Current `task_path.py` coverage belongs here.

## Layer 3: Randomized concurrency histories

A dedicated harness generates many concurrent histories involving:

- creates;
- updates;
- retries;
- operation-ID reuse;
- stale clients;
- delayed responses;
- dependency delay.

Each history records:

- invocation;
- completion;
- operation ID;
- expected version;
- result;
- observed final state.

Histories are checked against the allowed consistency model using a linearizability/model checker rather than merely checking that no process crashed.

## Layer 4: Concurrency under stress and faults

Concurrency testing is repeated while introducing:

- database delay;
- connection exhaustion;
- API saturation;
- process restarts;
- network interruption;
- event lag.

The system must continue to preserve integrity invariants even when latency objectives cannot be maintained.

---

# 11. Core Data-Integrity Invariants

The following invariants should be treated as first-class test requirements for the current task path.

1. An HTTP 200 mutation response means the authoritative persistence layer acknowledged the mutation.

2. Search visibility is independent of authoritative save acknowledgment.

3. Identical replay using the same valid operation ID returns the original successful result.

4. Reusing an operation ID with different logical content does not execute a new intention.

5. Two fresh competing mutations expecting the same version cannot both become the next version.

6. Versions never silently move backward.

7. An older search projection must never replace a newer authoritative or projected version.

8. A timeout after a command may represent an uncertain outcome. Retrying with the same operation ID must reconcile that uncertainty safely.

9. Unavailable search must not make authoritative task reads or saves logically disappear.

10. Unsafe persistence configuration must prevent readiness before mutations are accepted.

11. Failure of one client must not create unbounded abandoned database work.

These invariants should eventually be referenced directly from automated test metadata or names.

---

# 12. Requirements Traceability

YAJA already uses stable requirement, decision, question, backlog, and spike IDs.

Testing should preserve this strength.

Traceability should follow:

**Requirement → Decision → Implementation → Automated Tests → Evidence**

For important behavior, a reviewer should be able to answer:

- Which requirement defines this?
- Which architectural decision implements it?
- Which tests protect it?
- Which environment was used?
- What does the evidence prove?
- What remains unverified?

Not every individual unit test needs bureaucracy attached to it.

Critical acceptance and non-functional tests should, however, reference relevant `R-*`, `D-*`, `B-*`, `Q-*`, or spike IDs.

---

# 13. Security Test Strategy

Security testing shall follow the architecture, not be reduced to one vulnerability scanner.

## 13.1 Threat modeling

A threat model is required when introducing or materially changing:

- authentication;
- authorization;
- externally reachable APIs;
- browser sessions;
- file handling;
- plugins/extensions;
- query execution;
- multi-user capability;
- secrets;
- synchronization;
- remote hosting.

Threat modeling should identify:

- assets;
- actors;
- trust boundaries;
- entry points;
- abuse cases;
- required controls;
- verification method.

---

## 13.2 Static security testing

PR CI should eventually include:

- Rust security linting;
- Go security/static analysis;
- TypeScript/JavaScript analysis;
- Python script analysis where relevant;
- CodeQL or equivalent multi-language SAST.

Security warnings must be triaged, not blindly silenced.

---

## 13.3 Dependency and supply-chain testing

Automated checks should cover:

- Rust advisories;
- Go vulnerability database;
- npm/pnpm dependencies;
- Python dependencies when third-party packages are introduced;
- container image vulnerabilities;
- license policy where relevant.

Release qualification should additionally produce an SBOM.

Critical/high exploitable vulnerabilities block release unless an explicitly documented time-bounded exception is approved.

---

## 13.4 Secret scanning

Credentials, private keys, tokens, and sensitive configuration must be scanned before merge.

Historical secret exposure requires credential rotation, not merely deleting the file.

---

## 13.5 CI and artifact integrity

Release hardening should include:

- least-privilege GitHub Actions permissions;
- immutable action references;
- pinned release dependency/image digests where practical;
- provenance;
- signed release artifacts.

The existing `contents: read` and `persist-credentials: false` settings are good foundations.

---

## 13.6 API security

API security tests shall cover:

- invalid methods;
- request smuggling edge cases where supported by the stack;
- malformed bodies;
- body-size enforcement;
- content-type enforcement;
- duplicate fields;
- unsupported encodings;
- ID/path manipulation;
- header manipulation;
- rate/resource exhaustion;
- information leakage;
- error sanitization.

---

## 13.7 Authentication and authorization

Once authentication exists, every protected operation requires explicit tests for:

- unauthenticated access;
- valid owner access;
- expired sessions;
- invalid sessions;
- session recovery;
- logout/revocation;
- cross-user access if multi-user support is added;
- privilege boundaries;
- configuration access;
- read protection as well as write protection.

Origin and Host checking must never be considered authentication.

---

## 13.8 Browser security

When the browser UI exists, test:

- CSRF;
- CORS;
- origin validation;
- session cookie attributes;
- CSP;
- clickjacking protection;
- XSS;
- unsafe URL handling;
- browser storage exposure.

The current missing-Origin behavior is provisional and must be replaced by the supported browser-session contract.

---

## 13.9 Query security

The planned query compiler is a major attack surface.

Security invariants include:

- queries are built through typed ASTs;
- user input is never incorporated into database/search queries through arbitrary string concatenation;
- unsupported capabilities are rejected;
- browser-side and server-side compilation agree;
- excessive recursion/depth/size is bounded;
- malicious queries cannot generate catastrophic computation;
- emitted OpenSearch queries cannot escape intended field/value semantics.

Property testing and fuzzing are mandatory here.

---

## 13.10 Dynamic application security testing

Once a runnable browser/API product exists, release-candidate environments should receive authenticated and unauthenticated DAST.

Automated DAST complements manual review. It does not replace threat modeling or authorization testing.

---

# 14. Performance Engineering Strategy

Performance is treated as an engineering discipline, not a stopwatch attached to an integration test.

The current worker limits and timeouts are development controls. They are **not product SLOs**.

Product performance objectives must be explicitly defined and approved from:

- expected user behavior;
- supported hardware;
- supported dataset size;
- concurrency;
- reliability requirements.

Arbitrary latency numbers must not become contractual merely because they appeared in test code.

---

# 15. Performance Test Layers

## 15.1 Microbenchmarks

Used for hot pure operations such as:

- parser/compiler;
- scheduling;
- serialization;
- cost calculation;
- filtering;
- synchronization decisions.

Rust benchmark targets should use an appropriate statistical benchmark framework.

Go hot paths should use native Go benchmarks.

---

## 15.2 Component benchmarks

Measure:

- worker mutation latency;
- worker reads;
- API proxy overhead;
- query compilation;
- event processing;
- OpenSearch indexing.

---

## 15.3 End-to-end performance

Measure user-observable flows such as:

- save acknowledgment;
- authoritative read;
- search visibility lag;
- browser update;
- batch/saga completion.

Save latency and search visibility latency must remain separate metrics.

Combining them would contradict D-016.

---

# 16. Performance Workload Profiles

Performance qualification shall include distinct workload profiles.

### Baseline

Single-user or low-load measurements establishing expected latency and resource cost.

### Steady state

Sustained expected workload.

### Burst

Short rapid increases in activity.

### Stress

Increasing load until a limit is reached.

The goal is to understand failure behavior, not simply obtain the largest throughput number.

### Spike

Abrupt traffic increase.

### Endurance / soak

Long-duration operation intended to reveal:

- memory leaks;
- handle leaks;
- connection leaks;
- log growth;
- queue growth;
- compaction problems;
- cache deterioration.

### Volume

Large persisted datasets.

Especially important for YAJA because immutable operation history, custom fields, search indexes, planning history, and future cost/resource data can grow substantially.

### Recovery under load

A failed dependency is recovered while meaningful load continues.

### Scalability

Determine how throughput, latency, and resource consumption change as resources or workload increase.

---

# 17. Performance Metrics

At minimum, record:

- request rate;
- successful throughput;
- failure rate;
- p50 latency;
- p95 latency;
- p99 latency;
- maximum latency where diagnostically useful;
- CPU;
- memory;
- allocations/GC where applicable;
- database connection usage;
- worker semaphore saturation;
- queue depth;
- NATS consumer lag;
- CDC lag;
- OpenSearch indexing lag;
- WAL growth;
- disk consumption;
- disk I/O;
- network traffic;
- startup/recovery time.

Average latency alone is not an acceptable performance metric.

---

# 18. Performance Regression Policy

Once a stable benchmark is established:

- performance tests compare against a versioned baseline;
- statistically meaningful regressions are investigated;
- thresholds are specific to the metric and workload;
- environment noise must be measured;
- regressions may not be hidden by simply updating the baseline.

Small deterministic microbenchmarks may become PR gates.

Full load, volume, and soak testing belongs in scheduled or release qualification.

---

# 19. Capacity Qualification

Before the first supported release, define at least one supported operating envelope containing:

- number of projects;
- number of WorkItems/tasks;
- operation-history size;
- number of custom fields;
- query complexity;
- concurrent active users;
- event rate;
- search index size;
- disk budget;
- RAM budget;
- startup target;
- normal save latency;
- normal search visibility lag.

The existing SP-001 memory observations are valuable experimental evidence but are not minimum hardware requirements.

---

# 20. Chaos Engineering Strategy

Chaos testing validates known resilience hypotheses by deliberately introducing controlled failures.

Chaos tests must begin with a stated steady state and expected invariant.

Random destruction without a hypothesis is merely vandalism with telemetry.

---

# 21. Chaos Principles

Every chaos experiment defines:

1. steady-state behavior;
2. injected fault;
3. expected degraded behavior;
4. integrity invariants that must remain true;
5. recovery expectation;
6. observation points;
7. stop condition;
8. cleanup procedure.

Chaos experiments must be reproducible.

---

# 22. Priority Failure Domains

## PostgreSQL / authoritative storage

Inject:

- process termination;
- restart;
- network interruption;
- latency;
- connection exhaustion;
- unavailable database;
- disk pressure;
- replica-identity drift;
- publication drift;
- replication-slot lag;
- WAL growth.

Verify:

- acknowledged data integrity;
- bounded request failure;
- readiness behavior;
- retry safety;
- eventual recovery.

---

## FerretDB

Inject:

- process restart;
- disconnect;
- latency;
- upgrade;
- mapping/schema changes.

Verify compatibility assumptions against actual PostgreSQL mapping.

---

## NATS JetStream

Inject:

- broker crash;
- consumer restart;
- connection loss;
- delayed acknowledgments;
- redelivery;
- duplicate delivery;
- out-of-order processing where applicable;
- disk pressure.

Verify no duplicated logical outcome and eventual catch-up.

---

## CDC / Debezium

Inject:

- process crash;
- restart;
- delayed consumption;
- retained WAL backlog;
- schema change;
- malformed/poison event.

Verify durable continuation from the expected checkpoint and visible failure reporting.

---

## OpenSearch

Inject:

- complete outage;
- high latency;
- rejected writes;
- restart;
- stale projection;
- index deletion;
- reindex/rebuild.

Verify authoritative writes remain independent and projections recover without regression.

---

## Go API

Inject:

- crash;
- overload;
- slow clients;
- downstream delay.

Verify bounded resource use and clean recovery.

---

## Rust worker

Inject failure:

- before persistence;
- during database operation;
- immediately after persistence;
- before response;
- during contention.

The expected client behavior must be defined for each point.

---

## Host/runtime

Before supported release, test:

- application process restart;
- container recreation;
- container-engine restart;
- ordinary machine restart;
- low disk;
- backup restoration onto a clean installation.

---

# 23. Chaos Cadence

Deterministic fault regression belongs in ordinary CI where inexpensive.

Examples:

- database unavailable;
- stalled dependency;
- search unavailable.

Broader chaos campaigns belong in:

- nightly testing;
- architecture spikes;
- pre-release qualification.

Production chaos should not be introduced until YAJA has:

- production observability;
- explicit blast-radius controls;
- supported recovery;
- suitable deployment topology.

For the initial local-product scope, controlled pre-release environments are sufficient.

---

# 24. Recovery and Backup Testing

Backups are not considered valid merely because a backup file exists.

Recovery qualification must demonstrate:

1. backup creation;
2. backup age visibility;
3. failure reporting;
4. clean-instance restore;
5. restored authoritative data;
6. restored configuration;
7. preservation/reconciliation of operation replay history;
8. search reconstruction;
9. browser/client reconciliation after restore.

Recovery tests shall eventually measure:

- RPO;
- RTO.

Both must be product decisions.

Daily scheduling alone does not constitute a 24-hour RPO.

---

# 25. Upgrade and Migration Testing

Every persisted schema or storage-format change requires explicit compatibility testing.

Test:

- fresh installation;
- previous supported version → new version;
- restart during migration where relevant;
- migration retry;
- partially migrated state;
- rollback capability where promised;
- data preservation;
- index recreation;
- search rebuild;
- backup restore from supported historical versions.

Migration tests must use realistic persisted data, not an empty database only.

---

# 26. Custom Fields and WorkItem Types

Custom fields are a core YAJA architectural differentiator and require dedicated coverage as implementation expands.

Testing should cover:

- every supported data type;
- optional fields;
- required fields;
- hidden fields;
- default values;
- validation;
- field changes;
- schema-version changes;
- search/sort behavior;
- migration;
- archive behavior;
- different semantics for similarly named fields belonging to different WorkItem types.

Field identity must not accidentally collapse merely because two WorkItem types use the same display name.

Large-scale performance tests must include heterogeneous custom-field populations rather than uniform synthetic documents.

---

# 27. Scheduling, Resources, and Costs

These future domains are combinatorial and must not rely mainly on hand-written examples.

Testing should combine:

- example-based acceptance tests;
- property-based testing;
- state-machine testing;
- temporal boundary testing;
- concurrency testing.

Important future properties include:

- physical resource capacity is never created by scheduling;
- released expired time cannot become future availability;
- consumption of one resource cannot imply consumption of another;
- completion releases only applicable future reservations;
- reopening cannot steal capacity subsequently allocated elsewhere;
- history is preserved;
- actual, planned, and forecast cost remain distinct;
- refundable and non-refundable policies preserve their different accounting rules;
- scheduling conflicts remain visible rather than silently violating fixed constraints.

---

# 28. Search and CQRS Verification

YAJA explicitly separates authoritative state from search projection.

Testing must therefore treat the following as separate states:

- committed but not projected;
- projected stale version;
- projected current version;
- projected future/newer version;
- search unavailable;
- projection corrupted.

The UI/API must never interpret “not yet searchable” as “does not exist.”

Search reconstruction must be tested from authoritative data without requiring the original search index.

---

# 29. Event-Pipeline Testing

When the production CDC/indexer path exists, test:

- initial capture;
- event ordering;
- duplicates;
- redelivery;
- consumer restart;
- producer restart;
- checkpoint durability;
- poison events;
- old events;
- schema changes;
- prolonged consumer lag;
- replay;
- full index rebuild while writes continue.

Every event shall carry sufficient identity/version information to verify idempotency and ordering.

---

# 30. Test Data Strategy

Automated testing shall use synthetic data unless a specific approved reason exists otherwise.

Test data should support:

- deterministic fixed fixtures;
- seeded generated fixtures;
- boundary datasets;
- malformed datasets;
- realistic heterogeneous datasets;
- large datasets.

Generated-test failures must record the seed necessary to reproduce them.

Personally identifiable or production data must not be copied casually into test environments.

---

# 31. Environment Strategy

Different claims require different environments.

At minimum, YAJA should eventually maintain evidence for:

### Developer environment

Fast local pure and focused integration testing.

### Linux CI environment

Primary deterministic automation.

### Windows/Podman supported-runtime environment

Required because Windows/Podman is identified as the first supported runtime direction.

### Full integration environment

Actual PostgreSQL, FerretDB, NATS, CDC, OpenSearch, worker, API and eventually UI.

### Performance environment

Stable enough that measurements are meaningful.

### Security environment

Isolated environment suitable for active scanning.

No environment should be called “production-like” without defining which production properties it reproduces.

---

# 32. CI/CD Test Phases

| Phase | Required testing |
|---|---|
| During design | Acceptance criteria, invariants, testability, threat/failure/performance assessment |
| Developer inner loop | Unit, focused negative, property tests, lint/static checks |
| Pre-commit | Fast deterministic affected tests |
| Pull request | Static analysis, unit tests, security scans, build, relevant component/integration tests |
| Main branch | Full deterministic regression and integration suite |
| Nightly | Fuzzing, repetitive concurrency, extended integration, cross-runtime, performance smoke, selected chaos |
| Periodic | Dependency/container deep scans, mutation testing, long fuzzing, data-volume testing |
| Release candidate | Full functional qualification, security, performance, chaos, backup/restore, upgrade, supported-runtime matrix |
| Post-release | Operational health/SLO analysis, regression from escaped defects |

Until the suite becomes expensive, YAJA should prefer running the full deterministic suite on PRs rather than prematurely building clever test-selection logic.

Optimization comes after measurement.

---

# 33. Pull-Request Quality Gate

A normal executable-code PR should not merge unless:

- affected requirements/behavior are identified;
- tests for new behavior exist;
- relevant negative cases exist;
- regression tests pass;
- formatting/lint/static analysis passes;
- SAST/dependency/secret checks pass once introduced;
- applicable real-dependency tests pass;
- performance/security/resilience impact has been considered;
- limitations are documented;
- required documentation is updated.

Risk-A changes require explicit reviewer consideration of:

- concurrency;
- security;
- failure recovery;
- data migration/integrity.

---

# 34. Nightly Qualification

Nightly testing should eventually include:

- longer parser/API fuzzing;
- randomized property tests;
- repeated critical flows;
- randomized concurrency histories;
- race detection;
- Windows/Podman execution;
- performance smoke benchmarks;
- dependency-failure scenarios;
- service restart/recovery scenarios.

Nightly failures are defects.

They are not advisory merely because PR CI passed.

---

# 35. Release-Candidate Exit Criteria

A supported release may not rely only on green unit/integration CI.

The release candidate requires:

- all release-scope requirements classified;
- critical requirements mapped to automated acceptance evidence;
- no unresolved release-blocking functional defects;
- no known exploitable critical/high security vulnerability without approved exception;
- supported installation verified;
- supported upgrade path verified;
- backup and clean restore verified;
- supported runtime matrix passed;
- approved performance envelope passed;
- required resilience/chaos experiments passed;
- dependency inventory/SBOM available;
- release artifacts reproducible or otherwise integrity-verified according to release policy;
- known limitations documented.

---

# 36. Flaky Test Policy

A test is flaky when identical relevant inputs/environment can produce inconsistent outcomes without an intentionally randomized assertion.

Flaky tests are defects in:

- product;
- test;
- environment;
- or test architecture.

A flaky blocking test should be fixed promptly.

Repeated automatic retries must not turn red into green.

If temporary quarantine becomes unavoidable:

- record the defect;
- identify owner;
- state risk;
- define expiry;
- preserve visibility.

Critical integrity/security tests should not be silently quarantined.

---

# 37. Test Repetition Policy

For explicitly repetitive tests:

- repetition is declared before execution;
- all iterations count;
- first failure fails the campaign;
- failure seed/history is retained;
- test results state the number of successful repetitions completed.

A campaign passing 999 times and failing once is a failed campaign, not “99.9% passing.”

---

# 38. Observability for Testing

Testability requires observability.

As YAJA grows, tests should be able to correlate:

- client operation ID;
- request;
- authoritative database version;
- CDC event;
- NATS sequence;
- consumer attempt;
- OpenSearch projection version;
- browser synchronization event.

Structured logs and metrics should make that relationship observable without scraping human-oriented strings.

Performance and chaos tests require metrics rather than only process exit codes.

---

# 39. Test Evidence

CI should retain useful failure evidence.

Depending on test type this may include:

- test report;
- failing random seed;
- concurrency history;
- service logs;
- container status;
- metrics;
- resource samples;
- fault timeline;
- dependency versions;
- relevant configuration.

A passing result without knowing the environment or revision is weak evidence.

---

# 40. Code Coverage

Code coverage is a diagnostic tool, not the quality target.

Coverage should identify untested logic but must not replace risk reasoning.

For critical pure domain code, high branch coverage is expected.

Critical invariants should receive explicit tests regardless of line-coverage percentage.

Adapter code should be judged mainly by boundary/failure coverage rather than chasing arbitrary line percentages.

---

# 41. Mutation Testing

Periodic mutation testing is recommended for compact, critical pure logic such as:

- `task_contract`;
- query compiler;
- synchronization rules;
- scheduling;
- cost calculations.

Mutation testing answers a useful question that line coverage cannot:

**Would these tests actually detect incorrect logic?**

It need not run on every PR.

---

# 42. Test Tooling Direction

Specific tools may change without changing this strategy.

A sensible YAJA tool direction is:

### Rust

- `cargo test`
- `cargo fmt`
- `cargo clippy`
- `cargo llvm-cov`
- property testing such as `proptest`
- `cargo fuzz`
- Criterion-style benchmarks
- `cargo audit`
- `cargo deny`
- periodic mutation testing

### Go

- `go test`
- `go test -race`
- native Go fuzzing
- native benchmarks
- `go vet`
- `staticcheck`
- `gosec`
- `govulncheck`

### TypeScript / browser

When runtime code exists:

- TypeScript compiler
- linting
- unit/component testing
- Playwright-class browser testing
- accessibility testing
- dependency scanning

### API/system load

A workload-oriented tool such as k6 is suitable for HTTP/system load generation.

The workload definitions and assertions are more important than the particular load generator.

### Security

Suitable categories include:

- CodeQL/SAST;
- secret scanning;
- dependency scanners;
- container/configuration scanning;
- OWASP ZAP-class DAST.

### Fault injection

For the current Compose/local architecture:

- controlled process/container termination;
- dedicated network fault proxies for latency/drop/partition;
- explicit resource-pressure mechanisms.

A Kubernetes-specific chaos platform is unnecessary until Kubernetes actually becomes a supported architecture.

---

# 43. Change-to-Test Selection Matrix

| Change | Mandatory test emphasis |
|---|---|
| Pure domain rule | Unit, negative, boundary, property |
| Query grammar/compiler | Unit, property, fuzz, injection/security, benchmarks |
| HTTP boundary | Positive/negative protocol, fuzz, security, component |
| Persistence mapping/index | Real DB integration, migration, concurrency, volume |
| Mutation/idempotency | Replay, repetition, concurrency, uncertain-outcome recovery |
| NATS/event consumer | Duplicate, ordering, redelivery, restart, lag |
| CDC | Real replication, restart/checkpoint, WAL, schema-change, chaos |
| Search/indexing | Projection freshness, stale events, rebuild, load |
| Authentication | Threat model, authn/authz matrix, session/CSRF, DAST |
| Browser UI | Component, E2E, accessibility, security, performance |
| Wasm compiler | Cross-runtime equivalence, fuzz, security, browser performance |
| Scheduling | Model/property, temporal boundaries, volume, concurrency |
| Resource accounting | Invariants, state machine, concurrency |
| Costs | Precision/rounding, historical rate behavior, property testing |
| Schema/custom fields | Compatibility, migration, heterogeneous volume, query/sort |
| Installation | Clean install, readiness, least privilege |
| Upgrade | Old→new data migration, restart/retry, backup/restore |
| Recovery | Crash, restart, machine recovery, restore |
| Performance-sensitive path | Benchmark + representative load |
| Security-sensitive boundary | Threat model + security suite |
| Dependency version | Relevant integration + security + compatibility |
| Infrastructure config | Readiness + security config + recovery |

---

# 44. Feature Test Impact Record

Every substantial feature or architectural change should have a lightweight Test Impact Record.

It may live in the issue, PR, or dedicated test plan.

Template:

## Behavior

What user/system behavior changes?

## Traceability

Relevant requirements, decisions, backlog items and open questions.

## Risk

A / B / C / D and rationale.

## Invariants

What must never become false?

## Positive coverage

Expected valid behavior.

## Negative coverage

Invalid, stale, conflicting, unauthorized and malformed behavior.

## Boundary coverage

Relevant limits and edge values.

## Repetitive/concurrency coverage

What must be repeated or run concurrently?

## Integration coverage

Which real dependencies must participate?

## Security impact

New assets, trust boundaries, inputs or permissions?

## Performance impact

Could latency, throughput, memory, CPU, disk or data growth materially change?

## Resilience impact

Which dependency failures can affect this behavior?

## Recovery impact

Does this affect durable state or restoration?

## Environment/data

Required environment and dataset.

## Exit criteria

What evidence is required before the change is considered adequately verified?

This is the primary mechanism by which this strategy drives future test planning.

---

# 45. Immediate YAJA Priorities

The following work should be introduced progressively rather than creating one enormous “QA transformation” change.

## Priority 1: Formalize the strategy

Add this document and link it from:

- `README.md`;
- `CONTRIBUTING.md`;
- `docs/TESTING.md`;
- procedure index.

Keep `docs/TESTING.md` focused on executable test instructions and current coverage.

---

## Priority 2: Strengthen PR static gates

Add:

- Rust formatting and Clippy;
- Go formatting/vet;
- dependency vulnerability checks;
- secret scanning;
- SAST;
- container/configuration scanning.

---

## Priority 3: Introduce property and fuzz tests

Start with:

1. `yaja_query`
2. `task_contract`
3. operation-ID validation
4. HTTP mutation deserialization

These have high defect-detection value and small implementation scope.

---

## Priority 4: Add a real concurrency qualification harness

Keep the current 48-pair test.

Add longer randomized histories and a model/linearizability checker.

Do not merely increase 48 to 4,800 and declare victory. Repeating an assertion that is too weak 100 times merely produces very confident ignorance.

---

## Priority 5: Establish performance baselines

Before choosing product SLOs, measure:

- worker save/read;
- API save/read;
- search visibility lag;
- resource usage;
- operation-history growth.

Capture the first versioned baseline.

---

## Priority 6: Generalize deterministic failure injection

The current database fault proxy is useful.

Evolve fault testing into reusable dependency fault controls covering:

- database;
- NATS;
- CDC;
- OpenSearch;
- worker/API process lifecycle.

---

## Priority 7: Add supported-runtime CI

Add Windows/Podman qualification once the installation path is sufficiently automated.

Linux/Docker passing cannot by itself qualify the first intended Windows/Podman supported environment.

---

## Priority 8: Release-gate recovery

Before real supported user data:

- process crash persistence;
- container recreation;
- machine restart;
- backup;
- clean restore;
- search rebuild;
- operation replay after restore.

These should be release blockers rather than optional demonstrations.

---

# 46. Strategy Success Criteria

This strategy is working when, for any important proposed YAJA change, the team can answer before implementation:

1. What behavior are we promising?
2. What must never happen?
3. Which risks matter?
4. Which test levels provide evidence?
5. Which negative cases matter?
6. Does concurrency matter?
7. Does security change?
8. Does performance change?
9. Which dependency failures matter?
10. What proves recovery?
11. Which environment is representative?
12. What evidence is required to merge or release?

If those questions cannot be answered, test planning is incomplete.

The goal is not maximum test count.

The goal is justified confidence in the claims YAJA makes about its behavior.