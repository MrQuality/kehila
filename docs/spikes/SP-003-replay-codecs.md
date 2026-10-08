# SP-003 — Replay codecs and identifier representation

| Field | Value |
| --- | --- |
| Status / outcome | Planned / no results |
| Owner / assessor | Project maintainer; execution and assessment to be confirmed before starting |
| Created / last updated | 2026-10-08 / 2026-10-08 |
| Tracking issue | [39](https://github.com/MrQuality/kehila/issues/39) |
| Parent delivery issue | [#26](https://github.com/MrQuality/kehila/issues/26) |
| Prior evidence | [SP-002](SP-002-postgresql-project-create.md) |
| Effort limit / stopping condition | Proposed 90 minutes; confirm before execution. Stop on failed prerequisites or supported/bounded inconclusive conclusion. |

## Question, cases and execution plan

**Parent:** #26. Related: #9, #27, #8. Follow-up to SP-002 PostgreSQL project-creation feasibility.

### Pre-Execution Plan

1. **Goal & Context:** Choose an exact versioned request/result/snapshot representation before the native creation adapter is implemented. Resolve the proposed 128-byte opaque-ID bound and NUL rejection together with pure acceptance. Preserve D-034's recorded-time 90-day replay policy.
2. **Definition of Done:**
   - [ ] Record alternatives and an explicit codec/identifier recommendation; maintainer acceptance remains separate.
   - [ ] Run Rust/database goldens for exact strings, vector order, u64 values above 2^53 through u64::MAX, fractions/overflow, multibyte IDs and NUL.
   - [ ] Test changed-intent comparison, digest coherence, stored codec-version reuse and historical decoder compatibility.
   - [ ] Publish SP-003 observations, limits, and implementation follow-up; no production-route readiness claim.
3. **Dependencies & Prerequisites:** Published B-003 rules, SP-002, isolated PostgreSQL 16/Podman, Rust; inspect actual cross-language consumers before choosing a codec.
4. **Risks & Mitigations:** Prevent float conversion, Unicode normalization, order loss and unversioned changes. Preserve old-version fixtures. Never truncate identifiers or silently change pure acceptance.
5. **Spikes & Open Questions:** Byte encoding, version dispatch, historical support and identifier bounds. Proposed effort limit: 90 minutes, to confirm before execution; stop at a supported recommendation or explicitly bounded inconclusive result.
6. **High-Level Architecture / File Changes:** docs/spikes/SP-003-replay-codecs.md; bounded experiments/SP-003/ when executed. Prototype codecs remain separate from application adapters.
7. **Verification & Testing Plan:** Positive/negative and old-version goldens, exact PostgreSQL round trips and shared equality/digest cases. Record failures and skipped consumers explicitly.

This is a planned investigation, not an executed result. Clock sampling and compactor integration remain implementation tasks in #26 rather than additional engine spikes.

## Environment and reproduction

Not executed. Use Podman for database/service cases and the public contribution
and testing guides. Record tools, image pins, scope, timeouts, fixtures, commands,
barriers and fresh preflight before starting. Experimental artifacts belong in
`experiments/SP-003/` once needed; no executable prototype is claimed here.

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
