# AUTH-001: bounded local HTTPS transport probe

Disposable evidence for [A0](../../docs/implementation/local-owner-authentication-a0.md)
under [#40](https://github.com/MrQuality/kehila/issues/40) and
[#49 W2](https://github.com/MrQuality/kehila/issues/49). This standalone Cargo
workspace does not alter the production worker. Its diagnostic endpoints remain
transport-only; separately developed SQL and credential components are unconnected
until complete owner-login integration is qualified.

The separate opt-in [A1 storage proof](storage/README.md) exercises disposable
owner enrollment against PostgreSQL. It is outside this Cargo workspace and
does not add authentication to these diagnostic HTTP endpoints.

## Contract

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

## Unconnected bundle decoder and profile checks

The private `bundle` module checks bounded version-1 ASCII framing and canonical
base64, returning explicitly unvalidated bytes. Its private admission stage checks
the accepted disposable certificate/key profile and requires positive key
matching at a supplied time. Neither stage has a listener caller or public
conversion. The scoped dead-code expectation documents that deliberate boundary;
it must be removed when qualified admission is integrated. Existing server/client
paths retain their original separate DER inputs and host-loopback checks.

The earlier admission checkpoint had 37 unit and 14 transport tests. Ten
admission mutations were detected, including raw extension/default encodings,
signature shape and rejection of an explicitly matching inner P-256 parameter. See the [decoder contract](../../docs/implementation/local-owner-authentication-parser-adapter-plan.md)
and [admission result and limits](../../docs/implementation/local-owner-authentication-credential-admission.md#private-implementation-result-2026-10-23).
These checks do not establish issuer authenticity, client trust or ongoing expiry.
Safe file admission and the container proof remain unfinished. The
[dependency review](DEPENDENCIES.md) records the locked adoption and advisory scope.

## Credential core in progress

The unconnected password and random-secret modules implement the fixed policy
and strict verifier/token admission described in [A2](../../docs/implementation/local-owner-authentication-a2.md).
The restored primitive checkpoint passed 56 unit and 20 serial transport tests
on Windows; adding the owned worker passed 69 unit and 20 transport tests.
Five release samples measured verification plus replacement and joined cleanup
at 188.700–195.864 ms, with observed process peak working set 69.54–69.72 MiB.
See A2 for sampling limits and failed-attempt history. These observations do not
qualify login, session storage, Linux/container performance, the complete request
budget or supported secret provisioning. Coordinator and registry behavior remain
separate gates.

New-password worker operations now require the opaque output of blocklist
admission. Policy one pins an external 2,097-entry historical common-password
asset; neither source corpus nor derived data is bundled. Its license/coverage,
reproduction and opt-in test are recorded in A2. With the real asset explicitly
selected, final local verification passed 77 unit and 20 transport tests.
Ordinary CI omits the single external-asset test. Existing-password verification
does not reapply the new-password list. Startup file admission and actual
enrollment/recovery/HTTP callers remain unfinished.
The pure rate-admission engine adds independent bounded login/management
allowances; its latest selected-asset local suite passed 81 unit and 20 transport
tests. Actual request charging and HTTP saturation behavior remain unconnected.
See A2 for the newer-hosted-Clippy failure and decoder correction history.

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
