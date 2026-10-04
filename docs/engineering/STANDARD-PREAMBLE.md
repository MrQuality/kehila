# Kehila Engineering Standard

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
selected sample. ISO quality characteristics guide Kehila-specific criteria rather
than a certification claim. Include safety risks when loss or misleading state
could affect downstream decisions; Kehila is not qualified for safety-critical use.

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

Kehila is **E0 — Experimental**. Source tests and the historical full-suite evidence
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
