# W1 project snapshots: implementation and evidence

Related: [#49](https://github.com/MrQuality/kehila/issues/49),
[successor roadmap](m1-01-remaining-delivery.md),
[verification plan](m1-01-snapshot-verification.md). W1 execution was approved by
explicit maintainer instruction on 2026-10-13. This slice implements Accepted
[ADR-104](../architecture/ADR-104-project-create-codecs.md), with no acceptance
of Proposed ADR-103 or authorization of later work packages.

Planning [PR #50](https://github.com/MrQuality/kehila/pull/50) merged on 2026-10-14
at 0c89bfbfd0c20b8556894e4c91c7814d6eacc96c. W1
[PR #51](https://github.com/MrQuality/kehila/pull/51) now targets main and remains
unmerged for maintainer review. Integrating that baseline and updating tracking
does not change the implementation tree or replace the native evidence below.

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
