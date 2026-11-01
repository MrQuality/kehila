# AUTH-001: bounded local HTTPS transport probe

Disposable evidence for [A0](../../docs/implementation/local-owner-authentication-a0.md)
under [#40](https://github.com/MrQuality/kehila/issues/40) and
[#49 W2](https://github.com/MrQuality/kehila/issues/49). This standalone Cargo
workspace does not alter the production worker. Its diagnostic endpoints remain
transport-only. Linux [coordinator source](../../docs/implementation/local-owner-authentication-coordinator.md)
connects SQL and credential components with a real typed PostgreSQL/TLS journey.
The actual HTTP/client/provider path remains unqualified.

The separate opt-in [A1 storage proof](storage/README.md) exercises disposable
owner enrollment against PostgreSQL. It is outside this Cargo workspace and
does not add authentication to these diagnostic HTTP endpoints.

The Linux-only [database bridge](../../docs/implementation/local-owner-authentication-database-bridge.md)
now owns dedicated authority and serving connections in a synthetic PostgreSQL
TLS proof. Its narrow credential-read API is not connected to these endpoints.

## Contract

This section describes the diagnostic server/client. The coordinator's separate
ownership and qualification record is linked above. The separate experimental
[owner listener](../../docs/implementation/local-owner-authentication-listener.md)
connects its prepared HTTP dispatcher; actual client/provider qualification is
unfinished. It adds no diagnostic login route.

The server and client accept literal `127.0.0.1` only. TLS 1.3 uses normal
rustls certificate verification, an explicit CLI root set and IP SAN validation.
There is no certificate-ignore mode, redirect client, proxy identity, password,
cookie, session, actor, database or worker integration.

`GET /` returns a fixed diagnostic page. `POST /probe` accepts only `{}` with the
exact Host, same HTTPS Origin, JSON content type and two-byte Content-Length.
Missing/null/foreign origins, proxy assertions, credentials and unbounded framing
are rejected. Responses close the connection and carry no-store and restrictive
content headers. This is a closed diagnostic protocol, not an auth API.

The server owns at most 16 tasks, counting completed tasks until joined. Limits
are 32 HTTP headers, an 8 KiB HTTP buffer, two body bytes, two-second TLS/header
deadlines and a five-second total connection deadline. Shutdown drains owned work;
the example server has a three-minute lifetime. These are experiment bounds,
not measurements of the later authentication capacity or availability budgets.

## Bundle decoder, profile checks and protected startup

The private `bundle` module checks bounded version-1 ASCII framing and canonical
base64, returning explicitly unvalidated bytes. Its private admission stage checks
the accepted disposable certificate/key profile and requires positive key
matching at a supplied time. The Linux protected-startup loader now consumes
these checks and constructs a fixed TLS configuration. The diagnostic server/client
examples retain their original separate DER inputs and host-loopback checks.

The earlier admission checkpoint had 37 unit and 14 transport tests. Ten
admission mutations were detected, including raw extension/default encodings,
signature shape and rejection of an explicitly matching inner P-256 parameter. See the [decoder contract](../../docs/implementation/local-owner-authentication-parser-adapter-plan.md)
and [admission result and limits](../../docs/implementation/local-owner-authentication-credential-admission.md#private-implementation-result-2026-10-23).
These checks do not establish issuer authenticity, client trust or ongoing expiry.
The [protected startup record](../../docs/implementation/local-owner-authentication-linux-reader-slice.md#protected-authentication-startup-2026-10-29)
reports actual opened-file and Podman secret admission; the integrated container proof remains unfinished. The
[dependency review](DEPENDENCIES.md) records the locked adoption and advisory scope.

## Credential core in progress

The [A2 components](../../docs/implementation/local-owner-authentication-a2.md)
implement normalization, strict fixed-cost verifier/token admission, owned hash
work, typed new-password blocklist checks, separate rate allowances, 64 session
slots and 32 pre-session slots with owned allocation/one-time claims. Existing
password verification does not reapply the new-password list. Session/pre-session
state preserves original expiry, shared retirement and irreversible owner teardown.

Earlier selected-asset local verification passed 105 unit and 20 serial transport
tests, formatting and Clippy. It includes independent vectors, detected mutations
and a two-thread competing-claim case. The pinned 2,097-entry historical corpus
stays external with its own license/coverage limits; ordinary CI ignores its
single external-asset test. A2 records reproduction and executed evidence.

Historical Windows release worker samples measured verification plus replacement
and joined cleanup at 188.700-195.864 ms, with observed process peak working set
69.54-69.72 MiB. These are source-specific component observations; A2 preserves
sampling limits, failed attempts and the newer-hosted-Clippy correction history.
They do not qualify current-head Linux performance or the full request budget.

The later startup/revoke-other checkpoint passed Linux Clippy and 128 serial unit
tests with eight opt-in skips. Separate native fixtures exercised protected startup
assets, uncertain-commit consumers and revoke-other audit rollback/retry. The
[coordinator record](../../docs/implementation/local-owner-authentication-coordinator.md)
describes shared runtime/database ownership and retained reply bounds. These
component observations do not qualify a serving listener, full request budgets,
selected-provider isolation or actual Windows browser/CLI login. Diagnostic HTTP
endpoints have no authentication caller, and
no supported installation, secret provisioning or real-data readiness is claimed.

The prepared Linux `owner` example and first-party browser page now connect that
dispatcher through bounded TLS I/O and owned cleanup. The
[Windows client](clients/windows/README.md) has local DPAPI/file/JSON checks.
Neither implementation has a qualified live browser/CLI journey yet. The
[provider resource subset](../../docs/implementation/local-owner-authentication-container-plan.md#native-wsl-resource-subset-2026-10-30)
is separate from mounts, Windows publication, ingress isolation and authentication.

The opt-in [`native_cli_expiry_over_tls`](src/application/jobs/tests/expiry.rs)
test obtains real PostgreSQL/coordinator issuance, confirms that typed response,
then checks state over actual loopback TLS before and after constructing expired
idle or absolute history. The fixture is confined to Linux test builds; it uses
the existing policy constants and changes no production clock or lifetime.
Both histories returned canonical HTTP 401. An isolated bypass of expiry checks
in both lookup and current-state validation failed the new HTTP assertion; the
restored implementation passed again. This is state-route integration evidence,
not HTTP login, browser-cookie expiry, protected startup, provider publication or
elapsed 30-minute/12-hour waiting. See the [listener evidence record](../../docs/implementation/local-owner-authentication-listener.md)
for the separate runtime and client observations.

## Run

Set `CARGO_TARGET_DIR` to an external build directory before building so generated
debug artifacts stay outside the checkout. From the repository root:

```text
cargo fmt --manifest-path experiments/AUTH-001/Cargo.toml --all --check
cargo clippy --manifest-path experiments/AUTH-001/Cargo.toml --locked --all-targets -- -D warnings
cargo test --manifest-path experiments/AUTH-001/Cargo.toml --locked --all-targets -- --test-threads=1
cargo build --manifest-path experiments/AUTH-001/Cargo.toml --locked --examples
```

Tests use ephemeral ports by default. Optional `KEHILA_TLS_TEST_PORT` selects one
nonzero local port; an async test-owned mutex serializes network cases. An occupied
requested port is a failure, not a reason to choose another endpoint silently.
The tests preserve the same certificate and HTTP checks in either mode.

The `prepare` example takes an existing, caller-protected directory outside the
checkout and refuses overwrite. Optional `--profiles` adds renewed, wrong-SAN and
expired leaf fixtures under one fresh root. `serve` takes a fixture directory,
port and optional stop-file path; `probe` takes the root DER file and port.
Their bounded file readers and key-consistency checks precede server binding.
They are invoked from the external build directory's `debug/examples` outputs.

The generator never installs trust or provisions directory permissions. Its
caller must establish and check owner-only directory access, own every process,
and remove all generated private keys after success or failure. Browser trust
requires separate approval for the exact issuer and store; preserve existing
entries and remove only test-owned trust. Neither the examples nor tests modify
host security settings. The examples require the source checkout to remain
available for their compile-time checkout-boundary check.

Chrome's [headless CLI](https://developer.chrome.com/docs/automation-and-testing/headless-cli)
can check actual browser validation using isolated disposable profiles and
`--dump-dom`. Certificate failures may appear on stderr rather than in a DOM;
require the specific certificate error and absence of the success page. Do not
use certificate-ignore switches or bypass an interstitial. Headless CLI checks
do not exercise later password forms, cookie bootstrap, CSRF or session behavior.

## Observed Windows evidence, 2026-10-19

Toolchain: Rust/Cargo 1.93.0, Windows x86-64, Chrome 155.0.8059.84. The source graph
declares Rust 1.88; a complete-graph minimum-version build remains unqualified.

| Check | Observation |
| --- | --- |
| Rust suite | Three unit tests and 14 real transport tests passed. |
| Certificate failures | Wrong/untrusted root, wrong IP SAN and expiry produce typed certificate errors; unrelated resets cannot pass these assertions. Mismatched keys fail configuration. |
| HTTP ingress | Missing/null/foreign Origin, wrong Host and oversized body reject after valid TLS; unit cases also reject duplicate Origin and proxy/credential/framing assertions. |
| Resource ownership | Capacity overflow, stalled handshake/headers/body and shutdown with active peers reach bounded rejection/completion and release the listener. |
| Plaintext | Rejected without an HTTP response; a bounded fatal TLS alert is allowed. TCP alert fragmentation is handled. |
| Startup | Missing certificate and missing key both report file-not-found while the target port is deliberately occupied, proving file validation precedes binding. |
| Actual Chrome | Trusted and renewed leaves render the fixed page. Untrusted and trust-removed roots produce `ERR_CERT_AUTHORITY_INVALID`; wrong SAN produces `ERR_CERT_COMMON_NAME_INVALID`; expired leaf produces `ERR_CERT_DATE_INVALID`. Each case uses a new profile. |
| Cleanup | Browser/CLI test processes stopped, ports released, generated TLS private keys deleted and the exact temporary user trust entry removed. |
| Mutation | Reversing exact Host admission makes its behavior test fail; the original source was restored. |

The first plaintext assertion incorrectly assumed silence instead of allowing a
fatal TLS alert; it was corrected, then reviewed for TCP fragmentation. The first
Chrome assertion incorrectly required a rendered error page; Chrome emitted the
specific certificate error on stderr. These harness corrections do not bypass
certificate verification or reinterpret a generic network failure as success.

The [dependency review](DEPENDENCIES.md) and [notices](THIRD-PARTY-NOTICES.txt)
cover this experiment. The [dated advisory observations](advisories.json) apply
only to the versions and query scopes they record.
**Tokio patch, 2026-10-26:** Tokio 1.53.3 replaces the affected 1.53.1 pin for
[RUSTSEC-2026-0386](DEPENDENCIES.md#advisory-scope). Follow the
[canonical A0 disposition](../../docs/implementation/local-owner-authentication-a0.md#advisory-follow-up)
for verification and limits; the older whole-graph query remains historical evidence.
Linux CI runs the standalone formatter, linter and transport suite separately from
the existing shared verifier. A configured CI check is not a recorded Linux pass;
inspect its result before claiming qualification. No browser, trust installation,
native production adoption or Podman path is qualified by that Linux job.

The private Linux reader now has a Linux-only dependency/implementation
inside the [owner-login delivery package](../../docs/implementation/local-owner-authentication-delivery.md).
Its [98-entry advisory observation](advisories-reader.json) and dependency notices
are separate from older snapshots. Tests and source review must be reported
before claiming Linux descriptor behavior. The
[partial Linux observations](../../docs/implementation/local-owner-authentication-linux-reader-slice.md#partial-implementation-observations-2026-10-27)
record actual test/mutation results, corrected fixture failures and unqualified
oracles; this unconnected reader does not qualify Podman mounts, a listener or login.

The client preserves both exchange and connection-driver outcomes in bounded
diagnostics without response content or raw error chains. Earlier generic failures
cannot all be attributed to one cause. The
[initial diagnostic observations](../../docs/implementation/local-owner-authentication-a0.md#windows-diagnostic-follow-up-2026-10-27)
are historical; passing reruns alone did not establish a repair.

The [controlled Windows shutdown investigation](../../docs/implementation/local-owner-authentication-a0.md#controlled-windows-shutdown-investigation-2026-10-27)
records a reproduced post-response shutdown failure and a bounded close-lifetime
remedy. After sending TLS closure, the server waits for peer closure under the
same total deadline and connection slot. Silent peers time out; extra application
data or missing TLS closure fails. Stage counters and test diagnostics contain no
message content or raw error chains. Historical browser results do not qualify
this changed close path; remaining verification gates are recorded explicitly.

## Coding-standard scope and remaining gates

The affected module is this experiment only: explicit transport shape/admission,
bounded I/O, typed certificate failures and owned cancellation/task lifetimes.
Business/domain authority and trusted ActorContext rules are not applicable because
there is no command execution. Secrets are disposable and never logged as values;
the generator is not a production key-storage or installer design. Dependencies
are pinned and their license/advisory scope is recorded. No handwritten HTTP
parser or raw platform unsafe code is introduced.

This evidence qualifies the observed host transport subset only. A0's broader
auth-library selection, Linux backend secret-file admission, supported installation and
platform qualification remain open under #40/#49. A1-A5 and T01-T16 remain
separate gates; no authentication case, replicated support, real-data readiness
or ADR-103 adoption is established here.
