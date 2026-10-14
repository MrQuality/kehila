# M1-01 snapshot verification plan

Status: W1 execution approved by explicit maintainer instruction on 2026-10-13
in [#49](https://github.com/MrQuality/kehila/issues/49#issuecomment-6154165059).
W1 merged in [PR #51](https://github.com/MrQuality/kehila/pull/51) on 2026-10-14
at 2efd444e38a4d60eaeb195a00359813174aa3d23; native PostgreSQL 16.15 evidence passed
at the recorded pre-merge source revision. See the [W1 report](m1-01-project-snapshots.md) for results and
remaining application obligations. Final staged/hosted checks passed and are linked in the PR conversation.
This supplements the [successor plan](m1-01-remaining-delivery.md); its acceptance
requirements remain normative. Execution results below do not accept a new ADR.

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
owns container setup/cleanup and limits. Check available memory before heavy
builds or service execution and ask the maintainer to free more if insufficient; recheck before the operation.

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

## W1 execution result (2026-10-14)

The [W1 report](m1-01-project-snapshots.md) records implementation of the host
bridge and separate native command, actual relational/JSONB fixtures and fixed
pair verification. The text-size risk above was reproduced: 1,048,294 compact
bytes became 1,048,577 JSONB text bytes. New native admission reserves structural
formatting space and rejects NUL before database access. The exact 1 MiB text
case passes. Historical codecs and all accepted ceilings remain unchanged.
Original-byte duplicate checks and fixture reconstruction do not establish the
future adapter's transactional attribution, authority or route admission.

## Review follow-up acceptance checks

Subprocess execution must retain descendant ownership after an early parent exit.
Windows uses a supervisor that establishes a kill-on-close Job Object before
launching the command; POSIX uses an owned process group. Parent completion and
pipe drainage share the execution deadline. Cleanup must preserve the original
failure and must not depend on a separate `taskkill` command. Regressions must
check bounded return and descendant termination, not merely the direct exit code.

Negative native cases must identify the expected production codec error;
framing errors, process failures and subsequent fixture-equality failures cannot
satisfy them. Relational negatives must vary revision and sequence independently
and match the intended constraint or primary diagnostic as well as SQLSTATE.
Include stored-row revision/version mismatches while both snapshots agree.
Shared tests must detect omitted required native scenarios and exercise frame,
parameter, build/artifact and target-confinement failure propagation. These
orchestration tests do not replace execution against actual PostgreSQL.

The helper's IPC frame has a separate bounded hexadecimal transport budget.
Reject duplicate/unknown/missing fields and noncanonical decimal revisions there;
contained storage records still enter the production bounded decoders. Native
NUL rejection belongs to typed new-write preflight, with a distinct admission
error; historical storage acceptance remains unchanged. Representation-stress
fixtures use deterministic ASCII padding with independently verified JSONB text
sizes and remain separate from domain-valid relational fixtures.

Whole-creation compatibility, including the saved result, remains the explicit
[W3 admission gate](m1-01-remaining-delivery.md#w3-creation-envelope-admission-obligation).
