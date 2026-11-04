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

Subsequent maintainer direction selects SSH over tsnet for research and
authorizes the targeted Windows observer and WSL storage investigation. The
[SSH companion](../implementation/hosted-testbench-ssh.md) records the revised
candidate, costs, restriction/lifetime gaps and proposed policy laboratory.
This supersedes the preferred transport direction, not historical observations
or the underlying confinement and lifecycle obligations. A specific private
native Linux temporary-storage boundary was separately approved for the bounded
WSL socket/browser probes; no trust, mount or system security change follows.

One research record groups the six coupled questions; the labels below preserve
their original scope. The separate future exact-staged-tree gate migration is
excluded. Prototype code is research-only and must not become production or a
required check through this investigation.

| Premise | Status / consequence if false |
| --- | --- |
| Standard hosted capacity can run the current native fixture | Prior source/runtime evidence supports feasibility; fresh resource measurements remain necessary. Failure reopens capacity/preparation, not product limits. |
| Selected transport works on actual Windows and WSL paths | Unqualified. SSH is now preferred; earlier tsnet cross-compilation remains historical evidence, not runtime qualification of either transport. |
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
| B2 Actual isolation | Actual Windows/WSL transport, inspected effective policy, exact session identities | Fixed destination, retained port, denied reverse/other-session/database/engine paths; SSH policy laboratory precedes live admission |
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

### SP-007-R02: hosted capacity observation, 2026-11-04

[Capacity run](https://github.com/MrQuality/kehila/actions/runs/41733034777),
source `442da7bb61806e3860a91c033895955e85de718c`: admission and capacity
jobs passed. All four phases returned zero without sampling errors; the final
residual-container assertion passed. This was one hosted attempt, without an
explicit restored project cache. Observed system memory total was 15.61 GiB.

| Phase | Seconds | Samples | Minimum available system GiB | Storage GiB before / minimum / after |
| --- | --- | --- | --- | --- |
| Image preparation | 63.051 | 65 | 13.329 | 85.561 / 77.948 / 81.450 |
| Pinned database pull | 5.841 | 7 | 13.396 | 81.450 / 80.956 / 81.010 |
| Locked dependencies | 18.899 | 20 | 13.510 | 81.010 / 80.822 / 80.830 |
| Native fixture through owned cleanup | 200.612 | 202 | 13.864 | 80.830 / 80.541 / 80.830 |

These are the collector's one-second sampled system/filesystem observations,
rounded to three decimals; they are not process RSS, container peaks, general
hardware guarantees or an integrated browser-session measurement. Actual free
disk exceeded the documented minimum for this allocation; do not assume every
future runner will expose that amount. CPU count was not measured by this probe.
The workload's one-job build policy and existing container limits were retained.

Conclusion for A1/A2: standard hosted capacity supported this current native
fixture and preparation attempt with substantial sampled memory headroom. A
longer remotely attached session, WAN path and matched local memory comparison
remain unqualified. No paid runner/VM is justified by this observation alone.

### SP-007-R03: compile-only connector candidate, 2026-11-04

`tailscale.com v1.104.5`, Go 1.27.3 and Linux amd64 toolchain container:
module resolution and `go mod verify` passed. Linux/amd64 and Windows/amd64 API
surface builds both passed with cgo disabled. Linux ELF and Windows PE signature
checks passed; neither binary executed and no node started or enrolled.
Resolution took 111.11 seconds, verification 2.81, Linux build 112.61, Windows
build 112.89 and inventory 97.36; the full controller span was 451.83 seconds
including setup/export/cleanup. The owned build container was removed and the
idle provider stopped. No general host toolchain upgrade occurred.

The module graph inventory contained 610 modules including the probe; the Linux
package list contained 567 entries including standard-library packages. The
graph count is not a count of linked runtime dependencies. Probe artifacts were
approximately 29.1/29.3 decimal MB for Linux/Windows; this is not a complete helper
footprint or its runtime memory. These preliminary local API results lack a
versioned public reproduction harness and do not qualify a supported dependency.
Complete transitive license/advisory review and actual platform/runtime isolation
remain outstanding before adoption. The top-level upstream license is BSD-3-Clause;
that does not establish every transitive package's license.

### SP-007-R04: client tooling preflight, 2026-11-04

Windows executable metadata identified stock Chrome 156.0.8078.80 and Firefox
159.0. Neither WebDriver executable was found on the inspected Windows PATH;
Selenium was absent from the selected Python interpreter. These are scoped
discovery results, not a claim that no alternate installation exists.

The selected WSL Ubuntu environment reported Python 3.10.12 and no Chrome,
Firefox, Chromium, ChromeDriver, GeckoDriver or NSS `certutil` on its PATH.
No browser, driver or trust-store installation occurred. E therefore requires
explicit preparation of admitted stock executables/drivers and an isolated
automation environment before qualification. A Windows binary invoked through
WSL would not fill that missing Linux-client evidence.

### SP-007-R05: Windows browser preparation and failed observer, 2026-11-04

An isolated Windows automation environment was prepared with Selenium 4.52.0,
ChromeDriver 156.0.8078.80 and GeckoDriver 0.37.1. Eighteen selected Python wheels
were checked against exact PyPI release hashes; official driver archives were
checked against retained hashes and provider digests where available. Offline
installation retained license notices. Scoped independent review supported
unchanged private research use, including the relevant non-default license terms;
it did not clear supported redistribution or the full browser/driver dependency
closure. An OSV snapshot returned no matches for the queried Python package
versions; it does not establish absence of vulnerabilities or driver/browser
advisory clearance. No project dependency manifest or system Python changed.

Review found that Selenium environment variables can override an explicit driver
path. The probe cleared those overrides and checked the selected artifact path
before launch. It used a fresh profile, added no sandbox-disabling options and
requested `acceptInsecureCerts=false`, under a bounded process owner. This is not
an executed sandbox-isolation qualification.

The first Chrome attempt **failed at the socket-observation/admission stage**
after a successful WebDriver session initialization and before its DOM case.
The outer attempt duration was 6.312 seconds.
The recorded capabilities identified Windows Chrome 156.0.8078.80 with
`acceptInsecureCerts=false`. The ownership query returned no matching listener.
Normal driver cleanup returned without errors and the outer process owner joined.
The Firefox case was not run, and no browser or TLS case is counted as passed.

Two smaller driver-only controls then observed a responding status endpoint and
live selected process. CIM and .NET listener queries returned no matching row.
A final system-netstat control, with readiness checks before/after, observed an
owned established loopback connection but no matching listening row. This
establishes reachable local HTTP and partial driver startup; it does not prove
the complete socket ownership/confinement invariant. The underlying visibility
cause remains unresolved; no PID-indirection, observation-lag or security-product
explanation is established. Driver-only controls completed and no test driver
remained in the final inventory.

The failed attempt and diagnostic controls are retained separately. Investigation
stopped after the final bounded control: removing the confinement precondition
to obtain a DOM pass would change the agreed harness contract. No browser
security setting was relaxed, no certificate was imported and no Tailscale
enrollment occurred. The Windows browser preparation milestone is incomplete;
WSL browser/driver installation and strict-TLS/product qualification remain open.

### SP-007-R06: actual WSL Firefox headless admission, 2026-11-04

An actual nonroot WSL Linux client ran the official Firefox 159.0.1 Linux amd64
distribution with GeckoDriver 0.37.1 and Selenium 4.52.0. Browser bytes matched
Mozilla's published SHA-512; its detached checksum signature verified against
the release key obtained from Mozilla. Two earlier verification-command setup
failures remain retained; they were not browser or signature-validation passes.
The generic ZIP reader could not inspect the bundled
[optimized Gecko archive](https://firefox-source-docs.mozilla.org/python/mozpack.html);
this inspection gap does not establish corruption or complete license closure.
The complete official bundle and notices were retained, with scoped private-use
license review before execution.

Preparation used an isolated virtual environment and verified, hash-locked
Linux-compatible wheels, bootstrapped offline from a separately verified pip
26.3 wheel. No system package, global Python, repository key or browser trust
store was changed. The chosen storage uses Windows-derived permissions: the
approved Windows user was the sole access principal on the directory and a
WSL-created witness file. Reported Linux mode `0777` was not represented as POSIX
privacy or separation between Linux users. Cross-process `fcntl`/`flock`
exclusion controls passed. A disposable unprivileged-user-namespace probe passed;
this is an OS feature observation, not Firefox sandbox qualification.

Before browser execution, six standard-library fixtures exercised the actual
listener parser: owned IPv4/IPv6 loopback, wildcard address, foreign inode,
missing listener and changing process generation. The first fixture attempt
failed because its mock lacked a path operation; the corrected fixture passed
without changing the runtime parser. Review also required the final process scan
to run on failed stages and to reject incomplete observation; both controls were
in place before the actual client run.

| Actual phase | Seconds | Observed outcome |
| --- | --- | --- |
| Portable browser and offline environment preparation | 49.423 | Passed |
| Main executable library and version preflight | 0.376 | Passed |
| Firefox driver/listener admission and data-page DOM | 12.271 | Passed |
| Full controller span through cleanup observation | 62.084 | Passed |

The ELF checks and returned capabilities identified Linux amd64 Firefox 159.0.1.
The requested `acceptInsecureCerts=false` was returned. A selected-process socket
inode and stable process generation established one owned loopback listener
before and after session creation; the test attached to the existing driver
without starting another service. The data page's initial title and title change
after a button click matched their oracles. Normal browser/driver cleanup
reported no errors, the process-group owner joined, and the final scan found no
matching test processes or visibility failures. The fresh profile remains in
protected test storage for inspection.

This supports the bounded actual-WSL Firefox headless/driver/DOM path on the
observed tools and storage. It does not establish TLS rejection/acceptance,
product flows, WAN behavior, resource savings, WSL Chrome or Windows browser
qualification. No sandbox-disabling options were added; runtime sandbox
isolation remains outside the case. The earlier Windows observer failure remains
unresolved and was not rerun or waived. Actual remotely attached sessions still
need the account, transport, confidentiality, lifetime and exact trust admissions.
These local client observations do not yet have a versioned public reproduction
harness; supported adoption requires a reviewed reproducible client harness.

### SP-007-R07: distinct Windows GeckoDriver admission, 2026-11-04

A separate GeckoDriver-only control tested whether the Windows observer gap also
affected the unrun Firefox pair. It launched no Firefox browser and made no HTTP
request. The reviewed driver used explicit loopback binding and the documented
[`--port 0` atomic allocation](https://firefox-source-docs.mozilla.org/testing/geckodriver/Flags.html).
A bounded, complete startup line supplied the selected port; six parser fixtures
passed before execution. The subsequent single socket query observed a stable
selected-process generation but **zero matching listening sockets**.

The control failed its admission oracle in 1.640 seconds. Cleanup through the
owned process handle joined the driver without errors, and the outer bounded
process owner joined. The Firefox pair stopped before session creation. This
independent driver result extends the observed Windows harness limitation; it
does not establish its cause or a product/browser defect. The earlier Chrome
failure remains retained. Neither pair may proceed by removing the listener
ownership/confinement check. No certificate, security setting or account state
was changed.

### SP-007-R08: WSL stock-Chrome preparation and storage admission, 2026-11-04

Preparation selected ordinary Google Chrome 157.0.8103.38 from the official Linux
package and a matching ChromeDriver. Chrome for Testing was inspected as an
alternative but was not substituted for ordinary-Chrome evidence. Google's
signed repository metadata authenticated the browser package; current signed
Ubuntu security metadata authenticated the missing NSS/NSPR packages. The driver
came from official version-specific HTTPS metadata/downloads with a retained
local digest, without an independently verified vendor signature or checksum.
No Chrome or ChromeDriver executable was launched.

Only the browser application subtree and private dependency files were extracted;
no package-maintainer scripts, system registration or privileged helper modes
were installed. The private library versions are NSS `2:3.98-0ubuntu0.22.04.5`
and NSPR `2:4.35-0ubuntu0.22.04.1`. An isolated verifier-path setup failure and an
initial extractor rejection of internal NSS links remain retained. The corrected
extractor used a fresh directory and validated relative links to existing files
inside it. Artifact-manifest generation needed Linux-native link inspection;
the earlier host-side inspection failure remains retained. Notices were retained
for scoped unchanged private research use; this does not establish distribution
license closure, advisory clearance or permission to exercise bundled DRM/media.

Static inspection covered 107 ELF paths, including the selected executables,
private libraries and additional bundled components. The unresolved libraries
were Qt 5/6 dependencies of the two Qt shims. Current primary Chromium source
describes optional UI-backend selection; the exact release-tag implementation
was unavailable. This supported considering a bounded headless probe, not a
claim that the complete browser distribution's dependency graph was resolved.

The prepared harness checks reviewed artifact membership/bytes, matching versions,
private NSS/NSPR loader selection and actual owned loopback listeners before HTTP
and after session creation. Seven listener and six startup-parser fixtures passed.
The first listener-fixture run exposed a parameter-shadowing defect; the fix and
an unrelated-port rejection case were in place before any executable launch.
Runtime-candidate review initially supported the bounded case, but a subsequent
source-first filesystem check invalidated its temporary-storage premise.

[Chromium's temporary-directory implementation](https://raw.githubusercontent.com/chromium/chromium/main/base/files/file_util_posix.cc)
honors `TMPDIR`; its [process singleton](https://raw.githubusercontent.com/chromium/chromium/main/chrome/browser/process_singleton_posix.cc)
checks actual POSIX temporary-directory permissions and socket-path suitability.
An actual nonroot Linux fixture requested mode `0700` on the chosen Windows-backed
filesystem but observed `0777`. Its representative Unix socket pathname was
189 bytes, and one `AF_UNIX` bind failed with a path-too-long error. The fixture
closed its socket and removed the generated temporary directory. These are observed
**environment-admission failures**, not a Chrome runtime or product failure.
Windows access controls and the earlier Firefox file-lock pass do not establish
Chrome's POSIX temporary-directory/socket compatibility. The source inspection
is version-sensitive; it does not prove the inaccessible exact release's code.

Chrome/driver execution is held. A short, owner-only native-Linux temporary
directory is a candidate remedy, subject to explicit storage-scope approval and
actual permission/socket admission. No new storage location, mount setting,
trust store or browser security setting was changed. Do not waive the checks or
silently fall back to unrelated temporary storage. Profiles, temporary state,
diagnostics and trust need separately defined locations and lifetimes in the
proposed harness. Actual versions, headless DOM, TLS, sandbox behavior and
integrated Chrome qualification remain unexecuted.

### SP-007-R09: bootstrap/lifecycle proposal and provider inventory, 2026-11-04

An independent full-scope audit confirmed that live A-F qualification remains
incomplete but challenged a broader claim that independent research was exhausted:
C and D still needed concrete mechanism proposals. This is a source/evidence
finding, not a successful bootstrap or expiry experiment.

Read-only GitHub inspection returned no configured environments; environment,
workflow and permission metadata could be read. A protected-branch flag was
observed, but it establishes neither immutable workflow admission nor a protected
secret-delivery mechanism. No environment, permission, secret, deployment approval
or workflow was created or changed. Metadata visibility is not proof of write
capability or successful protection enforcement.

The [bootstrap candidate](../implementation/hosted-verification-testbench.md#confidential-bootstrap-candidate-for-spike-c)
now names the sender/recipient, immutable workflow/source and run/attempt/job/
environment bindings, delayed environment reference, protected-job release,
single-consumer restriction and interrupted-delivery reconciliation. In
particular, the provider approval endpoint names a run and environment IDs, not
an attempt. The initial candidate rejects provider reruns, including job-only
reruns with cached admission outputs. Retry means a fresh dispatch/run,
environment and node identities; stale approval/cancellation and pre-delivery
predicate ordering remain necessary actual canary observations.

The [expiry candidate](../implementation/hosted-verification-testbench.md#expiry-enforcement-candidate-for-spike-d)
maps local supervisor, runner watchdog and separate privileged revoker to their
owned resources and shared failure domains. It proposes a frozen deadline and
producer-bound nonsecret arming record, with explicit unqualified clock and
artifact-attempt assumptions. Arming consumes usable time; it does not promise
30 usable minutes after readiness. This budget refinement remains Proposed.
GitHub cancellation, actual termination and node revocation require separate
observations; the vendor's normal-operation disconnect claim is not an executed
partition or outage result.

These contracts advance research only. No harmless canary, credential transfer,
revocation, active-stream expiry or outage campaign ran. The actual workflow/
protection fixture, sole-maintainer release policy, provider ordering, recipient
isolation, record binding and enforcement observations remain gates. Do not turn
proposal detail or read-only API success into C/D qualification.

### SP-007-R10: SSH source investigation, 2026-11-04

The maintainer prefers SSH over tsnet. The
[SSH investigation](../implementation/hosted-testbench-ssh.md) compares the
broker topology, source/recipient admission, endpoint ownership, separate
local/broker/runner lifetimes and dated cost/effort estimates. Primary-source
inspection supports local/reverse forwarding primitives, not this integrated
bench. Installed client inventory found Windows OpenSSH 9.5p2 and WSL Ubuntu
OpenSSH 8.9p1. No connection, broker, policy canary or new secret was created.

The source-first independent challenge exposed three important gaps in a plain
pair of SSH forwarding commands: broker restrictions do not constrain the reverse
forward's runner-side destination; broker loopback is not session authentication;
and stock local forwarding does not prove continuous ownership of a separately
prebound endpoint. The candidate retains independent runner network restriction,
broker isolation and local admission ownership rather than weakening those
contracts. Active-stream cutoff remains distinct from new-login rejection and
remote backend death. Current upstream manuals do not qualify installed releases.

This closes the source-comparison checkpoint only. Next is a bounded synthetic
Podman policy laboratory, subject to its reviewed harness and memory budget;
provider/budget/trust and live integrated qualification remain separate. Existing
Tailscale account-access blockage does not prevent independent SSH research.

### SP-007-R11: observer and native temporary-storage controls, 2026-11-04

A known-owned IPv4 loopback `TcpListener` was started and retained by the actual
Windows diagnostic process. The port/state query using terminating-error
handling reported CIM `CmdletizationQuery_NotFound`; the independent .NET
port-selected listener enumeration completed with zero matching rows. Owned
listener stop completed in `finally`; total control duration was 0.889 seconds.
No browser, driver or HTTP session was involved. This reproduces the observation
gap independently of browser version and corrects the earlier suppression of
query errors. It does not identify the Windows root cause or prove endpoint
confinement. Keep both Windows browser campaigns stopped; accepting wildcard,
bound or established rows would not repair the listener oracle.

The separately approved native Linux storage control ran as nonroot UID 1000
on ext4. Root and fresh temporary-directory modes were `0700`, ownership matched,
and an 87-byte representative Unix socket path bound successfully. The socket
closed and its temporary directory was removed. The fixture took 0.002 seconds
inside Linux; this excludes WSL startup and controller overhead. No Chrome or
driver launched in this control. The approved root remains for bounded probes;
no mount, trust or global permission setting changed.

Review then found that nesting the first Chrome candidate's temporary layout
would produce a 108-byte representative socket path despite the shorter control
passing. The candidate was shortened within the same admitted root and requires
its own exact-layout bind before launch. The 87-byte pass alone cannot qualify
every later path. Memory cancellation is likewise a separate harness control,
not a property inferred from a browser timeout.

### SP-007-R12: actual WSL Chrome after storage repair, 2026-11-04

After the separately approved native-storage boundary and R11 control, the
reviewed candidate separated retained executable/library artifacts from a fresh
native runtime root for profile, HOME/XDG and temporary state. It authenticated
the unchanged artifact manifest and exact script inputs, checked the final
nested temporary layout with an actual Unix socket bind, then ran one stock
Chrome data-page case. No global package, mount, certificate or security change
was made. Old failed fixtures and scripts remain retained.

Prelaunch review shortened the nested socket path and replaced PID-based
cancellation with a Linux pidfd acquired before identity checks. The helper
validates UID, process generation, command and source, signals through that
handle, and has no numeric-PID fallback. The affected tiny cancellation fixture
passed in 4.219 seconds: the supervisor received interruption, its owned child
was no longer running, and the Windows launcher joined. This is cancellation
mechanism evidence, not an actual low-memory browser stop result.

Actual executable/library preflight passed in 0.804 seconds. Chrome
157.0.8103.38 with its matching ChromeDriver returned Linux/Chrome capabilities
and `acceptInsecureCerts=false`; two owned loopback listeners were observed
before and after session creation. The data-page title and button-click title
change passed in the 5.025-second browser stage. The Linux stage-controller span
was 5.834 seconds, excluding initial artifact guards; the Windows monitoring
span was 14.079 seconds including launch/preparation overhead. Normal browser
quit, owned driver terminate/join and the final owned-root process scan completed
with no cleanup errors, remaining matches or scan failures. The fresh synthetic
profile and diagnostics remain protected for assessment.

The host monitor sampled Windows available memory once per second and reported
a minimum of 2,160,840,704 bytes (about 2.01 GiB). The existing 1.5 GiB stop floor
was not reached and no interruption was requested. This is sampled system-wide
availability, not browser RSS, a true peak, a hard memory ceiling or measured
backend-offload savings. Cancellation latency and intervals between samples
remain limitations. Podman and a local backend were not started for this case.

This supports the actual WSL stock-Chrome owned-listener/data-page path only.
It establishes neither application TLS, sandbox qualification, WAN/product flows,
Windows behavior nor integrated bench relief. A strict-certificate capability on
a data URL is not a positive or negative TLS test. Both Windows campaigns remain
stopped at their unresolved observer admission. SSH policy execution is the next
independent experiment, held for reviewed harness and adequate memory headroom.

### SP-007-R13: contained SSH mechanisms and counterexamples, 2026-11-04

One synthetic Podman laboratory exercised both SSH roles and harmless target
listeners in a single network-isolated container, without host ports, external
SSH access, application credentials, product trust or a backend. Actual packages
were Debian 12 `openssh-client`/`openssh-server` version
`1:9.2p1-2+deb12u10`, with OpenSSL 3.0.22. The base image was pinned; package
installation was not a locked dependency closure. The actual built image,
package inventory, effective role policy and bounded case diagnostics were
retained; no image was published or complete advisory/license clearance claimed.
This record does not add a reusable contributor SSH harness or required CI job.

Preparation outcomes remain distinct from the final mechanism result:

- Two initial pre-VM admissions refused an author-selected 6.5 GiB estimate.
  That estimate incorrectly treated configured WSL maximum capacity as necessary
  startup reservation. Neither refusal started the VM or SSH. The
  [Microsoft WSL configuration](https://learn.microsoft.com/en-us/windows/wsl/wsl-config)
  describes the memory limit; it does not establish equivalent startup demand.
- The revised owner retained the existing 1.5 GiB stop floor and global settings,
  armed monitoring before exclusively owned startup, and separately admitted the
  build from observed post-start headroom. Its 5 GiB startup estimate and 4 GiB
  post-start estimate are experimental allowances, not accepted project policy
  or hard bounds. The latter includes the 2 GiB warning level, a 512 MiB build
  allowance and 1.5 GiB for engine/cache/cancellation overhead.
- The first startup attempt failed on an invalid inventory-template field before
  image building. A documented JSON query repaired that harness error. The next
  attempt passed inventory but rejected an unsupported build CPU shorthand before
  building; installed CLI help established the equivalent quota/period controls.
  Both attempts observed zero owned residual containers and stopped the owned VM.
- The first actual SSH run passed the allowed path and wildcard rejection, then
  failed an overly narrow session-rejection classifier. The client reported a
  generic channel-open failure while the authenticated broker logged its session
  limit rejection. The affected repair required fresh, bounded broker evidence
  of correct-role authentication and exactly one session-limit rejection, plus
  client channel rejection. A generic connection failure alone remains insufficient.
  The failed run, successful cleanup and earlier observations remain retained.

After diagnosis and scoped independent review, the affected mechanism attempt
passed the following observations:

| Observation | Actual result / interpretation |
| --- | --- |
| Permitted client-to-runner forwarding | Fresh synthetic bytes returned the selected target's marker |
| Twelve forbidden operations | Both roles' shell, command and subsystem requests rejected; client reverse forward, runner wildcard/extra listener, runner local destination and client extra/other-session port denied |
| Reverse destination substitution | The same permitted reverse listener could point at a different runner-side target; broker policy alone did not enforce that destination |
| Listener endpoint reuse | An existing client connection could reach the replacement target after the reverse listener port was reused; session teardown/non-reuse remains necessary |
| Controlled client stop with an active stream | Stream closure observed; this is not absolute expiry, outage or compromised-owner qualification |

The engine reported rootful cgroup v2. Build execution used one job, a 512 MiB
memory/swap-total limit and one CPU via equal quota/period. The runtime's inspected
configuration was 256 MiB memory/swap-total, no network and no host ports, with
bounded PIDs and an init process. These configuration observations are not an
independent proof of peak memory enforcement or total guest/host overhead.

The successful image-build phase took 14.046 seconds; container start/attach took
4.500 seconds, including a 3.439-second internal case span. The Windows owner
span was 48.235 seconds including startup, observations, copying and shutdown.
Earlier package/image caches could be reused; this is not a cold-build benchmark.
Minimum one-second sampled Windows available memory was 4,504,535,040 bytes
(about 4.20 GiB), with no floor stop or watcher error. The watcher joined before
its exclusive lease was released. Direct child cleanup had no errors; the
created container was removed, the independent owned-label query returned zero
residuals, and a separate machine observation confirmed stopped state. The built
image remains a local cache artifact. No offload-saving or hard memory-ceiling
claim follows.

The two counterexamples support the SSH design's independent destination and
session-isolation requirements. Their mitigations were not exercised here.
All participants shared one namespace and Linux SSH implementation; another
port's rejection does not qualify full cross-session isolation. No actual
Windows/WSL SSH client, hosted runner connection, broker provider, authenticated
source/recipient bootstrap, absolute deadline, partition or product TLS was
qualified. Reuse only within these version/configuration/case boundaries.

### Current case disposition and next observations

| Scope | Evidence / status | Next decisive observation |
| --- | --- | --- |
| A1/A2 Current hosted fixture capacity | Passed R02 with stated measurement limits | Reuse within this source/tool/workload boundary; actual remote session remains separate |
| B1 Transport mechanism | R03 historical tsnet build; R10 SSH source comparison and R13 contained policy/byte-forwarding support | Qualify the selected actual Windows/WSL clients and broker/runner deployment |
| B2 Live isolation | R13 allowed path/12 denials and two gap counterexamples observed; actual broker/runner/session boundaries unqualified | Demonstrate independent runner destination restriction, broker isolation and teardown/non-reuse before live admission |
| C1 Confidential bootstrap | R09 attempt-bound protected-environment protocol proposed; no actual channel qualified | Admit workflow/protection/release fixture; demonstrate provider ordering, wrong-run/rerun/duplicate-consumer rejection and interruption cleanup with harmless canaries before credentials |
| D1 Lifetime enforcement | R09 overlay candidate retained; R10 local/broker/runner SSH owners proposed; runtime behavior unqualified | Prove active-stream cutoff separately from key expiry and runner/backend death; disposition outage residuals |
| E1 Stock browsers | R05/R07 Windows admission failed and R11 reproduced the observation gap without a driver; R06 WSL Firefox and R12 WSL Chrome owned-listener/DOM passed; TLS remains unqualified | Preserve Windows stop; qualify the separately admitted trust/TLS/product cases after transport and lifecycle harness readiness |
| F1 WAN/product relief | Blocked on B-E | Real Windows/WSL journeys and matched resource observations |

The complete A-F objective is unfinished. Public source has no credential-bearing
workflow or new local connector runtime. Review of the capacity checkpoint at
`442da7b` found no remaining substantive findings; normal CI and later research
record review must be assessed at their actual source revisions.

### Proportional checkpoint retrospective

The current milestone delivers measured hosted-fixture feasibility and preliminary
cross-platform build evidence, not a working remote bench. The duplicate-trigger
defect was found during component review and repaired before hosted execution;
no avoided runtime/cost total is claimed. Tooling preflight and API source review
exposed concrete preparation/lifetime constraints before trust or enrollment.

Local gate, hosted phases and connector phases have the durations recorded above.
Hosted capacity, normal CI, connector builds and documentation/review overlapped;
their durations must not be summed as elapsed wall time. Fine-grained research,
review and waiting time is unmeasured. No failed capacity or compiler workload
occurred in these attempts; corrected authoring/review findings are not fabricated
runtime failures. Raw diagnostics remain separate from sanitized public results.

Later Windows browser preparation stopped at a failed socket-observation
admission before DOM execution. Separate driver-only controls narrowed the gap
without establishing its cause or weakening the invariant. This is a harness/
environment evidence limitation, not a demonstrated product defect. That failed
attempt remains visible alongside the passing capacity/build observations.

The subsequent WSL Firefox preparation and data-page case passed with actual
process/socket observations. Its 62.084-second controller span is recorded
separately from the earlier local gate, hosted run and connector builds. Setup
and fixture defects were corrected before client execution; archive-inspection
limits and earlier failed attempts remain retained. The Linux result
establishes its own target path and does not resolve the Windows observer gap.

The distinct 1.640-second Windows GeckoDriver control also stopped at listener
admission before any browser session. This bounded negative result closes that
independent diagnostic opportunity without reopening the stopped Chrome branch.
Further Windows execution depends on resolving the observer contract rather than
repeating a known-unqualified browser case.

Stock-Chrome preparation then exposed a separate temporary-filesystem mismatch
before any executable launch. Initial harness review covered artifact/library,
driver ownership and cleanup composition but missed this compatibility premise.
The later source-first check and pure-OS fixture reopened admission; its negative
result supersedes the earlier launch disposition. A clean component review and
passing pure fixtures were insufficient to establish actual environment support.
No avoided-time total or broad review-effectiveness claim follows. Keep this
filesystem admission ahead of future client launches and revalidate it on storage,
mount or browser changes. Preparation/review durations remain unmeasured.

A subsequent suspension audit correctly distinguished blocked live qualification
from remaining independent proposal work. It exposed the missing concrete C/D
contracts, which were then drafted using source research and read-only provider
inspection. This improves the decision record without executing their protocol
or establishing process effectiveness. Their unknowns remain explicit; do not
close the full objective from a more detailed proposal.

Next owned follow-up: the bench operator prepares actual client/driver admission
and confidentiality/lifecycle probes; the maintainer retains account, trust,
vendor/budget and architecture decisions. Recheck the next documentation-only
push's cheap admission job to observe that capacity is skipped. This is a targeted
workflow control, not another general qualification campaign. Full research closure
and process-effectiveness claims remain premature.

The later maintainer-directed SSH investigation changed the preferred research
mechanism without accepting a broker or relaxing the transport contract. The
independent challenge found concrete destination, broker-local isolation and
endpoint-ownership gaps before implementation. The Windows positive control
reproduced the observer limitation without another browser campaign. The native
Linux control and final-layout correction then supported a passing WSL Chrome
data-page attempt; they do not erase R08's failed admission. The pidfd correction
was source-discovered and its affected cheap cancellation case passed before
Chrome. No avoided-cost or general review-effectiveness claim follows.

The first documentation verification attempt in the restricted local environment
failed with 37 errors and one assertion failure among 389 tests (three skips),
including unavailable Windows account-directory resolution. This is retained
as failed restricted-environment verification, not silently replaced by a later
check. The inspected assertion expected an owned-start failure but instead
received the same account-directory error; not every other traceback was
individually classified. The admitted rerun passed all 389 tests with three
Windows skips, naming and 42 synchronized engineering requirements. Exact
published CI remains separate evidence. Browser stages, documentation checks and review overlapped; their
durations are not additive wall time. Research/review time remains unmeasured.

Next improvement observations: keep the final socket layout control before
browser launch, retain kernel-handle identity for cancellation, and test SSH
policy with harmless canaries before provider purchase or credential delivery.
The next owned SSH run needs adequate memory; available RAM near the existing
warning threshold is not permission to start an additional VM. Full A-F closure
remains premature.

R13 exposed a further resource-premise mistake: an author estimate equated WSL
maximum capacity with startup reservation and prompted unnecessary memory requests.
Independent challenge corrected the premise without lowering the existing stop
floor or changing global configuration. Staged observation supported the small
laboratory under its actual allowances; it does not prove arbitrary future loads
fit. Earlier refusals remain retained, not relabeled as workload failures.

Inventory-field and build-flag failures also show that installed CLI/schema
preflight should precede dependent workload preparation. The session classifier
then demonstrated why a client message alone cannot establish the server's
policy decision. These were harness/preparation defects, not SSH or product
defects. Their diagnosed repairs and affected reruns retained failures and
passed through scoped review. Actual stage durations above exclude unmeasured
authoring/review time; no general efficiency or savings claim follows.

Next owned follow-up is the mitigated isolation/admission/lifetime experiment,
not another repetition of the passing mechanism cases. Provider purchase and
trust remain maintainer decisions. Keep a server-side witness where generic
client failures would otherwise be misclassified; revalidate affected controls
on version, policy, namespace or owner changes.

## Risks, review and reuse

[ADR-110's risk dossier](../implementation/hosted-verification-testbench.md)
R1-R11 remains applicable. Account access expansion, identity confusion, secret
misdelivery and lifetime/trust escape retain High/Critical impact, unknown
likelihood and P1 urgency before dependent work. The bench operator owns probes;
the maintainer retains vendor, security, budget and architecture acceptance.
No policy or trust risk has been accepted by this experiment.

Revalidate affected observations on source/workflow, image/toolchain, provider,
resource policy, workload, connector, account/policy/tag membership or trust-store
change. A prior pass may be reused only within its inspected conditions.
Retain the capacity prototype for reproduction; no production adoption or
required-check promotion is proposed. Record final exact-head composition review
and required checks at their actual revisions in the PR. Full A-F closure remains
pending.
