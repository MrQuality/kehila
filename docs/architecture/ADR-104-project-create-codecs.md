# ADR-104 - Project-creation storage codec boundary

Status: Proposed. Owner: project maintainer. Recorded: 2026-10-12.
Acceptance: Pending explicit maintainer assessment. Publication as Proposed is
authorized; implementation authorization does not accept this ADR. Related: ADR-102, ADR-103, #47, #39,
#26/B-005, [implementation plan](../implementation/project-create-production-codecs.md).

Explicit maintainer approval of ADR-104 is required before PR #48 is merged.
Passing tests or implementation/publication authorization does not grant that
approval. The record remains Proposed pending the approval decision.

Maintainer review accepts isolation of the pure codec from runtime I/O. Acceptance
of this ADR as a whole remains pending the downstream contracts below.

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
Downstream transport, actor/routing, persistence and upgrade ADRs MUST implement
the following contracts. They MUST NOT weaken these requirements by treating them
as optional implementation advice or deferring them past native-route exposure.

### Contract 1: Authoritative ingress admission boundary

Every future transport/ingress adapter MUST enforce the codec admission ceilings
at the edge: at most 1 MiB of encoded JSON record bytes, 64 KiB per decoded UTF8
key/string, depth 16 with the root at zero, 1,024 entries per array/object and
16,384 JSON values including containers. All checks MUST succeed before any
per-request database-pool acquisition, database admission, application-worker
scheduling or application work queueing. Database constraints or a later worker
validation pass MUST NOT substitute for edge admission.

Adapters MUST use bounded reads with actual byte counting and explicit overflow
rejection; trusting Content-Length alone is insufficient. They MUST NOT buffer
arbitrary JSON and subsequently apply the record limit. A bounded buffer within
the record ceiling is permitted. Parsing original bytes MUST enforce decoded
string/key, depth, collection and node bounds before admitting the request.
Duplicate decoded keys, including escaped equivalents, MUST be rejected before
conversion to Value, JSONB or another representation that can discard duplicates.
No truncation, normalization or retry through a more permissive parser is allowed.

Generated persistence envelopes MUST also satisfy codec ceilings before database
admission; a small input does not authorize an oversized generated seed/result.
Historical reconstruction remains separate from current domain acceptance.

### Contract 2: Explicit actor and tenancy isolation

Decode success, structural validity, schema-version agreement and SHA-256
fingerprint matches MUST NOT be treated as evidence of caller authenticity,
tenant/project-boundary compliance or authorization. Downstream actor and routing
ADRs MUST explicitly prohibit those substitutions and define a separate trusted
actor binding, routing scope and authorization decision before protected access.

The adapter MUST bind command, operation/replay lookup, target routing and database
access to that trusted actor and authorized tenant/project scope. Caller-supplied
IDs and identity fields reconstructed from stored content MUST NOT confer access.
A valid payload or matching fingerprint MUST NOT bypass an absent/invalid actor,
scope mismatch or denied current authority. Cross-scope lookups and replay MUST
fail closed; #40/#41 must supply the binding/authority/allocation contracts before
native routes are exposed.

### Contract 4: Legacy and upgraded state gate

Unmigrated legacy records MUST remain isolated from native command/replay and
authoritative-state paths during database upgrades. They MUST NOT be dynamically
evaluated against live domain rules, silently reinterpreted as version-one data,
or promoted because a current validator/decoder happens to accept their shape.
An upgrade MUST identify unsupported/unmigrated state and preserve its isolation.

Translation requires a dedicated, approved migration RFC specifying source
representations, target codec versions and deterministic field/identity mapping.
The RFC MUST define preservation of historical meaning, actor/scope attribution,
operation/replay identity, integrity checks, invalid-record handling and
validation evidence, plus rollback/recovery and the gate for admitting translated
state. Until that contract exists, no automatic translation or admission is
permitted. Already versioned historical records continue to use their stored
codec version without reapplying mutable current-domain acceptance rules.

## Alternatives

- Reuse experiment sources in production: rejected; retain frozen evidence and
  establish direct package ownership and a normal workspace test path.
- Deserialize into Value first: rejected; duplicate object keys would be lost.
- Run current domain validators during reconstruction: rejected; a changed rule
  must not retroactively invalidate an accepted historical success.
- Add database/session behavior here: rejected; transaction, authority, retries
  and clock ownership belong to the following adapter/integration slices.

## Consequences and verification

The contracts above are acceptance obligations for downstream ADRs and adapters,
not disclaimers about missing integration. Adapter delivery MUST supply evidence
for each relevant boundary before it can claim conformance or expose a route:

- Ingress tests MUST exercise actual transport reads, misleading/absent lengths,
  exact/over-limit bytes, decoded keys/strings, nesting, collections, nodes and
  duplicate escaped keys. Instrumented pools/worker queues MUST demonstrate zero
  acquisitions or scheduled application work for rejected input. Generated-record
  overflow MUST likewise prevent database admission.
- Actor/routing tests MUST use valid-shaped, correctly fingerprinted payloads
  with missing/spoofed identities, wrong tenant/project scope, denied/revoked
  authority and cross-scope replay attempts; all MUST be denied independently
  of codec success, with no protected state disclosed or changed.
- Upgrade tests MUST demonstrate that legacy/unsupported records remain isolated
  without calling live domain validators, and that translation/admission is
  impossible before the migration RFC's validation and recovery gates succeed.

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

This PR implements the pure codec, not downstream transport/persistence or
migration enforcement. Those implementations remain required and MUST satisfy
the contracts above; passing pure codec tests cannot discharge their obligations.
Startup compatibility, production clock/compactor and authority/allocation work
remain mandatory prerequisites in their respective delivery slices. Native
Linux-host PostgreSQL and supported-release qualification require separate
evidence.
