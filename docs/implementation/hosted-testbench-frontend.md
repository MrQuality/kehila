# Hosted Linux frontend: Modal and E2B assessment

Status: research proposal, 2026-11-04. Related: [ADR-110](../architecture/ADR-110-hosted-verification-testbench.md),
[SP-007](../spikes/SP-007-hosted-testbench.md), [PR #93](https://github.com/MrQuality/kehila/pull/93).
No provider account, payment method, deployment or supported topology is selected.
Further evaluation is deferred in [#94](https://github.com/MrQuality/kehila/issues/94)
for future contributors; no near-term work is planned. The comparison below is
dated source research. Refresh provider terms and obtain the stated operating
decisions when the maintainer resumes the issue; proposed next steps are not an
active execution plan.

## Outcome and decision boundary

The maintainer requested Modal/E2B research as a frontend alternative to the
memory-constrained development computer, preferably using monthly free usage.
Assess the process that actually runs the browser or client, its user access,
and its connection to the proposed GitHub-hosted backend. Serving static files
remotely while the browser still runs locally does not offload browser memory.

Recommend **Modal as the first candidate for a bounded headless Linux test
frontend**, conditional on runtime, network and billing admission. E2B is a
credible trial alternative, but its current free credit is one-time. Neither
is established as a replacement for an interactive development workstation.
Hosted Linux results supplement the requested Windows/WSL paths; they cannot
qualify those operating systems, desktop trust stores or physical authenticators.
The earlier direct-SSH experiment and remaining A-F gates remain separate.

For unattended all-Linux automation, retain a browser in the **same GitHub job**
as the backend as the simpler baseline: it avoids a second provider and a
cross-cloud transport. GitHub's [job-local service networking](https://docs.github.com/en/actions/tutorials/use-containerized-services/use-docker-service-containers)
supports that distinction; a browser in another job does not automatically share
the service. That example uses Docker; it supports the job-local topology
comparison, without proposing a switch from the required Podman engine.
This baseline still needs browser packaging/qualification and cannot
replace local Windows/WSL results or an interactive desktop. Modal is the leading
candidate among the two requested external providers, not a demonstrated
improvement over that automated baseline.

This checkpoint is complete when current terms, capabilities, cost scenarios,
risks and next decisions are reviewable. Live provider execution is excluded.
Backend credentials remain GitHub secrets. Research permission does not authorize
copying them to another provider, changing application admission, installing
trust, adding payment details, merging or changing the PR's status.

## Findings from primary sources

Sources were inspected on 2026-11-04. Provider documentation establishes advertised
behavior, not compatibility with this project's fixture or a permanent offer.

| Question | Modal | E2B |
| --- | --- | --- |
| Free allowance | Starter advertises $30 compute credit each month | Hobby advertises $100 credit once; no recurring monthly credit |
| Entry requirement | Payment method required by current billing documentation | Pricing advertises no credit card required for the trial |
| Execution | Linux Functions or Sandboxes; Sandbox runtime may be gVisor or a VM | Isolated Linux sandbox VM with SDK/CLI control |
| Session lifetime | Sandbox default five minutes, configurable up to 24 hours | Hobby up to one hour continuously; pause/resume resets the runtime window |
| Browser evidence | Official headless Chromium example; stock Chrome/Firefox compatibility unexecuted here | Custom Linux templates and desktop/VNC example; stock Chrome/Firefox compatibility unexecuted here |
| Retained state | File APIs, volumes and snapshots available; select explicit retention | Pause preserves filesystem and RAM indefinitely; explicit kill required to discard the sandbox |
| User interaction | Stream commands/results; authenticated HTTP/WebSocket access can support a reviewed UI | Stream commands/results; desktop template can provide remote GUI |

Terms: [Modal pricing](https://modal.com/pricing),
[Modal billing](https://modal.com/docs/guide/billing),
[E2B pricing](https://e2b.dev/pricing),
[E2B billing and limits](https://docs.e2b.dev/billing).
Runtime/lifecycle: [Modal Sandboxes](https://modal.com/docs/guide/sandboxes),
[E2B lifecycle](https://docs.e2b.dev/sandbox),
[E2B persistence](https://docs.e2b.dev/sandbox/persistence),
[Modal filesystem access](https://modal.com/docs/guide/sandbox-files).

E2B's pricing table labels 4 GiB as default, whereas its current
[base-image documentation](https://docs.e2b.dev/template/base-image) specifies
512 MiB and two vCPUs for `base`. Resources belong to the selected template;
neither label proves the actual allocation or that a browser fits. Inspect the
chosen immutable template and observed sandbox resources before relying on them.
Current billing documentation describes custom template resource settings, while
the marketing plan comparison distinguishes default and custom resources. Confirm
Hobby entitlement to the required template size before spending its trial credit.

## Options and comparative judgment

Scores are qualitative research priorities out of ten, weighing recurring cost,
coverage, operational complexity and fit to bounded testing. They are opinionated
comparisons, not measured performance or acceptance of a security residual.
Effort estimates assume a reusable admitted harness and working backend channel;
unresolved transport, trust or browser defects can dominate them.

| Solution | Pros | Cons / material limits | Cost basis | Complexity / incremental effort estimate | Score |
| --- | --- | --- | --- | --- | --- |
| Modal Sandbox, headless stock browser or protocol harness | Recurring credit; explicit lifetime and resource limits; inspectable command execution; browser RAM offloaded | Payment method; SDK/image ownership; backend channel and browser sandbox unqualified; higher rate than ordinary Functions | Scenario below: about $0.238/hour before credits | Medium; roughly 2-4 engineering days after prerequisites | 8/10 |
| Modal Function, finite automated suite | Lower compute rate; suitable for disposable, reproducible test cases | Default preemption restarts the same input; unsafe for mutation tests without replay control; weaker interactive fit | About $0.079/hour in the same nominal scenario | Medium; roughly 2-4 days including interruption/oracle design | 7/10 |
| E2B Hobby, disposable headless/CLI trial | No-card entry; Linux templates and useful lifecycle APIs | Credit does not replenish; one-hour continuous window; selected template resources/entitlements require confirmation | About $0.166/hour at 2 vCPU/4 GiB; credit is finite | Medium; roughly 2-4 days after prerequisites | 5/10 for recurring-free objective; 7/10 for a trial |
| Remote GUI on either provider | Browser process runs remotely; visible manual investigation | Desktop/streaming overhead, latency, access control and profile retention; interactive development needs still undefined | Selected runtime plus GUI resources and billable idle time | High; roughly 4-7 days including access/session qualification | 5/10 |

The [Modal Chromium example](https://modal.com/docs/examples/webscraper)
supports feasibility of hosted browser execution, without proving stock-browser
coverage. Preserve actual stock Firefox via WebDriver: Playwright uses a patched
Firefox build, so its Firefox result is not stock Firefox evidence.
[Playwright browser documentation](https://playwright.dev/docs/browsers),
[Selenium Firefox](https://www.selenium.dev/documentation/webdriver/browsers/firefox/).

For GUI research, [Modal's VNC example](https://modal.com/docs/examples/computer_use_vnc)
and [E2B's Ubuntu desktop template](https://docs.e2b.dev/template/examples/desktop)
are capability demonstrations. Do not adopt their sample security settings as the
bench policy: the Modal example disables the Chromium sandbox and uses a
passwordless internal VNC server. Provider isolation is not evidence that the
project's browser sandbox or authenticated GUI-access requirements are met.
No LLM, GPU or computer-use agent is needed to run this test frontend.

## Cost scenarios and spending control

USD list-price illustrations, excluding tax, currency conversion, preparation,
image builds, storage, network excess, retries, other workspace use and idle
overhead. They are not measured browser requirements, a bill prediction or a
hard upper bound. One Modal physical core is advertised as two vCPUs equivalent;
that is not a cross-provider performance benchmark.

For a nominal one physical core plus 4 GiB:

- Modal Sandbox: `3600 * (0.00003942 + 4 * 0.00000667) = $0.23796/hour`.
- Modal ordinary Function: `3600 * (0.0000131 + 4 * 0.00000222) = $0.079128/hour`.
- E2B two vCPUs plus 4 GiB: `3600 * (2 * 0.000014 + 4 * 0.0000045) = $0.1656/hour`.

Rates: [Modal pricing](https://modal.com/pricing), [E2B pricing](https://e2b.dev/pricing).

| Billable runtime | Modal Sandbox gross compute / after $30 credit | Modal ordinary Function gross / after $30 credit | E2B gross compute |
| --- | --- | --- | --- |
| 10 hours | $2.38 / $0 | $0.79 / $0 | $1.66 |
| 40 hours | $9.52 / $0 | $3.17 / $0 | $6.62 |
| 160 hours | $38.07 / $8.07 | $12.66 / $0 | $26.50 |

The isolated $30 allowance corresponds to about 126.1 Sandbox hours or 379.1
ordinary Function hours at those assumptions. E2B's $100 corresponds to about
603.9 hours once, not each month. Do not assume a $150/month Pro subscription is
required merely because the trial runs out: current E2B billing describes adding
a payment method to continue, with Pro separately increasing limits. Either
paid continuation is outside this research authorization.

Modal bills the greater of requested and actual usage. A request is a minimum,
and bursting can cost more. Any later bounded experiment needs explicit CPU and
memory limits, concurrency one, a lifetime limit and observable termination.
[Modal resources](https://modal.com/docs/guide/sandbox-resources).
Ordinary Functions can be preempted and restarted with the same input; a
non-preemptible Function uses the higher CPU/memory class. Do not silently rerun
product mutations or apply ordinary rates to Sandboxes.
[Modal preemption](https://modal.com/docs/guide/preemption).

Modal's workspace budget applies before credits; its spend limit applies to net
charges. Defaults can permit paid usage. Before any experiment, the account owner
must inspect and approve payment handling and verify the chosen usage/spend caps,
their treatment of existing workspace use, and stop behavior. Starter lacks
environment-level budgets; do not prescribe upgrading just to obtain them.
[Modal budgets](https://modal.com/docs/guide/budgets).
E2B documents spending limits and blocking projects shortly after credits run out
when no payment method exists; that is not an exact instantaneous cutoff proof.
[E2B billing](https://docs.e2b.dev/billing).

## Topology and required code boundaries

```text
Operator -> authenticated provider control plane -> disposable Linux frontend
                                                  browser / test harness
                                                  owned loopback gate
                                                        |
                                              admitted transport (unqualified)
                                                        |
                                             GitHub backend authority fixture
```

The browser and its loopback gate must run in the same admitted network namespace
or have an explicitly designed equivalent. The browser retains
`https://127.0.0.1:P`, ordinary TLS verification, the selected issuer and exact
Host/Origin checks. A provider public URL is a management/streaming interface;
it cannot replace the product origin. Current contracts live in
`src/io/kehila_authority/src/startup.rs`, `http_auth.rs` and `listener/page.js`.

Moving the browser to another cloud does not make a standard GitHub runner
reachable. [Modal networking](https://modal.com/docs/guide/sandbox-networking)
documents outbound access to public IPs and authenticated HTTP/WebSocket Connect
Tokens. That is a possible transport leg, not an implemented opaque TCP/TLS
channel. An ordinary encrypted tunnel alone does not authenticate a caller.
[E2B outbound networking](https://docs.e2b.dev/network/internet-access) likewise
does not establish ingress to GitHub. A runner-initiated connection to a provider
endpoint changes the trust, direction and lifecycle boundaries and needs its own
review and exact authorization. No automatic switch to runner-to-local is proposed.

Expected changes are confined initially to a test image, provider adapter and
provider-neutral browser/result/lifecycle harness. Keep immutable source and
image identity, admitted driver/browser binaries, private temporary storage,
loopback ownership, TLS negative controls, deadlines and cancellation observable.
Use one worker and no implicit runtime downloads. No application admission change
is currently justified; demonstrated incompatibility reopens the design.

A Linux CLI here means a synthetic/protocol test harness. The experimental
Windows owner CLI uses .NET Framework and DPAPI CurrentUser; this research does
not port its credential storage or make it a supported Linux client. An actual
Linux product CLI would require a separate contract and acceptance decision.

Keep three authorities distinct: provider API credentials stay with the trusted
controller; a connector may need a narrowly scoped disposable transport token;
the browser receives only disposable test-user/session credentials. Backend
administrative credentials remain GitHub secrets. No signing key or provider API
token belongs in an image, browser profile, screenshot, public log or GUI URL.
Provider-side storage is a new data boundary needing explicit approval before
private material is uploaded. Export only reviewed evidence; destroy profiles
and secret-bearing state. Do not snapshot an authenticated browser session.

## Risk management and next observations

Likelihood is unknown unless a source establishes the failure; impact and
priority describe consequence and required ordering. The bench operator owns
probes and cleanup; the maintainer retains provider, spend, trust and architecture
acceptance. High scores do not accept these residuals.

| Risk / invariant | Evidence and likelihood | Impact / priority | Cheapest decisive control or observation / cost | Residual and invalidation |
| --- | --- | --- | --- | --- |
| No recurring allowance | E2B one-time credit established with high confidence | High for recurring-free objective / P1 before selection | Reject monthly-free premise; keep finite trial option / research only | Owner rechecks pricing before use or plan change |
| Billing exceeds allowance | Modal max(request,actual) and permissive possible defaults established; workload likelihood unknown | High / P1 before execution | Owner verifies caps; explicit resource limits, concurrency and timeout; meter one bounded synthetic run | No zero-invoice guarantee from docs; revalidate rates, limits and shared usage |
| Backend unreachable or wrong peer | No qualified provider-to-runner path | High / P1 before integration | Design exact transport; authenticate run/source/recipient; synthetic nonce plus server witness before product secrets | A public egress address does not prove ingress; topology/provider change invalidates result |
| Public control surface exposes desktop/code | Ingress settings and SDK behavior vary; likelihood unknown | Critical / P1 before exposure | Default no ingress; explicit private access, unauthorized-request negative control, token expiry and revocation | Encryption is insufficient; revalidate SDK/project defaults and resumed instances |
| Browser passes only after sandbox/TLS bypass | Sample flags differ from required contract; compatibility unknown | High / P1 before coverage claim | Nonroot stock-browser/driver identity and sandbox admission; positive DOM then issuer/IP/expiry negatives / proposed short probe | Provider VM isolation is not browser sandbox proof; browser/runtime change invalidates |
| Cancellation leaves resources or retained secrets | E2B pause preserves RAM/files indefinitely; remote cleanup may fail | High / P1 before credentials | Independent owner deadline, provider reconciliation, explicit kill and absence check; no secret snapshots | API outage leaves uncertainty; pause is not deletion, process exit is not remote cleanup |
| Linux-only result overclaims product support | Windows CLI/trust and local paths differ, established | High / P1 before qualification | Label actual OS/browser/mode; retain Windows/WSL gates; no stock-Firefox claim from patched build | Hosted test cannot accept an unspecified desktop replacement |
| Interrupted test replays mutations | Default Modal Function restart documented; incidence unknown | High / P1 before Function adoption | Explicit non-replay boundary or non-preemptible execution; preserve first failure and session identity | Lower pricing does not establish equivalent semantics; provider policy change invalidates |

For E2B, explicitly require private traffic rather than assuming a new-project
default applies to old or resumed sandboxes. Its documentation identifies older
SDK behavior that could override a private default; pin and inspect the SDK.
[E2B public-access restrictions](https://docs.e2b.dev/network/restrict-public-access).

Proposed next steps, **not executed or newly authorized**:

1. Prepare a provider-neutral packaging/case manifest from public source, without
   accounts or uploading private material. Admit actual stock browser/driver,
   filesystem/socket layout and expected result format before asking for user time.
2. After the maintainer accepts the exact provider/account/payment/data boundaries,
   prepare and independently review one at-most-20-minute synthetic frontend
   probe, concurrency one, explicit CPU/memory/lifetime limits and cleanup oracles.
   Begin without public GUI or backend secrets. Observe DOM, sandbox, controller
   cancellation, provider termination, charged usage and retained-state absence.
   Absence means no live or paused owned resource visible through the provider
   API; it does not independently certify backup or physical-media erasure.
3. Only after an admitted transport is available, qualify product TLS/origin and
   disposable authentication with negative controls and actual server witnesses.
   Keep a bounded interactive GUI comparison optional until its need is confirmed.

Use provider SDK lifecycle controls initially; OpenTofu is not required for this
research or a disposable browser run. Reassess infrastructure-as-code if durable
networking/storage is adopted, including state/secret handling. No provider plugin
or state store is installed here.

## Review, evidence validity and closeout

Independent request-first Phase A preceded the candidate. Phase B checked primary
terms, runtime/network contracts, arithmetic and project frontend scope. Its
material findings were resolved by limiting the recommendation to a headless
Linux test frontend, distinguishing resource requests from limits and separating
provider, transport and browser credentials. No provider runtime was executed;
no memory saving, cold-start latency or integrated compatibility is measured.

Research and review overlapped CI waiting; individual author/reviewer durations
are unmeasured and must not be summed as elapsed time. The recurring-credit premise
was challenged before account setup; this is a specific corrected assumption,
not measured process effectiveness. The next improvement observation is whether
an explicit image/resource/network/cleanup admission prevents a harness failure
before a later browser campaign. Refresh this assessment on pricing, selected
template, browser/runtime, topology, trust, resource policy or intended-use change.
Final composition review and applicable documentation checks remain required
before publication. Architectural acceptance remains with the maintainer.
