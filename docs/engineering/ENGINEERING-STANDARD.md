# YAJA Engineering Standard

Version 1. Baseline assessment: 2026-10-02. Owner: the project maintainer defined
in [CONTRIBUTING](../../CONTRIBUTING.md). This framework defines mandatory release
qualification; its adoption does not approve a release or certify compliance.

This directory keeps engineering qualification separate from product scope and
approved SOPs. Existing R-, D-, B-, Q-, SOP-, and SP-* records remain authoritative
for their subjects. Engineering IDs add independently verifiable quality gates.
The authored preamble is [STANDARD-PREAMBLE.md](STANDARD-PREAMBLE.md); structured
requirements and references live in [requirements.json](requirements.json). Regenerate
with `python scripts/check_engineering.py --write`. CI checks schema, unique IDs,
references, backlog destinations, evidence presence, and the generated view.
It cannot establish that an evidence claim is true.

## Normative rules and qualification

MUST and SHALL are mandatory; SHOULD requires a documented reason for departure;
MAY is optional. The register's explicit obligation field distinguishes required,
recommended and optional controls. Each control applies from its stated gate
onward. All applicable mandatory clauses must be satisfied. A partial
implementation does not pass. E5 is a later target, not current delivery scope.
No requirement permits weakening accepted product data-integrity rules.

A qualification record MUST identify the immutable source commit, artifacts,
runtime/service versions, applicable requirement IDs, test/evidence links and
results, unresolved risks, and maintainer assessment. Link Requirement → Decision
→ Backlog/Issue → PR → Test/Evidence → Release; use existing IDs wherever possible.
For each external framework, record the exact revision and a control-by-control
applicability/evidence matrix. A reference here is a source, not full equivalence.
ASVS L1/L2 and OSPS L1/L2 assessments must cover all applicable controls, not a
selected sample. ISO quality characteristics guide YAJA-specific criteria rather
than a certification claim. Include safety risks when loss or misleading state
could affect downstream decisions; YAJA is not qualified for safety-critical use.

Exceptions MUST identify scope, owner, rationale, mitigation, approver, and expiry
or review date. Not applicable MUST have evidence-based scope justification.
An exception is not satisfaction: a release with unmet mandatory requirements
MUST remain below that gate. Review applicability on every scope/architecture
change. The register deliberately has no blanket waived or compliant status.

`python scripts/check_engineering.py --gate E1` fails on recorded mandatory cumulative
gaps for Engineering Preview (use E2–E5 for later gates) and reports unsatisfied
recommendations/options as nonblocking findings.
Recommendations still require an evidence-based disposition or documented reason
for departure in the qualification assessment; passing this check does not supply it.
It is a necessary metadata check for qualification, not approval. Before claiming
a gate, the maintainer MUST inspect the evidence at the final commit and verify
remote required-check settings separately. No self-review approval is required
during the sole-contributor phase; self-assessment remains required.

## Maturity gates

Engineering gates below are independent of the backlog's existing **product
delivery milestones M0–M4** and child IDs M1-01–M1-08. Product M1 does not mean
Engineering Preview, and reaching a delivery milestone does not imply maturity.

| Gate | Required observable outcome |
| --- | --- |
| E0 — Experimental | Bounded components/spikes with explicit limitations; no supported release or real-data readiness claim. |
| E1 — Engineering Preview | Traceable requirements and architecture records; repeatable development setup; full unit/integration CI; formatting/lint baseline; contribution and security policies; explicit limitations; documented and verified merge controls. |
| E2 — Alpha | A coherent authenticated/authorized usable slice; reviewed threat model; ASVS 5 L1 and OSPS L1 evidence; SAST, vulnerability and secret scans; crash/restart durability, backup and tested clean restore; OpenAPI, errors, replay and resource bounds; parser properties/fuzzing and measured performance baseline. |
| E3 — Beta | Critical-path E2E; concurrency/property/model and failure-injection qualification; defined measured SLIs/SLOs and RPO/RTO; tested supported upgrades/migrations; performance envelope; OTel signals/runbooks; SBOM and hosted signed provenance with SLSA Build L2 assessment; API/event compatibility policy. |
| E4 — Production / 1.0 | SemVer commitment, supported upgrades/support matrix; ASVS L2 and OSPS L2 evidence; signed artifacts, SBOM and provenance; achieved SLO and RPO/RTO evidence including disaster recovery; API/event guarantees; incident response and WCAG 2.2 AA; no unresolved critical integrity architecture gaps in shipped scope. |
| E5 — Enterprise-ready | Justified enterprise identity/access/audit, HA/scaling/recovery/isolation qualification, hardening, external security assessment and customer-relevant compliance evidence. No speculative implementation is required now. |

The precise cumulative gate is the set of register entries with first gate ≤ the
candidate gate, plus the qualification rules above. A usable Alpha slice does not
reduce the agreed full first-release product scope; the release scope must be
explicitly decided through B-018/Q-022. Production excludes unresolved integrity
questions affecting its shipped behavior, even if a limited single-task test passes.

## Current assessment and evidence boundaries

YAJA is **E0 — Experimental**. Source tests and the historical full-suite evidence
in [IMPLEMENTATION](../IMPLEMENTATION.md) support bounded task semantics. E1 is
not yet qualified: final change/release assessment is missing. The dated main
protection inspection is recorded in [the assessment](ASSESSMENT.md). The setup is documented, but not a supported
installation. No authentication, real UI, production CDC/indexer, supported
backup/restore or release qualification exists. This change does not broaden
SP-001's Python-adapter results into Go/Rust or production guarantees.

E2 blockers include usable B-004/B-005/B-006/B-007 integration, identity/session
and project grants, security assessment/scanning, contract tests, durable
installation, compaction/rebuild and clean restore, parser bounds/fuzzing, and
performance baseline. E3 additionally requires integrated CDC/events, upgrade and
migration qualification, RPO/RTO and SLO decisions, telemetry/runbooks, critical
E2E, sustained concurrency/fault evidence, SBOM/provenance and builder assessment.
Q-019 remains unresolved for cross-record configuration/archive and resource/cost
consistency; Q-020 for recovery/runtime budgets; Q-022 for release scope/scale.

See [architecture governance](../architecture/README.md),
[threat model](THREAT-MODEL.md), [dependency policy](DEPENDENCIES.md),
[qualification and operations](QUALIFICATION.md), and
[Definition of Done](DEFINITION-OF-DONE.md) and
[coding standard](CODING-STANDARD.md). These define evidence obligations;
they do not claim the future mechanisms already exist.

## Reference standards

- **ISO25010:** [ISO/IEC 25010:2023 product quality model](https://www.iso.org/standard/78176.html).
- **SSDF:** [NIST SP 800-218 SSDF 1.1](https://csrc.nist.gov/pubs/sp/800/218/final).
- **ASVS:** [OWASP ASVS 5.0.0](https://github.com/OWASP/ASVS/tree/v5.0.0).
- **OSPS:** [OpenSSF OSPS Baseline 2025-02-25](https://baseline.openssf.org/versions/2025-02-25).
- **SLSA:** [SLSA 1.2](https://slsa.dev/spec/v1.2/).
- **Scorecard:** [OpenSSF Scorecard](https://scorecard.dev/).
- **OpenAPI:** [OpenAPI specification](https://spec.openapis.org/).
- **SemVer:** [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).
- **OTel:** [OpenTelemetry signals](https://opentelemetry.io/docs/concepts/signals/).
- **OCI:** [Open Container Initiative specifications](https://opencontainers.org/).
- **WCAG:** [WCAG 2.2](https://www.w3.org/TR/WCAG22/).
- **SPDX:** [SPDX](https://spdx.dev/).
- **CycloneDX:** [CycloneDX](https://cyclonedx.org/).

## Requirement register

Paths below are relative to the repository root in the register. Status applies to the entire requirement, not just its existing tests.

<a id="gov-001"></a>
### GOV-001 — Traceability and change completion

Changes MUST link acceptance criteria, decisions, backlog or issue, PR, evidence and release where applicable, and apply the Definition of Done with explicit justified not-applicable entries.

- **Rationale:** Review must distinguish specified behavior from shipped behavior.
- **First applicable gate:** E1 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** SSDF, OSPS.
- **Required evidence:** Final PR assessment, linked positive/negative evidence and scoped release record.
- **Enforcement:** Register validator plus maintainer PR/release assessment; human semantic review is required.
- **Current status:** partial. Product traceability exists; the extended completion and release chain has not been demonstrated.
- **Current evidence:** [docs/product/requirements.md](../../docs/product/requirements.md), [docs/product/backlog.md](../../docs/product/backlog.md), [CONTRIBUTING.md](../../CONTRIBUTING.md)
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-019](../../docs/product/backlog.md#b-019)

<a id="gov-002"></a>
### GOV-002 — Branch and review controls

Main and supported release branches MUST require PR-based changes and passing CI, prohibit force-push, restrict bypass and retain maintainer assessment; independent review SHALL become required when another qualified maintainer exists.

- **Rationale:** A workflow file cannot enforce repository access controls.
- **First applicable gate:** E1 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** SSDF, OSPS, Scorecard.
- **Required evidence:** Dated remote ruleset/branch protection and access assessment, required checks, and example final PR.
- **Enforcement:** Remote rulesets and periodic maintainer inspection; CODEOWNERS routes ownership without fake self-review.
- **Current status:** partial. Main protection was inspected on 2026-10-02: required verify, admin enforcement, no force-push/deletion and zero required approvals. Final PR assessment and future release-branch settings remain unevidenced.
- **Current evidence:** [.github/CODEOWNERS](../../.github/CODEOWNERS), [.github/workflows/ci.yml](../../.github/workflows/ci.yml), [CONTRIBUTING.md](../../CONTRIBUTING.md), [docs/engineering/ASSESSMENT.md](../../docs/engineering/ASSESSMENT.md)
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-019](../../docs/product/backlog.md#b-019)

<a id="arc-001"></a>
### ARC-001 — Architecture decision governance

Material technical choices MUST record context, alternatives, consequences, security, performance, operations, migration, reversibility and status in ADRs; product choices SHALL retain D-* records.

- **Rationale:** Technical acceptance must remain distinguishable from product scope.
- **First applicable gate:** E1 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010, SSDF.
- **Required evidence:** ADRs linked to accepted decisions/source and explicit unresolved consistency boundaries.
- **Enforcement:** Maintainer design review using ADR template; register validates referenced files.
- **Current status:** satisfied. Satisfied for the current bounded architecture record; future architecture changes require new evidence.
- **Current evidence:** [docs/architecture/README.md](../../docs/architecture/README.md), [docs/architecture/ADR-101-task-authority.md](../../docs/architecture/ADR-101-task-authority.md), [docs/product/decisions.md](../../docs/product/decisions.md)
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-019](../../docs/product/backlog.md#b-019), [docs/product/backlog.md#b-002](../../docs/product/backlog.md#b-002), [docs/product/decisions.md#d-016](../../docs/product/decisions.md#d-016), [docs/product/decisions.md#d-017](../../docs/product/decisions.md#d-017), [docs/product/decisions.md#d-018](../../docs/product/decisions.md#d-018)

<a id="test-001"></a>
### TEST-001 — Shared verification baseline

CI MUST execute the shared full unit/integration matrix, formatting and lint checks; unavailable dependencies or tools SHALL fail rather than report skipped checks as success.

- **Rationale:** Local and CI evidence must have the same scope.
- **First applicable gate:** E1 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010, SSDF, OSPS.
- **Required evidence:** Final-commit full verification, Rust fmt/Clippy and Go format/vet output against real services.
- **Enforcement:** scripts/verify.py and ci.yml; CI status required by GOV-002.
- **Current status:** partial. Source verification records are linked by immutable revision in the assessment; each candidate still requires its own final-source CI and qualification assessment.
- **Current evidence:** [scripts/verify.py](../../scripts/verify.py), [.github/workflows/ci.yml](../../.github/workflows/ci.yml), [docs/TESTING.md](../../docs/TESTING.md), [docs/engineering/ASSESSMENT.md](../../docs/engineering/ASSESSMENT.md)
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-019](../../docs/product/backlog.md#b-019)

<a id="doc-001"></a>
### DOC-001 — Development and limitations

The repository MUST document reproducible development prerequisites, commands, component status, evidence boundaries, contribution rules and private vulnerability reporting.

- **Rationale:** Users must be able to reproduce bounded results without confusing a prototype with supported deployment.
- **First applicable gate:** E1 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010, OSPS.
- **Required evidence:** Setup/testing guides, implementation inventory, contribution/security policies and links.
- **Enforcement:** Maintainer documentation review and setup regression tests.
- **Current status:** satisfied. Development documentation exists; this requirement does not claim supported installation or production security.
- **Current evidence:** [README.md](../../README.md), [docs/TESTING.md](../../docs/TESTING.md), [docs/IMPLEMENTATION.md](../../docs/IMPLEMENTATION.md), [CONTRIBUTING.md](../../CONTRIBUTING.md), [SECURITY.md](../../SECURITY.md), [scripts/setup_env.py](../../scripts/setup_env.py)
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-019](../../docs/product/backlog.md#b-019)

<a id="func-001"></a>
### FUNC-001 — Usable accepted product slice

Alpha MUST deliver a coherent declared product slice through its promised interfaces, including access control, reload, conflict recovery and accepted domain rules.

- **Rationale:** Pure rules alone are not a usable product.
- **First applicable gate:** E2 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010.
- **Required evidence:** Acceptance-to-interface tests and maintainer scope decision, without silently reducing B-018 scope.
- **Enforcement:** Product acceptance and release assessment.
- **Current status:** partial. Typed pure contracts exist; configurable storage/API/UI integration is pending.
- **Current evidence:** [docs/product/B-003-contract.md](../../docs/product/B-003-contract.md), [src/pure/task_contract/tests/item_mutation.rs](../../src/pure/task_contract/tests/item_mutation.rs)
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-004](../../docs/product/backlog.md#b-004), [docs/product/backlog.md#b-005](../../docs/product/backlog.md#b-005), [docs/product/backlog.md#b-006](../../docs/product/backlog.md#b-006), [docs/product/backlog.md#b-007](../../docs/product/backlog.md#b-007), [docs/product/backlog.md#b-018](../../docs/product/backlog.md#b-018), [docs/product/decisions.md#d-019](../../docs/product/decisions.md#d-019), [docs/product/decisions.md#d-024](../../docs/product/decisions.md#d-024)

<a id="sec-001"></a>
### SEC-001 — Threat modeling

Trust boundaries and abuse cases MUST have a reviewed threat model with mitigations, residual risks and test destinations, updated on boundary changes.

- **Rationale:** Security decisions need explicit adversaries and assets.
- **First applicable gate:** E2 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** SSDF, ASVS.
- **Required evidence:** Reviewed model and tests for authentication, authorization, replay, event and resource abuse.
- **Enforcement:** Security-impact PR review and Alpha assessment.
- **Current status:** partial. Initial current/planned boundary model exists; mitigations and formal assessment remain incomplete.
- **Current evidence:** [docs/engineering/THREAT-MODEL.md](../../docs/engineering/THREAT-MODEL.md)
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-020](../../docs/product/backlog.md#b-020)

<a id="sec-002"></a>
### SEC-002 — Identity and authorization

Every exposed read, mutation, configuration and replay path MUST authenticate and enforce current project/object grants; local owner onboarding, session expiry/recovery and browser CSRF protections SHALL be tested.

- **Rationale:** Loopback and origin checks do not establish identity or prevent IDOR.
- **First applicable gate:** E2 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ASVS, SSDF.
- **Required evidence:** Allow/deny, IDOR, cross-project, revoked-grant replay, session and CLI/browser negative tests.
- **Enforcement:** Authoritative service boundaries and security integration suite.
- **Current status:** partial. Origin checks and pure grant rules exist; no integrated authentication or authorization baseline.
- **Current evidence:** [go/io/task_api/server.go](../../go/io/task_api/server.go), [src/pure/task_contract/tests/operation.rs](../../src/pure/task_contract/tests/operation.rs)
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-007](../../docs/product/backlog.md#b-007), [docs/product/backlog.md#b-020](../../docs/product/backlog.md#b-020), [docs/product/decisions.md#d-017](../../docs/product/decisions.md#d-017), [docs/product/decisions.md#d-033](../../docs/product/decisions.md#d-033)

<a id="sec-003"></a>
### SEC-003 — Alpha security verification

Alpha MUST assess all applicable ASVS 5.0.0 Level 1 controls and OSPS 2025-02-25 Level 1 controls, retaining evidence and justified applicability decisions. An aggregate assessment score MUST NOT replace per-control evidence.

- **Rationale:** A targeted baseline requires complete scoped assessment rather than a badge.
- **First applicable gate:** E2 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ASVS, OSPS.
- **Required evidence:** Versioned per-control matrices and closed mandatory gaps, including repository MFA/access and private reporting availability.
- **Enforcement:** Alpha security qualification; incomplete matrices block release.
- **Current status:** planned. No full ASVS/OSPS assessment exists.
- **Current evidence:** None yet.
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-020](../../docs/product/backlog.md#b-020)

<a id="sec-004"></a>
### SEC-004 — Security analysis and scanning

SAST, direct/transitive dependency vulnerability scanning and secret scanning MUST run on changes and regularly on supported branches, with owned actionable findings and expiring exceptions.

- **Rationale:** New advisories affect unchanged code too.
- **First applicable gate:** E2 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** SSDF, OSPS, Scorecard.
- **Required evidence:** Pinned scanner configuration, real seeded detection regressions, scheduled output and triage records.
- **Enforcement:** Future security CI and vulnerability response; current compiler lints are not SAST coverage.
- **Current status:** planned. Scanner/tool/database choices and hosted feature access require qualification; tracked in B-020/B-021.
- **Current evidence:** None yet.
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-020](../../docs/product/backlog.md#b-020), [docs/product/backlog.md#b-021](../../docs/product/backlog.md#b-021)

<a id="sec-005"></a>
### SEC-005 — Production security assurance

Production MUST satisfy all applicable ASVS 5 Level 2 and OSPS Level 2 controls with versioned evidence, secure deployment settings and closed critical/high exploitable findings. An aggregate assessment score MUST NOT replace per-control evidence.

- **Rationale:** Broad adoption requires stronger verification than local development.
- **First applicable gate:** E4 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ASVS, OSPS, SSDF.
- **Required evidence:** Complete matrices, hardened install tests and remediated findings; any source-standard exceptions disclosed.
- **Enforcement:** Production security sign-off, never inferred from Alpha assessment.
- **Current status:** planned. Development credentials and disabled search authentication are explicitly unsuitable for production.
- **Current evidence:** None yet.
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-020](../../docs/product/backlog.md#b-020), [docs/product/backlog.md#b-021](../../docs/product/backlog.md#b-021)

<a id="dep-001"></a>
### DEP-001 — Dependency and license governance

Dependencies MUST follow the dependency policy for licenses, direct/transitive advisories, maintenance, updates and owned expiring exceptions.

- **Rationale:** A lockfile is inventory evidence, not security or license clearance.
- **First applicable gate:** E2 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** SSDF, OSPS.
- **Required evidence:** Dependency/license inventory, update/triage record, notices and exception reviews.
- **Enforcement:** Dependency PR review now; automated inventory/scanning qualification in B-021.
- **Current status:** partial. Policy and Rust lockfile exist; complete transitive/image license and advisory evidence is missing.
- **Current evidence:** [Cargo.lock](../../Cargo.lock), [docs/engineering/DEPENDENCIES.md](../../docs/engineering/DEPENDENCIES.md), [CONTRIBUTING.md](../../CONTRIBUTING.md)
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-021](../../docs/product/backlog.md#b-021)

<a id="api-001"></a>
### API-001 — Public contract and error governance

Public HTTP APIs MUST be OpenAPI contract-first with tested schemas, stable error codes/envelope, pagination/filter/sort rules, authorization, bounded requests, resource limits, concurrency conflicts, replay, versioning and deprecation.

- **Rationale:** Polyglot adapters must not independently reinterpret errors and retry semantics.
- **First applicable gate:** E2 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** OpenAPI, ASVS, ISO25010.
- **Required evidence:** Validated specification, cross-language positive/negative conformance and compatibility diff; documented endpoint applicability.
- **Enforcement:** Future contract validation and API integration tests; design review now.
- **Current status:** partial. Experimental errors include operation_id_reused/version_conflict; no supported OpenAPI contract exists.
- **Current evidence:** [go/io/task_api/server.go](../../go/io/task_api/server.go), [src/io/task_worker/src/main.rs](../../src/io/task_worker/src/main.rs), [packages/contracts/index.d.ts](../../packages/contracts/index.d.ts)
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-004](../../docs/product/backlog.md#b-004), [docs/product/backlog.md#b-022](../../docs/product/backlog.md#b-022), [docs/product/decisions.md#d-016](../../docs/product/decisions.md#d-016), [docs/product/decisions.md#d-032](../../docs/product/decisions.md#d-032)

<a id="api-002"></a>
### API-002 — Replay and conflict invariants

Same operation ID and intent MUST return the original accepted result within its retention contract; changed intent SHALL reject with operation_id_reused and stale unseen intent with version_conflict. Authentication/authorization MUST precede replay.

- **Rationale:** Unknown write outcomes must be safely reconciled without duplicate effects.
- **First applicable gate:** E2 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010, ASVS.
- **Required evidence:** Pure and actual-storage replay/race tests, scope/fingerprint/expiry/restore tests and API mappings.
- **Enforcement:** Domain replay logic, immutable storage uniqueness, service access checks and integration tests.
- **Current status:** partial. Bounded single-task ledger exists; scoped configurable replay/access/compaction/restore integration is pending.
- **Current evidence:** [src/io/task_worker/src/lib.rs](../../src/io/task_worker/src/lib.rs), [tests/integration/live_task_worker.py](../../tests/integration/live_task_worker.py), [src/pure/task_contract/tests/operation.rs](../../src/pure/task_contract/tests/operation.rs)
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-005](../../docs/product/backlog.md#b-005), [docs/product/backlog.md#b-007](../../docs/product/backlog.md#b-007), [docs/product/backlog.md#b-022](../../docs/product/backlog.md#b-022), [docs/product/decisions.md#d-016](../../docs/product/decisions.md#d-016), [docs/product/decisions.md#d-017](../../docs/product/decisions.md#d-017), [docs/product/decisions.md#d-032](../../docs/product/decisions.md#d-032), [docs/product/decisions.md#d-033](../../docs/product/decisions.md#d-033), [docs/product/decisions.md#d-034](../../docs/product/decisions.md#d-034)

<a id="data-001"></a>
### DATA-001 — Authoritative integrity and consistency

Accepted task versions MUST increase monotonically; mutation/version/replay acceptance SHALL be atomic for the promised scope and independent of search. Cross-record invariants MUST have a demonstrated commit/recovery protocol before exposure.

- **Rationale:** Single-record success cannot justify consistent configuration, allocation or accounting.
- **First applicable gate:** E2 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010.
- **Required evidence:** Storage uniqueness tests, contention/crash evidence and domain-to-enforcing-layer invariant table.
- **Enforcement:** Domain rules, database keys and service tests; unresolved Q-019 blocks affected routes.
- **Current status:** partial. Finite single-task race evidence is not linearizability proof or cross-document consistency.
- **Current evidence:** [src/io/task_worker/src/lib.rs](../../src/io/task_worker/src/lib.rs), [docs/product/B-003-contract.md](../../docs/product/B-003-contract.md), [tests/integration/live_task_worker.py](../../tests/integration/live_task_worker.py)
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-005](../../docs/product/backlog.md#b-005), [docs/product/backlog.md#b-009](../../docs/product/backlog.md#b-009), [docs/product/backlog.md#b-011](../../docs/product/backlog.md#b-011), [docs/product/backlog.md#b-012](../../docs/product/backlog.md#b-012), [docs/product/backlog.md#b-023](../../docs/product/backlog.md#b-023), [docs/product/decisions.md#d-016](../../docs/product/decisions.md#d-016), [docs/product/decisions.md#d-018](../../docs/product/decisions.md#d-018)

<a id="event-001"></a>
### EVENT-001 — Event and projection contracts

Events MUST carry ID/type/schema version, aggregate ID/version, timestamp and correlation ID, with causation ID where useful; consumers SHALL define duplicate, delayed, reordered, missing, future-schema and poison handling. Projections and clients MUST never regress authoritative versions.

- **Rationale:** Search freshness and authority are different contracts.
- **First applicable gate:** E3 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010, OTel.
- **Required evidence:** Versioned event schemas, producer/consumer tests, gap/rebuild and poison replay evidence.
- **Enforcement:** Schema/contract tests, consumer version gates, client merge rules and recovery tooling.
- **Current status:** partial. Minimum-version search check and bounded Python spike evidence exist; production CDC/indexer/event envelope is absent.
- **Current evidence:** [go/io/task_api/server.go](../../go/io/task_api/server.go), [packages/contracts/index.d.ts](../../packages/contracts/index.d.ts), [docs/spikes/SP-001-task-path.md](../../docs/spikes/SP-001-task-path.md)
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-005](../../docs/product/backlog.md#b-005), [docs/product/backlog.md#b-022](../../docs/product/backlog.md#b-022), [docs/product/backlog.md#b-023](../../docs/product/backlog.md#b-023), [docs/product/decisions.md#d-016](../../docs/product/decisions.md#d-016), [docs/product/decisions.md#d-017](../../docs/product/decisions.md#d-017)

<a id="persist-001"></a>
### PERSIST-001 — Durability and supported storage

Supported installation MUST retain acknowledged writes, configuration and replay state across process crash, container recreation and ordinary machine restart, with verified storage mapping/index/CDC readiness and separate setup privileges.

- **Rationale:** Disposable Compose and DB acknowledgment do not establish restart durability.
- **First applicable gate:** E2 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010.
- **Required evidence:** Actual supported-runtime termination/restart and durable-volume tests with independent reads.
- **Enforcement:** Provisioning/readiness plus supported-runtime qualification.
- **Current status:** partial. Bounded worker/spike evidence exists; main Compose has ephemeral data and no supported durability qualification.
- **Current evidence:** [docs/spikes/SP-001-task-path.md](../../docs/spikes/SP-001-task-path.md), [src/io/task_worker/src/lib.rs](../../src/io/task_worker/src/lib.rs)
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-005](../../docs/product/backlog.md#b-005), [docs/product/backlog.md#b-024](../../docs/product/backlog.md#b-024), [docs/product/decisions.md#d-016](../../docs/product/decisions.md#d-016), [docs/product/decisions.md#d-017](../../docs/product/decisions.md#d-017)

<a id="rec-001"></a>
### REC-001 — Backup and restore baseline

Supported real-data use MUST have automated daily backup with visible age/failure, an off-machine/failure-domain copy, clean-instance restore of authoritative/configuration/replay state, search reconstruction and stale-client/retry handling.

- **Rationale:** Backup execution alone is not recoverability.
- **First applicable gate:** E2 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010, SSDF.
- **Required evidence:** Restore exercise, backup failure/sleep scenarios, replay/tombstone and concurrent rebuild checks.
- **Enforcement:** Backup monitoring and restore qualification; no real-data readiness until demonstrated.
- **Current status:** planned. No supported backup or clean-instance restore exists.
- **Current evidence:** None yet.
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-005](../../docs/product/backlog.md#b-005), [docs/product/backlog.md#b-024](../../docs/product/backlog.md#b-024), [docs/product/decisions.md#d-017](../../docs/product/decisions.md#d-017), [docs/product/decisions.md#d-034](../../docs/product/decisions.md#d-034)

<a id="rec-002"></a>
### REC-002 — Recovery objectives

RPO/RTO MUST be explicitly decided for each supported failure class, with recovery procedures and measurable successful restore criteria.

- **Rationale:** A daily backup cannot justify an unconditional 24-hour RPO.
- **First applicable gate:** E3 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010.
- **Required evidence:** Approved objective record and timed exercises covering source data, replay state and search recovery.
- **Enforcement:** Recovery qualification and Q-020 decision; no invented numerical targets.
- **Current status:** planned. Exact loss/time objectives and backup destination remain open.
- **Current evidence:** None yet.
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-024](../../docs/product/backlog.md#b-024), [docs/product/backlog.md#b-018](../../docs/product/backlog.md#b-018)

<a id="rec-003"></a>
### REC-003 — Disaster recovery qualification

Production MUST demonstrate approved RPO/RTO under loss of the supported host/storage failure domain and verify source integrity, replay safety and rebuilt projections.

- **Rationale:** An in-place restore cannot establish disaster recovery.
- **First applicable gate:** E4 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010, SSDF.
- **Required evidence:** Timed independent-environment recovery, integrity comparison and operator evidence.
- **Enforcement:** Production release and recurring recovery campaigns.
- **Current status:** planned. Machine-loss recovery and tested objectives remain future work.
- **Current evidence:** None yet.
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-024](../../docs/product/backlog.md#b-024)

<a id="test-002"></a>
### TEST-002 — Property and fuzz coverage

High-risk parsers and mutation contracts MUST have property/fuzz coverage for valid/invalid Unicode and escaping, size/complexity boundaries, replay intent, stale versions and nondecreasing projection versions.

- **Rationale:** Example cases leave large input/state spaces unexplored.
- **First applicable gate:** E2 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010, SSDF.
- **Required evidence:** Seeded bounded PR properties, retained regression corpus and separate time-bounded Rust fuzz campaigns.
- **Enforcement:** Deterministic CI properties and scheduled qualification; fuzz discovery must become regression tests.
- **Current status:** partial. Example/boundary tests exist; systematic properties/fuzzing and parser complexity bounds are incomplete.
- **Current evidence:** [src/pure/yaja_query/src/lib.rs](../../src/pure/yaja_query/src/lib.rs), [src/pure/task_contract/tests/operation.rs](../../src/pure/task_contract/tests/operation.rs)
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-023](../../docs/product/backlog.md#b-023)

<a id="test-003"></a>
### TEST-003 — Concurrency and fault qualification

Critical invariants MUST be exercised by deterministic concurrency, sustained contention, state-machine/model tests where appropriate and hypothesis-driven dependency/crash fault injection.

- **Rationale:** Finite race tests are evidence, not proof; random chaos has no acceptance oracle.
- **First applicable gate:** E3 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010.
- **Required evidence:** Campaign parameters, interruption barriers, invariant oracles and failures across the QUALIFICATION matrix.
- **Enforcement:** Real-service release campaigns and bounded CI regressions.
- **Current status:** partial. Finite races/stalls and prototype failure cases exist; full production path and sustained/model qualification are pending.
- **Current evidence:** [tests/integration/live_task_worker.py](../../tests/integration/live_task_worker.py), [docs/TEST_STRATEGY.md](../../docs/TEST_STRATEGY.md), [docs/spikes/SP-001-task-path.md](../../docs/spikes/SP-001-task-path.md)
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-023](../../docs/product/backlog.md#b-023), [docs/product/backlog.md#b-024](../../docs/product/backlog.md#b-024)

<a id="test-004"></a>
### TEST-004 — Critical user-path end-to-end evidence

Beta MUST exercise critical supported user paths through client, API, worker, persistence, events and projection, including denied access, uncertain outcomes and reload.

- **Rationale:** Component probes cannot establish the integrated user experience.
- **First applicable gate:** E3 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010.
- **Required evidence:** Selective deterministic actual-stack E2E with bounded waits and final-state oracles.
- **Enforcement:** Future system test suite and release qualification.
- **Current status:** planned. No application UI or complete production event path exists.
- **Current evidence:** None yet.
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-006](../../docs/product/backlog.md#b-006), [docs/product/backlog.md#b-018](../../docs/product/backlog.md#b-018), [docs/product/backlog.md#b-023](../../docs/product/backlog.md#b-023)

<a id="perf-001"></a>
### PERF-001 — Performance baseline

Alpha MUST measure p50/p95/p99 for mutation, authoritative read and available search/propagation operations with dataset, concurrency, hardware, versions and resource usage recorded.

- **Rationale:** Timeout constants are not measured latency or scale.
- **First applicable gate:** E2 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010.
- **Required evidence:** Repeatable benchmark output including CPU, memory, storage growth and startup.
- **Enforcement:** Benchmark qualification separate from noisy PR latency assertions.
- **Current status:** planned. SP-001 resource observations are bounded prototype results, not application performance qualification.
- **Current evidence:** None yet.
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-025](../../docs/product/backlog.md#b-025)

<a id="perf-002"></a>
### PERF-002 — Operating envelope and scalability

Beta MUST publish a measured operating envelope for supported tasks/projects, fields, clients and throughput, including saturation, CDC propagation, index rebuild, startup, disk growth, CPU and memory; Production SHALL qualify the supported envelope.

- **Rationale:** An architectural ambition is not enterprise-scale evidence.
- **First applicable gate:** E3 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010.
- **Required evidence:** Workload distributions, p50/p95/p99, errors, saturation limits and repeatable volume/rebuild results.
- **Enforcement:** Beta/Production benchmark campaigns and release scope decisions in Q-022.
- **Current status:** planned. No measured Go/Rust operating envelope or approved capacity targets.
- **Current evidence:** None yet.
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-025](../../docs/product/backlog.md#b-025), [docs/product/backlog.md#b-018](../../docs/product/backlog.md#b-018)

<a id="limit-001"></a>
### LIMIT-001 — Explicit resource bounds

User/workload-controlled request/query complexity, filters, fields, task/event size, search results, concurrency and queue growth MUST have enforced bounds and rejection/backpressure contracts; future attachments SHALL be bounded before exposure.

- **Rationale:** Unbounded dimensions invite resource exhaustion and hidden growth.
- **First applicable gate:** E2 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ASVS, ISO25010.
- **Required evidence:** Limit inventory, maximum/over-limit tests and reviewed justification for any unbounded dimension.
- **Enforcement:** Domain limits, adapter admission and queue retention policies.
- **Current status:** partial. Worker and logical model have bounds; full API/query/queue/event limits and retained-history policy remain incomplete.
- **Current evidence:** [src/io/task_worker/src/main.rs](../../src/io/task_worker/src/main.rs), [src/pure/task_contract/tests/payload_limits.rs](../../src/pure/task_contract/tests/payload_limits.rs), [go/io/task_api/server.go](../../go/io/task_api/server.go)
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-004](../../docs/product/backlog.md#b-004), [docs/product/backlog.md#b-005](../../docs/product/backlog.md#b-005), [docs/product/backlog.md#b-022](../../docs/product/backlog.md#b-022), [docs/product/backlog.md#b-025](../../docs/product/backlog.md#b-025), [docs/product/decisions.md#d-017](../../docs/product/decisions.md#d-017), [docs/product/decisions.md#d-021](../../docs/product/decisions.md#d-021), [docs/product/decisions.md#d-023](../../docs/product/decisions.md#d-023), [docs/product/decisions.md#d-030](../../docs/product/decisions.md#d-030)

<a id="obs-001"></a>
### OBS-001 — OpenTelemetry-compatible signals

Services MUST expose structured logs, metrics and propagated trace context, including request rate/duration/errors/active requests, DB pools, mutation failures/retries, CDC lag, event backlog, projection lag, indexing failures and poison/dead-letter counts where applicable.

- **Rationale:** Projection lag must be diagnosable separately from durability.
- **First applicable gate:** E3 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** OTel, ISO25010.
- **Required evidence:** Telemetry schema, exporter/instrumentation tests, context propagation and bounded-cardinality/redaction checks.
- **Enforcement:** Instrumentation tests and operational dashboards/alerts; avoid IDs as metric labels.
- **Current status:** planned. No integrated OTel-compatible model exists.
- **Current evidence:** None yet.
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-026](../../docs/product/backlog.md#b-026)

<a id="rel-001"></a>
### REL-001 — Measurable reliability objectives

Supported releases MUST define measurable SLIs/SLOs for API availability, mutation durability, authoritative-read/search latency, search freshness and recovery success, including windows, populations and exclusions.

- **Rationale:** Objectives require reliable measurements and failure attribution.
- **First applicable gate:** E3 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010, OTel.
- **Required evidence:** Approved SLI formulas/targets and telemetry queries; Production must retain achieved-window evidence.
- **Enforcement:** SLO assessment, alert/runbook review and Q-020/Q-022 decisions; targets remain undecided.
- **Current status:** planned. Neither final numerical targets nor instrumented SLO evidence exists.
- **Current evidence:** None yet.
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-026](../../docs/product/backlog.md#b-026)

<a id="rel-002"></a>
### REL-002 — Production reliability acceptance

Production MUST demonstrate its approved SLOs over the declared qualification window and close critical integrity architecture gaps for shipped capabilities.

- **Rationale:** Defining an objective does not demonstrate it is achieved.
- **First applicable gate:** E4 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010.
- **Required evidence:** Actual SLO reports, integrity disposition and operational qualification for the support matrix.
- **Enforcement:** Production qualification review; unresolved Q-019 cannot be hidden by passing single-task tests.
- **Current status:** planned. No supported-window SLO results; cross-document protocol remains unresolved.
- **Current evidence:** None yet.
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-023](../../docs/product/backlog.md#b-023), [docs/product/backlog.md#b-026](../../docs/product/backlog.md#b-026), [docs/product/backlog.md#b-018](../../docs/product/backlog.md#b-018)

<a id="ops-001"></a>
### OPS-001 — Configuration and health

Supported configuration MUST have a schema, safe defaults, precedence, secret separation, startup validation, incompatible-combination rejection and compatibility rules; liveness, readiness and degraded capabilities SHALL be distinct.

- **Rationale:** A live process may be unable to serve authoritative operations.
- **First applicable gate:** E2 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010, ASVS.
- **Required evidence:** Invalid/startup configuration and dependency-failure tests with documented capability responses.
- **Enforcement:** Startup/service validation and dependency matrix in QUALIFICATION; future config validation CLI requires design.
- **Current status:** partial. Development bounds and index health exist; supported config schema and complete health model do not.
- **Current evidence:** [src/io/task_worker/src/main.rs](../../src/io/task_worker/src/main.rs), [scripts/healthcheck.py](../../scripts/healthcheck.py)
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-005](../../docs/product/backlog.md#b-005), [docs/product/backlog.md#b-027](../../docs/product/backlog.md#b-027)

<a id="ops-002"></a>
### OPS-002 — Operational runbooks

Supported releases MUST provide tested install, upgrade, backup, restore, health, database failure, search rebuild, CDC backlog, poison recovery, disk-full, secret-rotation and crash-recovery runbooks.

- **Rationale:** Operators need a recoverable workflow rather than undocumented repair commands.
- **First applicable gate:** E3 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010, SSDF.
- **Required evidence:** Runbooks executed by an operator with prerequisites, verification, rollback and escalation.
- **Enforcement:** Operational qualification using runbook template; placeholders are not completion.
- **Current status:** planned. Development setup guide exists; supported operational runbooks are pending.
- **Current evidence:** None yet.
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-024](../../docs/product/backlog.md#b-024), [docs/product/backlog.md#b-027](../../docs/product/backlog.md#b-027)

<a id="mig-001"></a>
### MIG-001 — Storage upgrades and migrations

Migrations MUST be explicitly versioned and deterministic, test every supported previous-version upgrade at realistic volume, expose progress/failure, document recovery and require backup before destructive changes.

- **Rationale:** Schema changes must preserve history and operation state.
- **First applicable gate:** E3 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010.
- **Required evidence:** Version fixtures, interruption/resume or rollback evidence and supported-path matrix.
- **Enforcement:** Future migration suite and release qualification; no ad-hoc framework is mandated now.
- **Current status:** planned. Index installation is not a supported migration system.
- **Current evidence:** None yet.
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-005](../../docs/product/backlog.md#b-005), [docs/product/backlog.md#b-024](../../docs/product/backlog.md#b-024), [docs/product/backlog.md#b-027](../../docs/product/backlog.md#b-027)

<a id="compat-001"></a>
### COMPAT-001 — Beta compatibility policy

Beta MUST declare compatibility/version/deprecation rules for HTTP API, events, configuration, storage, CLI and public contracts, with supported upgrade paths; 0.x breaking changes SHALL be documented.

- **Rationale:** Clients and stored data cannot safely upgrade on implicit assumptions.
- **First applicable gate:** E3 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** SemVer, OpenAPI.
- **Required evidence:** Compatibility matrix, schema diffs, old-client/event tests and upgrade fixtures.
- **Enforcement:** Contract/release review and migration tests.
- **Current status:** planned. No supported API or release compatibility contract exists.
- **Current evidence:** None yet.
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-022](../../docs/product/backlog.md#b-022), [docs/product/backlog.md#b-028](../../docs/product/backlog.md#b-028)

<a id="compat-002"></a>
### COMPAT-002 — 1.0 compatibility commitment

From 1.0, supported public surfaces MUST obey SemVer: incompatible changes require a major release; deprecations SHALL state replacement and removal version/window, and supported upgrades MUST preserve promised data/replay semantics.

- **Rationale:** Version numbers must carry a concrete supported behavior contract.
- **First applicable gate:** E4 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** SemVer.
- **Required evidence:** Published support matrix, changelog and deprecation policy with tested old/new paths.
- **Enforcement:** Release compatibility assessment and artifact qualification.
- **Current status:** planned. Workspace 0.1.0 is not a supported compatibility commitment.
- **Current evidence:** None yet.
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-028](../../docs/product/backlog.md#b-028)

<a id="supply-001"></a>
### SUPPLY-001 — Controlled build inputs

Release builds MUST bind an immutable source commit to pinned toolchains, lockfiles, digest-pinned images/actions and controlled fetched dependencies; OCI packaging SHALL apply where containers are distributed.

- **Rationale:** Mutable stable tags/version tags are insufficient release input control.
- **First applicable gate:** E3 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** SSDF, SLSA, OCI, Scorecard.
- **Required evidence:** Build input inventory, resolved digests and clean hosted build record.
- **Enforcement:** Future release pipeline and dependency pin/update review.
- **Current status:** partial. Rust lockfile/version-tagged development images exist; stable toolchain and action tags remain mutable.
- **Current evidence:** [Cargo.lock](../../Cargo.lock), [docker-compose.yml](../../docker-compose.yml), [.github/workflows/ci.yml](../../.github/workflows/ci.yml)
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-021](../../docs/product/backlog.md#b-021), [docs/product/backlog.md#b-028](../../docs/product/backlog.md#b-028)

<a id="supply-002"></a>
### SUPPLY-002 — SBOM and signed provenance

Beta releases MUST include per-artifact SPDX or CycloneDX SBOM, checksums and hosted signed build provenance verifiably bound to artifact digests and source, with an assessed SLSA 1.2 Build L2 builder and consumer verification instructions.

- **Rationale:** Provenance must be authenticated and checked against expected source/build identity.
- **First applicable gate:** E3 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** SLSA, SPDX, CycloneDX, SSDF.
- **Required evidence:** Artifact-level inventories, signatures, verification tests and Build L2 control assessment.
- **Enforcement:** Future hosted build/release pipeline; provenance file alone is not SLSA attainment.
- **Current status:** planned. No release SBOM, signed provenance or builder assessment.
- **Current evidence:** None yet.
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-028](../../docs/product/backlog.md#b-028)

<a id="supply-003"></a>
### SUPPLY-003 — Production artifact authenticity

Production artifacts or a digest manifest covering every artifact MUST be signed, with checksums, SBOM, provenance and documented trusted identity/key verification and rotation/revocation procedures.

- **Rationale:** Signed provenance and release-artifact authenticity are separate requirements.
- **First applicable gate:** E4 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** SLSA, OSPS, SSDF.
- **Required evidence:** Consumer signature verification, tampered-artifact negative tests and signing incident procedure.
- **Enforcement:** Publication gate after verification → security checks → build → tests → SBOM → provenance → signing → packaging.
- **Current status:** planned. No supported release signing/publication pipeline.
- **Current evidence:** None yet.
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-028](../../docs/product/backlog.md#b-028)

<a id="supply-004"></a>
### SUPPLY-004 — Continuous project security assessment

The project SHOULD run OpenSSF Scorecard continuously where hosted access permits, retain per-check findings and triage trends.

- **Rationale:** Repository controls can drift independently of source tests.
- **First applicable gate:** E3 (cumulative thereafter).
- **Obligation:** recommended.
- **Sources:** Scorecard, OSPS.
- **Required evidence:** Scheduled assessment and owned findings or a documented feasibility rationale with review date.
- **Enforcement:** Future read-only scheduled assessment, kept separate from deterministic PR checks.
- **Current status:** planned. Hosted permissions/publication prerequisites have not been qualified.
- **Current evidence:** None yet.
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-021](../../docs/product/backlog.md#b-021)

<a id="inc-001"></a>
### INC-001 — Security incident response

Supported releases MUST follow SECURITY.md response targets and a private-report → triage/severity → remediation → advisory/CVE or GHSA where applicable → patched release → coordinated disclosure → postmortem process.

- **Rationale:** Release users need a predictable remediation and learning path.
- **First applicable gate:** E4 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** SSDF, OSPS.
- **Required evidence:** Enabled private reporting, incident exercise, advisory/patch workflow and support contacts.
- **Enforcement:** Maintainer incident procedure and Production exercise; existing targets remain unchanged.
- **Current status:** partial. Reporting/response policy exists; supported-release response and exercise are future work.
- **Current evidence:** [SECURITY.md](../../SECURITY.md)
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-020](../../docs/product/backlog.md#b-020), [docs/product/backlog.md#b-028](../../docs/product/backlog.md#b-028)

<a id="access-001"></a>
### ACCESS-001 — Web accessibility

The supported web UI MUST meet WCAG 2.2 AA, including keyboard operation, semantic HTML, screen-reader behavior, contrast, focus handling and accessible validation/errors.

- **Rationale:** Interaction capability includes access beyond pointer use.
- **First applicable gate:** E4 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** WCAG, ISO25010.
- **Required evidence:** Scoped automated accessibility checks plus manual keyboard/screen-reader evaluation across supported browsers.
- **Enforcement:** UI acceptance and Production qualification; automation cannot replace manual testing.
- **Current status:** planned. No real application UI exists; no browser tooling is introduced for an absent UI.
- **Current evidence:** None yet.
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-006](../../docs/product/backlog.md#b-006), [docs/product/backlog.md#b-018](../../docs/product/backlog.md#b-018), [docs/product/backlog.md#b-027](../../docs/product/backlog.md#b-027)

<a id="ent-001"></a>
### ENT-001 — Enterprise qualification

Enterprise claims MUST be scoped to approved customer requirements and evidenced identity (OIDC, justified SAML/SSO, MFA), RBAC/ABAC, audit, HA, horizontal scaling/multi-node recovery, stronger backup, isolation if multi-tenant, observability, hardening and external penetration/compliance assessments.

- **Rationale:** Enterprise readiness is a capability/evidence decision, not a marketing label.
- **First applicable gate:** E5 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010, ASVS, SSDF, SLSA.
- **Required evidence:** Scope/applicability decisions, independent security assessment and realistic enterprise failure/isolation/recovery qualification.
- **Enforcement:** Later enterprise initiative; consider SLSA Build L3 after builder threat-model review.
- **Current status:** planned. Candidate later scope only; multi-tenancy, SAML and certification are not implied requirements today.
- **Current evidence:** None yet.
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-029](../../docs/product/backlog.md#b-029)

<a id="code-001"></a>
### CODE-001 — Scoped coding conformance

New and materially changed code MUST apply the Coding Standard with scoped review/test evidence; existing untouched gaps SHALL remain recorded with implementation destinations, and narrow exceptions SHALL have owner, rationale, mitigation and expiry/review date.

- **Rationale:** Coding rules must protect domain invariants and execution ownership without forcing unrelated legacy rewrites or claiming repository-wide conformance.
- **First applicable gate:** E1 (cumulative thereafter).
- **Obligation:** required.
- **Sources:** ISO25010, SSDF.
- **Required evidence:** PR scope and applicable-rule assessment, relevant compiler/format/lint and behavioral results, and existing gap or exception dispositions with backlog references.
- **Enforcement:** Existing Rust/Go format/lint and unsafe checks, shared register validation, and maintainer assessment using the contribution policy, Definition of Done and PR template. Semantic conformance remains a review obligation.
- **Current status:** partial. The scoped coding baseline and existing checks are documented; conformance across future material changes needs actual PR evidence. Architecture-boundary, Python/TypeScript and minimum-Rust-version qualification remain incomplete; no full legacy audit is claimed.
- **Current evidence:** [docs/engineering/CODING-STANDARD.md](../../docs/engineering/CODING-STANDARD.md), [CONTRIBUTING.md](../../CONTRIBUTING.md), [docs/engineering/DEFINITION-OF-DONE.md](../../docs/engineering/DEFINITION-OF-DONE.md), [.github/pull_request_template.md](../../.github/pull_request_template.md), [.github/workflows/ci.yml](../../.github/workflows/ci.yml)
- **Implementation / backlog / decisions:** [docs/product/backlog.md#b-019](../../docs/product/backlog.md#b-019), [docs/product/backlog.md#b-022](../../docs/product/backlog.md#b-022), [docs/product/backlog.md#b-023](../../docs/product/backlog.md#b-023)
