# M1-01 snapshot verification plan

Status: Proposed execution detail for [#49](https://github.com/MrQuality/kehila/issues/49).
Implementation and native execution await maintainer approval of the first slice.
This supplements the [successor plan](m1-01-remaining-delivery.md); it records
required evidence, not completed checks or new architectural acceptance.

## Production codec and native test boundary

The pure crate owns encode/decode APIs, fixtures and unit tests, with no database
driver, process invocation or container I/O. Share one version-one project
encoder/decoder between saved results and project snapshots. Extract shared
project preflight checks for ID, name and prefix before JSON tree construction;
retain aggregate, exact-shape and encoded-output limits. Reuse the bounded,
duplicate-aware parser and preserve existing request/result/configuration fixtures.

The integration boundary owns a host test executable using the production crate,
invoked by `scripts/check_postgresql.py` through an extension implemented in the
first slice. Its concrete Cargo packaging must preserve pure-crate isolation.
The execution contract is:

1. Build the host helper from the source revision under test. Generate project
   and configuration envelopes with the production encoders from known typed
   fixtures; SQL-authored lookalike envelopes alone are insufficient evidence.
2. Use the native runner's existing explicitly supplied disposable PostgreSQL 16
   container. Preserve its fresh-database, image, isolation and resource checks;
   do not repurpose development or retained databases. Pass fixtures through a
   bounded, unambiguous SQL/IPC encoding without interpolating raw JSON into SQL.
3. Read actual JSONB output with explicit row/column framing and byte bounds.
   Feed those bytes to the production version-dispatched decoders. Compare exact
   typed values, including ordered and empty configuration collections. JSONB
   containment, unordered comparison or raw JSON text equality is insufficient.
4. Propagate build, SQL, framing, decode and equality failures as nonzero runner
   exits. Bound subprocess time/output and retain helper ownership through its
   final use; clean temporary resources on success and failure.

The required native command remains
`python scripts/check_postgresql.py --disposable-container <name>` with the
optional documented Podman connection. The new codec phase must execute under
that command after implementation, not remain a disconnected manual example.
The [native guide](../../storage/postgresql/README.md#disposable-native-verification)
owns container setup/cleanup and limits. The maintainer must confirm memory
readiness before heavy builds or service execution.

The current shared verifier and hosted CI do not run this native suite. For the
first slice, native execution is separately required PR evidence alongside
applicable shared checks; absence/failure blocks snapshot integration acceptance.
Record exact source revision, command, platform, PostgreSQL version and outcome.
Hosted CI remains evidence only for the checks it actually executes. Adding
hosted native execution requires a separately reviewed provisioning change.

## Fixture classes and coherence

| Fixture class | Required evidence |
| --- | --- |
| Pure representation | Complete project shape; zero through u64::MAX, including values above 2^53; noncanonical/overflow rejection; version/kind/field errors; applicable resource boundaries and original-byte duplicates. No mutable current domain validation. |
| Standalone JSONB | Production-encoded, PostgreSQL-compatible envelopes through actual JSONB and back through production decoders; u64 extremes and exact scalar/collection preservation independent of relational seed policy. |
| Real revision rows | Valid parent project/operation/actor attribution and positive revisions/sequences, with deferred constraints exercised. Persist both production-encoded snapshots and verify row/snapshot coherence on readback. |
| Negative boundaries | Invalid relational values/attribution reject; malformed or mismatched snapshots fail the codec/coherence verifier even where SQL only checks object shape. Original-byte duplicate rejection is tested before JSONB conversion. |

Keep envelope decoding separate from a row/pair coherence verifier. Before
accepting persisted state, require project ID and configuration project ID to
equal the row project ID; both recorded revisions to equal the row revision;
recorded archived flags to agree; and both envelope versions to agree with the
stored version. Add negative cases changing each relationship independently.
The helper's exact API can be chosen during implementation; these are fixed
representation relationships, not mutable current business acceptance.

The first slice verifies these relationships using fixtures derived from one
trusted result. The later adapter must derive and reload actual persisted state
in its transaction and prove attribution/equality; the fixture check does not
establish adapter atomicity or authorization. Later current-state drift audits
must not require historical `next_sequence` or `ever_estimated` to equal live
values, since item commands can change them without a configuration revision.

## JSONB compatibility gates

[PostgreSQL JSONB](https://www.postgresql.org/docs/16/datatype-json.html) changes
text formatting and rejects NUL strings. Codec structural validity therefore
does not by itself establish native persistence compatibility.

- Test near-limit configuration envelopes through the exact intended read path.
  Compact JSON admitted below 1 MiB could exceed that ceiling after JSONB text
  output; this is an unmeasured boundary risk, not an observed failure. Establish
  a bounded admission/read strategy that guarantees persisted accepted records
  remain decodable. Do not silently raise ceilings, use an unchecked/lossy parser
  or truncate/normalize values. Any required architectural contract amendment
  needs explicit review before the affected persistence capability is accepted.
- Test codec preservation of historical NUL strings separately from PostgreSQL's
  rejection. Specify new-write compatibility checks for all generated snapshot
  content before database admission, rather than assuming request identifier
  checks cover it. Do not narrow historical decode to current SQL/domain rules.

These checks retain [ADR-104](../architecture/ADR-104-project-create-codecs.md)'s
ceilings and legacy isolation. Passing round trips for ordinary fixtures alone
does not resolve the compatibility gates or qualify a native route.
