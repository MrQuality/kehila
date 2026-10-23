# A0 bundle decoding adapter: bounded implementation proposal

Status: First text/decoding slice implemented, 2026-10-23; complete credential
admission and container integration remain pending. The contract below is the
bounded implementation baseline; the result record states actual evidence.
Baseline: merged PR #77, `a1ecc9d`. Related: [API review](local-owner-authentication-bundle-parser.md),
[container plan](local-owner-authentication-container-plan.md),
[#40](https://github.com/MrQuality/kehila/issues/40), [#49](https://github.com/MrQuality/kehila/issues/49).

## Objective and scope

Make one independently testable part of future certificate/key admission concrete:
check the complete version-1 text format and decode its two payloads. Recommend
this as the first implementation slice inside the standalone AUTH-001 experiment.
It supplies neither a usable TLS credential nor permission to start a listener.
Complete DER validation, certificate policy and positive key matching remain a
following stage, with unresolved safe-library behavior kept visible below.

The maintainer accepted ADR-106's bounded disposable architecture and reaffirmed
that decision after its plain-language explanation. After PR #78 merged, the
maintainer instructed continuation into this bounded implementation step and
separately permitted single-job builds/tests and required gates. This grants no
container proof or trust/security change. No change to
the agreed bundle grammar, TLS profile, production graph, trust or resume rule is
proposed. AUTH-001's existing host-only paths and tests remain unchanged.

| Consequential premise | Evidence / status | Consequence and next action |
| --- | --- | --- |
| Text framing/decoding can be isolated without admitting credentials | Proposed private output boundary below; needs implementation review | Reject any listener integration or conversion that bypasses later admission |
| General decoder success proves canonical input | Contradicted by selected source in the API review | Enforce whole-payload terminal padding before library decoding |
| Existing lock entries make direct use free of review | Contradicted by the dependency policy | Review actual runtime feature/graph promotion before implementation |
| The byte-slice parser proves safe file reads or input erasure | Unsupported; input ownership and reader are external | Keep bounded same-descriptor reading and caller cleanup as separate obligations |
| DER/profile/key matching can be deferred without declaring admission complete | Established scope distinction, not proof that the later stage is feasible | Keep that stage blocked until safe APIs and exact oracles are reviewed |

## Proposed module and ownership contract

Add a private `bundle` module to AUTH-001 only. The proposed entry point is
`decode_bundle(input: &[u8]) -> Result<DecodedBundle, BundleDecodeError>`.
Its first action rejects `input.len() > 65_536` before scanning or allocating
payload storage. This proves a parser boundary, not that a file reader stopped at
64 KiB plus one overflow byte. A future reader must independently establish that.

`DecodedBundle` has private fields containing certificate bytes and key bytes;
both are explicitly **unvalidated binary payloads**. No public constructor,
`Clone`, byte-printing `Debug`/`Display`, serialization, `CertifiedKey` conversion
or listener integration is allowed. Only the eventual internal admission stage
may consume the buffers. The first slice's tests may inspect bytes privately.
Synthetic nonsecret payloads that are not DER must decode successfully when their
text is valid: this is intentional and must not be described as certificate
acceptance. A later stage must reject those same bytes as invalid credentials.

Own joined payload and decoded buffers in zeroizing wrappers from allocation
onward, including partially decoded output on failure. Keep the input borrowed;
the caller owns its lifetime, protection and erasure. Do not derive the output's
formatting from its wrappers. The selected zeroize 1.9.1 has opaque `Debug`; that
corrects the earlier proposal's contrary generalization, without granting the
bundle a formatting interface.
Return only `TooLarge`, `Framing`, `Base64` or `Allocation` categories. Size wins
over other faults; framing is validated before decoding. During subsequent
processing, allocation failure can precede detection of bad base64. No source
bytes, labels, partial output or upstream error/debug object may escape in an error.

## Algorithm and bounded work

1. Reject oversized input before all other work. Scan every byte using explicit
   before-certificate / certificate / between / key / after states. Enforce the
   [existing exact grammar](local-owner-authentication-container-plan.md#diagnostic-bundle-format-version-1),
   including delimiter line boundaries, LF/CRLF and permitted exterior whitespace.
   This first pass allocates no payload storage and requires nonempty payloads
   containing only the standard base64 alphabet or `=`. Payload SP/HT and other
   alphabet violations are `Framing`; length/padding/pad-bit combinations are
   `Base64`. Finish framing validation before allocating or decoding payloads.
2. For each nonempty admitted payload, join only its 1..64-character lines. No
   general PEM iterator, normalization, unknown-section skipping or ASN.1 parsing.
3. Require payload length divisible by four and at most two `=` characters, only
   at the end of the whole joined payload. A padded suffix split across valid
   lines remains valid. No non-padding symbol may follow the first `=`.
4. Use library `BASE64.decode_len` to size the output and `decode_mut` to decode;
   pass exactly the documented output length to avoid its panic precondition.
   Discard/zeroize any partially written output on failure. Retain only the
   returned initialized prefix. The library checks alphabet, padding shape and
   unused trailing bits; do not hand-write those checks or use partial decoding.
5. Return the two owned unvalidated buffers only after both decodings succeed.
   Drop intermediate storage on every path. No callbacks, I/O, clock, task spawn,
   cryptography, unsafe application code or process-global state belongs here.

The encoded input is at most 65,536 bytes. Joined payload lengths together are
at most that input length; decoded output lengths together are at most 49,152
bytes. Use checked size arithmetic and fallible bounded reservations before
writing; never grow a buffer beyond its reviewed bound. These are logical byte
bounds, not a total process-memory guarantee: allocator capacity/overhead, stack,
caller copies and future TLS/provider copies need separate accounting. No secure
erasure of historical reallocations, registers, swap or all provider copies is
claimed. Process abort/termination does not promise execution of destructors.

## Direct dependencies and adoption review

| Implemented pin / features | Purpose and source | Disposition |
| --- | --- | --- |
| `data-encoding = "=2.11.1"`, defaults off, no features | `BASE64.decode_len` / `decode_mut`, [versioned source](https://docs.rs/data-encoding/2.11.1/src/data_encoding/lib.rs.html) | MIT; no normal dependencies in the published manifest. Existing development-transitive entry, new direct runtime use |
| `zeroize = "=1.9.1"`, defaults off, `alloc` | `Zeroizing<Vec<u8>>` for owned scratch/output, [versioned API](https://docs.rs/zeroize/1.9.1/zeroize/struct.Zeroizing.html) | Select Apache-2.0; alloc has no dependency edge in the published manifest. Already in the locked graph, new direct use |

Selected published manifests were inspected; both archive checksums match the
committed lock. These feature choices do not request optional derive,
serde or a different crypto backend. Cargo feature unification may still enable
features through existing dependencies; do not report the runtime closure as
minimal from our feature declarations alone. Actual metadata/feature-tree review
and the two Cargo-generated root dependency edges are recorded in the
[dependency review](../../experiments/AUTH-001/DEPENDENCIES.md#2026-10-23-decoder-direct-use-review).
x509-parser 0.18.1 remains investigated for the following stage,
not selected for this slice; no additional ASN.1 dependency is chosen.

Implementation followed review of the actual direct/runtime/test feature graphs,
lock delta, upstream maintenance/advisories and reachable risks. The existing
[selected licenses/notices](../../experiments/AUTH-001/DEPENDENCIES.md)
cover the exact artifacts; retain MIT and selected Apache texts in distributions.
The earlier point-in-time advisory query is not a current scan or qualification
of promoted serving dependencies. Any outage or finding needs an explicit
disposition under the [dependency policy](../engineering/DEPENDENCIES.md).

## Test oracles and completion gates

The text/base64 cases below were exercised using small literal nonsecret vectors;
no real keys, certificate generation, files, trust, VM or network are necessary
for these decoder tests. Ownership/resource invariants also require source review;
no dynamic allocator-failure test or physical erasure proof is claimed.

| Group | Exact expected outcome |
| --- | --- |
| Positive grammar | LF/CRLF, permitted exterior whitespace, END at EOF and valid 1..64 wrapping produce the exact expected two byte arrays; one 64-character line of `A` decodes to 48 zero bytes |
| Positive base64 | `TWFu`, `TWE=` and `TQ==` decode to `Man`, `Ma` and `M`; `TQ=` followed by newline then `=` produces the same `M` |
| Whole input boundary | Valid input padded with permitted exterior whitespace to 65,536 bytes succeeds; 65,537 bytes returns `TooLarge` even if other syntax is invalid |
| Framing failures | BOM/comments/headers/non-ASCII, lone CR, missing/repeated BEGIN/END, delimiter-label mismatch, missing newline after BEGIN/payload, indented or trailing-space delimiters, empty payloads, blank/65-character payload lines, payload whitespace/non-alphabet symbols, wrong order/duplicate/unknown/encrypted/key-specific blocks and forbidden tails return `Framing` |
| Base64 failures | `TQ`, `TQ===`, `TR==`, `TWF=` and `TQ==TQ==` return `Base64` when framing is otherwise valid; no partial bundle is returned |
| Partial failure and precedence | Valid first payload plus invalid second returns only the category; malformed later framing wins over an earlier bad payload; errors never format input or decoded bytes |
| Ownership/resource | Both scratch/output paths use zeroizing ownership, including decode failures; reviewed sizes and fallible reservations are enforced. Drop-call tests must not claim physical erasure proof or inspect freed memory |

Implementation completion requires exact bytes/categories, meaningful mutations
of overflow, state/order, terminal padding and decoder-failure propagation to be
detected, plus formatting, Clippy and applicable normal gates at the exact source.
Dependency review and explicit bounded implementation scope come first; fresh
permission is required for heavy compilation/gates. External build/temp storage
and single-job Cargo remain mandatory. The actual result record follows.

The next stage must establish complete DER consumption and scoped nested DER
behavior, extension-error/duplicate handling, agreed SAN/non-CA/serverAuth/curve/
signature/validity policy and a positively known key match. Parsing cannot prove
issuer trust or caller identity. Stop if safe library guarantees are insufficient;
do not add application ASN.1, silently strengthen policy or weaken the contract.
Actual file/mount/alias/ingress evidence and a dependable full resume barrier
remain separate prerequisites to the container proof. This proposal completes no
A0/T01-T16 authentication case and changes no A1-A5, ADR-103 or #65 gate.

## First-slice result, 2026-10-23

The private AUTH-001 `bundle` module implements the framing/decoding contract
without a public interface or listener caller. Its intentional non-test dead-code
expectation is scoped to that module and explains the missing credential-admission
caller; integration must remove that expectation when it becomes obsolete. No
warning blanket, runtime bypass or public conversion is introduced.

The incomplete stub produced four acceptance failures before implementation.
Final standalone verification passed 13 unit tests (ten decoder, three existing)
and 14 existing real transport tests, formatting and Clippy with warnings denied.
All allowed wrapping widths 1..64 and every truncated prefix of the positive
fixture are checked. Four deliberate mutations to overflow admission, block order,
terminal padding and decoder-failure propagation caused their intended tests to
fail; original source was restored before the final passing run. This verifies
text decoding, not DER/profile/key matching or secure file admission.

The dependency review found a new advisory in the existing verifier graph; a
separate focused change updates experimental rustls-webpki 0.103.15 to patched
0.103.16. The refreshed locked graph still has 85 registry packages and returned
no package/version OSV findings. This is not a full source audit, production
remediation or a new full authentication qualification. Normal shared gate
results and exact-source hosted evidence are recorded in the implementing PR.

The next [credential-admission API comparison](local-owner-authentication-credential-admission.md)
records concrete nested-consumption behavior, candidate graph costs and a proposed
key-only round-trip guard. Additional key restrictions and certificate-policy
oracles require disposition before dependent implementation. The decoder remains
unchanged and unconnected while that stage is unresolved.
