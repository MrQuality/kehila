# Remaining A0: local installation and protected storage

Status: Proposed; no experiment or dependency adoption authorized by this plan.
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

## Proposed installation profile

Start with the browser, CLI and Rust diagnostic process on Windows, running as
the same explicitly identified non-administrative OS user. Keep the Rust listener
on literal `127.0.0.1`; use one configured HTTPS origin and explicit CLI trust.
This is a proposal for proof scope, not a newly supported deployment.

Treat Podman as the backend-service environment for this proposed profile.
The first certificate/storage proof needs no backend services or VM. Before
claiming Windows/Podman support, review the exact service topology and separately
prove the required host-to-backend connections without exposing anonymous
application, worker or search endpoints as an alternate supported route.
Backend connectivity/role evidence belongs to the relevant A1/A3 integration gate;
this experiment alone cannot qualify that installation.

A Rust authority inside a Linux container is a different profile. Windows DPAPI,
Windows ACLs and host loopback cannot be assumed to carry across that boundary.
Record its key owner, mount access, container listener and host forwarding policy
in a separate reviewed proposal if required. Do not change to wildcard binding,
export a server key through a shared mount, or introduce a TLS proxy to make the
host proof pass. Linux file/storage qualification remains separate.

## Certificate lifecycle contract to prove

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
| Expiry/failure | Define a certificate validity check during service life; do not silently serve indefinitely past expiry. A failed renewal never permits HTTP fallback, certificate-ignore mode or installation of a new issuer without approval. An expired prior generation cannot be a rollback target. |
| Remove | While the valid diagnostic server is still reachable, remove only experiment-owned browser trust and CLI roots; require specific certificate-trust failures in fresh clients. Then stop owned processes, release the listener and delete generated keys. Handle interrupted setup/renewal/removal with a bounded inventory of owned resources and repeatable cleanup. |

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
| Windows server key | Compare the two candidate models below; both require a restricted owner ACL, explicit runtime-user access and no shared/public file fallback. Review the selected key loader and any decryption adapter; CLI DPAPI evidence does not qualify server-key handling. |
| Windows CLI bearer | User-scoped DPAPI plus restricted owner ACL, as required by ADR-105. Verify user scope and UI suppression; reject Machine scope. Use synthetic values for the adapter proof; no login endpoint or real bearer is needed. |
| Linux CLI bearer | Preserve ADR-105's owned mode-0700 directory and mode-0600 file contract. Ownership, symlink refusal and replacement need real Linux evidence before qualification. Windows success does not qualify this path. |

For the Windows server key, evaluate these candidates before selecting an adapter:

| Candidate | Required evaluation |
| --- | --- |
| ACL-restricted plaintext key file | Verify the opened object's owner/access rules and redirection refusal. Account for readable on-disk key material, temporary generations, replacement and cleanup. |
| User-scoped DPAPI key file, decrypted at startup | Verify the runtime identity can decrypt without UI, reject Machine scope, and review plaintext buffers, loader/provider copies and FFI allocation/free ownership. Account for decryption failure, replacement and recovery without a plaintext fallback. |

For both, record startup identity, unattended operation, plaintext copies,
replacement, recovery and failure behavior against the actual library and
filesystem. A zeroizing input buffer does not prove erasure of library-owned
key copies. Selection remains subject to dependency review and actual evidence.

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
diagnostics. No password, bearer, key or decrypted blob may enter logs, telemetry
or public evidence. DPAPI and ACLs do not contain code controlling the same OS
account; a DPAPI blob is not a portable identity backup. Restored CLI credentials
must not bypass ADR-105's restart/restore invalidation contract.

## Bounded execution proposal and exit criteria

1. Review this profile, unresolved adapter/issuance choices and failure oracles.
   Complete primary-source API and dependency/license/advisory review before
   adding even experimental dependencies. Retain #40's existing production
   dependency disposition and 2026-11-18 review deadline independently.
2. Submit a concrete isolated experiment scope and selected inputs for review.
   Propose a 90-minute active investigation budget excluding fetch/build,
   required verification and permission waits. Stop at the budget or an
   inconclusive ownership/atomicity/trust prerequisite; record the unresolved
   result rather than widening the profile.
3. Obtain separate permission for memory-heavy work and check available RAM;
   serialize builds and use one Cargo job. Obtain separate approval for the exact
   trust change. No new VM, backend service, account, security-setting change or
   certificate trust is authorized by approving this document.
4. Exercise real Windows files and actual browser/CLI validation: clean setup,
   all startup refusals, successful renewal, failed/interrupted renewal,
   expiry behavior, hostile storage cases, concurrent replacement and removal.
   For another-user denial, use a separately approved test identity or record
   the case unqualified; do not create accounts implicitly. Give each negative
   case a specific diagnostic oracle; unrelated startup/network failure cannot pass.
5. Retain confidential evidence, generated secrets and build/debug outputs outside
   the checkout. Record source, platform/filesystem/browser/tool versions,
   reached cases, failures and cleanup. Publish only reviewed, sanitized results.

Exit only when the selected profile and exact adapters are reviewed, the matrix
has actual results, failures preserve the stated storage invariants and cleanup
is confirmed. A failed or unexecuted case stays open. Report host installation,
Podman integration and Linux qualification separately. This proof cannot mark a
complete T01-T16 authentication case passed.

Update #40/#49 and the linked backlog/delivery records with the reached subset
and remaining gates after the experiment. A1 still requires a separately reviewed
schema/capability plan and applicable A0 prerequisites; this document authorizes
neither implementation nor release qualification.

## Primary references for adapter review

- [Microsoft CryptProtectData](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata): review user/Machine scope, UI flags and returned-buffer ownership.
- [Microsoft reparse points](https://learn.microsoft.com/en-us/windows/win32/fileio/reparse-points): review actual open/handle semantics rather than relying on path text.
- [Podman Windows tutorial](https://github.com/podman-container-tools/podman/blob/main/docs/tutorials/podman-for-windows.md): review the chosen VM provider, mounts and forwarding against the actual installed version before qualification.
