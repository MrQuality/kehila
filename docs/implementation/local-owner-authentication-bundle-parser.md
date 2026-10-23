# A0 diagnostic bundle parser API review

Status: read-only API research, 2026-10-23; implementation and qualification pending.
Baseline: merged PR #76, `3b99eb2`. Related: [container plan](local-owner-authentication-container-plan.md),
[#40](https://github.com/MrQuality/kehila/issues/40), [#49](https://github.com/MrQuality/kehila/issues/49).

## Outcome and boundaries

Determine whether safe library APIs can admit the plan's exact version-1 bundle
before a diagnostic listener starts. This is a prerequisite to a later executable
proposal, not a parser implementation or a production dependency selection.
The maintainer explicitly accepted [ADR-106](../architecture/ADR-106-local-container-transport.md)
on 2026-10-23 for the bounded disposable proof. That separate decision does not
authorize execution, production storage, trust changes or a weaker resume rule.
AUTH-001's existing separate-DER host-only probe remains unchanged.

| Consequential premise | Evidence / status | Consequence if false |
| --- | --- | --- |
| One bounded certificate/key generation is the required input | Container plan's exact version-1 grammar, not arbitrary PEM compatibility | Revisit framing and resource limits before implementing |
| Generic PEM decoding does not establish that grammar | Inspected pki-types source below; established API difference | A direct iterator could accept forbidden material |
| Composition with safe library decoding is feasible | Candidate API shape only; complete DER/profile behavior still unqualified | Stop rather than substitute application ASN.1 or cryptography |
| Architecture acceptance is distinct from qualification | Explicit bounded ADR-106 decision; runtime gates remain | Do not describe research as installation evidence |

## Source baseline and API findings

Reviewed selected source files from published registry archives whose SHA-256
values match `experiments/AUTH-001/Cargo.lock`: rustls-pki-types 1.15.1,
rustls 0.23.46, ring 0.17.14, x509-parser 0.18.1 and data-encoding 2.11.1.
This establishes correspondence to the locked archives, not a complete source,
reachability or supply-chain audit. The latter two packages are present through
development dependencies; that does not select them for serving code.

| API / source | Useful guarantee | Missing guarantee / required caller action |
| --- | --- | --- |
| pki-types `PemObject` / iterator ([PEM source](https://github.com/rustls/pki-types/blob/e29ad15465126acc089b39611bcec85e27ce0817/src/pem.rs)) | General PEM extraction into typed wrappers | Skips preamble/unknown sections, accepts broader delimiter and CR behavior. Cannot enforce exact order, tails or all version-1 line rules by itself |
| pki-types base64 ([source](https://github.com/rustls/pki-types/blob/e29ad15465126acc089b39611bcec85e27ce0817/src/base64.rs)) | Decodes ordinary PEM payloads | Ignores whitespace, permits omitted padding and does not enforce zero unused pad bits; not the canonical-input oracle |
| `CertificateDer::from` / `PrivatePkcs8KeyDer::from` ([source](https://github.com/rustls/pki-types/blob/e29ad15465126acc089b39611bcec85e27ce0817/src/lib.rs)) | Owns or borrows typed bytes | Construction alone does not validate DER, profile or key pairing |
| data-encoding `BASE64` ([versioned source](https://docs.rs/data-encoding/2.11.1/src/data_encoding/lib.rs.html)) | Standard alphabet, padding and checked trailing bits, no ignored whitespace in this constant | Accepts concatenated padded messages. Enforce terminal padding across the whole payload as well as line grammar; decoding alone is not canonical single-message admission |
| x509-parser `parse_x509_certificate` / extension APIs ([source](https://github.com/rusticata/x509-parser/blob/33b15d2db5a19b15c17bb15fa57b08691316ee95/src/certificate.rs)) | Structured certificate and a remainder; duplicate-extension detection available | Require empty remainder and explicit profile checks. Parsing alone is not signature/path validation or proof of universal nested DER canonicality |
| rustls `CertifiedKey::from_der` / `keys_match` ([source](https://github.com/rustls/rustls/blob/f04804fa13341a9b1dc02b5d1dd8a746f2c4b7fb/rustls/src/crypto/signer.rs)) | Provider key loading and public-key comparison | `from_der` permits an unknown consistency result. Explicitly require `keys_match() == Ok(())`; mismatch and unknown must fail admission |
| ring PKCS#8 loader ([published source](https://docs.rs/crate/ring/0.17.14/source/src/pkcs8.rs)) | Outer input consumption and algorithm-specific key loading | Ignored attributes and nested encodings still need scoped review/tests; outer consumption alone is not a universal DER validator |

These are differences between a strict application format and intentionally
broader library interfaces, not findings that those libraries are vulnerable.
The rustls ring provider also accepts ECDSA P-384; provider success alone cannot
establish the agreed P-256 profile.

## Candidate composition and alternatives

Prefer a small, bounded framing adapter around library base64, certificate and
key APIs. Keep cryptography and ASN.1 decoding in reviewed libraries. The adapter
must inspect every encoded byte and enforce the existing grammar; it must not
normalize arbitrary PEM into acceptance or silently skip unknown objects.

1. Enforce the complete 64 KiB encoded limit with one overflow byte before parsing.
   File admission remains the separate [descriptor/opening review](local-owner-authentication-linux-files.md).
2. Validate ASCII, exact labels/order, line boundaries, LF/CRLF, permitted exterior
   whitespace and nonempty 1..64-character payload lines. Reject all extra blocks
   and forbidden material before handing payloads to decoders.
3. Enforce one canonical payload before library decoding: after its first `=`,
   permit only a terminal suffix of at most two `=` characters, with total length
   divisible by four; let the reviewed decoder enforce padding length and unused
   bits. Reject concatenated individually padded messages. Padded
   RFC 4648 means `=` is required only when needed; a complete four-character
   group needs none. Valid wrapping can split terminal padding across lines.
4. Validate one complete certificate and one complete unencrypted PKCS#8 object,
   with no decoded tail. Review nested DER behavior and unsupported encodings;
   do not claim completeness from a type name or an outer empty remainder alone.
5. Apply the agreed certificate/key profile and require a positive public-key
   match. Publish a generation to listener startup only after all checks pass.

Two base64 routes remain reviewable: use the existing public pki-types PEM API
behind complete framing and canonical-input checks (its base64 module is private),
or use `data_encoding::BASE64` after exact
framing and whole-payload terminal-padding checks. The second avoids hand-written
pad-bit checks and is the leading research
candidate. Promoting a development-transitive package into serving code still
requires an explicit direct dependency/feature/graph, license/notice and advisory
review. Neither route nor x509-parser is adopted by this document.

Certificate admission must distinguish malformed extensions from valid unsupported
noncritical extensions. In x509-parser, the `basic_constraints` convenience helper
can return `None` for a present extension whose parsed variant is not valid basic
constraints; absence is not proof that parsing succeeded. Check parsed extension
errors and duplicate OIDs explicitly. Do not silently add an exactly-one-SAN rule,
an extension allowlist or a new mandatory-extension policy to the agreed profile.
Any such restriction needs a separate stated decision.

Require the agreed exact `127.0.0.1` iPAddress SAN, non-CA/serverAuth leaf,
P-256 key and SHA-256 certificate-signature profile. Inspecting a leaf/key bundle
cannot establish issuer P-256 identity, trust-anchor approval or a valid chain.
Those require separately reviewed issuer/root inputs and ordinary client TLS
verification. Parsing, matching keys and startup admission do not authenticate a
caller. Validity/clock checks do not establish the provider's resume barrier.

## Ownership, error handling and test oracles

The input bound is not a total-memory bound: encoded input, joined payloads,
decoded buffers, parser objects and TLS key copies have separate lifetimes.
Budget them explicitly and avoid unnecessary copies. An explicit zeroization API
on a key wrapper does not prove every temporary/provider copy is wiped on drop.
Use sanitized typed error categories; PEM errors can retain offending input or
labels, so raw error/debug formatting is unsuitable for public diagnostics.

Before executable approval, review positive and negative cases with exact outcomes:

- LF/CRLF, end-line EOF, variable valid wrapping, full base64 groups, and padding
  split across valid lines; exact size limit and one-byte overflow before parsing.
- BOM/preamble/comments/headers, non-ASCII, lone CR, indented/trailing-space
  delimiters, blank/oversized payload lines, block reorder/duplication/unknown
  labels, encrypted/algorithm-specific keys, extra objects and non-whitespace tails.
- Missing/extra/internal padding, concatenated padded messages, nonzero unused
  pad bits, payload whitespace,
  appended DER objects, malformed nested DER and unsupported PKCS#8 variants.
- Wrong key, an unavailable public-key comparison, wrong curve/algorithm/signature
  profile, wrong SAN encoding/value, CA/EKU failures, duplicate and malformed
  extensions; valid noncritical extensions must follow the stated profile policy.
- Validity boundaries under a controlled clock, redacted failures, retained-copy
  bounds and cleanup on each failure; no listen before complete admission.

No parser tests or candidate builds were run for this review. API composition is
plausible; exact strict admission, nested DER behavior and dependency adoption
remain open for complete admission. The subsequent
[bounded adapter implementation](local-owner-authentication-parser-adapter-plan.md#first-slice-result-2026-10-23)
implements text framing/base64 with reviewed experimental direct dependencies,
ten passing decoder tests and four detected mutations. Its output remains
unvalidated; full DER/profile/key-match admission is a following stage. That
implementation does not retroactively turn this source-only review into tests.
The unresolved resume mechanism, actual mount/alias/ingress evidence, fresh
container-proof execution permission, trust approval and wider A0/T01-T16 gates remain.
