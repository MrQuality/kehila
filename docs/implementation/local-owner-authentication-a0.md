# A0 - Local HTTPS and dependency investigation

Status: Host Windows transport/browser subset observed; broader A0 gates remain open.
Owner: project maintainer. Recorded: 2026-10-19. Related #40/#49 W2;
[Accepted local-MVP ADR-105](../architecture/ADR-105-local-owner-authentication.md),
[approval](https://github.com/MrQuality/kehila/issues/40#issuecomment-6262900825),
[delivery plan](local-owner-authentication-verification.md).

## Bounded scope

Investigate secure local transport before authentication implementation. Work is
limited to disposable certificate fixtures, a Rust HTTPS transport probe and a
CLI client, actual browser trust/origin observations, dependency review and cleanup.
Use a 90-minute active experiment budget excluding dependency fetch/build, required
gates and permission waits; stop on failed prerequisites or inconclusive outcomes
and record them. This does not authorize A1-A5, a public auth route, a database
schema, real credentials, native production adoption or replicated deployment.

The probe must bind literal 127.0.0.1, expose only a closed diagnostic response,
use bounded connections/headers/handshake/body/total deadlines and own all tasks
through cleanup. Use a reviewed HTTP library rather than a new hand-written HTTP
parser. Existing actor/codec rules remain future integration obligations, not
properties demonstrated by a transport-only response.

The [remaining container design/probe proposal](local-owner-authentication-container-plan.md)
addresses connection, key delivery and lifecycle prerequisites under ADR-106,
explicitly accepted on 2026-10-23 for the bounded disposable proof. It supplies
no new execution evidence or permission; the host-only
experiment and broader A0-A5 gates retain their scope.

## Preliminary dependency review - no adoption yet

The existing lockfile contains rustls 0.21.12 / tokio-rustls 0.24.1 transitively,
ring 0.17.14, Tokio 1.53.1, Axum 0.8.9 and Hyper 1.11.1. Their presence alone does
not select them as production security dependencies. The isolated experiment has
its own manifest/lockfile; the worker graph is unchanged. Production password/NFC/randomness/DPAPI dependencies
remain review work; do not install an auth framework to prove HTTPS.

| Candidate / source checked 2026-10-19 | Initial assessment and adoption condition |
| --- | --- |
| [rustls 0.23.46](https://docs.rs/rustls/0.23.46/rustls/) / [versioned manifest](https://github.com/rustls/rustls/blob/v/0.23.46/rustls/Cargo.toml) | Candidate for explicit client/server TLS, normal certificate verification and disabled key logging. Upstream manifest declares Rust 1.71; that is not a qualified complete-graph MSRV. Select Apache-2.0 from the declared alternatives; [actual license text](https://github.com/rustls/rustls/blob/v/0.23.46/LICENSE-APACHE) inspected. Full resolved-package license/notice and advisory review still required. |
| [Tokio-rustls](https://github.com/rustls/tokio-rustls) plus existing HTTP/runtime family | Candidate for owned asynchronous deadlines and integration with existing Tokio/Hyper. Resolve a published compatible version, inspect its actual source/license and feature graph, and pin the experiment separately before building. Do not infer crate publication from a main-branch manifest or update the production worker graph incidentally. |
| [ring](https://github.com/briansmith/ring) versus rustls default aws-lc-rs | Compare actual graph, bundled/native-code notices, build tooling, platform support and advisory exposure. Default rustls selects aws-lc-rs; disabling defaults can select ring explicitly. Existing ring in the repository is not a completed provider review. No FIPS or post-quantum claim. |
| [rcgen](https://github.com/rustls/rcgen) | Candidate for disposable test-only certificate generation, not a production CA/installer. Confirm published version, license/notice files and resolved time/crypto dependencies before adoption. Never publish generated keys or trust test roots automatically. |
| [rustls-native-certs](https://github.com/rustls/rustls-native-certs) / explicit client trust roots | Compare actual Windows/browser trust behavior with a narrowly configured CLI root set. Root selection is trust policy; it must not turn any self-signed certificate into an authenticated server. |
| [mkcert](https://github.com/FiloSottile/mkcert) / Windows certificate tooling | Operational candidates only, not selected or installed. Development certificate convenience is not proof of a supported production trust/renewal/removal workflow. Review provenance, license, key ACLs and exact store changes before use. |

The machine reports rustc/cargo 1.93.0. The workspace declares 1.75 while the
worker declares 1.88; neither a candidate's declared minimum nor current-toolchain
success qualifies the complete graph at the repository minimum. Record the actual
probe toolchain and review any package-specific minimum explicitly.

### Advisory follow-up

The [RustSec rustls register](https://rustsec.org/packages/rustls.html) contains
recent advisories, including [RUSTSEC-2026-0346](https://rustsec.org/advisories/RUSTSEC-2026-0346):
its affected range includes the existing 0.21.12; 0.23.46 is listed as patched.
That issue concerns custom client session stores formatting secret values with
Debug; ordinary built-in cache Debug output is stated unaffected. Package presence
is not proof that this repository exposes the vulnerable path. Static inspection
of project source plus the locally cached MongoDB 2.8.2 and tokio-rustls 0.24.1
TLS adapters found no custom client session store or secret-value Debug formatting;
the driver retains the built-in cache. This supports no observed path for this
specific advisory, not a complete advisory scan or runtime security proof. The
maintainer/#40 owns remaining graph/reachability disposition before A0 adoption
and medium review no later than 2026-11-18; do not suppress findings or
claim an unavailable complete-graph scan was clean. Review every resolved advisory,
not only this example, and keep the existing worker unchanged unless a separately
scoped fix is justified. No TLS/key/session material may enter evidence logs.

**New finding, 2026-10-26:** the
[Linux reader dependency research](local-owner-authentication-linux-files.md#dependency-research-2026-10-26)
queried all 98 registry entries in its metadata-only candidate and matched
[RUSTSEC-2026-0386](https://rustsec.org/advisories/RUSTSEC-2026-0386.html) against
existing Tokio 1.53.1 in both committed locks. The advisory concerns AsyncFd
constructor I/O unsoundness with potential memory corruption and provides no
CVSS/severity rating. Direct `src/`/AUTH-001 searches found no AsyncFd caller;
that is not transitive reachability evidence or an accepted risk disposition.
The maintainer/#40 owns priority-P1 triage, with a proposed next-review target of
**2026-11-02**, and a focused compatible patch proposal (the advisory lists
>=1.53.3 patched on this line),
with separate review and required verification before adoption. No patch,
suppression or risk acceptance is introduced here. The rustls 0.21.12 follow-up
and its 2026-11-18 deadline remain unchanged; the earlier point-in-time clean
package observations do not supersede this newer finding.

## Remaining authentication dependency candidates

These are API-level reconnaissance on 2026-10-19, not added dependencies or
production selections. Their resolved graphs, actual license texts, advisories,
maintenance and target behavior need review before the relevant A2/A3 adoption.
They do not block review of the isolated transport result.

| Boundary / primary source | Assessment and required adoption evidence |
| --- | --- |
| [RustCrypto argon2 0.6.0](https://docs.rs/argon2/0.6.0/argon2/) | Candidate for explicit Argon2id/v19 and PHC encoding. Upstream verification uses parameters from the parsed hash, so parsing a stored verifier is not admission: enforce approved parameter/encoded bounds before allocating or scheduling hashing. Measure the complete verify-plus-rehash operation under the one-job permit; do not use library defaults as the ADR policy. |
| [unicode-normalization 0.1.26](https://docs.rs/unicode-normalization/0.1.26/unicode_normalization/) | Candidate for NFC iterators with a recorded Unicode data version. Bound raw input before normalization and enforce normalized bounds afterward. Stream-safe normalization can insert joiners and is not a substitute for the accepted NFC password policy. Known-answer and version-upgrade tests remain required. |
| [getrandom 0.4.3](https://docs.rs/getrandom/0.4.3/getrandom/) | Candidate for system randomness. Retain failure as an error with no timestamp, deterministic or weak fallback. Review the actual selected Windows/Linux backend and feature graph before generating real credentials; an older transitive instance in the TLS graph is not a new direct selection. |
| [sha2 0.11.1](https://docs.rs/sha2/0.11.1/sha2/) and [subtle 2.6.1](https://docs.rs/subtle/2.6.1/subtle/trait.ConstantTimeEq.html) | Candidates for token fingerprints and fixed-size constant-time comparisons. A digest does not authenticate a caller or establish actor/tenant authority. Credential kind, format, expiry and routing remain explicit checks; ordinary equality on secret values is not an equivalent comparison contract. |
| [zeroize 1.9.1](https://docs.rs/zeroize/1.9.1/zeroize/) | Candidate for owned secret buffers; already present transitively in the probe. Review copies, string formatting, allocator ownership, cancellation and FFI buffers. A dependency or drop wrapper alone cannot prove that all secret copies disappear. No real credential-handling claim is made by this experiment. |
| [windows-dpapi 0.2.0](https://docs.rs/windows-dpapi/0.2.0/windows_dpapi/) / protected files | The documented wrapper exposes User/Machine scope through a safe API, making it a candidate rather than justification for raw application FFI. User scope, UI suppression, returned-buffer ownership, error handling, maintenance/license review and actual target tests remain required. DPAPI alone does not implement owner-only file creation, no-redirection semantics or atomic replacement. No storage wrapper is selected yet; #40 owns this gate before A3. |

## Execution and evidence gates

1. Resolve a disposable probe graph only after dependency provenance/features and
   actual license texts/notices are checked; lock exact inputs. Dependency
   download/compilation is recorded in the isolated experiment. Qualify with real certificate validation,
   never a permissive verifier, ignore-certificate switch or browser warning bypass.
2. Generate unique fixture roots/leaves outside the checkout with restricted
   key access. Before trust changes, show the root's identity/fingerprint, exact
   per-user store/profile, privilege scope and cleanup command for separate approval.
   Preserve existing trust material; no machine-wide root installation by default.
3. Prove CLI trust, exact IP SAN and expiry rejection; mismatched keys/missing files
   must fail startup. Demonstrate foreign/missing origin and altered Host rejection
   at the HTTP boundary, independently of TLS validity. Reject plaintext/downgrade.
4. With separately approved browser trust setup, observe real browser success and
   failure for trusted/untrusted/wrong-SAN/expired certificates and actual origin/
   cookie bootstrap headers. A CLI-only test or mocked HTTP response cannot pass
   browser qualification. Record browser/OS/runtime and certificate identity.
5. Replace/renew the leaf, restart safely, then remove only the approved test trust
   entry and confirm rejection again. Verify process/task/port/key-file/store cleanup.
   Windows/Podman and Linux results remain separately qualified; no VM is needed
   merely to start the first host-only transport probe.

Memory-heavy compilation/probes and required commit gates need separate permission
and fresh memory preflight. Shared verification may need disposable services; the
first TLS probe itself needs no PostgreSQL, search, broker or user data. Do not
start those services merely for certificate preparation. Certificate trust-store
changes always retain their separate permission gate, even after memory approval.

## Current result and next action

Deployment correction, 2026-10-21: the backend target is a Linux container.
Windows-host transport evidence does not establish native Windows backend
placement. Next reconcile the actual container/TLS/secret boundary before further
platform-specific server-key work; see the [post-mortem](../postmortems/2026-10-21-platform-assumption.md).
Windows CLI DPAPI and browser trust are separate concerns. The agreed PoC TLS
inputs and prior evidence remain unchanged; no Linux storage qualification follows.

The [standalone AUTH-001 probe](../../experiments/AUTH-001/README.md) records the
observed Windows subset: 17 unit/transport tests, six actual headless Chrome
certificate cases, missing-file startup rejection, bounded resource/shutdown
checks and a Host-admission mutation. Temporary user trust and TLS private keys
were removed after the browser checks. No certificate-ignore mode was used.

The later [text-decoding slice](local-owner-authentication-parser-adapter-plan.md#first-slice-result-2026-10-23)
adds ten passed decoder tests and four detected mutations. It has no listener
caller and validates no DER, certificate policy or key match. Its current complete
standalone set has 13 unit and 14 transport tests; it adds no new browser evidence.
The dependency review also records the separately patched experimental verifier
advisory; the original graph assessment remains historical.

The [locked dependency review](../../experiments/AUTH-001/DEPENDENCIES.md) covers
85 registry packages, actual selected license texts and retained notices. The
recorded OSV query returned no findings for those package/version pairs; it is
not a source/reachability audit or qualification of the existing worker. The
production worker manifest and lockfile are unchanged. The standalone Linux
CI check has been added; its actual run must pass before recording Linux evidence.

This does not complete all A0 gates. Authentication-library adoption, protected
storage/DPAPI, supported installation, browser cookie/bootstrap behavior and the
Podman/platform profile remain unqualified. The next step is review of the
[Proposed installation and protected-storage plan](local-owner-authentication-installation-plan.md),
including runtime ownership, certificate lifecycle, file replacement and failure
oracles. Its review refinements require issuer-key ownership, explicit server-key
storage candidates, measurable expiry/drain criteria, the renewal/restart logout
consequence and interrupted-run trust reconciliation. These are planning
requirements, not executed evidence. It authorizes no experiment or dependency adoption. A1 schema/capability
review and A1-A5 implementation retain their separate gates. Do not infer
authentication support from a closed transport diagnostic response.

On 2026-10-20 the maintainer agreed to the plan's
[bounded PoC TLS inputs](local-owner-authentication-installation-plan.md#agreed-poc-tls-profile):
a disposable private CA, ordinary server validation, TLS 1.3 with reviewed AEAD
suites, an initial ECDSA P-256/SHA-256 certificate profile and exact loopback IP
SAN. Certificate fees are excluded. The OWASP basis and RFC 9525 naming rationale
are recorded there. This selects PoC inputs only; tooling/storage selection,
execution, exact trust changes and broader platform qualification remain gated.
