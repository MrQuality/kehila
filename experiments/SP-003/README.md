# SP-003 codec experiment

Prototype only. The [spike record](../../docs/spikes/SP-003-replay-codecs.md)
contains the pre-execution matrix, observations and proposed recommendation.
This does not install a database adapter, migration or route. Product acceptance
of codec/identifier choices remains separate.

## Reproduction

Use Python 3.10+, Rust 1.88+ with the repository's locked dependencies and Podman
on Windows or Linux. Build the existing workspace normally first if its locked
dependencies are not cached; the oracle build then uses Cargo offline. No new
package graph or Python database driver is required. Rustfmt applies to these
standalone files separately from the workspace:

```text
rustfmt --edition 2021 --check experiments/SP-003/main.rs experiments/SP-003/codec.rs experiments/SP-003/tests.rs
python experiments/SP-003/check.py
```

The pure command runs eight Rust tests and five grouped Python golden/negative
checks. It is included in shared full/pure verification, but not prose-only
verification. It does not run PostgreSQL cases.

For database cases, start an existing Podman runtime and pull the immutable
PostgreSQL image recorded in SP-002 and imported by the runner. The runner
requires at least 3 GiB available host RAM and 2 GiB free host/container storage.
Use a new ignored output directory for every invocation:

```text
python experiments/SP-003/run.py --output .kehila/spikes/SP-003/local-run
```

If an existing remote connection is needed, supply
`--podman-connection CONNECTION_NAME`. This selects only the runner's child
processes and does not change the configured default. The observed Windows/WSL2
rootless connection could not enforce memory limits; the completed run used an
existing rootful connection with actual cgroup controls. Rootless operation is
not certified by that run. Do not omit resource controls to get a pass.

The runner creates a unique labelled container, verifies 256 MiB memory,
one CPU and 128-process ceilings, and uses a 256 MiB PGDATA tmpfs. It disables
networking and TCP database listening, publishes no ports and mounts no host
data. Local Unix-socket trust is used inside this isolated disposable container;
it is not a production authentication recommendation. No password is generated.
Tests use `podman exec psql`, a 10-second statement timeout and a 20-second
client timeout. Startup is bounded to approximately 30 seconds plus an active
readiness probe. Build/command/cleanup operations also have finite deadlines.

Identifier fidelity cases persist leading/trailing spaces, whitespace-only
strings, tabs/newlines and non-breaking spaces in the proposed text domain.
They compare JSON-encoded retrieved strings byte for byte and also check raw
single-cell output. The raw SQL reader removes only psql's final record newline;
trimming arbitrary whitespace would change valid identifier content.

The runner removes only its labelled container and associated anonymous volumes,
including on failed startup. It preserves ignored result summaries and Rust
test output. It does not stop a shared Podman machine or other containers;
after local work, stop services/runtime started for the session separately.
Never publish raw host diagnostics without inspection.

## Prototype representation

Request version one starts with UTF8 `kehila`, NUL, UTF8 `project_create`, NUL,
then unsigned 32-bit big-endian codec version `1`. It is followed by four
strings in this exact order: operation ID, project ID, name, prefix. Each has
an unsigned 32-bit big-endian UTF8 byte length followed by those exact bytes.
The final byte maps hours to `0`, points to `1`. No padding, optional fields,
normalization or trailing bytes are allowed. SHA-256 covers the whole encoding.
Actor identity remains in the authenticated operation key, not an untrusted
request field. The existing operation scope/permission rules still apply.

Result and snapshot JSONB use separate envelopes with exactly
`codec_version`, `kind`, and `value`. Version is JSON integer `1`; kinds are
`project_create_result` and `configuration_snapshot`. Result value contains
the complete Project, Configuration and seed profile. Snapshot value contains
the complete Configuration. The version-one keys, enum labels and null/array
representations are explicit in `codec.rs` and frozen seed fixtures. All u64
fields are canonical ASCII decimal strings: zero is `"0"`; no signs, leading
zeroes, fractions, exponent notation or overflow. All current fields are
required, including null options and empty arrays. Unknown/missing fields,
wrong scalar types and unknown enums/versions/kinds are rejected after parsing.

The codec preserves structural typed values; it does not apply current domain
validation to historical data or regenerate an old seed. Rust tests deliberately
populate normally empty collections and some structurally valid, domain-invalid
values to ensure that fields are not dropped. Production integrity validation is
separate. JSONB and the host oracle's JSON parser discard duplicate object keys;
this prototype does not certify duplicate-key rejection at a raw transport
boundary. Before production promotion, reject duplicate keys during the first
parse of original request JSON, before converting to Value/JSONB. The application
parser can own this check without requiring an API gateway.

`fixtures/request-v1.json` freezes independent Python bytes/digests for both
units. `fixtures/result-v1.json` freezes the actual trusted Rust revision-one
seed result and snapshot. Changes to these fixtures require explicit review;
do not regenerate them automatically to conceal compatibility changes.

The second request vector pins U+5B57, U+00E9, quote and backslash: seven
UTF8 bytes. The pure check independently pins these code points and length,
in addition to comparing Rust/Python encodings and the frozen SHA-256 digest.
R08's original observation summary is retained unchanged; the corrected vector
has separate follow-up evidence in [results.followup.json](results.followup.json)
(R09), including the executed source hashes and real probe results.

The storage probe and database both have unique names and ownership labels
registered before launch. Probe cleanup runs in finally, including after the
local Podman client times out. Cleanup diagnostics cannot replace the primary
execution error and also block successful runs when removal fails. To exercise
real probe success, nonzero exit and deadline paths on an existing connection:

```sh
python experiments/SP-003/check_probe.py --podman-connection YOUR_EXISTING_CONNECTION
```

This command uses the pinned image, no networking, 128 MiB memory, one CPU and
64 processes per disposable probe. The timeout case proves actual startup from
captured container output, then verifies container absence. It also removes
leftovers if the verification detects a regression. Error bookkeeping tests
in the shared Python suite complement these real Podman checks.

## Dependencies and reuse

The prototype links Cargo-reported artifacts from the existing locked workspace:
task_contract, serde_json 1.0.151 and sha2 0.10.9. It adopts no new versions or
runtime dependencies. Serialization and digest libraries retain their registry
provenance/checksums in the root Cargo.lock; their actual cached Apache-2.0 and
MIT texts were inspected. Apache-2.0 is the selected license for this use; preserve
upstream notices when packaging. These libraries are already workspace inputs;
dependency security/maintenance qualification remains subject to the repository
dependency policy, not established by this experiment. Changes use the existing
locked update/review process. No library source was copied into this prototype.

The build temporarily obtains sha2 through task_worker -> mongodb -> sha2.
Changing the legacy worker's dependency graph can therefore break this prototype
build even when the codec is unchanged. Building that worker also brings its
existing Rust 1.88 minimum and adapter compilation into shared pure verification;
pure verification still performs no service I/O. This is bounded spike coupling,
not the production dependency design. Before promotion, the crate that owns
hashing must declare sha2 directly and own its dependency/update review.

The build selects exact Cargo-reported artifacts, rejects ambiguous libraries
and preserves compiler outputs under Cargo's artifact directory. It does not
guess an rlib from a wildcard or mutate the production crate's derives.

The proposed identifier rule is nonempty UTF8, at most 128 bytes, no U+0000,
with C-collated database identity and no normalization/truncation. Operation
tokens keep their existing ASCII-graphic rule. This is a proposed narrowing of
several current pure String IDs; changing them needs maintainer acceptance,
coordinated domain/storage/interface validation, regressions and import
inspection, with identity types coordinated with B-007. Whitespace remains
accepted by this proposal; do not trim it to accommodate the test reader.
Binary codecs can preserve
NUL, but that alone cannot make relational text/JSONB accept it.

Unknown stored codec versions fail closed. Version-one fixtures exercise the
first historical decoder obligation; there is no implemented version two and
no future compatibility claim. Original digest versions must remain computable
for permanent tombstone comparisons. This prototype tests pure replay rules;
database adapter dispatch and production compaction remain #26 delivery work.

The version-one replay test explicitly constructs version-one fingerprints and
checks saved-result replay, changed-intent conflicts, digest inconsistency and
expired tombstones. It does not select an encoder from a stored database record.
Production dispatch must read that stored version and propagate unsupported-
version errors without fallback or treating the operation as unseen. Preserve
this obligation in #26; no artificial version two is needed for this spike.
