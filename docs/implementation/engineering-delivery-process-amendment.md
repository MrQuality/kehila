# PR #92 engineering process amendment

The maintainer accepted the delivery retrospective recommendations on 2026-11-03,
added implementation to PR #92 and required recurrence at every milestone or large
goal. This extends the earlier product checkpoint `10a6829`; its passing CI is
historical evidence, not qualification of a later amendment head. Current exact-head
review/check results and readiness are recorded in the PR.

## Implemented changes

| Accepted recommendation | Implementation / entry point | State and limits |
| --- | --- | --- |
| Publish coherent checkpoints | [Delivery rules](../procedures/engineering-delivery.md#scope-and-operating-record), contributor policy, checkpoint/publication field in existing task record | Manual rule in use; no arbitrary line/time quota or hook bypass. Adjacent commits may share a bounded push to avoid duplicate CI. |
| Check actual target environment early | [Environment/harness admission](../procedures/engineering-delivery.md#environment-and-harness-admission) and testing-guide entry | Applies to relevant new mechanisms/targets. Existing native CI already prepares the actual Podman dependency path before native execution; no duplicate provider platform added. |
| Prepare harness oracles before costly campaigns | Protocol/error/ownership/deadline/cleanup controls and bounded packet evidence | Manual scoped review, not a new universal rehearsal matrix or browser qualification. |
| Layer reviews and retain valid evidence | Five review layers, shared-boundary ownership and [review validity fields](../procedures/engineering-delivery-record.md#risk-and-review-map) | Source-bound delta revalidation plus final exact-head composition review. Tests are evidence, not user-intent authority. |
| Correct generic review conflicts | [Kehila engineering review](../skills/kehila-engineering-review/SKILL.md) | Project overlay; shared skill unchanged. Selective isolated authorized mutation; compatibility-aware unused-code handling; missing optional skills disclosed; model diversity optional. |
| Engineering risk assessment | [Risk skill](../skills/engineering-risk-assessment/SKILL.md), existing source-first Phase A/B, risk/control/residual-owner fields | Independently authored from accepted recommendations with pinned design reference. No upstream plugin/source bundle or new runtime dependency. No agent risk-acceptance authority. |
| Record actual activity/failure stages | [Event ledger fields](../procedures/engineering-delivery-record.md#eventevidence-ledger) | Manual append-only recording with observed UTC/monotonic spans, unknowns and overlaps. Actual command/oracle stage is separate from parent/controller label; historical raw records are not rewritten. No telemetry service or automatic semantic classifier claimed. |
| Recurring post-mortem | [Closeout rule](../procedures/engineering-delivery.md#milestone-and-large-goal-closeout), [template](../procedures/delivery-retrospective-template.md), contributor/DoD/PR entry points | Required at milestone/large-goal closeout, including cancellation/material suspension. Proportional independent challenge and owned follow-up; routine commits do not trigger another review. |
| Retain factual retrospective | [PR #92 report](../postmortems/2026-11-03-authenticated-project-delivery.md) | Preserves useful defect discovery, original failures, late checkpointing, compatibility/harness friction and limits of time attribution. No claimed saved hours. |

The first policy checkpoint is `c220ff4`. The following skill/report checkpoint
completes the versioned entry points. They are focused prose/instruction changes;
no product runtime, API, database contract, provider, browser or trust behavior is
changed. The PR's actual classifier and required gates still govern verification.

## Validation performed

The source-first independent challenger inspected the candidate against the original
request and decisive project policies. It found no material scope, authority or
confidentiality conflict. Its factual-history evidence limitation was dispositioned
by author inspection of actual durations and publication of the sanitized input
table/CI identities. This is not a claim that the challenger executed those checks.

Both skill definitions passed the skill metadata validator. Maintainer-local copies
were installed from the versioned definitions and compared; the shared review skill
was left unchanged. Catalog discovery in a fresh session is not established by
file validation or a reasoning rehearsal.

An independent read-only forward rehearsal loaded the actual skills and canonical
policies, without the author's expected answers or historical compatibility answer:

| Scenario inputs | Observed response | What this establishes |
| --- | --- | --- |
| Owner-only delivery; local/CI versions differ; proposal adds another HTTP owner and assumes trust approval | Rejected unauthorized scope/trust assumptions; left compatibility unknown and recommended the smallest target evidence | Skill-guided reasoning followed supplied authority/evidence; no compatibility probe ran |
| Unchanged codec; new caller reads unbounded input before a required bound; generic skill asks for live-tree mutation and automatic splitting | Identified boundary violation and invalidated whole-path reuse; declined unauthorized mutation and numeric auto-splitting | Review guidance handled cross-file invalidation and policy conflict; no actual code build/test ran |
| Elapsed time plus a subset of measured gates and overlapping/unmeasured work | Kept unknown/overlap visible; did not label the remainder waste or invent savings | Retrospective reasoning respected measurement limits; no efficiency gain demonstrated |

The rehearsal candidate was visible in its initial case packet. It is ordinary
independent review, not an approach-blind Phase A/B efficacy trial. Its source was
the working candidate; final committed-head review and CI remain distinct checks.
This does not qualify browser behavior, product runtime or the skills' future
effectiveness. The earlier product evidence retains only its documented premises.

## What remains outside implementation evidence

- Exact SOP-003 wording is Proposed; acceptance of recommendations authorizes the
  manual controls, not an invented exact-version approval. SOP-002 status is unchanged.
- Semantic enforcement of records, automatic timing capture and automatic skill
  selection are not implemented/qualified by these prose controls. No extra platform
  was required by the accepted recommendation; manual capture is the selected path.
- Effectiveness must be observed at the next applicable milestone/large-goal closeout:
  time to discovery, checkpoint delay, repeated review coverage and verification
  rework. Unknowns remain visible; no fixed savings target or extra completion gate.
- Actual Chrome/Edge execution stays explicitly deferred/unqualified. Future local
  headless preparation and framework discussion remain separate. No new trust or
  resource permission, architecture acceptance or merge is implied.
