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
| A0 private bundle reader | The [reader/startup record](local-owner-authentication-linux-reader-slice.md#protected-authentication-startup-2026-10-29) and [integrated journey](local-owner-authentication-listener.md#corrected-explicit-root-journey) record opened-file caps, UID/GID 10001 secret mounts, the pinned corpus and sampled other-UID denial. These qualify the inspected disposable mounts, not immutable provisioning or writable-alias exclusion for a supported installation. |
| Container transport | ADR-106's [authenticated-development extension](../architecture/ADR-106-local-container-transport.md#accepted-authenticated-development-extension-2026-10-27) is accepted for synthetic tests. Actual Windows clients, capped resource/lifecycle subsets and a sampled Android LAN negative are now recorded below. The full ingress matrix and browser host-listener cleanup disposition remain open; execution/trust approvals stay separate. |
| A1 owner storage/capabilities | The [exact A1 contract](local-owner-authentication-a1.md) defines records, role/function boundaries, installation-before-actor locks, generations, enrollment reconciliation and management audit identity. Its [disposable SQL boundary](../../experiments/AUTH-001/storage/README.md) has reviewed enrollment/authority/credential/session-audit functions and eleven native oracle groups with targeted regressions. The [Linux database bridge](local-owner-authentication-database-bridge.md) adds native TLS ownership/cancellation and typed transaction observations. The [coordinator record](local-owner-authentication-coordinator.md) adds actual PostgreSQL journeys for bounded jobs, publication, authority loss, withheld commit confirmation and revoke-other rollback/retry. Later client and complete-request observations are recorded below; the remaining profile gates are listed in the handoff audit. No production schema adoption follows from the delivery grouping. |
| A2 credentials/admission | The [credential core](local-owner-authentication-a2.md) and owned coordinator have native eligibility, hash ownership, generation retirement and revoke-other evidence. [Complete-request measurements](local-owner-authentication-listener.md#complete-request-workload-and-active-work-subsets-2026-10-31) now add three verification-plus-replacement requests, distinct process/cgroup memory counters and overlapping 200/429 admission. These are small samples; the rejected admission layer and sustained capacity are not isolated. |
| A3 browser/CLI | The [Windows journeys](local-owner-authentication-listener.md#windows-product-client-handoff-investigation-2026-10-31) record stock CLI and Chrome login, password change, revoke-other, logout and restart rejection. [Final certificate/Chrome observations](local-owner-authentication-listener.md#final-windows-certificate-and-chrome-expiry-observations-2026-11-01) add six diagnostic certificate cases and constructed browser expiry. Full ingress/lifecycle and the cleanup disposition below remain open. Client-side DPAPI is distinct from Linux server-key storage. |
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
stop during observed password work. A later
[directly measured blocked-write stop](local-owner-authentication-listener.md#measured-blocked-write-stop-2026-10-31)
returned normally in 3.829 seconds with unchanged durable credentials/audit and
released database resources; earlier failed external timing observations remain
recorded. This does not qualify all write/cancellation schedules. The
[explicit-root diagnostic certificate subset](local-owner-authentication-listener.md#explicit-root-diagnostic-certificate-subset-2026-10-31)
adds typed client-policy results without qualifying the Schannel/Chrome matrix
or authenticated browser expiry. These subsets do not complete the delivery.
The [short-lived certificate observation](local-owner-authentication-listener.md#short-lived-owner-certificate-and-held-connection-2026-10-31)
also records denial on one held connection after the expiry-detection allowance,
normal joined exit inside the detection/drain envelope and database-resource
release. It does not qualify platform-browser expiry or all active-work schedules.
The [partial Windows certificate campaign](local-owner-authentication-listener.md#windows-certificate-campaign-partial-observations-2026-10-31)
adds valid/renewed acceptance and trust-removed rejection. A failed DOM-only error
assertion remains recorded. The [subsequent final campaign](local-owner-authentication-listener.md#final-windows-certificate-and-chrome-expiry-observations-2026-11-01)
completed the six diagnostic certificate-policy cases and actual Chrome
constructed idle/absolute expiry observations. The browser run's host-listener
cleanup assertion failed; later verified absence, exact-root removal and the
trust-removed negative are separate reconciliation evidence. This does not
qualify every cleanup schedule, elapsed TTL or the full capability.
The [browser-cookie expiry wire prerequisite](local-owner-authentication-listener.md#browser-cookie-expiry-wire-prerequisite-2026-11-01)
now passes constructed idle/absolute histories over real TLS/HTTP, with an
intended expiry-bypass mutation failure and restored pass. This is an automated
prerequisite; the subsequent Chrome observations retain their test-owned
listener and constructed-history limits.

## Remaining capability handoff audit (2026-11-01)

This audit reconciles the existing evidence; it authorizes no new execution or
risk acceptance. Unchecked criteria below must not be read as saying that their
recorded subsets never ran.

| Item | Existing evidence and remaining disposition |
| --- | --- |
| Windows HTTP reliability | The [controlled shutdown investigation](local-owner-authentication-a0.md#controlled-windows-shutdown-investigation-2026-10-27) reproduced a post-response failure, failed the original lifetime order, and passed a controlled remedy plus targeted regressions. The later [outbound-confirmation correction](local-owner-authentication-listener.md#windows-product-client-handoff-investigation-2026-10-31) failed its original-order regression and passed real client journeys. This is an engineering disposition for those captured classes, not attribution of all historical failures or clean reciprocal closure. Maintainer review of that bounded disposition remains separate; #40's other obligations stay open. |
| Selected-profile ingress | The [selected-profile path observations](local-owner-authentication-listener.md#selected-profile-ingress-and-public-runtime-release-2026-11-01) now add assigned Windows IPv4/IPv6 negatives, guest/private paths, ordinary-guest TLS/HTTP rejection and before/after configuration/publication evidence. These remain bounded samples of the selected NAT profile, alongside the [database guest/container negatives](local-owner-authentication-listener.md#single-network-native-request-subset-2026-10-30) and [external phone sample](local-owner-authentication-listener.md#sampled-android-lan-connection-check-2026-10-31). They do not qualify other provider modes or every external route. |
| Listener release | Rust/coordinator joins and database release passed in the final browser fixture. Its host-listener cleanup assertion failed; later absence and stopped-provider/cleared-resource observations remain separate reconciliation. The [public-runtime follow-up](local-owner-authentication-listener.md#selected-profile-ingress-and-public-runtime-release-2026-11-01) observed normal lifetime exit, sampled post-exit TCP noncompletion and owned cleanup. It did not reproduce or establish the cause/release interval of the different browser fixture's failure. An empty Windows TCP inventory also occurred while live TCP connected; inventory alone is insufficient. No new trust ceremony or generic test series follows from this unresolved historical limit. |
| Candidate handoff | Synchronize this scope, evidence, failed attempts and limitations in the PR/issues, obtain exact-head review and final-head hosted CI. Keep the PR's status and merge decision with the maintainer. |

Domain-command/read authority, supported restore, supported installation and
real-data readiness remain later delivery/M1 obligations. They are not newly
added prerequisites for this disposable owner-login capability. Constructed
expiry evidence does not become elapsed-TTL or complete T01-T16 qualification.

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
