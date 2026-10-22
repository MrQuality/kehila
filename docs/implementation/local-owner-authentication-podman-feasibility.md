# A0: Podman/WSL 2 resume feasibility

Status: Research result; executable readiness unresolved. Recorded: 2026-10-22.
Owner: project maintainer. Related #40/#49;
[container plan](local-owner-authentication-container-plan.md),
[Proposed ADR-106](../architecture/ADR-106-local-container-transport.md).

## Outcome and authority

The maintainer selected the same-host first-proof topology: Windows MVP client,
Linux-container backend managed by Podman using its WSL 2 provider. WSL 2 is
infrastructure, not frontend code or a native Windows backend. Microsoft describes
its [lightweight managed VM](https://learn.microsoft.com/en-us/windows/wsl/compare-versions);
Podman documents its [Windows machine boundary](https://docs.podman.io/en/v5.8.3/markdown/podman-machine.1.html).
No additional separately managed VM is selected.

The authorized outcome is read-only installed-state and resume-feasibility
research, with a reviewable PR. Baseline: `9e307a2b436741978c5cf3485c9143b3a588dc33`.
The topology selection supersedes the earlier Proposed first-target choice only.
It does not accept ADR-106's listener amendment or disposable-storage residual
risk, select production storage, or authorize an executable transport proof.

| Consequential premise | Evidence/status | Consequence if false or unresolved |
| --- | --- | --- |
| Podman/WSL 2 is the first-proof target on the Windows client host | Established by the maintainer's explicit clarification and agreement, 2026-10-22. | Reframe only if the maintainer changes this target; do not infer a separate Linux host. |
| Installed metadata establishes active guest behavior | Contradicted: the registered machine is stopped. | Keep runtime engine, clocks and effective ingress unqualified. |
| A selected API meets pause-inclusive budgets and the resume barrier | Unresolved by the reviewed sources below. | Dependent integrated probe remains behind the existing executable-readiness gate. |
| Empirical samples establish every required ordering guarantee | Unsupported. | A counterexample can reject a mechanism; successful samples cannot replace its API contract. |

## Read-only observation and limits

Sanitized version/state summary, not a reproducible runtime qualification:

| Item | Observation | Limit |
| --- | --- | --- |
| Windows Podman client | 5.8.3 | Guest server version not queried. |
| WSL package | 3.0.3.0 | Package version, not engine or networking evidence. |
| WSL-reported kernel | 6.18.54.1-1 | Version reported by `wsl --version`, not a running guest `uname` observation. |
| Windows version report | 10.0.26300.9711 | No support/security claim follows from this build. |
| Registered Podman machine | WSL provider, stopped; rootful preference false | Stored preference, not proof of running UID mappings or rootless isolation. |
| Networking | No explicit relevant override found in the inspected WSL configuration | No effective mode, forwarding or firewall result; do not infer defaults. |

Inspection used host-side version/list operations and named machine inspection;
[Podman inspect](https://docs.podman.io/en/v5.8.3/markdown/podman-machine-inspect.1.html)
documents the state and rootful-preference fields. No guest command, machine start,
container run, sleep/hibernate/freeze, compilation, trust mutation or security
change occurred. Machine names, connection details and raw metadata are omitted.

## Mechanisms examined

These are source findings, not measured behavior of the installed provider.
References were inspected on 2026-10-22; versioned Podman sources match the observed
client version but do not qualify the stopped guest. No new dependency is adopted.

| Mechanism / primary source | What the source establishes | Remaining gap against this plan |
| --- | --- | --- |
| [Rust Instant](https://doc.rust-lang.org/stable/std/time/struct.Instant.html) | Suspend accounting is unspecified and platform/version dependent. | Cannot establish pause-inclusive expiry or the 180-second budget by itself. |
| [Linux timerfd](https://man7.org/linux/man-pages/man2/timerfd_create.2.html) | BOOTTIME includes system suspension; ALARM clocks additionally wake the system and require CAP_WAKE_ALARM. CANCEL_ON_SET can report a discontinuous realtime change. | Guest system suspend is not a documented guarantee for every host/VM freeze. A wake capability or later clock-change indication is not a pre-continuation barrier. No extra capability is selected. |
| [Hyper-V timer specification](https://learn.microsoft.com/en-us/virtualization/hyper-v-on-windows/tlfs/timers) and [Linux Hyper-V clocks](https://docs.kernel.org/virt/hyperv/clocks.html) | Hyper-V exposes reference counters/enlightenments; reference time can stop across partition save/restore. Linux clocksource choice depends on platform features. | Does not identify this stopped guest's active clocksource or guarantee all WSL pause classes. Do not assert that all guest clocks always freeze or always include host sleep. |
| [Windows suspend/resume registration](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registersuspendresumenotification) and [suspend notification](https://learn.microsoft.com/en-us/windows/win32/power/pbt-apmsuspend) | Host notifications exist; suspend handling has approximately two seconds before possible interruption. | These pages give no ordering guarantee before WSL application continuation or assurance an asynchronous guest stop completes. Adding a host coordinator would need separate ownership/failure design. |
| [systemd-logind](https://www.freedesktop.org/software/systemd/man/latest/org.freedesktop.login1.html) | PrepareForSleep signals describe the guest OS sleep lifecycle. | No reviewed guarantee maps every host/VM pause into this guest signal before application continuation. Guest-bus exposure is not selected. |
| [Podman events](https://docs.podman.io/en/stable/markdown/podman-events.1.html) | Container pause/unpause events can be reported. | The stable reference is not a pinned guest implementation guarantee, nor an interlock for every host/VM pause. No event consumer or engine socket is added. |

A wall-clock-minus-monotonic heuristic also fails to establish the contract.
This is an inference, not an observed WSL defect: if UTC and the elapsed clock
both remain stale until delayed synchronization, the first resumed application
can observe the same values as an uninterrupted execution. Their difference need
not cross a threshold. Later detection cannot retract application work already
performed. Polling, a timeout thread or a stale cached host heartbeat alone does
not resolve that indistinguishability or serialize every required continuation.

Elapsed-time accounting and detecting every pause are separate obligations.
Even a pause-inclusive clock may account for a short pause without distinguishing
it from ordinary descheduling. The existing plan requires suspension to end the
instance, not just shutdown after the total lifetime eventually expires.

## Disposition and next evidence

**Unresolved:** no dependable adapter satisfying the existing full resume contract
was established by this bounded source review. This is not a proof of universal
impossibility, a reproduced WSL bug, or a failure of the selected topology.
The integrated transport proof is not executable-ready. Its resume gate remains
in force; other ingress, key/file, dependency and trust gates also remain open.
No host supervisor, new privilege, awake-only assertion or eventual-detection
substitute is adopted. ADR-105 remains Accepted and unchanged; ADR-106 Proposed.

A candidate needs a reviewable contract covering all three obligations:

1. Pause-inclusive elapsed expiry/lifetime plus valid UTC, including host sleep,
   hibernate and VM suspension/freeze/restore; no unsupported clock extrapolation.
2. A barrier before new admission or continuation of owned pending application
   responses, including already-established connections. Define the synchronization
   point, loss/crash/reconnection behavior and prevention of stale permission reuse.
3. Bounded shutdown while runnable, fresh-instance restart after valid time, and
   no claim to retract bytes already handed to the kernel/client before a pause.

Before authorizing dependent implementation, review a concrete provider/API
contract and the mechanism's failure oracles. If no such contract can be obtained,
bring the smallest proposed change to the maintainer; do not relax the criterion
or add a Windows authority component under the heading of a test helper.

A possible follow-up is a separately reviewed, bounded characterization harness,
not the integrated TLS probe. Its design should specify independent host and
guest observations, an explicitly owned machine, sample/record limits, one
approved transition per case, failure/cleanup paths and relevant resource preflight.
[Windows interrupt time](https://learn.microsoft.com/en-us/windows/win32/sysinfo/interrupt-time)
is a candidate host elapsed observer; the unbiased variant excludes sleep and
hibernation. Neither is a guest admission mechanism or a qualified oracle here.
Bracket cross-boundary samples with host elapsed readings; record observation
uncertainty rather than subtracting unrelated clock epochs.

Keep injected stale-clock logic cases separate from real host sleep/hibernate
and VM freeze/restore cases. Guest clock samples and first-application-work
instrumentation can reveal violations; observing one instrumented task cannot
prove that all application continuation was barred. Passing samples characterize
only the exercised provider/version/case, not a universal ordering guarantee.
Where a transition cannot be safely arranged, mark its case unqualified.

No harness is implemented or authorized to run by this result. Later execution
needs exact reviewed code, fresh machine/resource and host-transition permissions,
RAM checks and owned cleanup. The current research needs no certificates or trust
changes. Report partial evidence as partial; no A0/T01-T16, A1-A5, ADR-103 or
replicated-deployment (#65) qualification follows.
