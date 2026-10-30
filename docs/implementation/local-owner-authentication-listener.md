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
flushes, reads that may flush TLS records, and closure. Each such operation checks
current reply/issued-session eligibility. Bytes already written cannot be recalled.
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
provider, Windows forwarding, guest/LAN exclusion, expired/revoked credential
delivery, process/container restart rejection or full-request performance. The
[container record](local-owner-authentication-container-plan.md) separately
describes the initial native-WSL resource observation. A4 domain commands,
supported installation/recovery, real-data readiness and replicated deployment
retain their own gates.
