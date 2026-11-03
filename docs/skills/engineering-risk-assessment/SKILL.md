---
name: engineering-risk-assessment
description: Assess material engineering and delivery risks for Kehila before dependent implementation, at changed boundaries, before qualification and at closeout. Use for scoped risk assessment; routine edits do not need another risk ceremony.
---

# Engineering risk assessment

Apply only to Kehila or an explicitly requested adaptation. Follow the repository's
accepted decisions, user authority and operating limits. Read
`docs/procedures/engineering-delivery.md` and its record fields from the repository
root; use the existing task/PR brief. If source is unavailable, name the gap rather
than claim these controls ran. The exact SOP text remains Proposed; accepted
manual controls apply without inventing semantic enforcement.

For material work, perform the existing independent framing Phase A before seeing
the candidate: inspect the original request, accepted contracts, source/environment
identities and contrary evidence. Preserve that framing, then challenge the
candidate in Phase B. When already exposed to the candidate, label the assessment
ordinary review; do not call it approach-blind. No extra per-commit reviewer is needed.

Prioritize concrete failure mechanisms and consequential unsupported premises,
including product integrity/security, ownership/cancellation, environment/tool
compatibility, harness validity, resources and delivery visibility when relevant.
Do not manufacture generic risks merely to fill categories.

For material risks record affected invariant/consequence, evidence and confidence,
likelihood (unknown when unsupported), impact and urgency separately, existing
controls, proposed mitigation or cheapest decisive observation and its cost,
residual risk, owner, acceptance authority and invalidation condition. Qualitative
rankings aid judgment; no numerical score accepts a severe risk or grants authority.

Output a bounded verdict: proceed, proceed on independent work only, or clarification
required for dependent work. Identify what the author should change/test first
and what nondelegated decision belongs to the maintainer. An agent cannot accept
architecture/security trade-offs, lift resource/trust constraints, waive checks,
merge, or create a new completion campaign. Distinguish a hypothesis, source finding
and executed observation. Rejected claims need sourced dispositions.

Refresh affected risks at material premise/boundary changes or contradictory
evidence, check controls/oracles before qualification, and identify residual owners
at closeout. Preserve valid evidence using explicit source/environment/fixture/oracle
conditions. Required hook/CI gates remain. At milestone/large-goal closeout use the
repository retrospective; do not claim savings or efficacy without observations.

This is an independently authored engineering application of the accepted
recommendations. Design reference: Anthropic's operational
[risk-assessment](https://github.com/anthropics/knowledge-work-plugins/blob/4fa3cb92e2942d6594200fa8d2c800708e086072/operations/skills/risk-assessment/SKILL.md).
No upstream plugin, scripts or source text are bundled; its installation or
effectiveness is not implied.
