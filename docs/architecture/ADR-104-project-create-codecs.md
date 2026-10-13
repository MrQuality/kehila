# ADR-104 - Project-creation storage codec boundary

Status: Proposed. Owner: project maintainer. Recorded: 2026-10-12.
Acceptance: Pending explicit maintainer assessment. Publication as Proposed is
authorized; implementation authorization does not accept this ADR. Related: ADR-102, ADR-103, #47, #39,
#26/B-005, [implementation plan](../implementation/project-create-production-codecs.md).

Explicit maintainer approval of ADR-104 is required before PR #48 is merged.
Passing tests or implementation/publication authorization does not grant that
approval. The record remains Proposed pending the approval decision.

## Context

The native schema stores versioned request bytes/fingerprints and JSONB saved
results/configuration snapshots. SP-003 supplies frozen representation evidence,
but its standalone oracle, String errors and worker-derived hashing dependency
are not an application storage boundary. Current domain validation can change;
historical replay must retain the meaning of accepted stored content.

## Decision

Own codecs in the pure project_create_codec crate, independent of runtime,
database drivers and experiment implementation. Preserve the version-one binary
header, field order, big-endian lengths, unit tags and SHA-256 fingerprints.
Preserve version-one JSON shapes, complete ordered collections and canonical
decimal-string u64 values. Dispatch by the stored version and check embedded
version/kind. Unknown versions and malformed content are explicit failures.

Parse original JSON bytes through a bounded duplicate-aware visitor before
creating a Value. Duplicate decoded keys, including escaped spellings, are
rejected before their next values are read. Reject trailing input. Version-one
shape mapping is private and never invokes mutable domain validation.

Key visitors check decoded length before creating an owned map key. Escaped keys
and values can still use serde_json scratch storage before visitor validation;
the encoded record ceiling bounds that temporary work. This is not an
allocation-free parsing guarantee.

New native requests instead enter through NativeRequest, combining the shared
creation validator with native identifier byte/NUL bounds. Existing public
String wrappers, legacy routes and historical reconstruction remain unchanged.
Future native adapters must require this checked boundary and validate their
actor/other storage inputs; this type supplies no authentication or allocation.

Proposed resource ceilings, implemented for review: 1 MiB encoded record, 64 KiB decoded string/key, depth 16
(root depth zero), 1,024 entries per collection/object and 16,384 JSON values
including containers. Keys are bounded separately and do not count as values.
Typed encoding checks aggregate entries/string bytes before tree construction,
then checks exact shape and writes to a bounded memory sink. Escaping cannot
grow output beyond the record ceiling. Reject overflow without truncation.
These are codec admission ceilings, not accepted product configuration limits.
Future adapters must align authoritative admission with them before exposure.

## Alternatives

- Reuse experiment sources in production: rejected; retain frozen evidence and
  establish direct package ownership and a normal workspace test path.
- Deserialize into Value first: rejected; duplicate object keys would be lost.
- Run current domain validators during reconstruction: rejected; a changed rule
  must not retroactively invalidate an accepted historical success.
- Add database/session behavior here: rejected; transaction, authority, retries
  and clock ownership belong to the following adapter/integration slices.

## Consequences and verification

Frozen request bytes/digests and saved JSON shapes are regression oracles.
Tests cover exact reconstruction, scalar/enum variants, order, numeric extremes,
malformation, duplicate keys, version disagreement and resource boundaries.
Guard-removal/inversion checks must demonstrate meaningful detection.

Direct serde/serde_json/sha2 ownership uses already locked versions; dependency
assessment is recorded in the [crate guide](../../src/pure/project_create_codec/README.md).
Bounded parsing still allocates a JSON tree and typed reconstruction; no claim
of zero-copy decoding or measured throughput is made. Valid-shaped tampering is
not authenticated by these codecs; request fingerprint verification supplements
exact typed comparison and trusted storage, not authorization.

No native adapter, project-only snapshot envelope, populated-database upgrade,
startup compatibility, production clock/compactor, authority/allocation mechanism
or route is supplied. A project-only snapshot representation must be specified
when the adapter adopts configuration_revisions.project_snapshot. Native
Linux-host PostgreSQL and supported-release qualification remain separate.
