# Product authority dependency adoption

Status: source, license and package-advisory assessment; integrated execution and
release qualification remain pending. Owner: project maintainer. Reviewed
2026-11-03, with the package query at 2026-11-02 23:56 UTC.

## Purpose and graph

The product-owned [manifest](../../src/io/kehila_authority/Cargo.toml) adopts the
bounded authentication foundation and adds the pure project domain and storage
codecs. The workspace lock records exact registry inputs. This is the product
graph; the isolated AUTH-001 lock is historical regression evidence.

| Dependency family | Responsibility and adoption boundary |
| --- | --- |
| Argon2/password-hash, zeroize, subtle | Owned bounded password verification/replacement and protected buffers; no claim of erasing allocator history or all library copies. |
| rustls/ring, Tokio-rustls | Explicit TLS provider and normal certificate validation; no insecure verifier or automatic client trust installation. |
| Tokio, Hyper/Hyper-util, Bytes, http-body-util | Owned async sockets, deadlines and bounded HTTP/1 exchanges. Hyper client support is test-only in this manifest. Feature unification in the workspace must still be reviewed; disabling defaults on one edge does not disable another edge's features. |
| x509-cert, pkcs8, sec1, data-encoding | Typed certificate/key/encoding admission with retained canonical-encoding checks. Parsing is not proof of key policy or transport trust. |
| rustix | Linux descriptor-relative protected-file admission and effective identity checks; no Windows backend is introduced. |
| tokio-postgres and rustls adapter | Native PostgreSQL transport, owned drivers, dedicated generation/advisory authority and private serving connection. No MongoDB dependency enters this authority. |
| serde/serde_json | Closed authentication messages and bounded project JSON through the pure codec parser. Duplicate, unknown, missing and null handling remain application contracts. |
| unicode-normalization | Bounded NFC password normalization; changing Unicode data requires known-answer revalidation. |
| rcgen/time | Development certificate fixtures only; no production CA or supported installer. |

The direct runtime and credential versions retain the reviewed experimental
selection except that product Bytes **1.12.1** and http-body-util **0.1.5** retain
the newer existing workspace versions. The codec and snapshot checker move from
serde_json **1.0.151** to **1.0.154** to resolve their exact-pin conflict with the
authority. The parser's [upstream comparison](https://github.com/serde-rs/json/compare/v1.0.151...v1.0.154)
and the prior [HTTP adoption review](../../experiments/AUTH-001/DEPENDENCIES.md)
are compatibility inputs; codec fixtures, ingress cases and worker regressions
must run against the resulting workspace graph.

The resolved Linux-filtered authority plus development closure contains **160
packages, including 157 registry entries and three project packages**. This count
is tied to this resolution and is not a count of tests or runtime-only packages.
Declared package requirements do not exceed Rust 1.88. The authority declares
1.88; the root workspace declaration remains 1.75. Execution uses the pinned
verification toolchain **1.99.1**, so complete minimum-version execution is not
qualified by these observations.

## Licenses and distribution

Actual packaged manifests, license texts and bundled notices were inspected for
the selected closure. [Retained notices](../../src/io/kehila_authority/THIRD-PARTY-NOTICES.txt)
group identical texts with package/version/source attribution. Include them when
distributing authority binaries; a crate's top-level SPDX label is insufficient.

Select Apache-2.0 where offered; select MIT for byteorder/memchr's Unlicense
alternatives and packages offering only MIT. Retain sole ISC grants for
rustls-webpki/untrusted and BSD-3-Clause for subtle. Ring's Apache/ISC obligations
and bundled BoringSSL, once_cell and fiat notices remain intact; no single
Apache selection removes its mandatory ISC portions.

Explicit compatibility review found foldhash 0.2.0's sole **Zlib** license
compatible: preserve its notice, do not misrepresent origin and mark modified
source versions. Unicode-ident and unicode-normalization select the Apache code
alternative **and mandatory Unicode-3.0**: retain the Unicode copyright/permission
notice and respect its restriction on promotional use of copyright-holder names.
These selections preserve upstream terms; they do not relicense third-party
material or establish a policy exception.

asn1-rs-impl 0.2.0 omits packaged license files. Its packaged VCS record identifies
commit `a20e5f7319c896737ad0f2557037817b91ad854f`; the retained Apache text comes
from that [exact upstream revision](https://github.com/rusticata/asn1-rs/blob/a20e5f7319c896737ad0f2557037817b91ad854f/LICENSE-APACHE).
Additional packaged alternative texts are retained without changing selections.

## Advisory and maintenance disposition

A fresh [OSV package/version query](https://google.github.io/osv.dev/post-v1-querybatch/)
covered the 259 registry entries returned by the workspace metadata resolution.
The selected authority's **157 registry entries returned no findings**. This is
a dated package query, not a source audit, image scan, absence-of-vulnerabilities
claim or result for a different platform/feature graph. The verification image
does not have cargo-audit installed; no cargo-audit pass is claimed.

The same query still reports findings in the retained legacy worker graph:
MongoDB 2.8.2, derivative 2.2.0, idna 0.2.3, rustls 0.21.12, rustls-pemfile 1.0.4,
rustls-webpki 0.101.7 and webpki-roots 0.25.4. They are outside the authority closure.
This does not clear their [existing triage obligations](local-owner-authentication-a0.md#advisory-follow-up)
under [#40](https://github.com/MrQuality/kehila/issues/40) and
[B-021](../product/backlog.md#b-021). In particular, the
[MongoDB TLS option advisory](https://osv.dev/vulnerability/GHSA-3p6w-gv5g-xjw9)
requires source/reachability disposition: the legacy store parses an operator's
connection URI and this review has not proved the affected option unreachable.
Do not expose that prototype as the authenticated native product path or treat
test use of a plaintext disposable database as a production TLS disposition.

The selected upstream TLS/runtime/HTTP, RustCrypto, parser and PostgreSQL projects
retain the upstream provenance and maintenance review recorded in the
[foundation review](../../experiments/AUTH-001/DEPENDENCIES.md). This carries its
bounded observations, not a support guarantee or a fresh audit of every upstream
source file. Monthly and immediate security-notice review under
[DEP-001](../engineering/DEPENDENCIES.md) remains owned by the maintainer; next
routine graph review is due **2026-12-03**, with earlier advisory-driven action.
Abandonment notices, source/license changes and new findings require individual
disposition. Full release and supported-installation claims remain outside this
development integration PR.
