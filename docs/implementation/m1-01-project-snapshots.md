# W1 project snapshots: implementation and evidence

Related: [#49](https://github.com/MrQuality/kehila/issues/49),
[successor roadmap](m1-01-remaining-delivery.md),
[verification plan](m1-01-snapshot-verification.md). W1 execution was approved by
explicit maintainer instruction on 2026-10-13. This slice implements Accepted
[ADR-104](../architecture/ADR-104-project-create-codecs.md), with no acceptance
of Proposed ADR-103 or authorization of later work packages.

Planning [PR #50](https://github.com/MrQuality/kehila/pull/50) merged on 2026-10-14
at 0c89bfbfd0c20b8556894e4c91c7814d6eacc96c. W1
[PR #51](https://github.com/MrQuality/kehila/pull/51) merged into main on
2026-10-14 at 2efd444e38a4d60eaeb195a00359813174aa3d23. W1 is delivered. The source-bound
native and hosted evidence below remains distinct from the merge record;
this tracking reconciliation introduces no implementation or new native run.

## Implementation boundary

The pure `project_create_codec` crate provides the version-one project snapshot
envelope, sharing the project encoder, extracted project decoder and resource
preflight with saved results. Historical encode/decode retains exact values,
canonical decimal-string u64s, stored/embedded version agreement and all codec
ceilings without consulting mutable current domain rules.

`verify_revision_snapshots` separately checks row/pair project identity, recorded
revision, archival agreement and stored-version dispatch. It does not compare
historical sequence or estimate-use flags with mutable current state.

`encode_native_revision_snapshots` derives both envelopes from one caller-supplied
trusted result and returns private read-only bytes. It checks pair coherence,
codec resources, NUL incompatibility and PostgreSQL 16 JSONB text output size.
It reserves one byte for each structural colon/comma, independent of punctuation
inside strings. Neither codec versions nor resource ceilings change; historical
decoding remains independent of these stricter new-write compatibility checks.
This type does not prove authority, SQL scalar acceptance or transaction ownership.

The host helper belongs to `src/io/project_snapshot_check`; no database driver,
process invocation or external I/O was added to the pure crate. Its only direct
external dependency is already locked serde_json 1.0.151, also owned by the pure
codec. Cargo.lock adds one local workspace package without external upgrades.
Existing dependency/license assessment applies; no new advisory or minimum-Rust
qualification is claimed.

`scripts/check_postgresql.py` now calls the production helper through
`scripts/postgresql_snapshots.py`. The bridge owns bounded process pipes/timeouts,
descendant termination, explicitly framed hexadecimal JSONB output, fixed/checked
SQL inputs and a temporary build directory through final oracle use and cleanup.
It retains the existing disposable-container and fresh-database safety checks.
Successful native fixtures feed the checked admission type's actual returned
bytes into PostgreSQL. Only NUL/overflow negative fixtures use historical encoding
past that boundary, and they must produce the exact expected admission error.

## Verification result

Native verification passed on 2026-10-14 using Windows, WSL2/Podman and the pinned
PostgreSQL 16.15 image, through the documented command with an explicit disposable
container and connection. The final run used code commit
f2c6f7069d864cbb83c6c07c8e428405e2299f22. The container was removed afterward;
test services and the VM started for this task were stopped. The native suite verified:

- Production Rust encoding to actual JSONB and back through production decoders
  for seed, reordered collections, escaped/Unicode strings, zero, values above
  2^53, u64::MAX and exact-limit configuration fixtures.
- Actual positive revision rows at 1, 2^53 + 1 and u64::MAX with valid project and
  operation attribution; separate rejection of zero parent/row revisions and
  wrong attribution. These are narrow row fixtures, not a complete seed adapter.
- Project/configuration identity, revision, archival and embedded-version
  mismatch rejection after actual JSONB conversion; exact typed equality keeps
  order and empty collections significant.
- A 1,048,293-byte compact configuration returning exactly 1,048,576 bytes as
  JSONB text, which remains decodable. Adding one source byte produces compact
  length 1,048,294 and JSONB text length 1,048,577. Native admission rejects this
  second record before database access; the historical decoder rejects its
  oversized readback. This resolves the previously unmeasured expansion risk
  for the supported PostgreSQL 16 text path without increasing the ceiling.
- PostgreSQL rejection of NUL content, alongside separate codec preservation and
  native admission rejection; original-byte duplicate tests remain separate from
  JSONB, which cannot retain evidence of original duplicates.
- The existing foundation installation, role, scalar, payload-integrity and
  guard-mutation regressions still pass under the extended native command.

Thirty-one codec tests and codec/helper Clippy passed locally. The tooling suite
ran 115 tests: 113 passed and two POSIX-only fixtures were skipped on Windows.
Eight new bridge failure/bounds tests and one native-phase omission regression
are included. Mutations removing project preflight, archival agreement, JSONB
formatting reservation and invocation of the native phase were detected; sources
were restored. The first compile attempts correctly failed before the new APIs
existed. One boundary fixture initially reached its string ceiling rather than
the record ceiling; it was corrected to exercise the intended record boundary.

Staged attempts stopped at readiness when services were off, including the
README batch after early cleanup. The snapshot and host-helper commits then passed full staged verification;
the revision-pair commit passed its pure gate. The final native-bridge commit
and the checked-byte oracle follow-up both passed full staged verification:
115 tooling tests (113 passed, two skipped),
naming/engineering checks, Go formatting/vet, workspace tests, frozen codec
oracles and live contention/deadline/recovery/search-unavailable checks. Those
live checks exercise the existing task path, independently of the native suite.
The README/evidence batch also passed its full staged gate. Final hosted results
belong in the PR. No hook was bypassed. The shared
matrix runs the bridge safety/orchestration tests, not live native PostgreSQL.

## Remaining obligations

The internal application adapter, authenticated routes, allocation/authority
contracts #40/#41, production clock/retry/compactor, configuration/item history,
archive/selection coordination, legacy migration and recovery/load qualification
remain W2-W7 work. The adapter must adopt the checked envelopes and verify actual
transactional derivation, attribution and reload against the one trusted result.
Pure/native fixture success does not establish those application properties.
Native Linux-host execution remains unverified. Frozen SP-002/SP-003 sources and
evidence are unchanged. Completion of W1 must not close the #49 umbrella.

## PR #51 review follow-up

The [maintainer reminder](https://github.com/MrQuality/kehila/pull/51#issuecomment-6161326535)
tracks the agreed changes. The execution deadline now includes parent exit and
pipe drainage. Windows launches the command through a supervisor that joins a
kill-on-close Job Object before command execution; POSIX retains a process group
through cleanup even after parent exit. The Windows path no longer uses
`taskkill`. Original failures survive cleanup, and pipe-reader failures propagate.
The early-parent regression checks elapsed time and actual Windows descendant
termination; existing timeout and pipe-overflow cases remain required.

Native rejection modes now succeed only for the specified production codec error.
Malformed IPC or later fixture-equality failure cannot satisfy those modes.
The helper uses a typed duplicate/unknown/missing-field-rejecting IPC frame and
canonical row revision strings; its three focused tests cover frame bounds,
error/equality separation and deterministic representation-stress boundaries.
It directly owns already locked serde 1.0.229 with derive support. Cargo.lock
changes only that local helper dependency edge; no external version is upgraded.

Native preflight rejects NUL before constructing JSON trees and returns
`InvalidNativeSnapshot`, separately from malformed historical records. JSONB
space accounting shares the existing exact-shape traversal and bounds the sink
before serialization. Historical codecs, version-one mappings and ceilings are
unchanged. Boundary fixtures use deterministic padding for a frozen 16-status
representation-stress shape, independently checked against actual JSONB output;
they are deliberately outside live scalar/domain acceptance and are not claimed
as relational seed fixtures.

The native scenario contract is now covered in shared tests, including required
round trips, specific rejection categories, invalid parameters/oversized frames,
build/artifact failure propagation and temporary-target confinement/lifetime.
Revision and next-sequence negatives are independent, with primary constraint
or error diagnostics matched alongside SQLSTATE. Native stored-row revision and
codec-version mismatches test coherent envelopes against disagreeing row metadata.
The [verification plan](m1-01-snapshot-verification.md#review-follow-up-acceptance-checks)
records the resulting acceptance requirements.

The [W3 creation-envelope obligation](m1-01-remaining-delivery.md#w3-creation-envelope-admission-obligation)
remains open under #49: admit the actual saved result and both snapshots before
database access and test all three through native storage/readback. W1's checked
snapshot pair does not admit a complete transaction. Its exact-limit configuration
has 1,048,576 JSONB text bytes; the corresponding saved result would have
1,048,824 bytes and must not be admitted through that read path. The relational
W1 fixtures still use a placeholder operation result and prove only the stated
snapshot/row relationships. A whole-creation admission API and application
adapter require the separate W3 plan; neither is introduced here.

Follow-up verification and the final source revision are recorded below and in
the PR conversation. The earlier native run remains historical evidence for its
recorded source; hosted checks alone do not qualify the updated native path.

### Follow-up verification (2026-10-14)

- All four focused source commits passed their full staged gates: `c6b6bf4`
  (owned process lifetime), `a2641c7` (native preflight/serialization), `bb51c60`
  (typed IPC and specific rejection oracles), and
  `1aba3566eb4f2342da270af18b7be43d5540dfc1` (native scenarios/constraint evidence).
  The final audit correction `b64c66769b3e4e0b71022a775ac6f34a6ac6f9e9` also passed its full staged
  gate, restoring the original UTF-8 text and bounding asynchronous descendant
  termination observation by the original execution deadline.
  The final gate ran 124 Python tests: 122 passed and two POSIX-only skips on
  Windows, plus workspace tests, frozen oracles, engineering/Go checks and live
  task-path contention/deadline/recovery/search-unavailable checks.
- The 31 codec tests and three host-helper tests passed. Final codec/helper
  Clippy with all targets and warnings denied, and workspace formatting, passed.
- The documented native command, with an explicitly selected disposable
  container and resource-capable test connection, passed at source
  `b64c66769b3e4e0b71022a775ac6f34a6ac6f9e9` on Windows/WSL2/Podman with the same
  pinned PostgreSQL 16.15 image. Both the extended snapshot phase and the
  foundation integrity/role/mutation suite passed. The source tree was unchanged
  during that run; documentation changes were separate.
- Two container-creation attempts on the rootless connection failed before
  verification because that connection exposed no cgroup controllers and could
  not enforce `memory.max`. The final run used the existing explicit root
  connection, retaining the memory/CPU/PID/network/mount restrictions; no default
  connection, machine mode or admission check was weakened. All task-owned
  disposable containers were removed, the four test services stopped and the VM
  stopped after verification.
- Final diff inspection found Windows default text decoding had changed the
  Unicode fixture name and unrelated backlog punctuation. Original UTF-8 text
  was restored and compared with the baseline before publication. One correction
  gate then exposed an immediate-handle-observation race during asynchronous
  Windows job termination. The regression now waits for actual descendant exit
  only within the original deadline; the original leaking implementation still
  fails it. The correction gate and native suite subsequently passed, without
  omitting the test or weakening its lifetime/elapsed-time assertions.
- New regressions first exposed the early-parent pipe lifetime and reader/close
  error masking. In-memory mutations omitting the positive fixture loop and
  ignoring the primary SQL diagnostic were detected. Removing production
  row/pair coherence made the specific-rejection helper test fail; the source was
  restored byte-for-byte before final Clippy/format checks. These strengthen the
  evidence without claiming that shared CI executes native PostgreSQL.

Hosted results for the final documentation head belong in the PR conversation.
Native Linux-host qualification, whole-creation admission and the remaining W2-W7
application obligations remain open under #49. No ADR status changes or PR merge
are part of this follow-up.
