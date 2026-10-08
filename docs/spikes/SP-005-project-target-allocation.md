# SP-005 — Stable project-target allocation across retries

| Field | Value |
| --- | --- |
| Status / outcome | Planned / no results |
| Owner / assessor | Project maintainer; execution and assessment to be confirmed before starting |
| Created / last updated | 2026-10-08 / 2026-10-08 |
| Tracking issue | [41](https://github.com/MrQuality/kehila/issues/41) |
| Parent delivery issue | [#27](https://github.com/MrQuality/kehila/issues/27) |
| Prior evidence | [SP-002](SP-002-postgresql-project-create.md) |
| Effort limit / stopping condition | Proposed 60 minutes; confirm before execution. Stop on failed prerequisites or supported/bounded inconclusive conclusion. |

## Question, cases and execution plan

**Parent:** #27. Related: #26, #11, #8. Follow-up to SP-002.

### Pre-Execution Plan

1. **Goal & Context:** Define who allocates a trusted ProjectId and how retries recover the same target when a response is lost. Operation lookup includes typed target, so a retry must not allocate a new target and accidentally create another project.
2. **Definition of Done:**
   - [ ] Record a concrete allocation/reconciliation protocol and its trust boundary.
   - [ ] Test response loss before and after allocation/creation, actor-scoped retries and deliberate target collision.
   - [ ] Prove the retry retains actor/family/target/token; a different actor/token must not adopt a recorded success.
   - [ ] Publish SP-005 recommendation, rejected alternatives and native integration follow-up.
3. **Dependencies & Prerequisites:** #27 identity requirements, #26 scoped operation key, #11 trusted actor context and SP-002 collision/replay observations.
4. **Risks & Mitigations:** New targets on retries can evade same-key lookup. Never use a display prefix as identity. Make recovery explicit and preserve conflicting-success isolation; no blind uniqueness-error replay.
5. **Spikes & Open Questions:** Allocation timing, stable retry handle and ownership of the binding. Proposed effort limit: 60 minutes, to confirm before execution; stop with a bounded recommendation or a precise unresolved dependency.
6. **High-Level Architecture / File Changes:** docs/spikes/SP-005-project-target-allocation.md; bounded experiments/SP-005/ when executed. A prototype allocation table or endpoint is not an accepted product API.
7. **Verification & Testing Plan:** Synthetic allocation attempts, controlled response-loss barriers, repeated concurrent retries and independent final-state checks. Record scope and exact identity used in every attempt.

Status: planned. Completing this spike will not close #27.

## Environment and reproduction

Not executed. Use Podman for database/service cases and the public contribution
and testing guides. Record tools, image pins, scope, timeouts, fixtures, commands,
barriers and fresh preflight before starting. Experimental artifacts belong in
`experiments/SP-005/` once needed; no executable prototype is claimed here.

## Run summaries

No runs. Expected cases in the plan are planned observations, not test results.

## Conclusion and reuse boundary

No conclusion or accepted architecture choice. Reuse SP-002 only within its stated
baseline, role/helper, runtime and fault boundaries. Further experiments must
resolve this record's stated uncertainty rather than repeat engine feasibility.

## Decision and follow-up

Deliver a supported recommendation and retain rejected alternatives/limitations.
Update existing decisions/questions only when the maintainer accepts a choice.
The parent issue remains responsible for production implementation and regression
coverage; finishing this investigation does not close it.

## Handoff

Latest work: planned record and linked GitHub issue. Next action: confirm the
bounded effort, required decision inputs and executable case plan before starting.
