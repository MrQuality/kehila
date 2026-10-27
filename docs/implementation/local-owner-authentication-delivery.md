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
| A0 private bundle reader | The [reader contract](local-owner-authentication-linux-reader-slice.md) defines descriptor ownership, exact object/owner/mode admission and a capped read. Implement it without a serving caller; review the exact Linux dependency graph and real Linux results separately. |
| Container transport | ADR-106's [authenticated-development extension](../architecture/ADR-106-local-container-transport.md#accepted-authenticated-development-extension-2026-10-27) was explicitly accepted on 2026-10-27 for synthetic disposable tests. Actual provider isolation, lifecycle and Windows-client qualification remain unexecuted gates; execution/trust permissions are separate. |
| A1 owner storage/capabilities | The [exact A1 contract](local-owner-authentication-a1.md) defines records, role/function boundaries, installation-before-actor locks, generations, enrollment reconciliation and management audit identity. Its [enrollment/authority/credential-management subset](../../experiments/AUTH-001/storage/README.md) has reviewed SQL and nine native oracle groups with targeted regressions. Session issuance/logout/individual-revocation audit APIs/proof and Rust integration remain pending. Review all actual SQL/rights before stateful runtime integration. No production schema adoption follows from the delivery grouping. |
| A2 credentials/admission | Follow ADR-105 password/session policy; select reviewed primitives and qualify complete hash-job ownership and bounded registries before HTTP integration. Provisional budgets are inputs to measure, not established performance. |
| A3 browser/CLI | Integrate actual TLS, pre-session/CSRF, cookies, CLI credential protection, logout/password change and rejected/saturated requests on the selected profile. Client-side Windows protection is distinct from Linux server-key storage. |
| Cross-package authority/recovery | Map remaining T01-T16 obligations to the later real command/read and supported restore deliveries. Never mark an entire case passed from an isolated helper or a partial oracle. |

No merged proposal, draft PR or reviewer verdict supplies missing maintainer
acceptance. The current Go/FerretDB prototype stays isolated and unauthenticated;
it must not become an alternate supported path.

## Definition of done and evidence

- [x] The bounded disposable authenticated-test profile has its
  [explicit acceptance](https://github.com/MrQuality/kehila/pull/87#issuecomment-6442532339).
- [ ] Exact A1 SQL, capabilities and transaction contracts have their required
  review before stateful runtime integration.
- [ ] Real Linux file admission and selected-provider mount/identity premises are
  observed; no Windows pass or mocked read substitutes for those observations.
- [ ] One-owner enrollment, uncertain-commit reconciliation, narrow serving versus
  administration privileges and generation fencing have real PostgreSQL oracles.
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

The existing root-TLS advisory triage and Windows diagnostic/shared-verifier
follow-ups in the [A0 record](local-owner-authentication-a0.md#advisory-follow-up)
remain open. This delivery introduces no risk acceptance or full-graph clean claim.
