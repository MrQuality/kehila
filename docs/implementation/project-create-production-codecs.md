# Production project-creation codecs and native validation

Status: Planned, 2026-10-12. Follows PR #46; related #26/B-005, #39.

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
