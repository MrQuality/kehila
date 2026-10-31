# Experimental owner listener and Windows clients

Status: prepared implementation within [PR #87](https://github.com/MrQuality/kehila/pull/87).
Source review and component checks do not qualify a live owner-login capability.
The [delivery definition of done](local-owner-authentication-delivery.md) remains
open until the actual provider and client journeys pass.

## Linux ownership and admission

The separate Linux-only `owner` example calls `serve_owner`. It loads the fixed
protected assets and pinned corpus before database/listener acquisition, starts
the owned coordinator and binds the explicitly configured private IPv4 address.
The public browser/CLI authority remains the same-port literal Windows loopback
origin. Existing diagnostic examples retain their independent profile.

The bind path requires `ip_nonlocal_bind` to be zero and uses a fresh socket with
an exact address, without free-bind or transparent options. This establishes
kernel locality at bind time. The launcher must independently verify that the
address is actually assigned to the intended container, the relevant namespace
and network configuration remain trusted, and publication is restricted to the
intended Windows loopback endpoint. A successful bind alone proves none of those
provider properties. This replaces the proposed interface-enumeration mechanism;
it does not relax the assigned-address or ingress-isolation requirement.

There are at most 16 owned connection tasks, including completed but unjoined
tasks. TLS and header deadlines are two seconds; HTTP allows 32 headers and an
8-KiB buffer. Closed authentication admission retains the 2-KiB target and 4-KiB
body limits, with a two-second body deadline. The original five-second connection
deadline starts at accept and includes scheduling, handshake, HTTP and closure.

The listener retains the accounted coordinator reply through actual TLS writes,
flushes, reads that may flush TLS records, and bounded cleanup. After Hyper
completes the framed HTTP response with no buffered surplus, an explicit final
TLS flush and exchange confirmation still check current eligibility and the
original deadline. Failure or cancellation before that boundary retires
unconfirmed issuance. Confirmation borrows the exchange: its coordinator slot
and guard remain owned until connection cleanup ends.

TLS closure follows outbound confirmation. The listener sends and flushes its
close notification, attempts reciprocal peer closure without surplus data, then
shuts down TCP, within the original connection deadline. Late closure failure is
still counted, but cannot retroactively retire a confirmed credential. Bytes
already written cannot be recalled; neither a successful flush nor reciprocal
TLS closure proves application receipt. A fully sent response lost by a client
can leave a bounded session until normal expiry; another login cannot replay its
secret. The independent diagnostic listener retains its existing strict closure
classification and has no credentials to confirm.

## Runnable validity and shutdown

The strict certificate profile now supplies both validity endpoints. UTC is
checked before listening and each handshake. A fixed elapsed expiry budget is
captured before database startup and cannot be extended by a UTC rewind. The
maximum runnable server lifetime is 180 seconds. A validity poll runs at most
one second apart while scheduled; any failed observation ends admission and
cannot reopen it.

Listener, coordinator, worker and database cleanup share the first irreversible
stop's five-second budget. Remaining database-driver work is aborted and joined
at that deadline. Cleanup retains ownership and observes joins, reporting an
overrun as failure; it does not claim forcible interruption of a running hash or
OS entropy call. Actual scheduling/drain measurements remain required. Any
observed or suspected development host/VM pause invalidates the run under
ADR-106; these timeouts do not qualify supported suspend/resume behavior.

## Client behavior

The first-party static page uses same-origin fetch, server-issued HttpOnly
cookies and independent CSRF proofs. It supports login, read-only reload,
logout audit retry, password change and revoke-other controls. Password fields
permit normal password-manager autocomplete and paste. There is no browser
bearer output, token URL or browser-storage persistence. CSP permits only the
served script and same-origin connections; no third-party resources are loaded.

A denied login clears cached preparation state before the next attempt.
Uncertain password change hides dependent forms and instructs reload/session
reconciliation rather than automatically repeating the change. Retained logout
state supports only its own retry; explicit browser-cookie removal proves no
audit result. These behaviors need actual Chrome observations.

The [Windows CLI](../../experiments/AUTH-001/clients/windows/README.md) uses the
installed .NET Framework and Schannel with TLS 1.3, ordinary certificate/IP
validation and an additional selected-root fingerprint restriction. Its fixed
cache uses DPAPI CurrentUser and checked owner-only ACLs, bound to the public
origin and issuer. Interrupted replacement requires explicit recovery; logout
marks the local cache retiring before the request. No password or bearer is
printed or accepted in command arguments. Library/managed-string copies are
outside any universal erasure claim.

The bounded BCL JSON reader/writer verifies the complete fixed canonical response
envelope, including duplicate/unknown metadata and trailing data rejection.
Response disposal stays registered through body consumption under the request
deadline; actual TLS negotiation, typed certificate rejection and stalled-body
cancellation remain unexecuted Windows qualification gates.

## Evidence and limits

Pure clock/HTTP checks and existing credential/coordinator regressions ran on
Linux: the publication candidate passed formatting, Clippy, 131 unit and 21
serial transport tests, with eight ordinary opt-in skips. Windows formatting,
Clippy, 115 unit and 21 serial transport tests passed, with the one external-corpus
skip. Native PostgreSQL opt-in fixtures were not repeated in this phase; their
earlier observations retain their original source limits.

A real-TLS Windows surplus-input regression failed the original diagnostic
close path and passed its buffer check. Local Windows CLI build, DPAPI/file
roundtrip/replacement/binding/corruption/interrupted-write/weak-ACL rejection, and
closed-response cases passed. Initial fixture ACL and metadata-parser failures
were corrected without weakening the protection contract. Hosted Windows CI is
configured to build and run these local client checks; it does not run a TLS or
application-container journey.

The first hosted Windows attempt failed a protection check during fixture
recovery; its stack did not identify the exact failed predicate. The valid
interrupted-write fixture now creates its pending file with the same explicit
owner/DACL contract as production, rather than assuming a copied file's ambient
owner. A separate copy observation retains owner-mismatch rejection. Production
checks remain unchanged; the PR records the hosted result for the corrected head.

These results do not establish actual owner serving, protected assets on the new
provider, the amended local/external ingress matrix, expired/revoked credential
delivery, process/container restart rejection or full-request performance. The
[container record](local-owner-authentication-container-plan.md) separately
describes the initial native-WSL resource observation. A4 domain commands,
supported installation/recovery, real-data readiness and replicated deployment
retain their own gates.

The [2026-10-30 local-caller amendment](../architecture/ADR-106-local-container-transport.md#accepted-local-caller-amendment-2026-10-30)
now permits untrusted local guest access for disposable authenticated tests.
The earlier TCP-only guest-isolation failure remains recorded in the container
plan; it supplies no actual owner-login or LAN-exclusion qualification.

## Native startup and first Windows request, 2026-10-30

A bounded native-WSL run of the Rust implementation at `7e033f5` started the
backend and PostgreSQL on separate serving/database bridges, with only the
application's configured loopback port published. Both containers' kernel
memory/swap/CPU/PID settings matched their proposed ceilings: 256 MiB memory and
zero swap each, 0.5/1 CPU and 64/128 PIDs for application/database respectively.
These are observed configured limits, not application-capacity or OOM evidence.
Actual file ownership/mode, effective read-only secret mounts and other-UID
read/traversal refusal checks passed. A wrong-source PostgreSQL connection using
normal certificate/hostname verification reached the intended TLS HBA refusal.
This is a startup/file/database subset, not the full ingress or login matrix.

The first Windows wrong-password request returned only the client's generic
unconfirmed-result message, so its HTTP status and failure stage remain unknown.
The run stopped before successful login. Source inspection separately found
that the backend emits a fixed error envelope which the client rejected as an
unknown field. Local response regressions reproduced that defect; the prepared
repair accepts only the exact canonical rejection on HTTP 400-599. It is not
yet established as the sole cause of the live failure. Malformed envelopes still
fail closed, and no TLS validation was weakened.

With the repaired client, a bounded diagnostic using the same production
transport decoded HTTP 400 with the fixed rejection envelope. Normal certificate
validation and the selected-issuer pin succeeded for that diagnostic; the exact
admission predicate remains unknown. The client now explicitly suppresses
`Expect: 100-continue`, which Framework defaults can add and this backend rejects.
A controlled TLS comparison using matched clients differing only in that setting
observed the header in the baseline and its absence with explicit suppression.
Selected Host, target, version, content-type and canonical-length checks passed
for both controls. These controls observe client headers, not Rust authentication.
The earlier HTTP 400 alone does not identify its rejected admission predicate.

The following Rust-backed attempt still failed at the first wrong-password
request: the client diagnostic reported an unclassified transport failure without a response status.
The backend remained running without an observed OOM kill. No successful login
or typed password rejection was established. Source review found no confirmed
denial-response defect; server-side connection outcomes are the next required
measurement before another full journey. Owned resource cleanup completed;
temporary trust removal remains a separate observed cleanup requirement.

Earlier attempts stopped on a rejected fixture mount option and an assertion
that confused inherited image-port metadata with actual publication. Those
failures remain recorded. Cleanup required reconciliation after the first
attempt; later owned cleanup and earlier temporary trust removals completed. Chrome,
external-peer rejection, actual login/password/logout/restart journeys, expiry,
graceful drain and complete request/capacity budgets remain unqualified.

## Single-network native request subset, 2026-10-30

A subsequent controlled request used the Rust source at `f234c8f`, an explicit
test root and ordinary TLS 1.3/IP verification. The authority and its necessary
PostgreSQL fixture occupied one restricted bridge, with separate namespaces,
identities and secrets; only the application endpoint was published. The earlier
two-network diagnostic returned a reset with zero Rust accepts. That observation
places that sampled failure before Rust admission; it does not identify the
cause of earlier Windows failures or establish a provider defect.

The single-network request returned HTTP 401 with the fixed rejection envelope.
The controlled server stop recorded exactly one accept and one completed
exchange, with zero recorded rejection/timeout categories and a normal exit.
This establishes one actual wrong-password rejection, not a successful login.

Against the live database, an ordinary guest UID with no effective capabilities
completed no TCP connection within two seconds. An unrelated container on its
own network reported refusal/unreachability. Database liveness was checked
before and after, and authority startup subsequently established its intended
verified TLS/SCRAM connection. These are bounded path observations, not proof
of every possible ingress route. Owned fixture cleanup completed; no Windows
trust-store change was made for this subset.

A following explicit-root Python/OpenSSL fixture, running on Windows, observed
HTTP 401 for a wrong password and HTTP 200 for login, followed by HTTP 401 for
the issued credential's state request. The journey stopped at that failure and
cleanup completed. A controlled repeat recorded three accepts, three peer-close
rejections and no completed exchanges, with a normal server exit. The login
credential was therefore not confirmed under that implementation's completion
rule and was retired.
This is not a passing login/session journey and does not classify older failures.

A focused real-TLS regression demonstrated that the original shutdown order
sent TCP FIN before receiving the required peer close notification. The corrected
order keeps TCP open until that notification arrives. This follows the pinned
[tokio-rustls shutdown implementation](https://docs.rs/tokio-rustls/0.26.6/src/tokio_rustls/server.rs.html#534-544),
which delegates shutdown to the underlying stream. The original regression failed;
the repair passed Linux Clippy with warnings denied, 132 unit tests and 21 serial
transport tests. Eight opt-in fixture tests were skipped: six PostgreSQL tests,
one external-corpus test and one protected-startup fixture. Neither
the regression nor source inspection alone establishes a defect in a forwarding
provider. Schannel, the product CLI's cache/parser and Chrome remain separate
qualification targets; external LAN exclusion, expiry/drain and complete request
budgets remain open.

### Corrected explicit-root journey

With the corrected close order in
[`b290968`](https://github.com/MrQuality/kehila/commit/b290968c823ec7d2d6be9fdd3bfab31efb4600ce),
the same Windows Python/OpenSSL fixture passed
16 sequential TLS/HTTP observations against the Linux authority and disposable
PostgreSQL: wrong-password rejection, login and active state, password change,
old-session rejection, new-password login, revoke-other with exactly one target,
rejection of that target while the initiating session remained active, rejection
after process restart, fresh login, logout and its retry-only state/audit retry.
The restart used a forced process stop; it does not demonstrate graceful drain.
The earlier failed journey and three peer-close rejections remain failures at
their original source, rather than retrospective passes.

The fixture checked the actual two-member restricted bridge, separate network
namespaces, runtime identities, read-only protected mounts, other-UID refusal,
TLS/SCRAM database startup and absence of database publication. The ordinary
guest and unrelated-container database negatives retained the bounded scope
above. Owned fixture cleanup completed. No Windows root trust was installed for
these runs.

In this one small workload, password change took approximately 0.34 seconds
from connection setup through TLS closure. The measuring clock's resolution was
15.625 ms. Before restart, application cgroup peak memory was 70,053,888 bytes
(about 66.8 MiB), with zero recorded OOM events at that observation. These are
single-run measurements under a 256-MiB container ceiling, not sustained-load,
concurrency-capacity or full request-budget qualification. They do not qualify
Schannel, the Windows product CLI, Chrome, external LAN exclusion, credential
expiry, supported installation or recovery.

## Bounded connection pressure and authority loss, 2026-10-30

Additional runs used the same Rust implementation at `93ab653`, normal explicit-root
TLS verification, the restricted two-component fixture and the existing container
ceilings. No production code or Windows trust configuration changed.

Sixteen clients completed TLS handshakes concurrently and withheld HTTP headers.
Their setup took about 42 ms; an actual socket observation still showed all sixteen
present when an explicit stop was requested. The seventeenth connection was
rejected. Server accounting recorded nineteen accepted connections: three completed
initial HTTP exchanges and sixteen HTTP-level rejections from the held connections,
plus one separate capacity rejection. No connection timeout or TLS/peer-closure
rejection was recorded in this sample. These are listener accounting categories,
not a claim that the held clients received an HTTP error response.

The explicit-stop interval was 2.67 seconds from before stop delivery through
observed process exit. It conservatively includes control and container-wait
overhead. The runtime exited normally after joining owned work. Application
cgroup peak memory at the held-connection observation was 69,836,800 bytes
(about 66.6 MiB), with zero recorded OOM events. This checks one saturated
header-wait case and the existing stop input; it does not qualify sustained
traffic, active hashing/SQL drain, OS signals or a supported container shutdown
policy.

After a fresh process established login and active state, deliberately stopping
PostgreSQL prevented another authenticated response. That client's observed
outcome was TLS EOF, not a certificate-policy rejection. The application ended
with its expected owned-listener error and exit code 1, without OOM or a panic.
The interval from before database stop through observed application termination
was 1.55 seconds. This is a real authority-loss subset; it does not cover every
in-flight commit or outbound-publication race. Owned cleanup completed for both
subsets. Product Schannel/Chrome, external-peer ingress, remaining expiry and
full-load/request-budget gates stay open.

### Natural runnable-lifetime subset

The unmodified public owner executable at `93ab653` also completed its natural
runnable-lifetime interval without a stop input, signal or database interruption.
The observed interval from before container creation through process exit was
181.93 seconds. Exit code was zero, the joined-stop message was observed and no
OOM was recorded. PostgreSQL remained running; the two runtime connections and
one runtime advisory lock present before the wait were absent afterward.

This host interval includes startup, scheduling, cleanup and container-wait
overhead. It supports this fixture's natural maximum-runnable-lifetime stop;
it does not independently measure an exact 180-second admission cutoff or a
separate five-second drain. The certificates remained valid throughout. It is
neither certificate-expiry nor session-expiry evidence. A preceding attempt
stopped before application startup and supplies no lifetime observation. Owned
cleanup completed after the successful fresh run.

### Controlled-history CLI expiry

The opt-in `native_cli_expiry_over_tls` fixture, introduced in
[`073c6ea`](https://github.com/MrQuality/kehila/commit/073c6ea42713ba27caf7e2b66bf6e0be140078e1),
adds the narrower expiry wire
observation required alongside the existing controlled-time core tests. Each
idle/absolute case obtains a separately issued credential from the actual
PostgreSQL/coordinator path, confirms that typed response and observes active
state over real loopback TLS. Linux-test-only code then constructs elapsed
history from the unchanged 30-minute/12-hour constants, preserving credential,
identity, binding and phase. The unchanged connection/adapter path returns the
canonical HTTP 401 for the same credential.

The normal fixture passed both histories. An isolated mutation bypassing expiry
in registry lookup and current-state validation failed the intended HTTP assertion
by returning success for the idle-expired history. The restored implementation
passed again. Each case released its runtime database connections and advisory
lock, and owned cleanup completed. Ordinary Linux Clippy and serial suites also
passed: 132 unit tests, 21 transport tests and nine opt-in fixture skips. The
explicit native expiry fixture ran separately from those skipped checks.

Issuance here is observed at the typed coordinator boundary; HTTP coverage is
the state route before/after constructed history. Absolute history also caps the
idle deadline, so this does not independently isolate the redundant absolute-only
guard. No OS clock, production policy or runtime limit changed. Protected
startup/provider publication, browser-cookie expiry, product Schannel/Chrome and
external ingress retain separate gates; this is not a complete T06 pass.

### Windows product-client handoff investigation (2026-10-31)

At pre-correction source `7b0b913`, the stock Windows CLI rejected a wrong
password, reported successful login, then rejected the issued credential's state
request. A controlled repeat waited six seconds after login, beyond the
five-second connection deadline, and still rejected state. Its listener recorded
two accepts and two peer-close rejections, with no completed exchanges or TLS,
HTTP-driver, TLS-shutdown or timeout rejections.

Actual Chrome UI testing at the same source reached login HTTP 200, followed by
HTTP 401 from the session/CSRF bootstrap; the UI remained signed out. Its eight
connections all ended with peer-close rejections. Ordinary certificate validation
was enabled. All owned resources and browser profiles were cleaned up. These
results establish failed product-client journeys, not a defect in a particular
network provider or the precise underlying close error.

The correction at
[`3eaac16`](https://github.com/MrQuality/kehila/commit/3eaac169b43b224790e9c44bc80f24094028425f)
separates guarded outbound completion from later TLS cleanup as described above.
The real TLS/PostgreSQL
[delivery regression](../../experiments/AUTH-001/src/application/jobs/tests/delivery.rs)
receives a complete login response, deliberately drops its transport without a
reciprocal TLS alert, and checks the same credential's subsequent state. Restoring
the original confirmation order in an isolated comparison failed that state
assertion; the correction passed. Both cases released runtime database
connections and advisory locks. This comparison does not emulate every platform
close failure.

Linux Clippy with warnings denied and serial standalone suites passed: 132 unit
tests and 21 transport tests, with ten opt-in unit cases skipped in the normal
run. The new native delivery case ran separately. The failed runs above remain
failures at their original source.

### Corrected Windows product-client journeys

The corrected candidate passed the following actual Windows client sequences
against the disposable Linux authority and PostgreSQL, with normal platform
certificate verification and the selected issuer pin for the CLI:

| Client observation | Reached result |
| --- | --- |
| Stock CLI: 12 sequential steps | Wrong-password rejection, login and active state, password change, old-password rejection, new-password login/state, revoke-other success for an empty target set, stale-session rejection after controlled restart, fresh login/state and audit-confirmed logout. |
| Chrome 156.0.8078.76: 12 UI checkpoints | Initial signed-out state, wrong-password rejection, login, reload, revoke-other success, initiating session retained after another reload, password change, old-password rejection, new-password login, stale cookie rejected after controlled restart, fresh login and audit-confirmed logout. |
| Cross-client: three CLI observations during the Chrome journey | A second CLI credential was issued and confirmed active, then denied after browser revocation while the browser remained active. This supplies a real revoke-other target. |
| Browser cookie/reload checks | The session cookie was Secure, HttpOnly, SameSite=Strict, host-scoped and path `/`; logout removed it. All three reload observations required a new document before checking UI state. |
| Controlled restart and ownership | The runtime generation advanced by one; both runs ended their owned services and removed temporary profiles/cache. |

These counts describe steps in two bounded journeys, not independent test cases
or a percentage of completed authentication coverage. The first browser runtime's
observed cgroup memory peak was 70,053,888 bytes (about 66.8 MiB) under its enforced
256-MiB limit, with no recorded limit hits or OOM events. This was a small serial
workload including password replacement, not a full-load/RSS qualification.

The browser/cross-client run accepted 28 connections before restart and eight
afterward; all 36 ended in the separately counted peer-close rejection, with no
TLS-handshake, HTTP-driver, TLS-shutdown or timeout rejection. Application
responses and state transitions passed; reciprocal TLS closure did not. The
precise underlying platform close error remains unclassified. Connection tasks
and accounted replies were still joined/released, and no cleanup failure was
reported. No certificate validation or admission limit was bypassed.

An additional check positively observed an authenticated read blocked in its SQL
fence before requesting the existing controlled stop. The process exited normally
and joined owned work within a conservative 4.687-second host interval, including
control-command and container-wait overhead. The blocker remained held through
application exit; the pending Windows CLI request produced no authenticated
success, and runtime database connections and advisory locks were absent afterward.
Owned blocker/client and fixture cleanup completed. An ordinary request/query
deadline may have settled that read after stop; this is not evidence of immediate
SQL cancellation or an exact stop-handler timestamp.

The normal shared staged verification passed for this correction, including 292
tooling tests (three documented platform skips), Rust/Go/static checks and the
required owned-service checks. Hosted CI remains an independent final-head gate;
its current result is reported in [PR #87](https://github.com/MrQuality/kehila/pull/87).
The complete external-ingress matrix, certificate matrix, full traffic, active
hashing/write drain and complete T01-T16 coverage retain their separate gates. These disposable
journeys do not qualify supported installation, production adoption or real data.

### Sampled Android LAN connection check (2026-10-31)

One Android phone on the same LAN attempted a direct HTTPS connection to the
Windows host's LAN address and application port during a measured live window.
The user-reported verbose curl output identified the intended target and a
connection-stage timeout after 5,003 milliseconds, with no successful connection
or TLS negotiation reported. The complete report arrived within the live window.
Authenticated loopback controls passed before and after the attempt with the same
credential; the running backend container identity and start time were unchanged.

This supports sampled client-observed TCP nonconnection from that phone. It does
not prove that packets reached the PC, identify the blocking layer or establish
exclusion across all devices, interfaces, routes or provider configurations.
Earlier attempts remain inconclusive because their timing or command output did
not establish the intended live request. No certificate bypass or phone trust
installation was used. Local controls used normal chain, validity and IP-name
verification against an explicit disposable test root; they add no new Windows
platform-trust or Chrome certificate-matrix qualification.

The public bounded runtime ended normally and reported joined cleanup. Runtime
database connections and advisory locks were absent afterward, and owned fixture
cleanup completed. This observation supplies one external-peer subset; the full
ingress matrix and the other capability-delivery gates remain open.

### Complete-request workload and active-work subsets (2026-10-31)

Three serial password-change requests traversed normal explicit-root TLS, HTTP,
password verification and replacement, PostgreSQL commit and complete response
closure. Their observed Windows-client intervals were 0.375, 0.391 and 0.390
seconds, each below the unchanged five-second request budget. The durable verifier
revision and authentication generation both advanced from one to four, with
three password-change audit events.

The Linux kernel reported a process `VmHWM` of 70,484 KiB (about 68.83 MiB).
The separately accounted cgroup memory peak was 70,365,184 bytes (about
67.11 MiB), with no memory-limit or OOM events under the enforced 256-MiB
application ceiling. These are different kernel counters, not interchangeable
memory measurements. A subsequent pair of overlapping valid login requests
returned HTTP 200 in 0.203 seconds and canonical HTTP 429 in 0.031 seconds;
the successful credential remained usable. This does not isolate hash-lane
rejection from other bounded admission or establish sustained-load behavior.

A separate stop request followed a positively observed running password worker
with recorded CPU time. The pending password-change request returned HTTP 503;
the runtime exited normally within a conservative 0.890-second interval from
request start. Durable verifier revision, authentication generation and
password-change audit count were unchanged. Runtime database connections and
advisory locks were released, and owned cleanup completed. This witnesses active
password work, not a particular Argon2 stage or every cancellation race.

Two blocked-write attempts reached the password-change SQL statement waiting on
its audit write and ended with normal runtime exit. Their conservative external
intervals were 6.016 seconds from request start and 5.152 seconds from stop-command
start, respectively. Those observations include work or command/notification
overhead before the actual runtime stop signal; neither independently establishes
the five-second stop-to-return bound. The test stopped at that unresolved timing
assertion, so its later rollback/output assertions do not qualify the case.
The first attempt also needed later listener-release reconciliation; the second
completed owned cleanup. Neither is reported as a write-drain pass. Direct
runtime timing remains required without relaxing the limit.

### Explicit-root diagnostic certificate subset (2026-10-31)

A host-only diagnostic TLS endpoint accepted the original and a distinct renewed
leaf under the same explicit test root. The Windows Python/OpenSSL client rejected
the wrong-IP leaf with verification code 64, the expired leaf with code 10, and
the untrusted issuer and omitted trust anchor with code 20. Each negative reached
a typed certificate-verification failure rather than a generic connection reset.
The endpoint exited normally and released its listener after each case.

These six observations qualify that explicit-root diagnostic client path. They
do not establish protected owner startup rejection, the Windows Schannel/Chrome
certificate matrix or authenticated browser expiry. OS trust was unchanged;
trusted platform-client checks retain their separate approval and evidence gates.

### Short-lived owner certificate and held connection (2026-10-31)

A separate disposable Linux-owner fixture used a normally validated certificate
with a 75-second validity horizon. Authenticated login and state passed before
expiry. A TLS connection established while the certificate was valid completed
its authenticated request after the one-second expiry-detection allowance and
received HTTP 503. The response was observed 1.547 seconds after TLS establishment,
before the separate two-second header deadline could explain it.

The public runtime exited normally and joined its work 1.635 seconds after the
certificate expiry timestamp, measured through host observation. That interval
includes observation overhead and falls within the one-second detection plus
five-second drain envelope. Runtime database connections and advisory locks were
absent afterward, and owned cleanup completed. No wall clock, session policy,
runtime lifetime limit or OS trust was changed. The issuer and leaf were
disposable; explicit client roots supplied ordinary chain, validity and IP checks.

This observes one actual certificate-expiry schedule and a held connection. It
does not cover the first second after expiry, every active-hash/write schedule,
clock jumps, suspend/resume or platform-browser certificate behavior. Earlier
preparation failures supplied no serving-lifetime evidence; a separate cleanup
reconciliation after one failed preflight remains distinct from this passed run.
