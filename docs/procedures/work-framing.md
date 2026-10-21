# SOP-002: evidence and relevance checkpoints

| Field | Value |
| --- | --- |
| Version / status | 1 / Proposed codification; maintainer-authorized pilot in use |
| Owner | Project maintainer |
| Authorization | On 2026-10-21 the maintainer accepted the four controls below and directed implementation and a post-mortem. This authorizes this pilot, not approval of this exact SOP text. |
| Exact-version approval | Pending maintainer review; drafting, pilot results and PR merge do not infer it. |
| Enforcement | Manual task gates and reviewer records; no CI semantic validator or branch-protection change |
| First application / follow-up | [Platform-assumption post-mortem and pilot](../postmortems/2026-10-21-platform-assumption.md); [B-019 / #71](https://github.com/MrQuality/kehila/issues/71) |

## Purpose and applicability

Before substantial research, design or implementation, establish that the work
answers the requested problem under supported premises. Repeat at a material
change of meaning and before publication. This complements technical review;
passing tests or internally consistent prose cannot establish relevance.

A premise is consequential when a different answer would materially change scope,
architecture, cost/effort, supported behavior, persistence, security, deployment or
external commitments. Routine reversible implementation choices do not require
another approval or a second agent. Small work can use a few sentences in the
existing task/issue/PR; do not create a parallel decision registry.

## 1. Early framing gate

The author records the [framing brief](work-framing-template.md) before dependent
work: original requested outcome and authority, non-goals, consequential premises,
source and status for each, consequence if false, and proposed work boundary.
Use accepted requirements, explicit user decisions and actual observations;
an author's summary or a Proposed document is not its own acceptance evidence.

Distinguish **established**, **inferred**, **proposed**, **unresolved** and
**contradicted** premises. An established fact needs evidence; conflicting sources
need reconciliation. A new user direction can replace an earlier decision, but
record what changed and preserve historical evidence. User intent cannot be
resolved by an agent vote. A merged proposal is not acceptance of every premise.

Pause only work dependent on a material unsupported premise. Continue independently
justified work. Ask the smallest necessary clarification when authoritative
evidence cannot settle the consequential choice. Elapsed time, silence, broad
permission to continue, or a validator verdict is not an accepting decision.

## 2. Independent relevance validation

For material work in the pilot, use a separate validation agent/context and the
[two-stage prompt](work-framing-validator.md). First provide the original request
and authoritative evidence, with source identities and relevant omissions; ask
the validator to frame the problem before supplying the author's candidate.
Then provide the candidate and ask it to challenge relevance, conflicts,
unsupported premises and approval scope. Preserve the first response.

Do not supply the author's rationale as the sole evidence or hide contrary
requirements. Record what the validator actually received. A context already
exposed to the solution is an ordinary review, not an approach-blind run.
Separate contexts reduce one source of anchoring; they do not prove independence,
eliminate shared model errors or give the reviewer architectural authority.

Require evidence-backed statuses, consequences and a bounded verdict: **proceed**,
**proceed on independent work only**, or **clarification required for dependent
work**. The author resolves findings with sources or a user decision and records
the disposition. Do not ignore a material conflict because another reviewer
agrees with the author. If a separate context is unavailable, record the missing
check and escalate that checkpoint; do not label self-review independent.

## 3. Revalidation and publication gate

Revalidate when the question, premises or consequences change: investigation to
adoption, prototype to supported behavior, changed component/actor/data boundary,
or new contradictory evidence. Update the existing brief and review the delta;
unchanged work does not need a fresh full process for every commit.

Before publishing, the author and relevance validator compare the final result
against the validated question and changed premises. Record the reviewed revision,
remaining unresolved choices, actual evidence limits and the disposition of
material findings in the PR. Do not promote proposals into facts in summaries or
issue updates. Keep the maintainer's final assessment and acceptance separate.

## 4. Pilot, evidence and adoption

Run a retrospective replay of the initiating failure, an unrelated consequential
assumption and a sufficient-evidence control. Fix expected decisions before the
run; withhold them and the candidate during Phase A. Record actual responses,
deviations and dispositions, including false positives and missed assumptions.
Do not tune a case and report the rerun as the original result.

The initial [pilot record](../postmortems/2026-10-21-framing-validation-pilot.md)
reports only those bounded observations. It cannot establish a detection rate or
prove early prevention in future work. Review the first three subsequent material
tasks for missed premises, unnecessary questions, time to discovery and review
effort; the maintainer owns that follow-up. No fixed percentage target is invented.

| Control | Evidence / implementation |
| --- | --- |
| Early question/premise check | Manual brief and source/disposition record |
| Independent relevance challenge | Separate-context Phase A/B records, not a GitHub approving review |
| Boundary/publication check | Final-source relevance assessment and issue/PR consistency review |
| Process effectiveness | Initial controlled pilot; prospective observations remain open |
| Automated enforcement | Not implemented; presence/link checks cannot establish semantic truth |

Contributor, spike, ADR and PR entry points link this pilot. SOP-001's approved
spike rules and existing review/permission requirements still apply. No waiver,
automatic approval, universal agent invocation or broader architecture change is
introduced. Record any pilot exception with its scope, owner, reason and follow-up.
The procedure register retains Proposed status until explicit version acceptance.
