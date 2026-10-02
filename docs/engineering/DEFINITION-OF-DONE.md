# Definition of Done

Applies to repository changes now; qualification obligations grow with shipped
scope. This is the canonical completion definition, used by the backlog and
[contribution policy](../../CONTRIBUTING.md).

Before merge, the author MUST record applicable criteria and evidence in the PR;
the maintainer MUST assess the final revision. Use explicit **not applicable —
reason** for criteria unrelated to the change. A checkbox alone is not evidence.

- Acceptance criteria and related R-/engineering IDs are satisfied; implementation
  works through the interfaces promised by the backlog item.
  A UI-only mockup, passing connection probe or written acceptance scenario alone
  is not implementation evidence for a complete feature.
- Relevant decisions/questions are resolved or explicitly bound outside this scope;
  material technical choices have an ADR.
- New/materially changed code meets the applicable [coding rules](CODING-STANDARD.md);
  the PR identifies scope, evidence, existing gaps and justified narrow exceptions.
- Unit, negative/boundary, integration and critical-path tests appropriate to the
  change pass. Real I/O claims have actual-service evidence.
- Security impact and changed trust boundaries are assessed; threats and scans
  have owned dispositions, without secrets in evidence.
- Performance/resource limits and concurrency/failure invariants are considered;
  high-risk changes have measured or qualification evidence.
- Observability makes new important failures and operational states diagnosable.
- Migration/restore and compatibility/deprecation effects are assessed and tested
  where supported state or interfaces change.
- Documentation, implementation status, contract/register entries and backlog are
  updated without broadening prior evidence claims.
  Scope changes remain proposals until reviewed; update affected requirements and
  backlog items after acceptance.
- Formatting/linting, shared verification, metadata validation and required CI
  pass on the final revision; environmental failures are recorded as failures.
- PR links tests/evidence and future gaps; release qualification links the source
  commit and artifacts when a release is involved.

Passing these criteria for a bounded component does not qualify an entire maturity
gate. Release records also need cumulative requirements and framework assessments.
No independent self-approval is required in the sole-contributor phase.
