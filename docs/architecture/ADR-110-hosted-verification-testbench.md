# ADR-110 GitHub hosted verification with local browser clients

Status: Proposed. Owner: project maintainer. Created: 2026-11-03.
Acceptance: pending an explicit maintainer decision; merging this proposal does
not authorize implementation, enrollment, trust changes or replacement of gates.
Related: [ADR-105](ADR-105-local-owner-authentication.md),
[ADR-106](ADR-106-local-container-transport.md),
[testing guide](../TESTING.md),
[research and risk plan](../implementation/hosted-verification-testbench.md).
Continuing obligations: [#40](https://github.com/MrQuality/kehila/issues/40),
[#41](https://github.com/MrQuality/kehila/issues/41),
[#49](https://github.com/MrQuality/kehila/issues/49) and
[#71](https://github.com/MrQuality/kehila/issues/71); none is completed here.

## Outcome and scope

Move backend compilation and service-backed verification away from a
memory-constrained development machine while preserving real Windows and Linux
browser testing. Use standard GitHub-hosted Linux capacity first; reconsider
paid runners or a cloud VM when observed capacity or operating costs justify it.
The requested deliverable is research, a design proposal and a risk management
plan for review. It introduces no executable changes.

The proposed Linux frontend is the existing backend-served web interface in a
Linux Firefox or Google Chrome process running under WSL. A Windows browser
launched from a WSL shell is still Windows evidence. This proposal does not
create a native Linux desktop client or port the experimental Windows CLI.

## Context and consequential premises

Source baseline: `7f9301601c5de4c87f8cad7c3b1c1681d0769336`.

| Premise | Status and evidence | Consequence if false |
| --- | --- | --- |
| Backend execution can use hosted Linux containers | Established for current native CI: `.github/workflows/ci.yml` uses Podman; [merge CI](https://github.com/MrQuality/kehila/actions/runs/41696939042) passed. The separate shared job currently uses Docker. | Investigate the specific failing lane; do not introduce a local fallback. |
| Standard public hosted capacity is the initial target | Maintainer-selected starting point. [GitHub specifications](https://docs.github.com/en/actions/reference/runners/github-hosted-runners) list 4 CPUs, 16 GB RAM and 14 GB SSD for the selected x64 Linux class. | Reassess with measurements, including disk and cold preparation. |
| Local Windows and WSL browsers are needed | Maintainer request, including Firefox/Chrome on Linux and consideration of headless automation. | Hosted-only browser results cannot replace the requested local paths. |
| Existing product behavior can remain intact | Inference from `startup.rs`, `http_auth.rs` and relative same-origin requests in `listener/page.js`. The client origin remains `https://127.0.0.1:P`. | Reopen this design before changing product origin, TLS or authentication. |
| Same-PC transport approval covers a remote bench | Contradicted by ADR-106's explicit same-PC scope and external-ingress exclusion. | A separate disposable remote-test topology needs acceptance. |
| Free compute means a free complete bench | Unsupported: connectivity, quota, storage and administration are separate costs. | Obtain eligibility and cost acceptance before enrollment. |
| Hosted committed-source tests replace the staged local gate | Contradicted by the current `.githooks/pre_commit.py` index-tree contract. | Preserve the gate until a separate replacement proves exact-source binding. |

The existing native fixture owns a complete internal test lifecycle. It is not
a reusable backend session for external browsers. Current page tests simulate
the DOM; they do not establish browser, trust or remote-network behavior.

## Proposed decision

### Separate unattended verification from local client sessions

Retain ordinary PR/main verification without local-machine connectivity. Keep
its existing required checks. Containerized backend workloads in the new bench
use Podman; this proposal does not migrate the separate existing shared CI job.

Add a separately dispatched, bounded integration-session workflow only after
acceptance. It runs a fresh synthetic backend/database fixture on a hosted Linux
runner and serves sequential local browser cases. Use its own concurrency domain;
a new push must not implicitly cancel a human-assisted session. Concurrent
requests against the same fixture are allowed only in explicitly selected cases.
This is a testing session, never an always-on development or application server.

Initially use one browser and one test worker at a time. Propose stock Chrome
and Firefox on Windows and WSL, headless by default. Headed checks remain a
bounded final campaign where they establish behavior headless execution cannot.
A local browser/WSL still consumes local RAM; backend offload is expected to
reduce demand substantially, but its size is not yet measured.

### Preserve the application authority through an opaque connection

Recommend evaluating an isolated Tailscale userspace connector as the first
transport candidate. The narrow implementation candidate is a small test-only
Go connector using documented `tsnet.Server.Dial` and `Server.Listen` APIs.
Windows/WSL binary support, dependency/toolchain compatibility and actual Podman
network behavior remain feasibility gates, not demonstrated properties.

```mermaid
flowchart LR
    W[Windows Chrome or Firefox] --> WL[Windows 127.0.0.1 P]
    L[WSL Linux Chrome or Firefox] --> LL[WSL 127.0.0.1 P]
    WL --> C[Local connector for selected environment]
    LL --> C
    C -->|Opaque application TLS over private overlay| R[Runner connector]
    R -->|One fixed private IPv4 and port| A[Rust TLS authority in Podman]
    A --> D[Disposable PostgreSQL]
```

Windows and WSL paths run separately; the diagram does not imply shared loopback
or simultaneous ownership of one port. The local connector binds only the exact
loopback endpoint and dials one authenticated session peer/port. It opens no
tailnet listener, general SOCKS/HTTP proxy, remote shell or network control API.
The runner connector listens only on its overlay endpoint and forwards only to
the independently inspected authority container address/port. It cannot forward
to PostgreSQL, engine sockets or arbitrary destinations. Raw `Listen` is intended;
`ListenTLS` or a TLS-terminating web proxy would change the application endpoint.

The helper carries bytes, not HTTP credentials or application authority. Rust
still terminates browser TLS. Preserve the exact port, IP certificate identity,
Host/Origin, protected cookies, CSRF and product deadlines. No CORS expansion,
certificate bypass, wildcard listener or forwarding-header trust is proposed.
Normal TLS validation must pass in each actual browser profile; explicitly set
WebDriver `acceptInsecureCerts=false` and verify that setting is effective.

The certificate's `iPAddress` SAN contains `127.0.0.1`, without a port; TLS IP
matching follows [RFC 9525 section 6.4](https://www.rfc-editor.org/rfc/rfc9525.html#section-6.4).
The application's `startup.rs` derives the public origin from its listener port,
and `http_auth.rs` checks exact Host/Origin. Therefore the browser-facing port
and configured authority listener port must agree for this unchanged product.
A connector's intermediate overlay port may differ if its mapping is explicit;
certificate IP matching is not the reason for application port agreement.

Before dispatch, the selected local helper must bind and retain ownership of
the exact loopback endpoint through preparation and attachment. Probe-and-close
only observes availability; it does not reserve a port during hosted queueing.
Do not forward application traffic before peer/source admission completes. On
lost ownership or a collision, fail that attempt; a different endpoint requires
fresh admitted session parameters. Qualify Windows and WSL paths under the
selected networking mode rather than requiring NAT and mirrored mode to behave
identically.

Use a dedicated bench network policy. Permit only the selected local connector
identity to the selected session peer and port; deny new reverse connections and
other destinations. Return traffic on that connection remains necessary. Remove
broad default grants rather than adding a narrow rule beside allow-all. Do not
join the whole development machine, install a system VPN, alter routes/DNS,
advertise subnets or expose a public endpoint as an implicit consequence.

This introduces vendor/control-plane metadata and availability dependencies plus
a new Go dependency closure. Their acceptance and cost are separate decisions.
If the narrow connector or session isolation cannot be supported, reconsider
an SSH broker or hosted-only automated lane; do not broaden access to obtain a pass.

### Own the session and its evidence

The implementation must distinguish admission, preparation, readiness,
attachment, execution, stopping and terminal states. One terminal outcome binds
the immutable source SHA/tree, workflow/harness revision, run attempt, image IDs,
fixture identity and client/browser versions. Missing cases are incomplete,
not passed. Preserve product, harness, transport, capacity, cancellation and
cleanup failures separately; a successful cleanup cannot clear the first failure.

Admission permits only an explicitly reviewed immutable revision of both the
workflow and tested source. A maintainer dispatch is not evidence that arbitrary
PR scripts are trustworthy. No fork/PR job receives enrollment credentials or
private-network reach. Pin external actions/tools; isolate credential issuance
from code under test. The public project must never receive real user data.

Do not treat authenticated GitHub API access as confidential storage. A public
result descriptor may contain reviewed source/run identity, stage/status and
sanitized version information. It is not a credential. Local tooling checks the
expected repository, workflow revision, source SHA, attempt and freshness against
GitHub, then authenticates the exact overlay peer separately. Machine/network
identifiers, owner secrets, enrollment keys, cookies, bearer tokens and TLS private
keys stay in protected session storage outside public logs/artifacts. Certificate
fingerprint delivery needs an authenticated channel and explicit trust approval;
private key delivery to browsers is unnecessary.

Preserve ADR-106's key separation: the operator owns the browser-trusted issuer
signing key outside the tested serving container. The authority receives only
its disposable leaf key/certificate; it cannot mint new trusted certificates.
Fresh session trust must not silently reuse earlier development trust. Changing
issuer placement or scope requires an explicit decision before live admission.

Select the confidential delivery mechanism before transferring a leaf key,
fixture password or enrollment credential. Its contract must identify the sender
and recipient, authenticate the intended session/run attempt, protect transit and
storage, reject replay or delivery to another session, and reconcile interrupted
delivery and cleanup. A public descriptor or workflow-dispatch input cannot
carry a secret. GitHub's repository/environment/organization secret scopes do
not themselves establish a single-run delivery protocol; ordinary environment
variables are not an encrypted transport. Keep the issuer signing key outside
the transfer. The mechanism remains a pre-use admission decision, not a claimed
existing confidential channel.

Before implementation admission, select the enforcement mechanism for session
expiry. The required behavior is: stop new local attachments and existing streams
at the approved deadline; stop the remote listener/backend; expire enrolled
access and reconcile both sides. Local connector timers are independent of the
runner, but not independent of a compromised local connector. A proposed trusted
local supervisor owns the deadline and teardown independently of test scripts.
Remote revocation requires a separately protected administrative control path.
Runner loss, supervisor loss and control-plane outage must remain explicit failure
states; no hard remote deletion guarantee follows from this design alone.
The unresolved enforcement choice blocks live session admission, not this PR.

Use finite preparation, attachment and execution budgets, plus bounded cleanup.
Proposed initial session ceiling: 30 minutes from readiness, within a separate
45-minute overall job ceiling; these are reviewable harness allowances, not
measured operating limits or extensions to five-second product deadlines.
Allow one fresh attempt after diagnosis only through explicit operator choice.
Discard interrupted fixtures; never automatically replay uncertain mutations.

### Use browser automation that matches the claim

Recommend Selenium WebDriver with Python for the stock Chrome/Firefox matrix,
building on the existing Python harness language. Use explicit, reviewed browser
and driver versions/paths, isolated profiles and local-only driver endpoints.
No runtime auto-download/update, broad WebDriver listener or remote browser
control endpoint is admitted. WSL headless execution does not need WSLg display;
WSLg is relevant to headed Linux checks.

For each actual browser/OS combination, specify the trust store and inherited
trust sources, the exact certificate and scope added, and removal/reconciliation
after normal exit and interruption. A fresh browser profile does not alone prove
trust isolation: [Chromium on Linux](https://chromium.googlesource.com/chromium/src/+/main/docs/linux/cert_management.md)
uses a shared NSS database. Require positive validation in the selected browser,
negative TLS controls, and evidence that ordinary browser environments gain no
unintended trust. Deleting a profile cannot substitute for removing trust from
a different store. No host-wide or browser-specific trust operation is authorized
by this proposal.

Playwright offers stronger integrated test/trace ergonomics, but its Firefox
requires patches and is not branded Firefox. That is valid engine evidence,
not equivalent to the requested stock-browser coverage. Chromium is likewise
not automatically branded Chrome. API/CLI probes complement DOM journeys;
neither replaces cookie/CSRF/page behavior or Windows-specific client checks.
No Edge-on-Linux coverage or new Linux credential-store/CLI product is included.

### Keep staged verification intact until a separate cutover

An interactive session proves neither the complete mandatory suite nor an exact
staged index. The current hook and all required hosted checks stay unchanged.
Moving only browser sessions will not remove the local full-gate memory cost.

For eventual full relief, prefer an asynchronous hosted gate that exports and
verifies the exact immutable staged Git tree, records its base/scope and returns
an authenticated verdict for that tree. Before commit, the local hook must check
both index tree and HEAD still match the admitted inputs. Uncommitted snapshots
must not be published automatically; transport confidentiality, publication
review, remote workspace semantics and trusted/untrusted execution separation
need a separate design amendment. Disabling hooks, trusting a nearby commit,
or treating a browser-session pass as the full gate are rejected shortcuts.

## Alternatives and tradeoffs

| Alternative | Advantages | Disadvantages and disposition |
| --- | --- | --- |
| Tailscale isolated connector | NAT traversal/relay support; no owned public broker; process-scoped API candidate | Vendor, quota, dependency and cross-platform proof costs; preferred candidate subject to admission gates. |
| Raw WireGuard | Small transport protocol; opaque TCP carried through tunnel | Needs a reachable endpoint or traversal infrastructure; keys/routes/relay operations remain. Retain if vendor dependency is rejected. |
| OpenSSH forwarding through a broker | Mature forwarding; local loopback and application TLS can remain | Owned reachable broker, restricted accounts/keys, hard-expiry enforcement and hosting cost. Credible fallback, not zero infrastructure. |
| Public web tunnel or Funnel | Convenient endpoint discovery | Public ingress and hostname/routing mismatch; TLS-terminating modes violate the selected endpoint. Raw TCP modes exist but do not establish compatible private access. Not selected. |
| Hosted Linux browser beside backend | Simplest unattended integration, no local bridge | Does not qualify local Windows/WSL paths. Useful complementary lane. |
| CLI/API only | Lower resource cost and good protocol controls | Misses browser runtime, DOM, cookie and interaction behavior. Complement only. |
| Always-on VM or self-hosted runner | Persistent interactive environment and caches | Paid resources, patching, state and trust management. Reconsider on evidence; not selected now. |

The [Funnel CLI](https://tailscale.com/docs/reference/tailscale-cli/funnel) and
[requirements](https://tailscale.com/docs/features/tailscale-funnel) document its
raw forwarding options and public endpoint restrictions. The SSH fallback uses
[OpenSSH forwarding](https://man.openbsd.org/ssh.1) with explicit
[server-side restrictions](https://man.openbsd.org/sshd_config.5), not an
unrestricted shell account.

OpenTofu is optional for a future broker/VM, network, firewall, IAM or protected
storage. Actions YAML, Podman and bounded coordinators own ephemeral test runs.
Do not introduce IaC merely to wrap process launch/cleanup commands. Keep test
credentials out of state; display redaction alone does not encrypt state.

## Consequences and decisions requested

Accepting the topology would expand disposable test transport beyond ADR-106;
it would not add supported remote product deployment, production ingress,
replication, upgrades or real-data durability. Authentication, Argon2 settings,
admission/cancellation and ownership remain unchanged. WAN latency/relay failures
may invalidate a test environment; do not extend product deadlines to hide them.

The maintainer is asked to decide separately:

1. Accept the proposed bounded hosted-backend/local-browser topology and stock
   Chrome/Firefox headless-first matrix, with headed cases separately qualified.
2. Accept Tailscale as the first feasibility candidate, subject to plan eligibility,
   account/data exposure, dependency review and an explicit spending limit. If
   declined, select broker research or hosted-only automation without claiming
   local-client replacement.
3. Accept the lifecycle invariants while requiring a concrete expiry/revocation
   mechanism and trust scope before any live connectivity. Exact issuer/store,
   deadlines, local helper execution and resource approvals remain separate.
4. Keep required gates intact; authorize any later staged-tree cutover separately.

Recovery impact: all bench data is disposable; interruption invalidates a run
and requires fresh enrollment. This adds no supported backup/restore behavior
under #60. Language/RTL impact: no product UI/content change; future locale
coverage under #61 remains separate. Agent execution impact: automation is a
test operator, not a new product actor or delegated credential under #62.

The change is reversible before gate cutover: disable the session workflow,
revoke connector identities/credentials and remove the approved local artifacts;
existing gates still apply. Reversibility after trust installation requires its
explicit removal/reconciliation evidence. Research, risk controls, future
qualification criteria and delivery checkpoints are in the linked dossier.
