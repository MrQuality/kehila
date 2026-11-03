# SP-007: hosted testbench feasibility

| Field | Value |
| --- | --- |
| Status / outcome | Active / no live bench qualification |
| Owner / assessor | Project maintainer; agent executes bounded research and independent review challenges evidence |
| Created / last updated | 2026-11-04 |
| Related work | [PR #93](https://github.com/MrQuality/kehila/pull/93), [ADR-110](../architecture/ADR-110-hosted-verification-testbench.md), #40, #41, #49, #71 |
| Effort limit / stop | One 30-minute hosted capacity attempt; one 45-minute connector build/API investigation. Stop dependent live work at unmet policy, credential, trust, lifecycle or resource admission. Preserve failures before any scoped rerun. |
| Prior evidence | Existing native CI passed the product fixture; documentation-only CI skipped workloads. Neither measured the proposed bench nor qualified local remote clients. |

## Question, scope and framing

Can a bounded hosted Linux backend replace backend work on the constrained
development machine while real Windows and WSL stock-browser clients retain
strict TLS, origin and authentication behavior? The maintainer requested execution
of the pending research spikes while away and reported freeing local memory.
This extends the earlier documentation-only design milestone into experimental
research. It does not accept ADR-110, replace gates or authorize paid services,
unspecified trust installation, merging or PR status changes.

One research record groups the six coupled questions; the labels below preserve
their original scope. The separate future exact-staged-tree gate migration is
excluded. Prototype code is research-only and must not become production or a
required check through this investigation.

| Premise | Status / consequence if false |
| --- | --- |
| Standard hosted capacity can run the current native fixture | Prior source/runtime evidence supports feasibility; fresh resource measurements remain necessary. Failure reopens capacity/preparation, not product limits. |
| Process-scoped tsnet works on actual Windows and WSL paths | Unqualified; cross-compilation is insufficient. Do not claim these paths from Linux-only results. |
| Account sign-in makes automation access available | Contradicted by empty browser-tool inventory. Account/policy inspection and enrollment remain blocked. |
| Available GitHub secrets provide confidential per-attempt delivery | Unsupported without sender/recipient, source/run binding and interruption proof. Do not use secrets merely to test that assumption. |
| Generated certificates authorize installing trust | Unsupported. Exact store/scope/deadline/cleanup approval remains separate. |
| Local memory relief is already demonstrated | Unsupported. Preflight resources do not establish matched workload savings. |

Request-first independent Phase A inspected the original outcome, current source,
SOP-001 and the risk/review controls before the candidate. Phase B supported
bounded secret-free/container experiments, with conditions: preserve history;
bind source/workflow/run; distinguish sampled system memory from container peaks;
preserve failures and owned cleanup; leave blocked live cases visible. These
conditions govern the prototype. Independent review is not architecture approval.

Component/harness review found that a PR path filter would repeat expensive runs
after documentation-only pushes because its diff is cumulative. The event-delta
admission job resolves that finding; `synchronize` requires valid previous/after
identities, and only `opened` may compare against the base. A focused composition
test also checks that a raw child canary remains private when final sampling fails.
The original review finding is retained here rather than treating its discovery
as a runtime failure. Seven focused unit checks passed; hosted execution remains
separate.

## Cases and decision oracles

| Case | Variable / fixed controls | Required observation / supported conclusion |
| --- | --- | --- |
| A1 Hosted capacity | Fresh hosted Ubuntu runner, Podman, pinned current native toolchain/database, one Cargo job | Image/dependency/native phase outcomes, elapsed time, sampled system available memory and storage; only this workload's bounded feasibility |
| A2 Capacity limits | Same run, 1-second samples, existing owned fixture cleanup | Missing measurements or workload/cleanup failure prevents qualification; sampling can miss peaks, and system usage is not workload-attributed RAM |
| B1 Connector build/API | Reviewed pinned tsnet graph, Linux container build, Windows/Linux targets | Build result and dependency/API constraints only; no runtime Windows/WSL isolation inference |
| B2 Actual isolation | Actual Windows/WSL connectors, inspected effective policy, exact enrolled peers | Fixed destination, retained port, denied reverse/other-session/database/engine paths; blocked until live admission |
| C1 Bootstrap | Actual confidential mechanism bound to source/workflow/run/attempt/recipient | Reject wrong/replayed transfer and reconcile interruption without disclosure; secret existence alone is insufficient |
| D1 Lifecycle | Selected supervisor and independent control path, active streams and owned processes | Observe deadline/kill/outage/restart behavior separately from enrollment expiry; job timeout alone is insufficient |
| E1 Stock browsers | Actual executable identity and OS, approved trust scope, bypass disabled | Headless product/TLS positive and wrong-issuer/IP/expired negative controls per browser; inventory alone is insufficient |
| F1 Integrated relief | Admitted hosted session plus local Windows/WSL journeys, independent final-state oracle | Product flows, WAN behavior and matched local memory observations; blocked until B-E prerequisites |

## Capacity prototype and reproduction

Artifact: [capacity sampler](../../experiments/SP-007/capacity_probe.py).
The separate [capacity workflow](../../.github/workflows/hosted-bench-spike.yml)
is restricted to same-repository PR #93. Its cheap admission job compares the
event's previous and current head for experimental-input changes; opening a PR
uses the base instead. A missing comparison commit fails admission. The PR path
filter alone is insufficient because it evaluates the cumulative PR diff.
Later documentation-only pushes therefore skip capacity execution even while
the cumulative diff still contains the prototype. A deliberate rerun of the same
event can repeat work and needs its own scoped attempt disposition.
It has a read-only repository token, immutable external action revisions,
non-persisted checkout credentials, no secret references and no network enrollment.
It checks out the event head, exports that committed tree outside the checkout,
and records head/tree, workflow revision, run and attempt. A PR trigger does not
establish reviewed-source admission for later credential-bearing work.

The workflow executes the existing Podman image preparation, pinned PostgreSQL
pull, explicit locked dependency preparation and `--native-authority` fixture.
The fixture retains its own lifecycle, assertions and cleanup. No browser or
overlay access is involved. The 30-minute platform limit bounds this experiment;
it does not prove cleanup after forced cancellation or the proposed session expiry.
The final check reports residual container count without deleting unowned state.
Dependency caches/images are intentional disposable-runner disk state.

For an admitted Linux environment, invoke each measured command as:

```text
python experiments/SP-007/capacity_probe.py PHASE --output ABSOLUTE_FRESH_EXTERNAL_DIRECTORY --storage EXISTING_STORAGE_DIRECTORY -- COMMAND ARGUMENTS
```

Valid phases are `image`, `database-image`, `dependencies` and `native`; exact
commands and pinned image inputs are in the workflow. The source export is
required by `container_verify.py`. On Windows, use the existing Podman lifecycle
lane for required gates; this Linux sampler is not a Windows resource observer.

Measurements are system `MemTotal` and minimum sampled `MemAvailable`, and free
bytes on the selected storage filesystem before/minimum/after each phase.
Sampling cadence is one second plus boundary samples; subsecond peaks can be
missed. These are system-wide observations, not per-container peaks, RSS totals,
or a measured offload saving. Preparation begins on a fresh hosted VM without
an explicit restored project cache; base image/tool/provider caches may exist.
Dependency preparation warms the named cache before the native phase, while
compilation outputs belong to a fresh native runner. No warm/cold speed ratio
is inferred from these different phases.

The collector emits only a fixed numeric/closed-label JSON report to public
output and job summary. Raw child diagnostics remain in protected temporary
runner storage and are not uploaded. They are lost at hosted teardown; a failed
attempt may therefore need a separately scoped diagnostic rerun. A measurement
failure cannot turn child success into a qualified pass; a child failure remains
the primary exit outcome. Abrupt cancellation or process-launch failure may
prevent the report entirely and cannot count as success.

Applicable coding controls: explicit byte/time units and validated observations,
one owned sampler thread stopped/joined after the child, bounded platform lifetime,
external raw-output storage, no exception suppression producing success, and
oracles for malformed measurements and failure precedence. Tests cover collector
contracts; real workload results remain separate evidence. No product/domain,
persistence or wire-format implementation changes are included.

## Runs and current handoff

### Source findings that constrain B-E

The pinned candidate `tailscale.com v1.104.5` declares
[Go 1.27.3](https://github.com/tailscale/tailscale/blob/v1.104.5/go.mod).
A Windows host with Go 1.27.0 cannot establish a build under that minimum while
automatic toolchain download is disabled. Use the already prepared container
toolchain for the compile-only probe. This selects a research candidate, not a
supported project dependency; transitive license/advisory review remains necessary.

The same version's [tsnet implementation](https://github.com/tailscale/tailscale/blob/v1.104.5/tsnet/tsnet.go)
calls synchronous `Start` before `Dial` waits on the caller context, and documents
that `Close` must not run before or concurrently with `Start`. Consequently a
context timeout plus an unconditional concurrent `Close` is not a supported
startup-cancellation design. Serialize the API lifecycle and use an external
process/container owner for forced termination. That owner still shares a host
failure domain; this source finding does not prove independent hard expiry.

[GitHub's secrets reference](https://docs.github.com/en/actions/reference/security/secrets)
states that repository/organization secrets are read when the workflow is queued,
whereas environment secrets are read when the referencing job starts. Updating a
repository secret after dispatch is therefore not an established bootstrap channel
for that queued run. An admission job followed by an explicitly protected,
session-scoped environment is a candidate worth testing, not an implemented
solution. [Environment protections](https://docs.github.com/en/actions/reference/workflows-and-actions/deployments-and-environments)
can withhold environment secrets until job approval; branch restrictions alone
do not prove an immutable reviewed source/workflow. Secret confidentiality,
wrong-run/replay rejection and interrupted cleanup still require actual probes.

[Chromium's Linux certificate documentation](https://chromium.googlesource.com/chromium/src/+/main/docs/linux/cert_management.md)
now identifies a version-sensitive shared NSS path: from M146 the default is
`$HOME/.local/share/pki/nssdb`, with an existing `$HOME/.pki/nssdb` taking precedence.
This strengthens the need to inspect actual trust-store selection rather than
assuming a fresh `--user-data-dir` isolates trust. No NSS or Windows trust store
was modified. Firefox's current policy documentation is the
[administrator reference](https://firefox-admin-docs.mozilla.org/reference/policies/certificates/);
the older policy-template page explicitly labels its documentation outdated.

### SP-007-R01: preparation, 2026-11-04

Baseline: `572cf1a7fc85861c0bd3514a66976639802bfd85`; capacity prototype:
`5be8892c6ac581b2e39300162bcc58c126e2ee5b`.
Fresh local observation: approximately 6.3 GiB available host RAM before VM start
and 4.5 GiB afterward. Saved VM configuration and observed runtime memory differ;
neither is a measured safe peak allowance. Existing cached verifier tooling can
be reused after identity checks. Heavy phases remain sequential and supervised.

Seven collector unit checks passed on Windows. The exact staged code snapshot
passed the full Podman gate: 389 Python checks with two Linux platform skips,
page/Rust/Go/oracle checks, the native PostgreSQL/HTTP/TLS fixture, broker/search
readiness and the task path including 48 competing pairs. Container phase duration
was 490.06 seconds; the commit controller observed 495.88 seconds through commit.
The minimum sampled available host RAM was 1.99 GiB at a five-second cadence
(99 samples); no memory-floor stop was requested. Owned runner/service/network
cleanup and staged-source verification passed. Provider shutdown is separate
because this run retained the already started VM. These local gate observations
do not measure the proposed offload saving or a guaranteed memory peak.

No hosted capacity run, tsnet build,
overlay enrollment, trust installation, real-browser campaign or WAN/product
campaign is established by this preparation. Runtime reports and immutable source
identities will be appended after actual execution, preserving failures.

Blocking live prerequisites: account automation access/current effective policy,
confined identity issuance and confidential bootstrap, concrete expiry mechanism
and failure domains, exact local helper/trust scope, and actual browser admission.
Do not replace these cases with synthetic-model passes or declare all spikes
complete. Next independent work: publish the reviewed capacity checkpoint,
inspect its actual hosted run and perform the bounded compile-only probe.

## Risks, review and reuse

[ADR-110's risk dossier](../implementation/hosted-verification-testbench.md)
R1-R10 remains applicable. Account access expansion, identity confusion, secret
misdelivery and lifetime/trust escape retain High/Critical impact, unknown
likelihood and P1 urgency before dependent work. The bench operator owns probes;
the maintainer retains vendor, security, budget and architecture acceptance.
No policy or trust risk has been accepted by this experiment.

Revalidate affected observations on source/workflow, image/toolchain, provider,
resource policy, workload, connector, account/policy/tag membership or trust-store
change. A prior pass may be reused only within its inspected conditions.
Retain the capacity prototype for reproduction; no production adoption or
required-check promotion is proposed. Final exact-head composition review,
required checks and a proportional milestone retrospective remain pending.
