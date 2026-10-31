# Owner authentication: first capability delivery

Status: delivery scope authorized on 2026-10-27; implementation and qualification
in progress. Unaccepted contracts below remain Proposed. Related: #40/#49 W2,
B-007/B-020/B-022 and the [authentication verification plan](local-owner-authentication-verification.md).

## Outcome and merge boundary

Deliver one coherent owner-login capability through small implementation commits:
an operator provisions one disposable owner, and a Windows browser or CLI logs
in to the Rust authority in the selected same-host Podman/WSL 2 Linux container.
Login, logout, session expiry/revocation and restart rejection must be observed
on the explicitly qualified development profile. Do not describe this as a
supported installation, production secret store or real-data readiness.

The maintainer accepted capability-sized PRs on 2026-10-27; the
[delivery decision record](https://github.com/MrQuality/kehila/pull/87#issuecomment-6433529952)
records that authorization and its limits. Reader, schema,
credential, HTTP/client and qualification work are internal tasks of this owning
delivery package, not automatic independent merge units. The accepted ADRs,
dependency review and internal prerequisite gates still apply. A planning or
reader-only result does not complete this delivery.

Domain commands and authorized project/search reads belong to the later native
project delivery. This package supplies the credential/context infrastructure;
it does not qualify their A4 authority/revocation cases. Supported restore and
full installation remain later A5/M1 qualification obligations.

## Internal contracts and current gates

| Work | Current status / prerequisite |
| --- | --- |
| A0 private bundle reader | The [reader/startup record](local-owner-authentication-linux-reader-slice.md#protected-authentication-startup-2026-10-29) now includes opened-file checks, closed asset caps and actual UID/GID 10001 Podman secret admission, including the pinned corpus. Immutable provisioning, writable-alias exclusion, listener lifecycle and the integrated provider path remain gates. |
| Container transport | ADR-106's [authenticated-development extension](../architecture/ADR-106-local-container-transport.md#accepted-authenticated-development-extension-2026-10-27) was explicitly accepted on 2026-10-27 for synthetic disposable tests. Full application-container lifecycle, provider isolation and Windows-client qualification remain gates; the resource/restart subset is recorded separately below. Execution/trust permissions remain separate. |
| A1 owner storage/capabilities | The [exact A1 contract](local-owner-authentication-a1.md) defines records, role/function boundaries, installation-before-actor locks, generations, enrollment reconciliation and management audit identity. Its [disposable SQL boundary](../../experiments/AUTH-001/storage/README.md) has reviewed enrollment/authority/credential/session-audit functions and eleven native oracle groups with targeted regressions. The [Linux database bridge](local-owner-authentication-database-bridge.md) adds native TLS ownership/cancellation and typed transaction observations. The [coordinator record](local-owner-authentication-coordinator.md) adds actual PostgreSQL journeys for bounded jobs, publication, authority loss, withheld commit confirmation and revoke-other rollback/retry. Client/provider qualification and full request budgets remain pending. No production schema adoption follows from the delivery grouping. |
| A2 credentials/admission | The [credential core](local-owner-authentication-a2.md) connects through the owned coordinator: exclusive hash completion, fresh SQL fences, pre-session claims, generation retirement and fixed revoke-other batches have native subset evidence. Protected startup separately admitted the actual pinned corpus. Single-journey latency/RSS and typed adapter confirmation do not qualify a serving listener, Windows clients or full-request budgets. |
| A3 browser/CLI | The [listener and clients](local-owner-authentication-listener.md) connect bounded TLS dispatch, first-party browser controls and a Windows DPAPI CLI. The [corrected explicit-root journey](local-owner-authentication-listener.md#corrected-explicit-root-journey) observed 16 Windows Python/OpenSSL-to-container checks, including password change, other-session revocation, restart rejection and logout retry. Actual protected mounts and the two-related-component fixture were inspected. These establish that bounded functional journey; product Schannel/Chrome, external ingress, expiry/drain and full-request/capacity gates remain. Client-side Windows protection is distinct from Linux server-key storage. |
| Cross-package authority/recovery | Map remaining T01-T16 obligations to the later real command/read and supported restore deliveries. Never mark an entire case passed from an isolated helper or a partial oracle. |

No merged proposal, draft PR or reviewer verdict supplies missing maintainer
acceptance. The current Go/FerretDB prototype stays isolated and unauthenticated;
it must not become an alternate supported path.

On 2026-10-30 the maintainer explicitly accepted the
[local-caller amendment](../architecture/ADR-106-local-container-transport.md#accepted-local-caller-amendment-2026-10-30)
for disposable authenticated tests. Ordinary same-PC Windows/WSL/container
callers may reach the application but remain untrusted; external ingress,
protected files/database/admin boundaries and all authentication controls remain
qualification gates. The earlier guest-reachability failure is preserved against
its original contract. Complete product-client and ingress qualification is
still required; neither this
decision nor the passing component CI completes this delivery. The subsequent
[explicit-root journey](local-owner-authentication-listener.md#corrected-explicit-root-journey)
is partial integration evidence; product-client and complete ingress qualification
remain outstanding.

Additional [runtime and expiry subsets](local-owner-authentication-listener.md#bounded-connection-pressure-and-authority-loss-2026-10-30)
now observe sixteen held TLS connections with excess rejection, a 2.67-second
explicit stop, fail-closed database loss, natural runnable-lifetime termination
and controlled-history CLI expiry over actual TLS/HTTP. Their stated timing,
workload, test-history and client limitations remain part of the evidence.
They do not complete browser/product-client, full-load or ingress qualification.

The [product-client handoff correction and Windows journeys](local-owner-authentication-listener.md#windows-product-client-handoff-investigation-2026-10-31)
now distinguish complete guarded outbound delivery from later TLS cleanup.
The original order fails the new real-TLS credential-state regression; the
correction passes. Stock Windows CLI and fresh-profile Chrome lifecycle journeys
also passed, including an actual cross-client revocation target, password change
and controlled restart rejection. Peer-close failures remain counted and do not
establish clean reciprocal TLS closure. The normal shared staged gate passed for
the correction; current final-head CI is reported in the PR checks. Remaining
ingress, certificate and workload gates stay open; this is not full T01-T16 or
supported-installation qualification.
One positively witnessed blocked SQL read also ended with normal, joined cleanup
inside a conservative 4.69-second stop-to-exit interval and no authenticated
success after stop. This does not qualify active hashing, writes or all drain races.

The [sampled Android LAN check](local-owner-authentication-listener.md#sampled-android-lan-connection-check-2026-10-31)
observed a direct phone connection-stage timeout while authenticated loopback
controls passed before and after against the same running backend. Normal runtime
exit, database connection/lock release and owned cleanup were checked separately.
This supplies one external-peer subset, not the complete ingress matrix or a
diagnosis of the blocking layer. Earlier inconclusive attempts remain recorded.

The [complete-request workload and active-work subsets](local-owner-authentication-listener.md#complete-request-workload-and-active-work-subsets-2026-10-31)
add three measured verification-plus-replacement HTTP requests, process and
cgroup memory observations, overlapping 200/429 admission, and a normally joined
stop during observed password work. Blocked-write drain remains unresolved:
external timing includes notification overhead and does not establish the exact
runtime bound. The [explicit-root diagnostic certificate subset](local-owner-authentication-listener.md#explicit-root-diagnostic-certificate-subset-2026-10-31)
adds typed client-policy results without qualifying the Schannel/Chrome matrix
or authenticated browser expiry. These subsets do not complete the delivery.
The [short-lived certificate observation](local-owner-authentication-listener.md#short-lived-owner-certificate-and-held-connection-2026-10-31)
also records denial on one held connection after the expiry-detection allowance,
normal joined exit inside the detection/drain envelope and database-resource
release. It does not qualify platform-browser expiry or all active-work schedules.

## Definition of done and evidence

- [x] The bounded disposable authenticated-test profile has its
  [explicit acceptance](https://github.com/MrQuality/kehila/pull/87#issuecomment-6442532339).
- [x] The disposable A1 SQL, capabilities and transaction contracts have source
  review and native subset evidence before stateful runtime integration.
- [ ] Rust owns the dedicated PostgreSQL connection, bounded serving work and
  uncertain-outcome cleanup; observed loss suppresses admission and output.
- [x] Real Linux file admission and selected-provider mount/identity premises
  were observed in the bounded disposable fixture. This does not qualify a
  supported installation or every provider/file failure case.
- [x] Disposable one-owner enrollment, simulated lost-response reconciliation,
  narrow serving/admin rights, generation fencing and fixed audit confirmation
  have real PostgreSQL oracles. This does not qualify Rust credential eligibility.
- [ ] Browser/CLI login, rejection, logout, session expiry/revocation, password
  change and stale credentials after restart have applicable actual TLS/HTTP
  evidence. Domain-command and supported-restore cases remain explicitly mapped.
- [ ] Disposition the existing Medium/P2 Windows HTTP reliability follow-up before
  A3 qualification. Preserve both exchange and connection-driver outcomes and
  failed attempts; distinguish the documented serial harness from historical
  default-harness results. An unchanged passing rerun does not settle the cause.
- [ ] Complete verification-plus-replacement hashing latency, peak memory,
  capacity rejection and owned work/connection cleanup are measured against the
  proposed profile limits. Report overruns without silently relaxing limits.
- [ ] Each evidence record identifies source, commands/targets/harness, platform,
  tools/provider, sample/fixture, observed results, failed attempts and limits.
- [ ] Relevant failure regressions, mandatory commit gates and final-source CI
  pass; no skipped platform case is reported as qualified.
- [ ] Exact-head source review and dispositions are complete, related records are
  synchronized, and the final PR summary identifies what remains unqualified.

Trust-store changes require approval for the exact issuer, store, scope, lifetime
and removal. Do not disable TLS verification or change security-product settings.
Memory-heavy local execution requires its separate permission and RAM preflight;
keep build, temporary and confidential evidence outputs outside the checkout.
Apply the [user-assisted final-verification rule](../TESTING.md#automated-checks-and-user-assisted-final-verification):
finish and review harness preparation and automated checks before arranging the
bounded certificate/device campaign. Retain relevant evidence and repeat only
affected cases and controls after changes; do not use user confirmations to
debug unfinished harnesses.

The existing root-TLS advisory triage and Windows diagnostic/shared-verifier
follow-ups in the [A0 record](local-owner-authentication-a0.md#advisory-follow-up)
remain open. This delivery introduces no risk acceptance or full-graph clean claim.
