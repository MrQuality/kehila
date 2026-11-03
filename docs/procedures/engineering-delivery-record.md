# Engineering delivery record fields

Use these fields within the existing [work-framing brief](work-framing-template.md),
task or PR. Keep one canonical record; omit irrelevant detail with a reason. Follow
[SOP-003](engineering-delivery.md). Confidential raw events, packets, machine
configuration and credentials stay outside public source and output.

## Scope and checkpoint plan

- Authoritative request and accepted decisions; original/revised outcome and scope.
- Current revision, operating profile and actual execution permissions.
- Coherent checkpoint contracts; included/excluded work and completion criteria.
- Publication state and reason for any deliberate hold; no arbitrary commit quota.

## Risk and review map

| Failure mechanism / affected invariant | Evidence / confidence / likelihood | Impact / priority | Existing control / proposed action and cost | Residual risk / owner / acceptance authority | Invalidation / next observation |
| --- | --- | --- | --- | --- | --- |
| Specific material risk, not a generic category | Source identity; distinguish observation, hypothesis and unknown | Explain each separately | Cheapest decisive probe or implementation, within authority | An agent cannot mark a nondelegated risk accepted | Changed premise or bounded follow-up |

| Review layer / owner | Base and head/tree | Covered contracts and shared boundaries | Source inspected / checks actually executed | Findings and dispositions | Exclusions / validity conditions |
| --- | --- | --- | --- | --- | --- |
| Applicable layer and independent context | Exact identities | Include producer/consumer ownership | Reading is not execution | Severity, priority, source and closure evidence | Source, environment, fixture, oracle or contract changes |

## Event/evidence ledger

Append events or spans to the existing record; do not require a new telemetry
service. An event records:

- Event/span identity and optional parent; actor and actual activity/stage.
- Category: preparation, compilation, tests, cleanup, review, waiting or other/unknown.
- UTC start/end; monotonic duration and clock scope when available; unknown if absent.
- Source/tree and environment/fixture identities; relevant operation/command.
- Result: running, passed, failed, stopped, deferred or unknown; evidence reference.
- On failure: actual failing command/oracle stage, separately from controller stage.
- Cleanup result, evidence applicability and affected rerun/disposition.

Start/finish events should be recorded as work occurs, including review and waiting.
Mark earlier uninstrumented spans unknown. Identify overlaps; do not sum parallel
spans as elapsed time. A successful command is not an independent observer. Event
shape validation alone would not establish truthful classification or correctness.

Before a source change/rerun, identify which earlier observations remain valid,
which assumptions changed and what affected checks/controls must rerun. Preserve
required hook/CI gates and original failures. Sanitize any public summary.

## Publication and closeout

- Exact proposed committed head; final composition/relevance review and findings.
- Required local/CI checks with actual source identities; unresolved gates/decisions.
- PR description/issues/documents reflect scope, evidence limits and readiness.
- Follow the existing authorized ready/merge boundaries; reviews grant no permissions.
- At a milestone/large-goal boundary, complete the
  [retrospective fields](delivery-retrospective-template.md) in the same record or
  link a sanitized report. Name the next observation for each accepted improvement.
