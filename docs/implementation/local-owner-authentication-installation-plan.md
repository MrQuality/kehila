# Remaining A0: local installation and protected storage

Status: Proposed, except the agreed PoC TLS inputs below; no experiment execution
or dependency adoption authorized by this plan.
Owner: project maintainer. Recorded: 2026-10-20. Related: #40/#49 W2,
[ADR-105](../architecture/ADR-105-local-owner-authentication.md),
[A0 results](local-owner-authentication-a0.md) and
[A0-A5 delivery gates](local-owner-authentication-verification.md).

## Purpose and decision boundary

AUTH-001 demonstrated a disposable Windows HTTPS transport/browser subset. It
did not establish a supported installation, protect files on behalf of callers,
or qualify CLI credential storage. Resolve those boundaries before A1. This plan
proposes the next bounded proof; it does not complete A0 or accept new architecture.

Keep A1-A5, production schema/runtime changes, real credentials and ADR-103
adoption outside this work. Browser cookies/CSRF remain A3 evidence. Broader
recovery, languages and agent execution remain post-MVP; replicated authority
and remote deployment remain B-032/#65. Stable actor/domain identities must not
depend on an OS account, file path, DPAPI blob or issuing process.

## Deployment clarification and remaining design

On 2026-10-21 the maintainer clarified that the intended backend runs in a Linux
container. Windows is a development/client host, not evidence of native Windows
backend placement. The former host-first diagnostic profile did not establish
the intended backend architecture; its reuse for server-key research is addressed
in the [post-mortem](../postmortems/2026-10-21-platform-assumption.md).

Client platforms are a separate scope: the client may target Windows and Linux,
with Windows the MVP priority. This does not select a Windows-only long-term
client or qualify a Linux client. Host-specific CLI credential storage and browser
trust must follow the client's actual OS, independently of the container backend.

AUTH-001's actual Windows-host diagnostic evidence remains a bounded transport
subset. It does not qualify container key storage or installation. Windows CLI
credential protection and browser trust remain distinct host concerns. A Windows
key-provisioning component is not assumed merely because the host is Windows.

Before backend-specific adapter work, validate the container boundary, TLS
termination/listener exposure, runtime identity and secret provisioning/mounts.
Reconcile container listener/forwarding with ADR-105's literal-loopback constraint
in a reviewed design; this correction grants no wildcard binding, proxy, shared
key mount or HTTP fallback. No container implementation or installation support
is claimed. Use the [framing pilot](../procedures/work-framing.md) before selecting
platform-specific mechanisms; do not defer the intended container target as a
later unrelated profile.

The [container design and probe proposal](local-owner-authentication-container-plan.md)
and [Proposed ADR-106](../architecture/ADR-106-local-container-transport.md) now make
that review concrete. They propose a specific container listener distinct from the
unchanged client origin and disposable mounted-secret generations. ADR-105's
current bind rule remains authoritative until explicit amendment acceptance;
neither proposal authorizes implementation, VM startup or trust changes. The
[review readiness checklist](local-owner-authentication-container-plan.md#review-follow-up-and-executable-readiness)
also gates clock/resume feasibility, canonical configuration and exact bundle
parsing; effective WSL state and real ingress/pause results remain unqualified.

The first-proof topology is now selected: same-host Podman with WSL 2, a Windows
client and Linux-container backend (maintainer clarification, 2026-10-22).
[Read-only feasibility](local-owner-authentication-podman-feasibility.md) found no
established full resume mechanism; active engine/network/clock behavior remains
unqualified. This is not a native Windows server or a separate Linux-host choice.

## Certificate lifecycle contract to prove

### Agreed PoC TLS profile

The maintainer explicitly agreed to these bounded PoC inputs on 2026-10-20.
This records that decision, not acceptance of the whole installation/storage plan
or qualification of a supported installation. Zero certificate fees are a project
constraint; the PoC requires no paid issuer or service.

| Input | Agreed scope |
| --- | --- |
| Issuance/trust | A disposable private CA signs a separate server certificate. Browser and CLI use ordinary certificate validation with explicitly approved trust; no warning bypass or permissive verifier. Keep issuer and server keys distinct. |
| Protocol | TLS 1.3 only; no TLS 1.2 compatibility profile or older protocol fallback. |
| Cipher suites | The reviewed rustls/ring TLS 1.3 AEAD suites: AES-128-GCM/SHA-256, AES-256-GCM/SHA-384 and ChaCha20-Poly1305/SHA-256. Retain library negotiation rather than inventing cryptography or copying an OpenSSL cipher string. |
| Initial certificate profile | ECDSA P-256 keys with SHA-256 certificate signatures for the disposable issuer and leaf; the leaf is a server certificate, not a CA, and has server-authentication usage. This is one test profile, not a general algorithm-support claim. |
| Server identity | Exact `iPAddress` SAN `127.0.0.1`; no wildcard or substitution of a DNS SAN/common name. Keep the literal-loopback listener and one configured HTTPS origin. |
| Lifecycle | Protected key storage and the renewal, expiry, interrupted-run reconciliation and removal oracles in this plan remain required. Trust changes need approval for the exact issuer/store/scope and cleanup. |

The [OWASP TLS Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Transport_Layer_Security_Cheat_Sheet.html)
recommends TLS 1.3 by default, optional TLS 1.2 compatibility, AEAD suites,
strong protected keys, SHA-256 certificate signatures, patched libraries and
testing the deployed configuration. It permits internal CAs for internal users
who explicitly trust them. This supports the selected approach without requiring
paid certificates; it is not an OWASP compliance certification.

OWASP's general naming guidance also advises against IP addresses in certificates.
The private loopback profile deliberately differs on this point:
[RFC 9525 sections 6.2 and 6.4](https://www.rfc-editor.org/rfc/rfc9525.html#section-6.2)
specify exact matching of an IP reference identity against an `iPAddress` SAN.
Document that scoped rationale rather than claiming literal adherence to every
OWASP deployment recommendation. No DNS, public issuer or remote deployment
profile is selected by this exception.

The existing AUTH-001 adapter supplies a single leaf and an explicit CLI root;
arbitrary intermediate chains, other algorithms and other trust configurations
are outside its demonstrated coverage. Keep issuance separate from certificate
loading so a test CA does not become a permanent issuer dependency. The concrete
tool/library inputs, adapters, operating limits and executable experiment still
need review; the rest of this plan remains Proposed and no new test is passed.

### Lifecycle acceptance criteria

Use disposable certificates and synthetic storage values only. Before execution,
choose and review an issuance method, exact tool/library versions, licenses,
features, key format and protected storage adapter. AUTH-001's fixture generator
is not an installer. No production issuer or storage wrapper is selected here.

Distinguish operator-supplied certificates, where issuance stays outside the
runtime, from local issuance. If the selected experiment generates or retains an
issuer signing key, record its owner, permitted readers, storage protection,
renewal access and retention/destruction policy. Include issuer keys in setup,
interruption and cleanup evidence; the serving process must not require access
to them. No issuer-key persistence or production CA is selected by this plan.

| Stage | Required behavior and failure evidence |
| --- | --- |
| Prepare | Identify the runtime OS user, installation identity, exact origin, file owners and trust consumers. Create a dedicated protected directory before writing a key. Refuse an existing incompatible owner, permissive access or redirected path. |
| Trust | Present the disposable issuer fingerprint, exact per-user store, privilege scope, lifetime and removal command for separate approval. Browser trust and explicit CLI roots are separate inputs. Preserve unrelated entries; no machine-wide trust or warning bypass. |
| Install/start | Load only the approved certificate/key pair through the protected adapter. Reject missing, oversized, malformed, expired, wrong-SAN or mismatched material before listening. Record readable error classes without secret values. |
| Renew | Prepare and validate a fresh key/leaf pair as one generation. Serialize replacement, switch the pair together and restart the diagnostic process; live reload is outside scope. A failed replacement leaves a complete prior generation or a stopped service, never a mixed pair. Prove the new leaf with actual browser and CLI validation. |
| Expiry/failure | Before execution, specify the measurable validity/admission and connection-drain policy below. A failed renewal never permits HTTP fallback, certificate-ignore mode or installation of a new issuer without approval. An expired prior generation cannot be a rollback target. |
| Remove | While the valid diagnostic server is still reachable, remove only experiment-owned browser trust and CLI roots; require specific certificate-trust failures in fresh clients. Then stop owned processes, release the listener and delete generated keys. Handle interrupted setup/renewal/removal with a bounded inventory of owned resources and repeatable cleanup. |

Before setup, compare the selected stores and artifact locations with the owned
resource inventory from any interrupted run. Record exact trust fingerprints,
store scope and owned artifacts before mutation; display names are not ownership
proof. Reconcile positively identified leftovers only within the approved trust
and cleanup scope, or stop for review. Preserve unfamiliar entries and files.
Never broaden cleanup to remove a certificate merely because its name resembles
a fixture. Resume only with a known baseline and repeatable owned cleanup.

Renewal restarts only the session-free diagnostic process in this proof. If that
strategy is later adopted for the authenticated local-MVP runtime, ADR-105's
restart invalidates browser, CLI and pre-session state; the owner signs in again.
Diagnostic renewal success is not evidence of authenticated logout behavior or
transparent session continuity.

The executable proposal must define the certificate validity-check mechanism,
its clock source, forward/backward clock-change behavior and a numeric maximum
detection delay. Specify when admission stops, how handshakes in progress and
connections established before expiry are handled, and a numeric drain deadline.
Define assertions at expiry and at the stated detection/drain boundaries; an
unbounded periodic check is insufficient. Server enforcement and browser/CLI
rejection of expired certificates require separate oracles. These operating
limits remain unselected and unqualified until reviewed and measured.

An old leaf may remain browser-trusted while its issuer remains trusted; renewal
alone is not evidence of revocation. Suspected key/issuer compromise requires a
separate replacement/removal procedure. Do not claim that file deletion securely
erases every storage copy or that uninstall invalidates an exported key.

## Protected storage contracts to review

Keep TLS server keys and CLI bearer files as distinct typed records with separate
access rules and lifecycles. Do not persist a password. Do not reuse a TLS private
key as a session secret or a CLI bearer as an installation identity.

| Record | Proposed contract |
| --- | --- |
| Backend server key | Intended runtime is a Linux container. Ownership, permissions, safe file operations and secret provisioning must be designed for that boundary; no Windows storage model is selected for it. |
| Windows CLI bearer | User-scoped DPAPI plus restricted owner ACL, as required by ADR-105. Verify user scope and UI suppression; reject Machine scope. Use synthetic values for the adapter proof; no login endpoint or real bearer is needed. |
| Linux CLI bearer | Preserve ADR-105's owned mode-0700 directory and mode-0600 file contract. Ownership, symlink refusal and replacement need real Linux evidence before qualification. Windows success does not qualify this path. |

For each adapter, review creation, opening, reading, replacement and cleanup as
one resource-ownership contract. Require bounded reads before parsing/decryption,
checked record kind/version and installation/origin binding. Propose a 64 KiB
encoded-file ceiling for the diagnostic proof, not a future credential format.
Reject corruption or unknown versions without fallback or overwriting evidence.

Check ownership and permissions on the opened object and address parent-directory
and final-component redirection, including Windows reparse points. A path check
followed by an ordinary open is insufficient against replacement races. Review
safe wrapper/native API guarantees and handle ownership; do not add raw unsafe
platform code to work around a missing library contract.

Create protected temporary files without a permissive interval. Serialize writers,
bound temporary generations and define same-filesystem replacement, flush and
restart reconciliation. Inject failures before/after replacement; never report a
durability guarantee stronger than the target filesystem proof. Concurrent readers
must see a complete old or new record. Wrong user, tampering, broad inherited ACLs,
redirection and concurrent replacement must deny or reconcile explicitly.

Review plaintext buffer copies, cancellation, FFI allocation/free ownership and
diagnostics. A zeroizing input buffer does not prove erasure of library-owned
key copies. No password, bearer, key or decrypted blob may enter logs, telemetry
or public evidence. DPAPI and ACLs do not contain code controlling the same OS
account; a DPAPI blob is not a portable identity backup. Restored CLI credentials
must not bypass ADR-105's restart/restore invalidation contract.

## Next proposal and execution gates

Before execution, submit a container-specific proposal defining the backend
boundary, TLS listener exposure/termination, runtime identity, key provisioning
and mounts. Justify any Windows CLI or host provisioning proof separately.
The retired Windows backend candidates and execution outline remain available
in the [historical plan](https://github.com/MrQuality/kehila/blob/708f8ab18f574dd07a4400cc24ef3b48935d376a/docs/implementation/local-owner-authentication-installation-plan.md#retained-windows-diagnostic-proposal-not-backend-qualification);
they are not the next backend qualification path.

The proposal must select adapters and issuance inputs after primary-source API,
dependency, license and advisory review. Preserve #40's separate production
dependency disposition and 2026-11-18 review deadline. Define numerical limits,
specific positive/negative oracles, failure recovery and owned cleanup for the
actual platform: clean setup, startup refusals, successful and interrupted renewal,
expiry, hostile storage, concurrent replacement and removal. An unrelated startup
or network failure cannot satisfy a negative oracle. Stop at an inconclusive
ownership, atomicity or trust prerequisite rather than widening the profile.
The earlier 90-minute active-investigation budget is a proposal,
not authorization for a new experiment; review its suitability with the scope.

Execution still requires separate permission for memory-heavy work, an available
RAM check, serialized builds and one Cargo job. Any trust change needs approval
for its exact scope. No new VM, backend service, account, security-setting change
or certificate trust is authorized by this document. Another-user tests require
an approved test identity or remain unqualified.

Keep confidential evidence, generated secrets and build/debug outputs outside the
checkout. Record actual source, platform/filesystem/tool versions, reached cases,
failures and cleanup; publish only reviewed, sanitized results. Exit only when the
selected adapters and matrix have actual evidence, storage failures preserve the
stated invariants and cleanup is confirmed. Unexecuted or inconclusive cases stay
open. Report host, Podman integration and Linux qualification separately; this
proof cannot mark a complete T01-T16 authentication case passed.

Update #40/#49 and linked backlog/delivery records with the reached subset and
remaining gates. A1 still requires a separately reviewed schema/capability plan
and applicable A0 prerequisites; this document authorizes neither implementation
nor release qualification.

## Primary references for adapter review

- [Microsoft CryptProtectData](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata): review user/Machine scope, UI flags and returned-buffer ownership.
- [Microsoft reparse points](https://learn.microsoft.com/en-us/windows/win32/fileio/reparse-points): review actual open/handle semantics rather than relying on path text.
- [Podman Windows tutorial](https://github.com/podman-container-tools/podman/blob/main/docs/tutorials/podman-for-windows.md): review the chosen VM provider, mounts and forwarding against the actual installed version before qualification.
