# SP-004 — Trusted actor binding and initial-owner authority

| Field | Value |
| --- | --- |
| Status / outcome | Planned / no results |
| Owner / assessor | Project maintainer; execution and assessment to be confirmed before starting |
| Created / last updated | 2026-10-08 / 2026-10-08 |
| Tracking issue | [40](https://github.com/MrQuality/kehila/issues/40) |
| Parent delivery issue | [#11](https://github.com/MrQuality/kehila/issues/11) |
| Prior evidence | [SP-002](SP-002-postgresql-project-create.md) |
| Effort limit / stopping condition | Proposed 90 minutes; confirm before execution. Stop on failed prerequisites or supported/bounded inconclusive conclusion. |

## Question, cases and execution plan

**Parent:** #11. Related: #32, #26, #27. Follow-up to SP-002.

### Pre-Execution Plan

1. **Goal & Context:** Establish how an authenticated local actor becomes trusted command context, and define initial-owner rights plus current grant/revocation ownership before native routes are exposed.
2. **Definition of Done:**
   - [ ] Record the actor/session trust boundary and a proposed initial-owner permission mapping against B-003/B-007.
   - [ ] Demonstrate denied spoofed actor input, missing/expired identity and forbidden role escalation using a declared experimental integration.
   - [ ] Demonstrate current grant/event agreement and atomic revocation in a separately scoped management prototype; retained history alone must not authorize.
   - [ ] Repeat selected creation/replay versus revocation orderings; preserve observed lock barriers.
   - [ ] Publish SP-004 recommendation and gaps; actual authentication and grant-management implementation remain open.
3. **Dependencies & Prerequisites:** #11/#32 access requirements, public B-003 rules and SP-002 actor/core locking evidence. Review local access model before choosing an identity mechanism.
4. **Risks & Mitigations:** A supplied actor ID is not authentication. Bind context at the trusted boundary; no client-chosen authority or broad permission UPDATE grants. Keep unauthorized precedence before replay/conflict/expiry details.
5. **Spikes & Open Questions:** Session/actor mapping, owner permissions and management lock ownership. Proposed effort limit: 90 minutes, to confirm before execution; unresolved policy is a decision dependency, not a reason to claim a pass.
6. **High-Level Architecture / File Changes:** docs/spikes/SP-004-actor-authority.md; bounded experiments/SP-004/ when executed. The baseline creation-only grant-event guard must not be silently removed.
7. **Verification & Testing Plan:** Independent restricted connections, explicit trusted identity fixtures, allow/deny cases, grant/history mismatch, revoked authority and reached concurrent schedules. Experimental identity providers cannot certify a production integration.

Status: planned. Completing this spike will not close #11 or #32.

## Environment and reproduction

Not executed. Use Podman for database/service cases and the public contribution
and testing guides. Record tools, image pins, scope, timeouts, fixtures, commands,
barriers and fresh preflight before starting. Experimental artifacts belong in
`experiments/SP-004/` once needed; no executable prototype is claimed here.

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
