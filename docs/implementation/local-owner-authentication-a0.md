# A0 - Local HTTPS and dependency investigation

Status: Authorized investigation; native/browser execution not started.
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

## Preliminary dependency review - no adoption yet

The existing lockfile contains rustls 0.21.12 / tokio-rustls 0.24.1 transitively,
ring 0.17.14, Tokio 1.53.1, Axum 0.8.9 and Hyper 1.11.1. Their presence alone does
not select them as new direct security dependencies. No manifest/lockfile has
changed in this investigation. Production password/NFC/randomness/DPAPI dependencies
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

## Execution and evidence gates

1. Resolve a disposable probe graph only after dependency provenance/features and
   actual license texts/notices are checked; lock exact inputs. No dependency
   download/compilation has occurred yet. Qualify with real certificate validation,
   never a permissive verifier, ignore-certificate switch or browser warning bypass.
2. Generate unique fixture roots/leaves in an owned ignored directory with restricted
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

Architecture acceptance and read-only dependency reconnaissance are complete.
Transport, certificate trust/renewal, graph/license/advisory qualification and all
A0 execution observations remain pending. Memory-heavy work has been requested;
trust-store changes have not been requested or performed. Prepare the isolated
probe only with the reviewed graph, then execute after permission. Stop before
claiming support if secure installation/renewal/removal cannot be demonstrated.
