---
name: kehila-engineering-review
description: Apply Kehila's layered engineering review and evidence-validity rules to material changes and PRs. Use as the project overlay for code-review-and-quality; do not impose it on unrelated repositories.
---

# Kehila engineering review overlay

Read `CONTRIBUTING.md` and `docs/procedures/engineering-delivery.md` from the
repository root. Apply this project policy over conflicting generic guidance;
do not edit a shared skill to enforce a Kehila-only exception. If the shared
code-review skill is available, use its correctness, readability, architecture,
security and performance axes with this overlay. If not, disclose that and assess
those axes directly; do not pretend unavailable skills/tools ran. A required
specialist check remains unresolved until supplied or explicitly dispositioned.

Begin with original outcome and accepted contracts, then invariants/tests and
implementation. Tests cannot establish missing intent. Follow applicable intent/risk,
component, integration/harness and final exact-head layers; assign shared boundaries
and do not make each reviewer repeat a whole-repository scan.

Record base/head or tree, covered contracts/files, decisive source inspected,
actual execution versus source-only assessment, exclusions, findings by severity
and priority, dispositions, evidence and validity conditions. Review producer/consumer
and ownership/failure composition. Reuse prior review only after assessing the
intervening delta, shared-contract/caller/environment changes and invalidation
conditions. Final exact-head composition review is mandatory for the delivery;
matching hashes or component approvals alone are insufficient.

Apply the accepted contract-based verification practices in
`docs/procedures/engineering-delivery.md` from the repository root.
Trace changed contracts through producers, consumers, permissions and older tests.
Challenge acceptance/rejection oracles: keep unrelated inputs valid, establish
reachability where needed and reject passes caused by the wrong failure. Review
meaningful composition under actual relevant roles/storage/transport, not just
component success. For selected fault tests require observed activation, permitted
outcomes, independent state where relevant and bounded owned cleanup/recovery.
Use the existing delivery record for current source/evidence, remaining work,
blockers and live resource use. These are proportional manual assessments, not
automatic semantic test selection or a universal chaos campaign.

Keep commits focused and PRs coherent. Generic line/file thresholds are inspection
signals, not rejection/splitting rules. Large exceptions need coupling justification,
component map and separate treatment of adopted source, generated locks/notices,
tests and prose. Publish coherent checkpoints after applicable gates; document a
short deliberate hold if a diagnosed CI failure would otherwise just repeat.

Mutation experiments are selective, isolated, bounded and execution-authorized.
Never mutate the shared working tree or automatically run a suite to satisfy a
generic instruction. Investigate reachability/equivalence before reporting a
surviving mutant as a missing-test defect. Source inspection alone is not execution.

Remove newly introduced unused code within authorized scope after establishing
unused status; consider compatibility/API, migration and retained-evidence obligations
before older-code removal. Ask only for material unresolved intent/authority. Keep
optional cleanup separate from substantiated blocking defects. Do not make missing
optional skills, a model preference or a routine edit another permission ceremony.

Use only available authorized models/tools. Separate contexts and source evidence
matter; model diversity is optional and does not establish independence. Risk review
cannot accept product/security/architecture decisions. Required checks, confidentiality,
resource/trust constraints and user merge/status boundaries remain unchanged.

Before costly qualification review target engine/version/tool options, actual
ownership/permissions, protocol/error oracles, deadline/permit lifetime and owned
cleanup. Preserve failures, valid unaffected evidence and named affected reruns.
At milestone/large-goal closeout independently challenge the retrospective's
causal/time claims and follow-up; do not add a new verification campaign.
