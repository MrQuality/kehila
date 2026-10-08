# SP-003 — Replay codecs and identifier representation

| Field | Value |
| --- | --- |
| Status / outcome | In progress / pre-execution plan; no experiment results |
| Owner / assessor | Project maintainer; execution authorized; recommendation assessment remains separate |
| Created / last updated | 2026-10-08 / 2026-10-09 |
| Tracking issue | [39](https://github.com/MrQuality/kehila/issues/39) |
| Parent delivery issue | [#26](https://github.com/MrQuality/kehila/issues/26) |
| Prior evidence | [SP-002](SP-002-postgresql-project-create.md) |
| Effort limit / stopping condition | 90-minute investigation budget; report setup/verification separately. Stop on failed prerequisites or supported/bounded inconclusive conclusion. |

## Question, cases and execution plan

**Parent:** #26. Related: #9, #27, #8. Follow-up to SP-002 PostgreSQL project-creation feasibility.

### Pre-Execution Plan

1. **Goal & Context:** #42 is merged; work proceeds on spike/issue-39-replay-codecs. Specify an injective project-creation request encoding and complete saved-result/configuration snapshot JSONB representation. Produce a recommendation supported by Rust/PostgreSQL fixtures, not a production adapter or accepted product decision. Preserve the 90-day replay policy unchanged.
2. **Definition of Done:**
   - [ ] Compare binary request bytes with incidental/canonical JSON alternatives and document a version-one recommendation.
   - [ ] Verify actual ProjectCreateCommand and ProjectCreateResult Rust types, complete configuration fields, both units, ordered arrays, optional values and enum labels.
   - [ ] Verify u64 precision, malformed values, digest coherence, changed intent and stored-version dispatch.
   - [ ] Document the mismatch between current String-backed ID acceptance and the proposed text domain; recommend a boundary without changing pure acceptance.
   - [ ] Publish source-linked observations, failed/blocked cases, reproduction instructions and production follow-up. No E2E creation-route claim.
3. **Dependencies & Prerequisites:** Existing Rust domain crate, existing locked serde_json/SHA-256 tooling, Podman and the SP-002 PostgreSQL 16 image pin. Inspect installed versions, free RAM/disk, machine/container state and image availability before database startup. Use one disposable, labelled database with no published ports or credentials. Shared full verification requires the existing development services, separately from this database.
4. **Risks & Mitigations:** Binary ambiguity: fixed domain/version tags, length-prefixed UTF8, explicit enum tags and reject trailing/truncated bytes. JSON integer loss: recommend canonical decimal strings for every u64, strict range/spelling checks, no floating-point conversion. Configuration drift: serialize/decode every current field, including normally empty seed collections. Compatibility: select the stored codec version; unknown versions fail closed and cannot admit an operation as new. ID narrowing: record as a proposed product change; never truncate or silently normalize.
5. **Spikes & Open Questions:** Evaluate version-one binary requests plus versioned JSONB results/snapshots versus JSON request hashing and JSON numeric u64. Investigate for at most 90 minutes, excluding environment startup and required repository verification. Stop early on a supported recommendation, failed prerequisite, or bounded inconclusive result. No future-version compatibility or unimplemented consumer is presumed tested.
6. **High-Level Architecture / File Changes:** experiments/SP-003/ will contain an isolated Rust prototype using the actual domain types, frozen fixtures and a bounded Podman test runner. docs/spikes/SP-003-replay-codecs.md records evidence; the spike register and #39 receive status/results. No production migration, route, pure validation, or replay timing changes. Prototype dependencies remain isolated and locked.
7. **Verification & Testing Plan:** Run the matrix below against explicit fixtures. Rust decodes request/result/snapshot data; a separate Python encoder/hash oracle checks exact bytes and SHA-256; PostgreSQL BYTEA/JSONB round trips supply actual retrieved values to the Rust decoder. Negative cases must fail explicitly. Record consumer coverage and limits rather than counting unavailable integrations as passes.

### Code review before execution

- src/pure/task_contract/src/project_create.rs owns trusted creation and exact typed equality; no native persistence codec exists.
- src/pure/task_contract/src/operation.rs accepts a trusted versioned fingerprint. Full records compare typed equality and fingerprint; tombstones retain only the original versioned digest. Adapters must compute retries using that stored version.
- experiments/SP-002/common.py hashes experimental JSON bytes. It is not the proposed production binary codec.
- ProjectCreateResult includes the Project, complete Configuration and SeedProfile. Revision/sequence values are u64; configuration vectors retain order under derived equality.
- The legacy Rust task worker and Go task API carry the old task contract, not the M1 creation command. No Go/browser M1 creation consumer is implemented. Existing task-path integration does not establish M1 codec compatibility.
- Operation tokens already require 1?128 ASCII graphic bytes. Project/actor/other opaque IDs currently lack the proposed universal 128-byte/NUL restriction. Project names already reject controls. The proposed ID limit remains subject to maintainer acceptance and later pure-contract alignment.

### Planned test matrix

| Case | Expected observation |
| --- | --- |
| R01 exact request goldens, both units, UTF8/escaping | Rust/Python bytes and digest agree; Rust decode equals the actual typed command |
| R02 changed field, boundary concatenation and Unicode normalization pairs | Different typed requests produce distinct bytes; no normalization |
| R03 truncated/trailing/invalid UTF8/enum/version | Decoder rejects; no default/current-version fallback |
| R04 retained request/fingerprint and tombstone retry | Equal intent replays before expiry; changed intent conflicts; equal expired intent stays expired using stored version |
| J01 complete result and configuration snapshot | Actual Rust result equals decoded saved data; every field retained, including populated normally empty seed collections |
| J02 0, 2^53-1, 2^53, 2^53+1, i64::MAX+1, u64::MAX | Exact decimal strings and exact typed u64 after JSONB retrieval |
| J03 fractional/negative/overflow/leading-zero/null/missing/extra fields | Strict wire decoder rejects rather than rounds/defaults/ignores |
| J04 vector reorder, object-key reorder, Unicode | Array order remains meaningful; object order does not; exact strings retained |
| J05 stored version, unknown versions, frozen version-one fixtures | Version one remains readable; unknown versions fail closed; future codecs explicitly untested |
| I01 empty/128/129-byte IDs, multibyte boundary, NUL | Record current pure acceptance and proposed storage acceptance separately; NUL rejected by PostgreSQL text/JSONB |
| D01 BYTEA, JSONB and unrestricted numeric | Actual PostgreSQL round trips match Rust/Python expectations; numeric can retain u64 but float bridges can lose precision |

## Environment and reproduction

Not executed. Use Podman for database/service cases and the public contribution
and testing guides. Record tools, image pins, scope, timeouts, fixtures, commands,
barriers and fresh preflight before starting. Experimental artifacts belong in
`experiments/SP-003/` once needed; no executable prototype is claimed here.

## Run summaries

No runs yet. Expected cases in the plan are planned observations, not test results.

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

Latest work: code-reviewed pre-execution plan. Next action: execute the bounded
prototype matrix, then publish evidence and a recommendation for assessment.
