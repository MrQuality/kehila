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

## Operating path

Keep the record in the existing task, issue or PR:

1. Complete the [brief](work-framing-template.md) under the
   [early framing gate](#1-early-framing-gate).
2. Send the request and authoritative evidence for
   [Phase A](work-framing-validator.md#phase-a-request-and-authority-first);
   preserve the response before sending the candidate.
3. Send the candidate for [Phase B](work-framing-validator.md#phase-b-challenge-the-candidate)
   and resolve findings under the [validation rules](#2-independent-relevance-validation).
4. Apply the [revalidation and publication gate](#3-revalidation-and-publication-gate)
   at material changes and before publication.

The sections below define the evidence and review requirements for these steps.

## Explain the process to the user

Assume the user has not read this procedure. On first applying it, briefly explain:
we check that the work answers their intended problem, a separate reviewer checks
consequential assumptions, and we ask only for decisions the available evidence
cannot settle. The author and reviewer maintain the detailed records.

When user input is needed, present a few sentences or short bullets covering:

- The intended outcome and the specific unresolved choice, in plain language.
- What is established, what is uncertain and why the choice changes the work.
- The recommendation, the decision requested and what approving it would cover.

Keep supporting sources and detailed reasoning available through links or a
separate detail section. Do not require the user to read validator transcripts,
learn procedure labels or repeat an established decision. Keep material risks
and alternatives visible; brevity must not hide information needed to decide.
Explain again when the process or decision boundary changes, rather than repeating
the full introduction at every checkpoint.

## Visible stage and decision record

At meaningful transitions, state **Assessment**, **Implementation** or
**Verification**, with one sentence describing the current authorized work and
any decision needed. Assessment establishes the problem and prepares a proposal;
Implementation carries out agreed work; Verification checks the result against
requirements. These are communication labels, not Codex permission modes or new
approval gates. Do not repeat them on every tool call or routine update.

Use the [brief](work-framing-template.md) as the canonical field list for the
existing task, issue or PR, including its decision and revision record. Apply the
[revalidation rules](#3-revalidation-and-publication-gate) when meaning changes.

After resuming work or losing context, reload that brief, check it against the
current source and latest user direction, and reconcile material differences
before dependent work. Keep runtime permissions separate: a stage label, reviewer
verdict or decision record cannot grant tool access or lift a user restriction.
See [Codex feasibility and supporting guidance](work-framing-support.md) for the
observed/manual boundary and sources; no new automation is installed by this SOP.

## 1. Early framing gate

The author completes the [brief](work-framing-template.md) before dependent work.
State the need independently of the proposed solution. Distinguish confirmed
requirements from inferred requirements and proposed mechanisms. Use accepted
requirements, explicit user decisions and actual observations; an author's summary
or a Proposed document is not its own acceptance evidence.

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
Phase B challenges relevance, conflicts, unsupported premises and approval scope.

Do not supply the author's rationale as the sole evidence or hide contrary
requirements. Record what the validator actually received. A context already
exposed to the solution is an ordinary review, not an approach-blind run.

Within authorized access, the validator inspects decisive source material
directly, identifies missing or contrary evidence, and records which sources and
revisions it actually checked. If a decisive source cannot be inspected, record
the gap and leave dependent conclusions unresolved until sufficient authoritative
evidence is available. A closed-evidence rehearsal discloses that restriction;
it cannot settle missing requirements for real work.

Separate contexts reduce one source of anchoring; they do not prove independence,
eliminate shared model errors or give the reviewer architectural authority.

The author owns the proposal and evidence; the validator challenges and reports;
the maintainer decides unresolved product intent and architectural acceptance.
These roles do not create another approval for already authorized routine work.

Require evidence-backed statuses, consequences and a bounded verdict: **proceed**,
**proceed on independent work only**, or **clarification required for dependent
work**. The author resolves findings with sources or a user decision and records
the disposition. Do not ignore a material conflict because another reviewer
agrees with the author. If a separate context is unavailable, record the missing
check and escalate that checkpoint; do not label self-review independent.

## 3. Revalidation and publication gate

Revalidate when the question, premises or consequences change: investigation to
adoption, prototype to supported behavior, changed component/actor/data boundary,
or new contradictory evidence. Update the existing brief and review the delta.
A material change to scope or a premise reopens only the affected decision and
dependent work; routine changes within authorization do not require whole-plan
reapproval or a fresh full process for every commit.

Before publishing, the author and relevance validator compare the final result
against the validated question and changed premises. Assess relevance (does the
result still address the intended need?) separately from technical correctness
(does it meet the specified requirements?). Passing one does not establish the
other. Complete the [brief's publication fields](work-framing-template.md) in the
PR or link its existing record. Do not promote proposals into facts in summaries
or issue updates. Keep the maintainer's final assessment and acceptance separate.

## 4. Approval, use and evaluation

First review the proposed version for clarity, feasibility, consistency with
accepted requirements and a usable explanation for the user. The maintainer then
explicitly approves a specific version for use. Future effectiveness measurements
are not a prerequisite to this initial approval, and approval does not establish
effectiveness. Until that decision, exact codification remains Proposed; the
previously authorized controls and bounded pilot retain their recorded scope.

The initial [pilot record](../postmortems/2026-10-21-framing-validation-pilot.md)
is a retrospective rehearsal of the initiating failure, an unrelated assumption
and a sufficient-evidence control. For such rehearsals, fix expected decisions
before the run, withhold them and the candidate during Phase A, and retain actual
responses, deviations and dispositions. Do not tune a case and report its rerun as
the original result. These observations check execution of the proposed steps;
they cannot establish a detection rate or early prevention in future work.

After version approval, review its first three material tasks for missed premises,
unnecessary questions, user understanding, time to discovery and review effort.
The maintainer owns that follow-up and decides whether evidence warrants revision.
Keep the approved version and observations identifiable; material revisions return
to Proposed under the register's approval rules. No fixed percentage target is
invented, and evaluation must not become a condition for the initial approval.

| Control | Evidence / implementation |
| --- | --- |
| Early question/premise check | Manual brief and source/disposition record |
| Independent relevance challenge | Separate-context Phase A/B records, not a GitHub approving review |
| Boundary/publication check | Final-source relevance assessment and issue/PR consistency review |
| Process effectiveness | Unqualified; review the first three material tasks after version approval |
| Automated enforcement | Not implemented; presence/link checks cannot establish semantic truth |

Contributor, spike, ADR and PR entry points link this pilot. SOP-001's approved
spike rules and existing review/permission requirements still apply. No waiver,
automatic approval, universal agent invocation or broader architecture change is
introduced. Record any pilot exception with its scope, owner, reason and follow-up.
The procedure register retains Proposed status until explicit version acceptance.
