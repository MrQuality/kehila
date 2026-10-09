# SP-003 — Replay codecs and identifier representation

| Field | Value |
| --- | --- |
| Status / outcome | Concluded / supported recommendation; maintainer assessment pending |
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
   - [x] Compare binary request bytes with incidental/canonical JSON alternatives and document a version-one recommendation.
   - [x] Verify actual ProjectCreateCommand and ProjectCreateResult Rust types, complete configuration fields, both units, ordered arrays, optional values and enum labels.
   - [x] Verify u64 precision, malformed values, version-one digest/replay coherence, changed intent and unknown-version rejection. Production stored-version adapter dispatch remains in #26.
   - [x] Document the mismatch between current String-backed ID acceptance and the proposed text domain; recommend a boundary without changing pure acceptance.
   - [x] Publish source-linked observations, failed/blocked cases, reproduction instructions and production follow-up. No E2E creation-route claim.
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
- Operation tokens already require 1 to 128 ASCII graphic bytes. Project/actor/other opaque IDs currently lack the proposed universal 128-byte/NUL restriction. Project names already reject controls. The proposed ID limit remains subject to maintainer acceptance and later pure-contract alignment.

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

Use [the executable reproduction guide](../../experiments/SP-003/README.md).
The final run used Windows/WSL2, Python 3.10.11, rustc 1.93.0, Podman 5.8.3
and PostgreSQL 16.15 with UTF8 encoding. The image is pinned by digest, imported
from the established SP-002 runner; Compose is not used for this experiment.

Fresh preflight recorded approximately 5.15 GiB available host RAM, 134 GiB
free artifact disk and 950 GiB free container-storage filesystem space; logical
host CPU count was 20. These are point-in-time availability measurements, not
sustained consumption measurements. The container's actual cgroup controls were
verified: 256 MiB memory, one CPU and 128 processes; PGDATA used a 256 MiB tmpfs.
The final run used an existing rootful connection without changing the default.
Networking/TCP listening were disabled and no ports or host data were exposed.

Source inputs and normalized UTF8/LF SHA-256 fingerprints are recorded in the
[sanitized R08 summary](../../experiments/SP-003/results.observed.json).
The experiment built exact Cargo-reported libraries from the locked workspace.
No production dependencies or domain acceptance rules changed.

Run pure checks with:

~~~text
python experiments/SP-003/check.py
~~~

After starting Podman and pulling the pinned image, run the database cases with
a new ignored output directory:

~~~text
python experiments/SP-003/run.py --output .kehila/spikes/SP-003/local-run
~~~

An explicit existing connection can be selected with --podman-connection.
See the reproduction guide for deadlines, isolation, credentials, cleanup and
the observed rootless resource-control limitation.

## Run summaries

### SP-003-R08 - 2026-10-09, completed

Eight actual Rust domain/codec tests passed. Ten grouped Python/PostgreSQL
records passed; there were no failing or blocked records and no cleanup errors
in this final run. Group counts are not independent product acceptance cases.

Subsequent fixture inspection found that R08's second frozen request name
contained two ASCII question marks instead of its intended multibyte characters.
Its frozen bytes and digest agreed with that corrupted name, so consistency
alone did not detect the loss. Separate dynamic Unicode and database cases
were intact, but R08 does not establish the intended frozen multibyte vector.
The original summary and source hashes remain unchanged. A new code-point/byte-
length guard detects this regression independently; corrected execution must
be reported separately rather than attributed to R08.

| Case | Observed | Outcome |
| --- | --- | --- |
| R01/R02 | Frozen Rust/Python bytes and SHA-256 agree for both units, including the corrupted ASCII replacement; separate dynamic Unicode/escaping cases distinguish intent | PASS with frozen-vector limitation above |
| R03 | Every truncated prefix, trailing bytes, invalid field UTF8, unit and version fail explicitly | PASS |
| R04 | Pure retained records replay the saved result, changed intent conflicts, inconsistent fingerprints reject; permanent tombstones remain expired/conflicting with version-one hashing | PASS, pure protocol only |
| J01 | Complete actual Rust result/configuration equality after decode; optional IDs, all enum variants and populated normally empty seed collections covered | PASS |
| J02/J03 | Exact u64 decimal strings through u64::MAX; fractions, negatives, overflow, noncanonical spelling and wrong/missing/extra fields reject | PASS |
| J04 | JSONB object-key rewriting is harmless; ordered vectors and exact Unicode remain distinct | PASS |
| J05 | Frozen proposed version-one request/result/snapshot fixtures remain readable; unknown versions/kinds fail closed | PASS within version-one/unknown-version scope |
| I01/I02 | Current pure ID acceptance differs from the proposed 128-byte text domain; ASCII/multibyte byte boundaries verified; C collation distinguishes normalization forms; PostgreSQL rejects NUL | PASS; product narrowing awaits acceptance |
| D01 | BYTEA and JSONB written to ordinary tables and read over separate connections decode exactly; unrestricted numeric preserves u64 and fractions; a Python IEEE-754 float bridge loses 2^53+1 | PASS |

The richer Rust codec fixtures test structural preservation, including some
domain-invalid combinations, rather than certifying configuration admission.
Database JSONB fixtures cover the real revision-one seed with scalar/order
variants. Production relational reconstruction of all configuration variants
remains outside this spike.

### SP-003-R09 - 2026-10-09, correction follow-up completed

Eight Rust tests and all ten grouped Rust/Python/PostgreSQL records passed
again on Windows/WSL2, Podman and PostgreSQL 16.15, with no failed/blocked
records or cleanup errors. This run verifies the corrected frozen multibyte
request vector with its independent code-point and seven-byte UTF8 guard,
and the request decoder that moves owned strings without cloning. Actual
256 MiB/one-CPU/128-process database ceilings were verified again, with
networking and TCP listening disabled. Production/domain inputs are unchanged.

Separate real Podman checks passed for storage-probe success, nonzero exit and
timeout after confirmed container startup. Each verified container absence
after finally cleanup. Before the fix, the timeout check detected a leftover
named --rm probe and recovered it. Three shared Python tests cover primary-error
preservation, cleanup-failure rejection and missing/malformed ownership data;
they complement the actual container checks rather than certifying remote I/O.

The [R09 summary](../../experiments/SP-003/results.followup.json) records its
own source fingerprints and probe results. R08's summary remains unchanged
and identifies the earlier corrupted fixture. Full staged verification of the
correction code passed: 86 Python tests (two POSIX-only skips on Windows),
workspace Rust/Go checks, SP-003 pure cases and live task-path regression,
including 48 competing create/update/retry/reuse pairs. Final-head hosted CI
status is recorded on PR #43 separately from native database evidence.

The reproduction guide now states Rust 1.88+ and explains the temporary
task_worker -> mongodb -> sha2 build coupling. Production hashing ownership
requires a direct dependency before promotion. No codec/identifier product
decision or additional database-engine spike follows automatically from R09.

### Earlier bounded attempts

Pure checks passed before database execution. Initial memory preflights stopped
before database creation because available host RAM was below 3 GiB. Subsequent
rootless attempts failed resource-control startup: the runtime did not expose
the required memory controller. Limits were not removed to bypass this failure.

The first rootful bootstrap rejected its password-free initialization with
host authentication set to reject. It failed readiness and its labelled
container was removed. Bootstrap was corrected to use local trust with both
networking and TCP listening disabled. R07 then passed nine grouped checks;
R08 strengthened those observations with table persistence across connections,
the actual ID domain and both-unit Rust result coverage. Earlier setup failures
are not counted as successful database observations.

The missing frozen-fixture files were also detected by the initial pure run;
fixtures were then established and checked explicitly. Proposed golden files
are now retained, not regenerated automatically.

The investigation stopped at a supported bounded recommendation. Runtime setup
and repository-wide verification are recorded separately; this record does not
claim a precisely measured 90-minute performance result.

## Conclusion and reuse boundary

### Proposed version-one choice

1. **Requests:** use domain-separated, versioned binary bytes with fixed field
   order, big-endian length prefixes and explicit unit tags. Hash those exact
   bytes with SHA-256. Full records retain bytes, decode to the typed command and
   verify the stored digest; a matching digest alone does not replace exact
   typed equality. Actor/family/target scoping remains in the operation key.
2. **Saved results and snapshots:** use JSONB envelopes with explicit version
   and kind, complete fields, fixed enum labels, required null/empty collections,
   preserved vector order and strict shape decoding. Keep separate result and
   snapshot kinds. Return the saved result; do not regenerate it from current
   tables or the current seed function.
3. **u64 scalars in JSON:** encode canonical decimal strings and decode strictly
   to u64. Keep relational columns in the proposed exact numeric domain. This
   avoids relying on every future JSON consumer having a lossless number parser.
4. **Opaque IDs:** recommend nonempty UTF8, a 128-byte limit, NUL rejection,
   C-collated identity and no normalization/truncation. Keep operation tokens'
   existing ASCII rule. This requires an explicit product acceptance change and
   import review before applying it to existing String-backed ID types.
5. **Versions:** choose the request encoder from the stored operation version on
   retry, including tombstones. Unknown versions must stop with an explicit
   unsupported-data failure, never fall through to new execution. Retain old
   fingerprint encoders for the lifetime of permanent tombstones and old
   snapshot decoders for retained history. Do not silently relabel old bytes.

The exact prototype header, field order, JSON keys and enum labels are described
in the [reproduction guide](../../experiments/SP-003/README.md) and frozen
fixtures. They are proposed version-one choices, not an approved external API.

### Alternatives and consequences

| Alternative | Assessment |
| --- | --- |
| Hash incidental JSON output | Reject: harmless formatting/key/escape differences can change bytes; SP-002's helper is not a production contract |
| Fully specified canonical JSON requests | Viable if all escaping, field/type/version and numeric rules are frozen; not inherently incorrect. Binary is recommended for this small fixed request because its injectivity is simpler to specify and test |
| JSON numbers for u64 | PostgreSQL can preserve them; the risk is intermediary floating-point conversion. Decimal strings make the lossless obligation explicit; consumers must parse/range-check and SQL numeric queries need casts |
| Store JSON text instead of JSONB | Unnecessary for result/snapshot object ordering; preserving incidental text does not improve typed equality |
| Accept NUL by storing all IDs as BYTEA | Technically possible but requires broader relational, JSON history and interop changes; not justified by a current product need |
| Silently narrow IDs only in the adapter | Reject: creates pure/storage acceptance mismatch. Change the accepted contract and all authoritative validation together |

PostgreSQL documents exact JSONB numeric mapping, loss risks in other JSON
implementations, NUL rejection, and object-key/duplicate-key rewriting in its
[JSON type documentation](https://www.postgresql.org/docs/16/datatype-json.html).
A further implementation trap is JSONB containment: its array matching ignores
order. Do not use containment (@>) as a substitute for typed equality. This
observation comes from that documentation, not an additional executed case.

### Limits

- No production migration, native adapter, authenticated route, production
  compactor or stored-version adapter dispatch was delivered.
- Pure replay tests complement SP-002; they do not prove PostgreSQL production
  replay integration, clock sampling, permissions, recovery or restore.
- No version two exists; future compatibility is an obligation demonstrated by
  retaining version-one fixtures, not an experiment against an invented upgrade.
- Actual Rust and Python consumers were executed. No Go/browser M1 creation
  consumer currently exists; the float counterexample is not browser E2E.
- Native Linux PostgreSQL execution was not performed. Shared Linux CI may test
  the pure prototype; that is separate from this Windows/WSL2 database run.
- Raw duplicate JSON object keys are collapsed before Value/JSONB decoding.
  Prototype shape checks do not certify duplicate-key rejection at ingress.
- Production decoders need explicit transport/allocation/collection bounds and
  corruption errors. This prototype's bounded input is not release hardening.
- Timestamp/revision/sequence decimal representation is covered where present
  in the creation result. Other command families and future field-value codecs
  require their own versioned shapes and fixtures.

## Decision and follow-up

Recommendation awaits maintainer assessment; no new D-* decision is accepted
by executing this spike. #39 can be assessed against its scoped deliverables;
keep it open until that assessment. #26 retains production implementation.

Production promotion also requires direct sha2 dependency ownership in the
fingerprinting crate and duplicate-key rejection during the first parse of
original request JSON, before Value/JSONB loses that information. The application
parser can enforce this without requiring a separate gateway. After identifier
policy acceptance, align authoritative domain validation, storage constraints,
interfaces and imports together, coordinating identity types with B-007.

After acceptance, record the chosen codec and identifier policy, align pure
ID validation and schemas without silent compatibility changes, then implement
the native migration/roles and adapter with stored-version dispatch, exact
reconstruction, digest coherence and corruption tests. #40/#41 remain separate
route-integration dependencies. Preserve the proposed-v1 goldens as regression
fixtures and add genuine older/newer fixtures when a second version is designed.

## Handoff

Latest completed native run: R09, with R08 retained as historical evidence.
The bounded recommendation is ready for
maintainer assessment. No additional database-engine spike is required for
these questions. Next decision: accept or revise the representations and
identifier policy before promoting them into production code.

## Verification correction plan (2026-10-09)

1. **Goal & Context:** Repair the frozen multibyte request fixture and prevent a self-consistent ASCII replacement from passing unnoticed. Give the storage-preflight container the same explicit ownership and cleanup guarantees as the database. Preserve the original R08 execution evidence.
2. **Definition of Done:**
   - [x] An independent expected-code-point check fails against the existing corrupted fixture and passes after correction.
   - [x] Normal, failing and timed-out probes leave no owned container; timeout verification uses a real Podman process and retains its primary error.
   - [x] Request decoding moves owned strings without extra clones or unchecked conversion.
   - [x] Rust 1.88+ and temporary MongoDB-to-sha2 build coupling are documented.
   - [x] A new native run records corrected source fingerprints; R08 observations remain unchanged.
   - [x] Full staged verification passes for correction code. Final-head CI and public publication completion are tracked on PR #43 and issues #39/#26.
3. **Dependencies & Prerequisites:** Existing locked Rust dependencies, Python 3.10+, Podman with enforceable cgroup limits, pinned PostgreSQL image, unchanged 3 GiB host-memory and storage preflights. Existing development services are needed separately for full staged verification.
4. **Risks & Mitigations:** UTF8 output cannot repair characters already lost upstream: pin expected Unicode code points and byte lengths independently. Client timeout does not guarantee remote container exit: register an explicit name/label before launch and reconcile/remove it in finally. Keep original execution errors and expose cleanup failures.
5. **Spikes & Open Questions:** No new design spike. Verify the identified timeout path against an actual disposable Podman container. Existing production codec/identifier recommendations remain pending assessment.
6. **High-Level Architecture / File Changes:** Correct request-v1.json and its pure guard; move shared container cleanup into a small experiment resource module used by the probe and database; add an explicit real-probe verification command; simplify decoder ownership; publish a separate follow-up summary and documentation.
7. **Verification & Testing Plan:** Retain the failing fixture guard before repair, then run pure codec checks. Verify real probe success, nonzero exit and deadline cleanup. Run a fresh native PostgreSQL matrix with final source hashes and required staged checks. Preserve R08 rather than relabeling its fixture evidence.

## Identifier fidelity and evidence clarification plan (2026-10-09)

1. **Goal & Context:** Preserve identifier whitespace in the PostgreSQL verification harness and distinguish tested version-one replay from future stored-version adapter dispatch. Retain direct hashing ownership, duplicate-key rejection and coordinated identifier acceptance as production prerequisites.
2. **Definition of Done:**
   - [ ] Real PostgreSQL regression cases detect the current whitespace loss before repair, then preserve leading, trailing, whitespace-only and embedded whitespace identifiers after repair.
   - [ ] Identifier values are persisted in the proposed text domain and retrieved through a lossless JSON representation, with an additional raw single-cell reader check.
   - [ ] Pure acceptance remains unchanged; Rust/Python checks explicitly cover whitespace identifiers.
   - [x] Evidence wording limits completed work to version-one replay/fingerprint coherence and unknown-version rejection; production dispatch stays in #26.
   - [ ] Separate native evidence records final source fingerprints while R08/R09 summaries remain unchanged.
   - [ ] Required staged checks pass; final-head CI and issue/PR publication are reported on GitHub. PR #43 retains its existing status.
3. **Dependencies & Prerequisites:** Existing locked tools and pinned PostgreSQL image, Podman with enforced resource ceilings, unchanged 3 GiB memory and 2 GiB storage preflights. Existing development services are required separately for full staged verification.
4. **Risks & Mitigations:** Broad whitespace stripping destroys valid identifiers. Remove only psql's final record newline from raw single-cell output; JSON-encode retrieved identifiers to preserve data independently of line framing. Compare exact UTF8 bytes and include whitespace-only and embedded-newline cases. Preserve earlier run summaries.
5. **Spikes & Open Questions:** No new design spike or invented version two. Actual stored-version dispatch belongs to the production adapter. Codec/identifier product acceptance remains separate.
6. **High-Level Architecture / File Changes:** Extend experiments/SP-003/run.py and tests.rs with whitespace cases, correct the SQL output reader, clarify this record and the reproduction guide, and publish a new native observation summary. No production schema, dependency graph or domain-validation change.
7. **Verification & Testing Plan:** Retain a failing real-database run against the new cases before repairing the reader. Run the complete native matrix and pure checks after repair, verify source fingerprints and prior-summary stability, then complete staged verification and final-head CI. Update #26/#39 and add a final summary comment to PR #43 without changing its status or merging.
