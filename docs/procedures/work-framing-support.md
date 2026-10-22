# Evidence/relevance workflow: Codex feasibility and supporting guidance

Recorded: 2026-10-22. Status: supporting assessment for Proposed SOP-002 version 1.
This records feasibility and selected guidance, not new permissions, automated
semantic enforcement, standard adoption or a conformance assessment.
The [procedure](work-framing.md) remains the operational entry point.

## Requested outcome and boundary

The maintainer agreed to a visible work stage, a concise revision-linked decision
brief and separation of judgment from enforcement, subject to Codex feasibility,
and requested relevant standards. Preserve low cognitive load, approval before
prospective evaluation, existing authorization and small focused changes. Current
work produces documentation; native configuration and enforcement changes require
separate review. The exact SOP version remains Proposed.

## What can be used in Codex

| Practice | Evidence / status | Limit and consequence |
| --- | --- | --- |
| Brief Assessment / Implementation / Verification announcement | Observed: the working environment supports user-facing progress messages. | Manual convention; no custom native UI mode or automatic action restriction is claimed. |
| Revision-linked brief in the existing task/issue/PR | Observed: text records and Git revision inspection are available. | The author must reload and reconcile the record after resumption/context loss; persistence alone does not ensure it is read or correct. |
| Separate-context framing and final review | Observed: a separate reviewer can receive a scoped evidence packet and return findings. | Shared blind spots and incomplete packets remain. A read-only assignment is a behavioral instruction, not a separate sandbox. |
| Runtime sandbox, approvals and command rules | Product documentation describes these controls separately from task instructions. | Actual host policy applies. A process stage cannot change it; shell rules alone do not cover every connector/API path. |
| Hooks for selected pre-tool and session events | Documented option; no new hook is configured or qualified by this change. | Exact hook trust, event/tool coverage and failure behavior must be reviewed and tested on the intended Codex surface before depending on it. |

Official sources checked for this assessment:

- [Codex subagents](https://learn.chatgpt.com/docs/agent-configuration/subagents):
  children inherit parent sandbox/approval settings; custom instructions do not
  create a separate read-only sandbox. Do not import another product's permission
  behavior or assume a local CLI config controls the current application session.
- [Sandbox controls](https://learn.chatgpt.com/docs/sandboxing) and
  [rules](https://learn.chatgpt.com/docs/agent-configuration/rules): runtime
  restrictions and command matching complement the manual procedure.
- [Hooks](https://learn.chatgpt.com/docs/hooks): pre-tool and session/compaction
  events offer possible integration points. Non-managed definitions require trust;
  coverage has exceptions. Availability in documentation is not evidence that a
  hook is enabled, trusted or effective in a particular session.

The manual additions can operate with the current tools. There is no verified
new automatic gate that proves a premise, user intent or architectural acceptance.
If enforcement is proposed later, first specify the exact observable condition,
tool paths, source/revision binding, failure behavior and bypass limits. Missing
fields can be detected mechanically; justified meaning still requires assessment.

## Relevant standards and guidance

The adaptations below are project recommendations, not claims that each source
prescribes this workflow. Review used public issuer descriptions and freely
available guidance; full normative ISO/IEEE texts and clause-level conformance
were not assessed. No purchase or certification is needed for these additions.

| Source and type | Problem it addresses | Useful adaptation for this project |
| --- | --- | --- |
| [ISO/IEC/IEEE 29148:2018](https://standards.ieee.org/ieee/29148/6937/): requirements-engineering standard | Quality and lifecycle management of requirements, rather than accepting a plausible implementation as proof of the need. | Keep the requested outcome, source, assumptions and acceptance evidence connected to the reviewed revision. The issuer's public scope supports relevance; this is not a clause-by-clause implementation. |
| [ISO 9241-110:2020](https://www.iso.org/standard/75258.html): interaction-principles standard | Understandable interaction between a user and a system across technologies. | Make the current stage and requested decision understandable without SOP knowledge; show material facts briefly and make detail available. This is an application of its scope, not an AI-specific safety prescription. |
| [NIST AI RMF 1.0 Core](https://airc.nist.gov/airmf-resources/airmf/5-sec-core/): voluntary risk-management framework | Human/AI oversight responsibilities (GOVERN 3.2), context and oversight processes (MAP 1 and MAP 3.5). | Separate author, reviewer and maintainer authority; record uncertainty and limits. Its functions are not a mandated linear approval sequence or a certification of this process. |
| [NASA Systems Engineering Handbook, Appendix C](https://www.nasa.gov/reference/system-engineering-handbook-appendix/) and [section 6.8](https://www.nasa.gov/reference/6-8-decision-analysis/): engineering guidance | Checking assumptions before baselining requirements, and scaling decision analysis to consequences. | Ask whether a consequential assumption is correct; retain its rationale/source and unresolved work. Use a short record for small decisions rather than importing a large project bureaucracy. |

Use requirements engineering as the main conceptual basis, interaction principles
for presentation, and NIST for human/AI responsibility boundaries. NASA supplies
accessible checklists. Keep the existing brief and review record; do not create
parallel compliance forms or require a second approval for routine authorized work.

The selected practices are operational in the proposed procedure, not a separate
compliance checklist: [early framing](work-framing.md#1-early-framing-gate) separates
the need from the solution and gives unresolved consequential premises a resolver
and next action; [validation](work-framing.md#2-independent-relevance-validation)
requires direct inspection of decisive sources and distinguishes author, reviewer
and maintainer authority; [publication review](work-framing.md#3-revalidation-and-publication-gate)
assesses relevance separately from correctness. The existing
[user explanation](work-framing.md#explain-the-process-to-the-user) keeps decisions
brief and supporting detail available. The [brief](work-framing-template.md) and
[validator assignment](work-framing-validator.md) carry these practices into work.

Initial version approval authorizes use; it does not establish effectiveness.
After approval, observe the first three material tasks under [#71](https://github.com/MrQuality/kehila/issues/71)
and decide revisions from actual results. Standards and reviewer agreement do not
supply missing user intent or substitute for that decision.
