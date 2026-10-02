# Project procedures

This is the entry point for YAJA's standard operating procedures (SOPs) and
existing contributor guides. The register distinguishes approval of a procedure
from implementation of its automated checks. An entry here is not evidence that
GitHub enforces it.

## SOP register

| ID | Procedure | Version / status | Owner | Approval | Enforcement |
| --- | --- | --- | --- | --- | --- |
| SOP-001 | [Technical spikes](../spikes/README.md) | 1 / Approved | [Project maintainer](../../CONTRIBUTING.md#repository-roles) | Maintainer approval recorded 2026-09-25 | Manual use in effect; spike validator and runner preflight not implemented |

SOP IDs describe procedures; SP IDs describe individual spikes. Use a new SOP ID
for a new procedure and a new version for a revision of the same procedure.
Start future procedures from [TEMPLATE.md](TEMPLATE.md). Existing procedure files
can stay in their subject directories; link them here instead of duplicating them.

## Existing guides

These documents already define project rules. Listing them does not assign a new
approval or claim additional automation.

| Guide | Scope | Current verification described in the repository |
| --- | --- | --- |
| [Contributing](../../CONTRIBUTING.md) | Changes, testing, and merge policy | CI; maintainer inspection of scope, evidence, and limitations |
| [Testing](../TESTING.md) | Supported checks and their limits | Local verification scripts and CI workflow |
| [Test strategy](../TEST_STRATEGY.md) | Proposed quality goals, test levels, and future qualification | Planning guidance; not an implemented CI gate |
| [Security](../../SECURITY.md) | Development exposure and vulnerability reporting | Contributor and maintainer responsibility |
| [Naming and attribution](../BRANDING.md) | Project identity and attribution | Naming check plus manual review of matters the check cannot establish |
| [Product planning](../product/README.md) | Requirements, decisions, questions, and backlog | Stable IDs, linked records, and maintainer assessment |
| [Engineering standard](../engineering/ENGINEERING-STANDARD.md) | Cumulative maturity requirements and evidence gaps | Shared register validator; semantic release assessment remains manual |
| [Architecture records](../architecture/README.md) | Technical decisions, product boundary and ADR template | Maintainer design review; current accepted behavior restated with evidence limits |
| [Definition of Done](../engineering/DEFINITION-OF-DONE.md) | Applicable change completion criteria | PR evidence and maintainer assessment |
| [Coding standard](../engineering/CODING-STANDARD.md) | New/materially changed code, language conventions and existing gaps | Existing format/lint checks plus scoped PR assessment; broader checks remain planned |
| [Qualification and runbooks](../engineering/QUALIFICATION.md) | Future operational/release evidence and runbook template | Supported qualification remains backlog work |

## Proposed approval process for SOPs

The project maintainer approves a specific version. While there is only one
contributor, this does not depend on a separate PR reviewer. Approval must be an
explicit decision; drafting the procedure, running checks, or an assistant's
recommendation does not constitute the maintainer's approval.

Before seeking approval, make the version ready to use:

- Define the purpose, work it applies to, owner, required actions, and outputs.
- Provide the template or commands people need, with unimplemented parts marked.
- Map requirements to automated checks, human assessment, or a documented gap.
- Resolve contradictions with other guides and identify the first application.
- State whether a trial is needed, its limits, and what prompts a revision.

SOP-001 version 1 was approved by the project maintainer on 2026-09-25 for
initial use with manual checks. Its first application is SP-001/B-002. The
procedure and this register preserve that approval; automation gaps remain
visible until their checks are implemented and verified.

On explicit approval, record the version, approver, date, and a durable approval
note in the procedure, update this register, and include the record in the
published documentation change. A PR description or repository decision note can
record approval by the maintainer who authored the change. Record only decisions
actually made; do not manufacture a GitHub approving review.

Use Proposed, Approved, Superseded, or Retired for SOP status. A material revision
returns the new version to Proposed until approved; keep the previous approved
version identifiable through Git history. Record replacement links and the reason
for superseding or retiring a procedure. Explicit exceptions should name their
scope, rationale, approver, and expiry or follow-up.

## Enforcement plan

Approved SOPs apply to everyone doing work within their scope, including the
maintainer and automated contributors. Keep the required instructions in shared
project documentation. Contributor and automation entry points should link here.

For each SOP, list which requirements are automated and which need judgment.
Proposed shared checks should validate registration, links, versions, status,
approval metadata, and the existence of the procedure's stated checks. A spike
validator would additionally check record completeness appropriate to its status.
Passing structural checks cannot establish the truth of experimental conclusions.

The supported spike runner should execute preflight before the workload and
produce run evidence. These checks are planned, not implemented by this document.
Templates and local hooks help contributors, but shared enforcement requires the
corresponding CI checks to be required for merging on GitHub. Verify those settings
separately from the workflow files, and keep bypass permissions limited.

During the sole-contributor phase, retain passing CI and a recorded maintainer
check of the change and evidence; an independent approving review is optional.
Do not configure required approving reviews or required code-owner approval as a
merge condition in this phase. Reconsider that policy when an independent reviewer
is available. This register does not change remote repository settings.
