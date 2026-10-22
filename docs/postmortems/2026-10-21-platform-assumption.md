# Post-mortem: platform assumption redirected A0 storage research

Recorded: 2026-10-21. Owner: project maintainer. Status: corrective controls in
pilot; prospective effectiveness and deployment-plan reconciliation remain open.
Related: [B-019 / #71](https://github.com/MrQuality/kehila/issues/71), #40/#49, [SOP-002](../procedures/work-framing.md),
[pilot evidence](2026-10-21-framing-validation-pilot.md).

## Summary and impact

An explicitly Proposed Windows-host diagnostic profile became the working premise
for increasingly specific server-key research without validating its relevance to
the intended backend deployment. The maintainer clarified that the backend is
intended to run in a Linux container. The failure was misdirected investigation
and misleading next-step guidance, not an established production security defect.

PR #70 contains 304 added lines and one removed line across eight documentation
files at `024e42a`; its two commits investigated Windows server-key storage and
candidate APIs. The primary backend recommendation does not answer the clarified
deployment question. Some findings could inform a Windows client or a separately
required provisioning component; that possible reuse does not justify the detour
or make the backend recommendation applicable.

No executable, manifest or lockfile changes occur in PRs #68-#70. Their ordinary
documentation checks do not establish architectural relevance or installation
qualification. No product vulnerability, production deployment or data exposure
is established by this incident. Effort and opportunity cost were not measured;
no invented hours, money or prevention rate are reported.

The maintainer separately clarified client scope on 2026-10-21: clients may target
Windows and Linux, with Windows prioritized for MVP. The correction concerns
backend placement; it does not establish a permanently Windows-only client.
Client-platform support and its evidence remain distinct from container backend
placement and host-specific credential storage.

## Evidence and chronology

Times below are public GitHub event times in UTC; the report date follows the
maintainer's 2026-10-21 session. Maintainer clarifications in that session are
recorded here as current direction, not presented as a public historical quote.

| Event | Observed evidence and boundary |
| --- | --- |
| Existing controls | [SOP-001](../spikes/README.md#opening-a-spike) already requires assumptions/alternatives; its environment section places application and supporting services in containers for service-based spikes. [D-036](../product/decisions.md#d-036) defines host support without qualifying deployment. |
| PR #68 merged 2026-10-20 16:56:45 UTC | [PR #68](https://github.com/MrQuality/kehila/pull/68), merge `a916518`. Its [plan at the reviewed source](https://github.com/MrQuality/kehila/blob/7eb7c502273b35dafb96644a6c69456d11a67d1a/docs/implementation/local-owner-authentication-installation-plan.md#proposed-installation-profile) explicitly proposes browser, CLI and Rust diagnostic on Windows, backend services in Podman, and a separate Linux-container authority profile. It also explicitly retains Proposed status. |
| PR #69 merged 2026-10-20 19:13:42 UTC | [PR #69](https://github.com/MrQuality/kehila/pull/69), merge `3a90ce2`, records the agreed private-CA/TLS inputs and zero certificate fees. That narrow agreement does not accept every deployment/storage premise in the surrounding proposal. |
| PR #70 opened 2026-10-20 19:47:04 UTC | [PR #70](https://github.com/MrQuality/kehila/pull/70), head `024e42a`, develops Windows server-key/file-operation research. It was open and unmerged when inspected for this report. Its [summary](https://github.com/MrQuality/kehila/pull/70#issuecomment-6299963151) accurately limits native evidence, but does not repair the unsupported deployment premise. |
| Detection on 2026-10-21 | The maintainer questioned why Windows server-key storage was being investigated for a Linux-container backend, then requested a general process diagnosis rather than a blame account or platform-specific instruction patch. |
| Corrective direction on 2026-10-21 | The maintainer accepted early evidence framing, independent relevance validation, boundary/publication revalidation and a controlled pilot, and requested implementation plus this report. Exact new SOP-version acceptance remains separate. |

The host-native investigation was visible in Proposed documentation. It was not
hidden executable behavior, nor proof that a Windows-native production architecture
had been accepted. Merging a proposal and authorizing further research did not
establish the original premise independently of the author's framing.

## Failure point and causal assessment

The missing effective control was **evidence-backed validation of consequential
premises before dependent effort**, followed by revalidation as the work changed
meaning. The author treated a development/test environment as enough basis for a
service-placement proposal, then reused that proposal when recommending the next
research task. Contrary or limiting requirements were not reconciled first.

The observable chain is: development host context; Proposed host-first profile;
later research using that profile; technically detailed source findings and green
documentation checks; maintainer detects the relevance mismatch. Repetition made
the premise appear settled without new acceptance evidence. This is a process
interpretation supported by that chain, not a measured account of model cognition.

| Existing control | Why it did not prevent this instance |
| --- | --- |
| Assumption and ADR requirements | Present in the guides, but the workflow did not produce a consequential-premise disposition reconciling host support, container guidance and actual backend placement before the research. |
| Proposed/Accepted distinction | Labels were retained, yet proposed placement still governed downstream recommendations. Correct labels alone did not control reuse. |
| Technical source/security review | Tested or inspected correctness within the chosen framing; it did not independently establish that the framing answered the intended problem. More review of Windows APIs would not resolve deployment intent. |
| CI and documentation checks | Checked documented technical/structural scope. They could pass while the selected investigation was irrelevant. |
| Maintainer PR review | Remained a real control and the maintainer detected the issue, but relying on that late detection transferred premise discovery to the final reviewer after effort had accumulated. |

The correction must therefore change when and how relevance is checked, not just
add a rule saying to remember the deployment platform. No individual reviewer or
agent is assumed immune to the same unsupported framing.

## Corrective controls, ownership and remaining work

| Action | Owner | Implemented evidence / remaining acceptance |
| --- | --- | --- |
| Early framing gate | Task author | [Brief](../procedures/work-framing-template.md): requested outcome, consequential premises, evidence/status, counterfactual impact and dependent work. Manual pilot; no automated semantic enforcement. |
| Independent relevance check | Task author and separate validator | [Two-stage assignment](../procedures/work-framing-validator.md); original request/authority before candidate. Actual pilot inputs/results and deviations are recorded separately. |
| Boundary/publication revalidation | Author and validator; maintainer decides intent | [SOP-002](../procedures/work-framing.md) requires delta review and final-source consistency. An agent verdict is not architectural acceptance. |
| Pilot and future evaluation | Project maintainer | [Pilot record](2026-10-21-framing-validation-pilot.md) retains failed protocol attempts and bounded results; effectiveness observations of the first three material tasks after version approval remain open, not prerequisites to initial approval. |
| Stop reuse of host-first placement as backend direction | Project maintainer / #40/#49 | Current plan correction distinguishes intended container backend, historical host evidence and unresolved container-specific design. No implementation or trust change is authorized here. |
| PR #70 disposition | Project maintainer | Closed without merging on 2026-10-21 under a separate explicit maintainer instruction; see the [closure record](https://github.com/MrQuality/kehila/pull/70#issuecomment-6307732761). Its research remains historical, not an adopted backend direction. |

Contributor, spike, ADR and PR entry points are updated for the pilot. The exact
new procedure text remains Proposed under the existing procedure approval rules.
This implements the controls for authorized pilot use without claiming their
general effectiveness or silently modifying branch protection/required reviews.

The next backend task must establish the actual container boundary, TLS listener
exposure/termination and key provisioning/storage requirements before choosing
platform-specific adapters. Preserve accepted identity/security/TLS constraints;
do not silently resolve container binding/forwarding by widening exposure.
Linux implementation and a revised installation design are outside this process
change. Windows CLI credential storage remains a distinct concern under ADR-105.

## Validation limits

Retrospective evidence can show which checks would flag these bounded cases; it
cannot establish that future agents will always retrieve the right sources or
that this workflow would have prevented all earlier mistakes. Separate contexts
can share model blind spots, and author-selected evidence can omit decisive facts.
Mechanical checks validate files/links, not intent. Record actual pilot deviations
and future misses rather than treating this document as proof that the problem
is solved. Final-source publication assessment and documentation verification
are recorded in the implementing PR and pilot record.
