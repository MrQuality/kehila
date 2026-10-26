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

At the preliminary 2026-10-19 review, the existing lockfile contained rustls 0.21.12 / tokio-rustls 0.24.1 transitively,
ring 0.17.14, Tokio 1.53.1, Axum 0.8.9 and Hyper 1.11.1. Their presence alone does
not select them as production security dependencies. The isolated experiment has
its own manifest/lockfile; that investigation did not change the worker graph. Production password/NFC/randomness/DPAPI dependencies
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

#### Root TLS advisory triage, 2026-10-26

A targeted OSV package/version query of three entries in the root lock confirmed
these additional findings. This is not a complete scan of its 228 registry
entries, a fresh AUTH-001 scan or proof of application exploitability.

| Root locked package | Confirmed advisory / condition to assess | Open disposition work |
| --- | --- | --- |
| rustls-webpki 0.101.7 | [0382](https://rustsec.org/advisories/RUSTSEC-2026-0382.html): distrusted intermediate paths; [0098](https://rustsec.org/advisories/RUSTSEC-2026-0098.html) / [0099](https://rustsec.org/advisories/RUSTSEC-2026-0099.html): URI/wildcard name constraints; [0104](https://rustsec.org/advisories/RUSTSEC-2026-0104.html): malformed CRL parsing. | Trace the selected verifier, trust anchors, name constraints and CRL use; assess a compatible driver/library upgrade and retain per-advisory exposure conclusions. The CRL advisory does not affect applications that do not use CRLs. |
| webpki-roots 0.25.4 | [0381](https://rustsec.org/advisories/RUSTSEC-2026-0381.html): bundled emSign trust anchor can permit distrusted intermediate paths. | Trace built-in versus configured trust roots and select a reviewed compatible replacement/update. Package presence alone does not establish the deployed trust path. |
| rustls-pemfile 1.0.4 | [2025-0134](https://rustsec.org/advisories/RUSTSEC-2025-0134.html): unmaintained package, with no patched version. | Trace driver PEM-loading use and assess maintained parsing/driver alternatives. This is a maintenance notice, not a demonstrated exploitable vulnerability. |

The maintainer owns all three package dispositions under [#40](https://github.com/MrQuality/kehila/issues/40)
and [B-021](../product/backlog.md#b-021), with **P1 triage** and next review
**2026-11-02**. Application vulnerability severity and deployment exposure remain
unclassified. The Medium assessment-gap rating concerns missing dispositions;
it does not rate each vulnerability or downgrade upstream findings. No risk
acceptance, scanner suppression or dependency remediation is introduced here.
The separate rustls 0.21.12 / 0346 review deadline remains **2026-11-18**.

#### Tokio patch and verification, 2026-10-26

**New finding, 2026-10-26:** the
[Linux reader dependency research](local-owner-authentication-linux-files.md#dependency-research-2026-10-26)
queried all 98 registry entries in its metadata-only candidate and matched
[RUSTSEC-2026-0386](https://rustsec.org/advisories/RUSTSEC-2026-0386.html) against
existing Tokio 1.53.1 in both committed locks. The advisory concerns AsyncFd
constructor I/O unsoundness with potential memory corruption and provides no
CVSS/severity rating. Direct `src/`/AUTH-001 searches found no AsyncFd caller;
that is not transitive reachability evidence or an accepted risk disposition.
The [four/ten immediate-consumer inventory](local-owner-authentication-linux-files.md#dependency-research-2026-10-26)
bounds the initial triage scope. Unix `tokio/net` enables the relevant API; neither
that availability nor a lockfile edge proves an affected constructor is reached.
**Focused patch, 2026-10-26:** both committed locks now select Tokio 1.53.3,
which the advisory lists as patched. AUTH-001 updates its exact direct pin;
the root retains its existing major-version requirement and uses the reviewed
lock. Cargo regenerated both locks without other package, dependency-edge or
feature changes. This disposition applies to these committed locked graphs,
not arbitrary downstream resolutions.

Source inspection found no AsyncFd reference in project sources or the `src/`
trees of the four experiment and ten root immediate consumers listed in the
research record (including their shared packages). This bounds observed exposure;
it is not a complete call-graph or runtime proof. The upstream remedy deprecates
the old safe constructors and adds unsafe registration APIs; it does not make
arbitrary descriptor ownership safe. No unsafe registration workaround is added.
The package-version update is the mitigation, rather than a reachability-based
suppression or claim that the vulnerable constructor contract is repaired for
existing callers. The new OSV query for Tokio 1.53.3 returned no findings; it
covers that package/version only, not either complete graph.

**Windows verification, 2026-10-26:** formatting and clippy with warnings denied
passed. All 37 unit tests passed in each full-suite execution. The initial
transport run passed
13/14, with an HTTP connection error in the oversized-body case; the unchanged
immediate rerun passed 14/14. A bounded three-run full-suite comparison on each
of baseline 1.53.1 and patched 1.53.3 produced two complete passes and one 12/14
transport result per version. The oversized-body case failed on both; the other
failures were renewal on the baseline and origin/Host rejection on the patch.
Twelve isolated oversized-body runs per version passed.

Those full Windows runs used
`cargo test --manifest-path experiments/AUTH-001/Cargo.toml --locked`, with the
default target selection and test harness; the isolated runs selected the
transport test and exact case. The [documented command](../../experiments/AUTH-001/README.md#run)
and Linux CI instead use `--all-targets -- --test-threads=1`. No matching serial
Windows run was performed for this patch. The network cases already hold a
shared mutex during network ownership; default harness scheduling does not imply
simultaneous network cases. The observed baseline failures predate this patch,
but neither the operating system nor the harness difference is a proven cause,
and an additional regression is not excluded. No assertion or error handling was
weakened. The client helper's
generic connection error does not distinguish an unsuccessful HTTP exchange
from a teardown failure after a collected response; further diagnosis is open
under #40, **Medium/P2 engineering reliability**, separate from advisory severity.
Do not describe Windows transport reliability as qualified. Follow-up acceptance
under #40 is bounded, sanitized stage/error information that preserves both the
HTTP exchange and connection-driver outcomes. A collected response must not
silently erase a connection failure, and a generic driver error must not hide
whether the exchange completed. Investigate their cause before changing teardown
or relaxing any response/closure assertion.

The [shared-verifier scope gap](../TESTING.md#standalone-a0-verification-boundary)
is a separate **Medium/P2** follow-up under #40 / B-019. It omits AUTH-001; the
standalone local and hosted results must remain distinct from the root staged
gate until the entry points are aligned.

Mandatory full staged verification passed on dependency commit `9bd2d62`:
292 tooling tests (289 passed, three documented Windows skips), Rust/Go/static
checks, and owned live-service tests including 48 competing worker pairs,
stalled-body/database recovery and save/replay/read with search unavailable.
Hosted Linux evidence must be checked at the patch PR's final source; this local
result does not replace it. The maintainer/#40 retains final patch review and the
Windows reliability follow-up, with next review **2026-11-02**. The separate rustls 0.21.12 follow-up and its
2026-11-18 deadline remain unchanged. No risk acceptance or authentication
qualification is implied.

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

**Status reconciled 2026-10-26:** the backend target is a Linux container, with
Windows prioritized for the MVP client. ADR-106 accepts only the bounded
disposable Podman proof; Windows-host transport evidence supplies no Linux file
or supported-installation qualification. Windows CLI and browser trust remain
separate concerns; the [platform post-mortem](../postmortems/2026-10-21-platform-assumption.md)
retains the earlier correction and evidence limits.

The latest recorded [standalone AUTH-001 suite](../../experiments/AUTH-001/README.md)
has **37 unit and 14 transport tests**. The
[private credential-admission result](local-owner-authentication-credential-admission.md#private-implementation-result-2026-10-23)
adds strict certificate/key profile checks and positive pair matching at supplied
time; it has no listener caller and does not establish issuer trust or ongoing
validity. Ten admission mutations were detected. The earlier host-only result
had 17 unit/transport tests and six actual Chrome certificate cases; the
[decoding-only milestone](local-owner-authentication-parser-adapter-plan.md#first-slice-result-2026-10-23)
then reached 13 unit/14 transport tests. Those are dated intermediate results,
not alternative current totals. No new browser/trust experiment occurred in the
later parser/profile slices. The browser fixtures' temporary trust and keys were
removed; no certificate-ignore mode was used.

The [locked dependency review](../../experiments/AUTH-001/DEPENDENCIES.md) records
**94 committed registry entries** and their scoped adoption review. Its clean
85-entry (2026-10-19) and 94-entry (2026-10-23) OSV observations are historical.
The [advisory JSON](../../experiments/AUTH-001/advisories.json) remains the dated
2026-10-23 snapshot, with its explicit future-advisory limitation; it is not a
current clean-graph claim. The newer
[2026-10-26 finding and patch](#advisory-follow-up) records the affected Tokio
1.53.1 baseline and its replacement by 1.53.3 in both locks, the passing local
full gate, and the unresolved Windows transport instability. Final-source hosted
checks and maintainer patch review remain required. The separate
[98-entry reader candidate](local-owner-authentication-linux-files.md#dependency-research-2026-10-26)
is metadata-only and has not changed either project manifest or lockfile.

[Hosted CI at source 42722c0](https://github.com/MrQuality/kehila/actions/runs/39961255336)
passed the standalone Linux checks alongside shared verification. This is actual
historical CI evidence; the newer patch verification is recorded above. Neither
qualifies browser trust, Podman file admission or complete authentication.

**Next action:** review the focused Tokio 1.53.3 patch and its exact-source hosted
checks under #40; retain the Windows reliability investigation and the
2026-11-02 follow-up. The
[Linux reader-slice proposal](local-owner-authentication-linux-reader-slice.md)
and candidate dependency research make the subsequent descriptor-based reader review
concrete. Runtime adoption, actual Linux/provider tests and heavy executable
verification still require their own acceptance/evidence and resource permission.

Supported installation, issuer-key/credential storage, renewal, expiry/drain,
ingress and browser cookie/bootstrap qualification remain open under the
[installation/lifecycle plan](local-owner-authentication-installation-plan.md).
The diagnostic interruption rule is invalidation and a fresh rerun, not supported
authentication suspend handling. A0/T01-T16, A1 schema/capabilities and A1-A5
implementation retain their gates; a transport response is not authentication.

On 2026-10-20 the maintainer agreed to the plan's
[bounded PoC TLS inputs](local-owner-authentication-installation-plan.md#agreed-poc-tls-profile):
a disposable private CA, ordinary server validation, TLS 1.3 with reviewed AEAD
suites, an initial ECDSA P-256/SHA-256 certificate profile and exact loopback IP
SAN. Certificate fees are excluded. The OWASP basis and RFC 9525 naming rationale
are recorded there. This selects PoC inputs only; tooling/storage selection,
execution, exact trust changes and broader platform qualification remain gated.
