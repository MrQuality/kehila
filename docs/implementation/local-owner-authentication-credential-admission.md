# A0 credential admission: API comparison and remaining decisions

Status: bounded private profile stage implemented, 2026-10-23; runtime and full
credential qualification remain open. The first-proof key restriction and explicit
certificate markers were accepted separately as recorded below. The API comparison
below retains its research-time scope; see the implementation result at the end.
Baseline: merged PR #79, `3fd1d28`. Related: [decoder result](local-owner-authentication-parser-adapter-plan.md#first-slice-result-2026-10-23),
[container plan](local-owner-authentication-container-plan.md), [#40](https://github.com/MrQuality/kehila/issues/40),
[#49](https://github.com/MrQuality/kehila/issues/49).

## Outcome and authority

Determine how safe library APIs could turn the private decoder's unvalidated bytes
into one profile-checked certificate/key generation. The maintainer requested
continuation after PR #79 merged. This research informs the next bounded proposal;
it does not connect a decoder result to server startup or qualify credentials.
The agreed [TLS profile](local-owner-authentication-installation-plan.md#agreed-poc-tls-profile)
still requires P-256/SHA-256, a non-CA server leaf and exact loopback iPAddress SAN.

| Consequential premise | Evidence / status | Consequence and next action |
| --- | --- | --- |
| Empty outer parser remainder proves complete nested consumption | Contradicted by the existing sequence helper below | Do not use it as the sole structural admission check |
| A schema decoder automatically validates all embedded data | Unsupported: some fields are opaque or explicitly discarded | Inspect each relevant nested boundary and define negative oracles |
| Existing development packages are the smallest serving choice | Contradicted as a general inference by the graph comparison | Compare runtime reachability, not just new lock entries |
| All extra restrictions are already part of the agreed profile | Only the key-metadata rule and explicit BC/EKU markers were accepted on 2026-10-23 | Keep other acceptance rules source-backed and unresolved choices visible |

## Research snapshot: two routes and actual graph costs

Metadata-only copies of AUTH-001 retained its current lock and added these exact
candidate declarations. No package code was executed. Both keep the existing
rustls/ring provider; neither is adopted by this comparison.

| Route | Candidate direct declarations | Registry packages / Linux normal-build tree |
| --- | --- | --- |
| Current baseline | Existing AUTH-001 dependencies | 85 / 40 |
| A: existing inspection stack | x509-parser `=0.18.1`, defaults off, `validate` | 85 / 66 |
| B: schema decoding | x509-cert `=0.3.0`, pkcs8 `=0.11.0`, sec1 `=0.8.0`, all defaults off | 94 / 49 |

The tree comparison targets `x86_64-unknown-linux-gnu`, includes normal/build edges
and excludes the root package and development edges. These counts are graph
measurements, not binary-size, performance or platform-build evidence. Route B's
resolved additions are x509-cert 0.3.0, pkcs8 0.11.0, sec1 0.8.0, der 0.8.2,
der_derive 0.8.0, spki 0.8.1, const-oid 0.10.2, flagset 0.4.7 and base64ct 1.8.3.
The full graph still includes existing fixture dependencies. Test-inclusive
feature unification must not be reported as the minimal serving feature set.

Route B requests no PEM, builder, key-encryption or alternative crypto backend.
Its resolved der enables alloc/derive/flagset/oid through x509-cert; pkcs8 and sec1
have no selected features. Direct declarations plus the resolved graph need a
locked adoption review. The candidate 94-package OSV query returned no findings
on 2026-10-23; it includes Route A's unchanged package/version set. This is a
point-in-time package query, not source/reachability clearance. Complete license
texts/notices, feature/platform/toolchain behavior and maintenance review remain
required before adoption; a manifest's license label is insufficient.

## Decisive source findings

- **A can discard inner remainder.** [der-parser 10.0.0](https://github.com/rusticata/der-parser/blob/79ec2d0cfe1e3ba2deb49ee1f68f984f4ef3a971/src/der/multi.rs)
  `parse_der_container` discards the callback's `_rest`. Its defined-sequence
  helper is used by [x509-parser certificate/TBS parsing](https://github.com/rusticata/x509-parser/blob/33b15d2db5a19b15c17bb15fa57b08691316ee95/src/certificate.rs).
  A successful result with no outer tail can therefore leave inner fields
  unexamined. The structure validator inspects the resulting fields; it does not
  recover discarded bytes. An outer `all_consuming` wrapper cannot repair this.
- **B checks typed boundaries explicitly.** [der 0.8.2](https://github.com/RustCrypto/formats/blob/7637997c168f710f02ae4037a9f134c31dc8f9ff/der/src/reader/slice.rs)
  checks nested completion; `Decode::from_der` also calls `finish` at the outer
  boundary. Unmatched context-specific tags are left unconsumed in this resolved
  version, rather than silently skipped. This does not decode opaque payloads.
- **Extension checks remain explicit.** [x509-cert 0.3.0](https://github.com/RustCrypto/formats/blob/c478c6bb0db65677e85e2c0297962817eed90352/x509-cert/src/certificate.rs)
  `get_extension<T>` checks duplicates for the requested OID and decodes its
  value with `T::from_der`. Check duplicate OIDs globally too; untouched extension
  OCTET STRINGs are not semantically validated by the certificate schema.
- **Typed PKCS#8 still discards fields.** [pkcs8 0.11.0](https://github.com/RustCrypto/formats/blob/a582243775ec15a53ac1797b679e63344d0104d1/pkcs8/src/private_key_info.rs)
  reads attributes as an opaque sequence and ignores additional context-specific
  fields. Encoding omits those fields. Its private-key OCTET STRING also remains
  opaque until separately decoded. Outer consumption alone is insufficient.
- **Inner key and provider checks are separate.** [sec1 0.8.0](https://github.com/RustCrypto/formats/blob/47a3448f8c59fd35f48075a6014123cfd6968020/sec1/src/private_key.rs)
  exposes typed EC-key decoding. The existing [ring ECDSA loader](https://docs.rs/crate/ring/0.17.14/source/src/ec/suite_b.rs)
  uses PKCS#8 V1, requires the embedded EC public key and checks its consistency.
  This does not establish a match to the certificate. Explicitly require
  [rustls `keys_match() == Ok(())`](https://github.com/rustls/rustls/blob/f04804fa13341a9b1dc02b5d1dd8a746f2c4b7fb/rustls/src/crypto/signer.rs),
  rejecting unknown as well as mismatch. Provider success alone also permits
  curves outside the agreed P-256 profile.

These are scoped API behaviors, not claims that either library family is
vulnerable. Selected published sources were inspected; no complete source audit
or empirical malformed-DER acceptance result is claimed.

## Recommended candidate and accepted first-proof restriction

Prefer Route B for the next bounded feasibility scope: its typed completion
checks address the known structural gap with fewer Linux normal/build graph
entries than Route A. Nine new registry entries are still a real review cost.
Keep the decoder private and unchanged while this candidate remains unqualified.

One proposed way to reject the PKCS#8 fields discarded by its decoder is to decode
and re-encode the **outer key structure only**, into bounded zeroizing scratch,
then require byte equality with the original key bytes. Its encoder omits
attributes/additional fields, so their presence would fail this guard. This is a
source-supported candidate, not a tested guarantee or universal DER validator.
Decode the embedded SEC1 structure and algorithm parameters separately; retained
opaque bytes can round-trip unchanged without being valid.

**Explicit maintainer decision, 2026-10-23:** reject keys with optional attributes
or extra fields for this first disposable proof. The maintainer supported this
recommendation when directing PR #80's merge and continuation; acceptance is
separate from the merge itself. Some rejected keys may be valid PKCS#8: report
unsupported profile rather than asserting malformed DER. The round-trip mechanism
still requires actual tests; a mismatch does not identify its exact cause.
State the ring loader's V1/embedded-public-key compatibility limits; do not claim
all unencrypted PKCS#8 variants.

**Separate explicit certificate decision, 2026-10-23:** require BasicConstraints
to be present with `ca=false` and EKU to be present and include `serverAuth`.
The maintainer chose both explicit markers after being told that ordinary TLS
can accept some certificates when these extensions are omitted. This applies only
to the first disposable proof. A present BasicConstraints extension can encode
its DEFAULT false value by omitting the inner BOOLEAN; an explicitly encoded
false DEFAULT is not required. No exactly-one-SAN rule or noncritical-extension
allowlist was accepted. This grants no trust or integrated-proof execution permission.

Do not apply byte-equality re-encoding to certificates: schema/profile encoding
can normalize representations and would introduce unrelated restrictions.
Use supplied bounded time/policy inputs; library key loading may use OS randomness
and can fail. Do not describe the whole admission stage as pure text processing.
Account separately for decoded buffers, schema-owned data, zeroizing encode scratch
and provider copies; neither a byte ceiling nor wrappers prove total RAM/erasure.

## Research-time policy and test gates

| Gate | Required resolution / oracle before complete admission |
| --- | --- |
| Certificate structure | Reject outer tails, extra fields inside Certificate/TBS, invalid lengths/tags/default encodings and malformed required nested structures. Test resolved library behavior, not just API names |
| Extensions | Reject global duplicate OIDs and malformed checked extensions. Specify handling of opaque noncritical values and unsupported critical extensions; do not invent an extension allowlist or exactly-one-SAN rule |
| Agreed profile | Exact iPAddress match, non-CA/serverAuth and P-256/SHA-256, including inner/outer signature identifier consistency and curve parameters. Require the explicitly accepted BC/EKU markers; do not reinterpret an absent extension as present |
| Time | Invalid intervals, not-yet-valid/expiry boundaries, supplied clock errors and existing expiry/drain limits need exact expected outcomes. These checks do not establish a provider resume barrier |
| Key restriction | A producer key round-trips exactly; attributes (valid and opaque malformed contents), additional tagged fields and unsupported variants fail with the specified category. Inner SEC1 tails/unknown fields and inconsistent parameters/public components must also fail |
| Provider/pair | Wrong curve/key, loading/RNG failure, mismatch and unknown consistency all fail before constructing a profile-checked generation. Ordinary client trust/path verification and issuer-key policy remain separate |
| Resources/errors | Bounded fallible allocations, zeroizing scratch on encoding/error paths and sanitized categories; no raw parser/provider Debug. Do not claim physical erasure or dynamically tested allocator failures without evidence |

Next verify the accepted restrictions and source-backed policy oracles through
reviewed dependency adoption and bounded private admission implementation.
If safe library guarantees remain insufficient, stop rather than add application
ASN.1/cryptography or weaken the contract. No build, fixture/key generation,
admission test, listener integration, VM/service start or trust change occurred.
The actual file/mount/alias/ingress and full resume gates remain unresolved;
A0/T01-T16, A1-A5, ADR-103 and #65 gain no qualification. #40's separate production
dependency disposition and 2026-11-18 review deadline remain unchanged.

## Private implementation result, 2026-10-23

The private `bundle::admission` stage now consumes `DecodedBundle` and a supplied
`UnixTime`, returning an unexported `ProfileCheckedGeneration`. It has no listener,
file, system-clock or production caller. Existing examples retain their separate
DER-input path. The accepted key/BC/EKU restrictions apply only to this bounded
first-proof profile; implementation does not broaden their acceptance.

The locked Route B adoption and actual features/licenses/advisory scope are in
[DEPENDENCIES](../../experiments/AUTH-001/DEPENDENCIES.md). Compilation required
`sec1/der`, which also enables its zeroization feature; the research declaration
without that feature was insufficient. The graph remains 94 registry packages.

The stage checks typed certificate/TBS completion, V3, matching inner/outer
ECDSA-SHA256 identifiers with absent parameters, a P-256 subject key, required
non-CA BasicConstraints and serverAuth EKU, exact loopback IP SAN, global duplicate
extension OIDs and supplied-time validity. If KeyUsage is present it must permit
digital signatures and forbid certificate signing. Extra supported SANs/EKUs are
allowed. Unsupported critical extensions reject; unknown noncritical values need
one complete library-decoded DER envelope, with their inner semantics opaque.

A separate library-reader check requires exactly two nonnegative DER INTEGERs
inside the ECDSA signature sequence, with complete inner/outer consumption. It
does not verify scalar ranges, signature mathematics, issuer policy or trust.
A regression deliberately accepts a syntactically shaped, unauthenticated issuer
signature so callers cannot mistake this type for an authenticated certificate.

Typed decoding alone hid explicitly encoded DEFAULT values and discarded
undefined KeyUsage bits. Four regression cases failed before the repair:
explicit BasicConstraints false, explicit extension critical false, undefined
KeyUsage bits and nonminimal named-bit encoding. After typed certificate decoding,
schema-aware library orchestration projects original extension wrappers and
matches their presence/count/order to the typed list. Bounded re-encoding equality
checks apply to those wrappers and interpreted BasicConstraints/KeyUsage values.
No whole certificate/TBS re-encoding or application tag/length decoder is used.
The projection relies on the preceding typed schema check; it is not a second TBS
validator or a universal validator for opaque extension contents.

The key stage separately checks PKCS#8 V1, EC/P-256 identifiers, inner SEC1
completion/parameters and embedded public key. Outer-key re-encoding equality
rejects discarded metadata as unsupported profile. The fixed ring provider loads
a borrowed protected key; rustls positive `keys_match()` is required, rejecting
unknown as well as mismatch. Errors are static categories without parser/provider
Debug or secret values. Application-owned encoding scratch uses bounded fallible allocation and zeroizing
storage; other application collections reserve fallibly. Provider/schema
allocations and copies remain outside any universal allocation/erasure guarantee.

Windows Rust/Cargo 1.93.0 verification passed formatting, clippy with warnings
denied, **37 unit tests and 14 real transport tests**. Fixtures were generated only
in test memory. The unit total includes ten decoder, seven key and seventeen
certificate cases alongside the original three transport-policy cases. Nine
behavior mutations were detected: key metadata, key curve, BC presence, IP
identity, expiry, unknown pair consistency, raw extension wrapper, typed default
and signature shape. Source was restored before the final passing run.

Remaining limits and gates:

- Validity is a supplied-time snapshot with inclusive certificate endpoints and
  invalid-interval rejection. No clock acquisition, clock-error injection,
  continuous expiry/drain enforcement or provider resume barrier is implemented.
- The selected GeneralName schema does not support x400Address. This is a stated
  compatibility limit, not an accepted general SAN restriction or evidence that
  such certificates are invalid. Unknown noncritical contents remain opaque.
- Issuer-key policy, ordinary client path/trust verification and full credential
  authenticity remain separate. Real allocator/RNG failure injection and a full
  minimum-Rust-version build were not executed.
- Descriptor-based Linux file admission, mount/alias/ingress behavior, renewal,
  expiry/drain/resume and integrated Windows-client/Podman-backend qualification
  remain open. No trust-store or security-setting changes were made.

The next bounded implementation proposal should address Linux file admission
using the [reviewed API contract](local-owner-authentication-linux-files.md),
while retaining the resume gate before any integrated proof. A0/T01-T16, A1-A5,
ADR-103 and #65 receive no complete qualification; #40's separate production
dependency disposition and 2026-11-18 deadline are unchanged.

## PR #81 review follow-up, 2026-10-23

The existing inner-key contract test now also encodes an explicit matching
SEC1 P-256 parameter, requires successful provider loading and checks that the
public identity equals the original key's. Wrong-curve and missing-public-key
rejections remain covered. A targeted mutation rejecting the present matching
curve made this positive assertion fail; source was restored. This extends one
existing test, so the suite remains 37 unit and 14 transport tests. The final
standalone run passed all 51 tests, formatting and clippy with warnings denied.

Local documentation on `ProfileCheckedGeneration` and `admit` now states directly
that profile/key matching does not establish issuer-signature authenticity or
certificate trust. TLS client path verification and ongoing expiry/resume remain
separate requirements. This follow-up changes neither admission policy nor API
visibility, and grants no new integration or trust permission.

## Development-proof lifecycle update, 2026-10-24

[ADR-106 now records the accepted interruption/rerun concession](../architecture/ADR-106-local-container-transport.md#development-proof-interruption-amendment-2026-10-24).
Any observed or suspected host/VM pause invalidates the complete disposable run;
stop/reconcile and rerun from a fresh instance. Earlier references in this dated
research/result to the full diagnostic resume barrier describe the prior gate.
This amendment supplies no automatic pause detection, safe-file evidence,
supported authentication/suspend qualification or integrated-test permission.
