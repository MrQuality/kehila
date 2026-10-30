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
flushes, reads that may flush TLS records, and closure. It sends and flushes the
TLS close notification, receives the peer's TLS closure with no surplus data,
then shuts down TCP. Calling tokio-rustls shutdown first also shuts down the
underlying transport; that TCP half-close must not precede the peer notification
which this listener still requires. Each operation checks current
reply/issued-session eligibility. Bytes already written cannot be recalled.
Only observed completion with no buffered or subsequent surplus application bytes
permits exchange confirmation; cancellation/failure drops the retained guard and
retires unconfirmed issuance.

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
credential was therefore not confirmed; retiring it is required failure behavior.
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

With the corrected close order, the same Windows Python/OpenSSL fixture passed
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
