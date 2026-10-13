# Production project-creation codecs and native validation

Status: Implemented for PR review, 2026-10-12. Follows PR #46; related #47,
#26/B-005, #39. ADR-104 and the concrete resource ceilings remain Proposed;
publication as a proposal is authorized, acceptance is pending.

## Pre-Execution Plan

1. **Goal & Context:** Provide a pure production codec boundary between typed
   project-creation values and native persistence. Preserve SP-003 version-one
   bytes and JSON shapes without depending on experiment implementation code.
2. **Definition of Done:**
   - Versioned request bytes and SHA-256 fingerprints preserve frozen goldens.
   - Stored-version result/configuration reconstruction preserves exact values.
   - Typed errors distinguish unsupported versions, malformed/corrupt records,
     digest mismatches and resource-limit violations.
   - Raw JSON parsing rejects duplicate keys before conversion to a JSON value.
   - Byte, depth, collection and total-node limits bound decoding; encoding
     checks source collections/strings before constructing serialized trees.
   - A checked native request boundary enforces SQL identifier/token bounds.
     Historical reconstruction does not reapply mutable domain acceptance.
   - Regression/mutation tests, shared verification, dependency review and
     documentation substantiate the scope; commits remain focused.
3. **Dependencies & Prerequisites:** Existing locked serde, serde_json and sha2
   packages, owned directly by a new pure Rust codec crate. No database driver,
   network, clock or runtime dependency. Full shared verification uses the
   existing disposable development services.
4. **Risks & Mitigations:** Keep stored representations independent of public
   DTOs and current domain validation. Never trim/truncate identifiers or invent
   a second codec version. Reject unknown fields and trailing input. Apply
   resource ceilings to historical decoding without imposing current product
   policy. Preserve frozen experiments and observations.
5. **Spikes & Open Questions:** No further engine spike. Resource ceilings are
   codec admission limits, not newly accepted product configuration limits.
   #40/#41 remain prerequisites for exposing a native command route. ADR-103
   production adoption remains explicit maintainer work.
6. **High-Level Architecture / File Changes:** A pure project_create_codec crate
   owns version dispatch, request framing, JSON envelopes and checked native
   request admission. Version-one mapping remains private. Shared verification
   includes the new crate. Public domain wrappers and the legacy worker remain
   unchanged; the future native adapter must use the checked boundary.
7. **Verification & Testing Plan:** Retain failing behavioral regressions before
   implementation. Test frozen fixtures, all request truncations, malformed
   lengths/UTF8/tags, decimal boundaries, exact ordered round trips, duplicate
   keys, unknown versions, corruption/digest mismatch and each resource limit.
   Invert a meaningful guard and require failure. Run targeted pure checks,
   formatting/Clippy and required full staged checks before publication.

## Delivery boundaries

This slice supplies codecs and native write admission, not a database adapter or
authenticated route. Application ingress and native SQL adapters must adopt the
boundary before it becomes authoritative for a supported path. Database schema
startup checks, transaction ownership, replay clocks/retries, compaction,
authority (#40), stable target allocation (#41), legacy import and recovery/load
qualification remain subsequent work. Issue #26's closed state does not establish
completion of those obligations. Action stacks remains outside this work.

## Implementation and verification result

The pure project_create_codec crate owns request framing/fingerprints,
duplicate-aware raw JSON parsing, configuration snapshots and complete saved
success envelopes. It owns already locked serde/serde_json/sha2 dependencies
directly. NativeRequest reuses the domain creation-input validator and adds
native identifier storage bounds; the domain validator extraction preserves
existing decision/replay ordering and acceptance. No legacy route adopts it.

Twenty focused Rust tests passed, covering frozen bytes/digests/JSON shapes,
request truncation, malformed framing/UTF8/version/kind/decimal/enum cases,
rich typed reconstruction, ordered collections, u64 precision/extremes,
new-write admission and all codec resource boundaries. Tests failed before the
corresponding implementation. Deliberate mutations were detected for fingerprint
comparison, duplicate-key rejection, aggregate node counting, escaped-output
limiting and a one-byte identifier-limit relaxation. Files were restored and
staged tests passed afterward. Formatting and codec Clippy checks passed.

The full staged shared suite passed for the request/native-admission and
saved-result commits: 106 Python tooling tests run (104 passed, two POSIX-only
fixtures skipped on Windows), naming/engineering checks, Go formatting/vet,
service readiness/NATS, Rust/Go workspace tests, frozen SP-002/SP-003 pure oracles
and live task-path contention/deadline/recovery/search-unavailable tests.
Parser/configuration and final boundary-test commits also passed their selected
staged checks. These shared live checks exercise the existing task path; they do
not establish a native project-creation adapter or database adoption of codecs.

The first full staged attempt failed during SP-003 executable cleanup with a
Windows access-denied error. The standalone diagnostic passed, followed by a
passing full staged retry; the cleanup failure's cause was not established.
A targeted working-tree build also reported missing exports from an earlier
staged snapshot in the shared Cargo cache; isolated-target validation passed.
Targeted checks used a separate target thereafter. No test or hook was bypassed.
Historical experiment sources, fixtures and observations remain unchanged.

## Next slice

Review the proposed codec ceilings and ADR-104 explicitly. Then implement the
internal PostgreSQL adapter: startup schema checks, caller/transaction ownership,
coherent persistence from a single trusted result, and request/core/result
attribution. Define the project-only snapshot envelope needed by
configuration_revisions.project_snapshot; the frozen evidence defines saved
results and configuration snapshots, not that separate envelope. Integrate
#40/#41 and trusted replay clocks/retries before exposing a native route.

## Object-key review follow-ups (2026-10-13)

Object keys now pass through a string visitor that validates decoded UTF8 byte
length before creating the owned key retained by the map. The previous guard
already rejected oversized keys; this change avoids its unnecessary owned copy.
Escaped input can still require serde_json scratch allocation before visitor
validation, bounded by the outer record limit. No zero-allocation or measured
throughput claim is made, and the proposed resource ceilings are unchanged.

Three new regressions cover exact 64 KiB key acceptance/one-byte overflow, literal
and escaped spellings, multibyte decoded lengths, rejection before parsing a
malformed value, and maximum-length escaped duplicate detection. The boundary
regression passed with the previous guard and failed with that guard removed.
A one-byte relaxation of the new visitor limit was also detected; source was
restored before staged verification. All 23 codec tests and codec Clippy passed.

The key-handling commit passed full staged shared verification: 106 Python tests
run (104 passed, two POSIX-only skips on Windows), naming/engineering checks,
Go formatting/vet, service readiness/NATS, workspace Rust/Go tests, frozen codec
oracles and live contention/deadline/recovery/search-unavailable checks. These
live checks remain evidence for the existing task path, not a native adapter.
The snapshot envelope was expanded for readability and the testing-guide grammar
corrected, preserving the stored version-one representation.

ADR-104 remains Proposed and requires explicit maintainer approval before PR #48
is merged. The [PR approval prerequisite](https://github.com/MrQuality/kehila/pull/48#issuecomment-6137865036)
records that requirement; neither passing checks nor implementation/publication
authorization grants acceptance. Project-only snapshots and the adapter/route
obligations above remain subsequent work.

## Downstream architectural contracts

ADR-104's Decision and Consequences sections define mandatory contracts for future
adapters; these are implementation/verification requirements, not optional scope
disclaimers. Edge admission MUST complete before per-request database-pool access
or application-worker scheduling. Authentication, actor binding and tenant/project
authorization MUST be established independently of decode/fingerprint success.
Unmigrated legacy state MUST remain isolated until an approved migration RFC
defines translation and admission/recovery gates; upgrades cannot substitute
live domain evaluation for that process.

Pure codec tests do not demonstrate those integration properties. The transport,
actor/routing, persistence and upgrade slices MUST provide the boundary evidence
specified by ADR-104 before claiming conformance or exposing native routes.
The pure crate's isolation from runtime I/O is accepted; the complete ADR remains
Proposed and still requires explicit approval before PR #48 is merged.
