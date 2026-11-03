# Hosted verification research and risk management plan

Status: Proposed design support, 2026-11-03. Owner: project maintainer.
Decision: [ADR-110](../architecture/ADR-110-hosted-verification-testbench.md).
The original design milestone contained documentation only. Its probes below
remain admission or qualification criteria, not executed results. On 2026-11-04
the maintainer requested completion of the pending feasibility spikes; actual
experimental scope, permissions, observations and remaining gates are recorded
separately in [SP-007](../spikes/SP-007-hosted-testbench.md). That research does
not accept this architecture or change required gates.

## Research findings and evidence limits

Research date: 2026-11-03. Repository source baseline:
`7f9301601c5de4c87f8cad7c3b1c1681d0769336`.
External sources are first-party documentation fetched during this assessment;
their current descriptions are not compatibility evidence for this project.

| Finding | Evidence and implication |
| --- | --- |
| Hosted capacity and billing | [GitHub runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners): standard public x64 Linux jobs offer 4 CPUs/16 GB RAM/14 GB SSD; standard public compute is free. [Limits](https://docs.github.com/en/actions/reference/limits) include a six-hour hosted-job maximum, not a session expiry guarantee. Queue latency and cold preparation remain unmeasured. |
| Platform usage | [GitHub product terms](https://docs.github.com/en/site-policy/github-terms/github-terms-for-additional-products-and-features) tie hosted use to the repository's software lifecycle and prohibit disproportionate burden. Bounded testing is the intended use here; continuous hosting is excluded. No provider-specific approval is inferred. |
| Jobs do not share a server automatically | [Service-container networking](https://docs.github.com/en/actions/tutorials/use-containerized-services/use-docker-service-containers): services belong to their job. Separate jobs require real networking and overlapping lifetimes; a completed backend job cannot serve a dependent client job. |
| A private overlay is plausible | [GitHub WireGuard guide](https://docs.github.com/en/actions/how-tos/manage-runners/github-hosted-runners/connect-to-a-private-network/connect-with-wireguard) explains reachable-endpoint requirements and Tailscale NAT traversal. [Connection types](https://tailscale.com/docs/reference/connection-types) distinguish direct and relay paths; relay timing is not a latency guarantee. |
| Process-scoped connector primitives exist | [tsnet overview](https://tailscale.com/docs/features/tsnet) and [API](https://tailscale.com/docs/reference/tsnet-server-api) provide embedded nodes and raw Dial/Listen. Userspace operation avoids a required system-wide interface. The project's exact Windows/WSL binaries, policy, forwarding and dependency closure are unqualified. |
| Enrollment and expiry differ | [Auth keys](https://tailscale.com/docs/features/access-control/auth-keys): enrollment-key expiry does not revoke an enrolled node; tagged node-key expiry is disabled by default. [Ephemeral nodes](https://tailscale.com/docs/features/ephemeral-nodes) normally disappear 30–60 minutes after inactivity. Neither establishes a 30-minute active-session cutoff. |
| Rules must exclude broad defaults | [ACL examples](https://tailscale.com/docs/reference/examples/acls) and [grants](https://tailscale.com/docs/reference/syntax/grants): review the entire effective policy, including defaults and additive grants. Identity/tag ownership is as important as the narrow port rule. |
| Stock browser choice affects tooling | [Playwright browsers](https://playwright.dev/docs/browsers) says Firefox requires patches; branded Chrome can be selected explicitly. [Selenium Firefox](https://www.selenium.dev/documentation/webdriver/browsers/firefox/) and [Chrome](https://www.selenium.dev/documentation/webdriver/browsers/chrome/) document browser selection and headless arguments. Prefer one WebDriver-based matrix for actual stock executables; browser/profile trust remains a separate proof. |
| Implicit downloads undermine reproducibility | [Selenium Manager](https://www.selenium.dev/documentation/selenium_manager/) automates driver/browser management. Explicit preparation and fixed binaries are proposed instead of discovery/download during a test. [WebDriver options](https://www.selenium.dev/documentation/webdriver/drivers/options/) include certificate-error bypass; the harness must explicitly disable it. |
| WSL is a distinct client environment | [Microsoft networking](https://learn.microsoft.com/en-us/windows/wsl/networking) distinguishes NAT and mirrored behavior. [WSLg](https://learn.microsoft.com/en-us/windows/wsl/tutorials/gui-apps) supports Linux GUI applications under WSL 2, not a full independent desktop. Headless Linux execution is still Linux even without a visible WSLg window. |
| OpenTofu has a narrower role | [Overview](https://opentofu.org/docs/intro/) covers provider-managed infrastructure; [sensitive state](https://opentofu.org/docs/language/state/sensitive-data/) and [encryption](https://opentofu.org/docs/language/state/encryption/) require deliberate state/key controls. It is useful if a broker or VM is adopted, not necessary for the initial hosted-job lifecycle. |
| Certificate identity and port agreement differ | [RFC 9525 section 6.4](https://www.rfc-editor.org/rfc/rfc9525.html#section-6.4) matches an IP SAN against IP bytes. The port agreement comes from `startup.rs` and exact HTTP Host/Origin admission; do not mint an IP SAN containing a port. |
| Browser profile and certificate trust can have different lifetimes | [Chromium Linux certificate management](https://chromium.googlesource.com/chromium/src/+/main/docs/linux/cert_management.md) describes a shared NSS database. Actual trust-store isolation and cleanup need browser/OS-specific evidence. |
| Dispatch and secrets are different interfaces | [GitHub dispatch inputs](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#workflow_dispatch) and [secret scopes](https://docs.github.com/en/actions/how-tos/write-workflows/choose-what-workflows-do/use-secrets) do not supply a documented automatic confidential single-run payload. Select and qualify the session-bound delivery mechanism before secret use. |

### Cost and capacity assumptions

[Tailscale pricing](https://tailscale.com/pricing), as fetched on the research
date, lists Personal at $0 with 1,000 ephemeral-resource minutes/month, Standard
at $8/user/month with 1,000, and Premium at $18/user/month with 10,000. Extra
ephemeral capacity requires a sales inquiry. Eligibility, billing classification
of each connector and applicable terms must be confirmed before adoption.

Illustration only: 20 sessions x 30 minutes x one ephemeral runner identity =
600 runner-identity minutes, excluding preparation, delayed cleanup and other
identities. Charging three ephemeral identities for those intervals gives 1,800
minutes. Neither calculation is a bill prediction. Do not keep sessions alive
to exploit a different accounting category. No paid purchase is authorized.

Keep one Cargo job and serial heavy service phases. Measure total job RAM,
container peaks, available disk before/after preparation, image/cache growth,
preparation/test/cleanup time, client RAM and direct/relay behavior. Compare
the same client scenario with and without backend offload; use no invented
percentage savings. Local WSL/browser memory and the unchanged staged gate
remain material costs. The current 2 GiB runner limit excludes service/engine
overhead; no service-wide total bound follows from it.

## Component and contract map

| Boundary | Required behavior and proposed code location |
| --- | --- |
| Workflow admission | New separate session workflow; trusted workflow and reviewed immutable source; read-only repository token by default; no secrets in ordinary PR jobs. The existing `ci.yml` is not repurposed into a waiting server. |
| Hosted fixture | New session lifecycle entry point adjacent to container verification; reuse protected startup/fixture primitives after review, not diagnostic production endpoints or imported experimental application code. Backend/database remain contained and isolated. |
| Local connector | Test-only helper retains one bound loopback endpoint across dispatch and attachment, then forwards only after admission to one fixed peer destination. No generic proxy/control socket; independent build/dependency review. No production linking to tsnet is proposed. |
| Runner connector | Raw overlay listener in its own restricted container/network boundary; fixed authority container address/port; no engine socket, host namespace, privileged mode or database forwarding. |
| Browser runner | Python WebDriver suite with pre-admitted browser/driver, loopback driver endpoint, fresh protected profile, separately qualified trust store, one worker and no automatic mutation retries. Windows and WSL emit separate results. |
| Secret delivery | Select authenticated sender/recipient and confidential transit/storage bound to the session and run attempt; reject replay/cross-session delivery and reconcile interruption/cleanup. No secret dispatch input or public artifact; issuer signing key stays outside the transfer. |
| Evidence collector | Bind exact source/workflow/run attempt and case manifest; distinguish passed/failed/incomplete and cleanup outcome; public allowlist of reviewed fields. Raw screenshots, network traces, profiles and driver logs stay protected. |
| Future gate replacement | Separate exact-staged-tree design and policy review. No source upload, bypass flag, new remote hook or removed check in this PR. |

Expected application changes: none identified for the proposed browser path.
`startup.rs` and `http_auth.rs` retain exact origin/TLS admission; `page.js`
retains relative requests. A demonstrated incompatibility reopens ADR-110 rather
than justifying silent relaxation. The Windows CLI's DPAPI credentials do not
become a Linux credential-store design.

## Risk management plan

Likelihood is unknown unless an observation is stated. Impact and urgency are
separate: Critical/High describes consequence; P1 means resolve before the named
dependent work, P2 before cutover/qualification. Owners below are proposed roles;
the maintainer assigns them and retains acceptance authority. Risk scores or
reviewer agreement cannot accept architectural/security tradeoffs.

| Risk and invariant | Evidence, confidence and likelihood | Impact / urgency | Control and cheapest future check / cost | Residual owner, acceptance and invalidation |
| --- | --- | --- | --- | --- |
| R1 Untrusted revision receives network or fixture credentials | Public PR scripts execute arbitrary code; [GitHub secure-use guidance](https://docs.github.com/en/actions/reference/security/secure-use). High source confidence; unknown likelihood. | Critical / P1 before enrollment | Admit workflow and source SHAs separately; narrow credentials and immutable dependencies. Negative admission test for fork, substituted SHA and changed workflow; low compute, no local network needed. | Workflow owner; maintainer accepts execution boundary. Reopen on action/workflow/token-scope changes. |
| R2 Overlay provides new access to local or unrelated services | Userspace is not an access policy. Source-derived threat; unknown likelihood. | Critical / P1 before local connection | Dedicated identities, effective deny-by-default policy, no reverse listener/proxy, fixed destination. Retain the bound local endpoint across dispatch. Check port collision/lost ownership, remote-originated local connection, DB/engine sockets and other-session peers; bounded isolated network cost. | Transport owner; maintainer accepts policy/vendor boundary. Reopen on policy/tag/connector/network change. |
| R3 TLS/origin protection is bypassed or trust escapes its approved scope | Exact loopback is enforced today; WebDriver can ignore certificate errors; Chromium Linux can use shared certificate storage. High confidence; unknown likelihood of misconfiguration. | Critical / P1 before browser qualification | Keep Rust TLS and same origin; explicit false bypass capability. Prove intended positive and wrong-CA/IP/issuer/expired-cert negative cases in each browser, actual trust-store scope, no unintended ordinary-browser trust, and exact removal after interruption. Bounded temporary trust work only after exact approval. | Security/harness owner; maintainer controls trust approval. Browser/profile/trust changes invalidate affected evidence. |
| R4 Access or active streams outlive the session owner | Auth-key expiry and ephemeral GC do not impose active-session expiry; co-located timers share runner failure. High confidence; unknown incident likelihood. | High / P1 before live session | Select trusted supervisor and protected remote revocation path with explicit failure domains. Separately observe stream closure, owned-process termination and enrolled access. Test controller death, runner loss, active-stream expiry, control-plane outage and local restart; medium harness work. Withhold live admission until bounded residual exposure is explicitly accepted. | Transport owner; maintainer accepts outage/expiry residual. Reopen on supervisor, credential or provider semantics. |
| R5 Descriptor/result is forged, replayed or secret delivery crosses sessions | GitHub authentication does not privatize public artifacts or establish a confidential per-run channel. High confidence; unknown likelihood. | High / P1 before secret use or attachment/publication | No secret descriptor fields; authenticate expected run/attempt/workflow/source and peer. Select confidential delivery; reject stale, replayed or wrong-recipient transfers; reconcile interrupted delivery/cleanup. Exercise collector redaction with synthetic canaries offline. | Evidence/secret-delivery owners; maintainer owns channel acceptance and publication. Channel, schema, collector or storage changes invalidate controls. |
| R6 Passing subset or error classification hides a product/harness/cleanup failure | Current reporting has distinct ownership and failure obligations; expected HTTP rejection is case-specific evidence. High source confidence; unknown likelihood. | High / P1 before gate use | Exact case manifest, separate cause/stage and qualification, durable first unexpected failure and independent DB/read-back oracle. Confirm expected rejection can pass its oracle; product failure, missing report, TLS/transport/environment interruption and cleanup failure cannot produce a campaign pass. Low focused cost before browser campaign. | Harness owner; maintainer accepts evidence. Case/oracle/result-contract changes reopen review. |
| R7 Capacity or WAN delay produces false eligibility claims | Current CI passes do not bound cold disk, relay latency or local RAM. High confidence in gap; workload probability unknown. | Medium / P2 before qualification | Measure both client environments, cold/warm hosted phases and relay path. Preserve deadlines; classify environment failure and retain attempt, not blind retries. Medium compute/time after resource approval. | Verification owner; maintainer selects paid capacity if warranted. Runner/image/region/browser changes invalidate measurements. |
| R8 Browser label overstates platform coverage | Patched Firefox, Chromium and Windows-launched-from-WSL differ from intended binaries. High source confidence; unknown occurrence. | Medium / P2 before qualification | Record actual executable identity, version, OS, profile and capability; prove JS/cookie/CSRF flow. Fresh fixtures for password/revocation cases. Small sequential matrix. | Browser owner; maintainer accepts support scope. Binary/profile/OS changes require affected checks. |
| R9 Local gate is removed before equivalent hosted proof | Current hook checks immutable staged index plus unchanged HEAD. High confidence; unknown migration likelihood. | High / P1 before cutover | Keep gate; separately test index/HEAD mutation, wrong tree/base, absent report and uncommitted-source disclosure controls. Cutover amendment and rollback rehearsals required. | Delivery owner; maintainer accepts gate policy. Hook/source-transfer/required-check changes reopen admission. |
| R10 Connector quota or vendor failure blocks testing | Current pricing has finite ephemeral minutes; service dependency is new. High source confidence; unknown usage/outage likelihood. | Medium / P1 before vendor enrollment | Confirm plan and identity accounting; explicit budget and quota monitor. Recheck pricing and measure session lifespan. Low research cost; no purchase now. | Bench operator; maintainer accepts vendor/budget. Pricing, usage or account changes reopen decision. |

Review risks at topology/dependency changes, before live admission, before gate
cutover and at delivery closeout. Do not repeat the entire register per commit.
Stop affected work on a failed isolation/TLS/expiry control; preserve failure
and owned-cleanup evidence, repair the harness, then rerun only affected cases
and necessary controls. Existing required gates are not waived by this rule.

## Qualification criteria and staged delivery

| Checkpoint | Required evidence before continuing | Remaining authority |
| --- | --- | --- |
| A Design acceptance | Maintainer dispositions of ADR-110's four decisions; topology and lifecycle invariants may be accepted with mechanism/plan admission gates still outstanding; no claim of runtime proof | No implicit trust, account creation, purchase or test execution permission |
| B Harness preparation | Exact expiry/control-path mechanism and vendor plan selected; dependency/connector review; bounded positive/negative lifecycle tests; actual Windows/WSL and Podman compatibility admission; immutable source/descriptor verification | Exact resources/network/local-helper permissions before live probes |
| C Automated integration | Sequential stock browser cases with strict TLS; sign-in, pending save without creation, reload/discovery, explicit execution/cancel, reconciliation and fresh authorized read; independent final-state oracle | Product behavior and five-second limits unchanged |
| D Final local qualification | Batched Windows/WSL headless cases plus justified headed cases; explicit certificate approval/cleanup; failure, disconnect, expiry and revoked-session controls; measured RAM and timing | User attention/trust scheduled only after harness repair/review |
| E Gate cutover | Separate accepted staged-source contract; proof missing/wrong/stale evidence fails; existing coverage preserved; rollback and publication controls | Current local/CI policy remains until explicit cutover acceptance |

Isolation controls include allowed application port and denied database/engine/
unrelated local ports, other sessions, reverse initiation and wrong overlay peer.
Lifecycle controls include duplicate attachment, timeout before readiness,
stale run attempt, connection loss before/after commit, controller kill without
finalization and expiry during an active stream. A cancelled or incomplete
campaign cannot yield a passing terminal result. A retry preserves the original
failure and receives a new attempt/fixture identity.

Admission controls must additionally cover retained-port ownership through a
delayed dispatch and safe failure if that ownership is lost; each selected
Windows/WSL path uses its actual networking configuration. Browser trust controls
must identify all affected stores and show that removing the test profile alone
does not conceal residual trust elsewhere. Secret-delivery controls must reject
wrong run/attempt/recipient and replay, preserve issuer-key separation, and
reconcile partial delivery without exposing keys in public output. Result-model
controls must retain an unexpected failure through later transport loss, cleanup
and successful fresh attempts, while allowing a correctly witnessed negative
application case to pass. These refine existing R2–R6 admission obligations;
they do not report an executed defect or authorize a live campaign.

Keep commits focused within coherent capabilities: session ownership/transport,
browser journeys, then separately accepted gate migration. Do not split tightly
coupled producer/consumer changes just to satisfy a line count. Queue existing CI
at the first coherent checkpoint; no generic extra qualification campaign.

## Review and delivery record

Authorized scope: research and a Proposed design/risk PR. Completion criteria:
source-backed alternatives and recommendation; Windows/WSL/code-change map;
concrete risks, owners and admission controls; external-cost/compatibility gaps;
independent relevance/final-head review; required documentation checks and PR.
No implementation, service/container/browser run, enrollment or trust operation
is part of this delivery. Component runtime reviews and live harness execution
are inapplicable here; their future admission obligations are defined above.

Request-first independent framing inspected the baseline contracts before the
candidate. It identified the remote-topology acceptance gap. Candidate review
required separating public descriptor authentication from confidentiality,
naming connector APIs instead of assuming userspace isolation, and exposing
expiry/revocation as an admission gate. These dispositions are incorporated in
ADR-110 and R2–R5. Final committed-head review and actual check results belong in
the PR record; neither changes Proposed status.

Proportional closeout: the research/design milestone delivers a decision aid,
not memory savings or a working bench. The stock-Firefox distinction changed
the automation recommendation; provider lifecycle and pricing prevented claims
of automatic expiry or zero total cost. Retain source-first challenge at the
implementation boundary. No process effectiveness or time savings is established.
Preparation, compilation, runtime tests and cleanup phases were not executed;
research/authoring/review overlap and fine-grained durations are unmeasured.
At the implementation closeout, compare actual resources and failure discovery
against these premises without rewriting retained failed evidence.

The bounded documentation follow-up clarifies endpoint ownership, IP identity
versus application port agreement, actual trust-store scope, confidential secret
delivery and cause-versus-qualification reporting. It retains the existing
expiry, quota, pinned-tool, WAN and staged-gate controls. These are source-backed
admission refinements, not proof of a working bench, isolated browser trust,
independent revocation or measured memory relief. Exact-head review and actual
checks for the follow-up belong in PR #93; architectural acceptance remains
separate and the maintainer has requested no PR status change or merge.
