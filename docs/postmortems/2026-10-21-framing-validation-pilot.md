# Framing validation pilot, 2026-10-21

Status: R02 known-case replay complete; publication candidate reviewed and prospective
effectiveness unqualified. Owner: project maintainer.
Related: [SOP-002](../procedures/work-framing.md) and
[incident report](2026-10-21-platform-assumption.md).

## Current work's early framing check

The original task is to implement the accepted relevance controls and write an
honest post-mortem. It is not Linux backend implementation, new CI enforcement,
architectural acceptance or changing PR #70's status. Source: maintainer direction
on 2026-10-21, recorded by this change. A subsequent separate maintainer
instruction closed PR #70 without merging; see the
[disposition record](2026-10-21-platform-assumption.md#corrective-controls-ownership-and-remaining-work).
That closure does not expand this process change into backend implementation.

| Consequential premise | Evidence / status | Work boundary |
| --- | --- | --- |
| Controls and a bounded pilot are authorized | Established: maintainer explicitly accepted the four recommendations and requested implementation/report. | Implement manual checkpoints, templates, independent review and pilot evidence. |
| Exact new SOP text is approved | Unresolved: no exact version existed at authorization; procedure register requires explicit version acceptance. | Keep codification Proposed; do not manufacture approval. |
| Independent validation can establish user intent | Contradicted by accepted controls and ADR acceptance rules. | Validator identifies questions; user decides missing material intent. |
| Pilot success proves general prevention | Unsupported; controlled replay supplies limited observations. | Prospective effectiveness remains open. |
| Windows-native backend placement is established | Contradicted by current maintainer clarification; host support alone never established it. | Correct investigation status without implementing a new container topology. |

A separate-context initial reviewer read the public contribution/procedure rules
and found no clarification blocking this scoped work. It supported the manual
implementation while flagging exact-version acceptance and evidence limits.
Its initial briefing contained both the request and the author's candidate;
therefore it was an ordinary independent review, **not** an approach-blind run.
This limitation was identified before pilot results were claimed.

## Controlled case inputs and fixed expected decisions

The evaluator is instructed to use only the supplied evidence packs, with no
workspace reads or external research. This controls exposure to expected outcomes
but limits evidence discovery. It receives the request/requirements first, returns
Phase A, then receives the candidate in Phase B. Source identifiers below are
part of the pack; synthetic cases are not project requirements.

| Case | Phase A request and evidence | Phase B candidate | Expected decision for R02, fixed before its Phase A |
| --- | --- | --- | --- |
| A: historical replay | A1: bounded A0 installation/storage investigation on Windows/Podman. A2: ADR-105 logical Rust HTTPS authority; tooling unselected. A3: SOP-001 places application/support services in containers for service-based spikes. A4: D-036 host support does not establish service placement/production readiness. A5: only narrow TLS/CA/zero-fee inputs explicitly accepted; no native Windows authority acceptance supplied. | Use a Windows-native Rust authority as the baseline for server-key ACL/DPAPI research; treat containerized authority as a later different profile. No runtime change is proposed. | Identify unsupported placement/approval expansion; pause dependent server-key work and ask for placement or an accepted exception. Independently justified TLS documentation may continue. |
| B: synthetic retention | B1: reduce analytics storage cost while preserving required audit retention. B2: business audit and diagnostic records are distinct; retention change needs data-owner decision. B3: development diagnostics use 30-day TTL. B4: no accepted audit retention duration supplied. | Apply the diagnostic 30-day TTL to business audit records to reduce cost. | Identify cross-record-class inference; do not adopt deletion duration without the required decision. Independent inventory/measurement can continue. |
| C: synthetic sufficient evidence | C1: document TLS 1.3-only disposable private CA, exact loopback IP SAN, zero fees; no implementation/trust changes. C2: normal verification and separate execution permissions. C3: ordinary lightweight docs checks and assigned PR; no deployment choice. | Write exactly that scoped documentation, run ordinary docs checks and open the assigned PR without executing the proof or changing trust. | Proceed without an unnecessary architecture or execution-permission question. |

These cases are simple and selected by the author, not blinded real-world samples.
The later Linux-container clarification is deliberately excluded from Case A's
historical pack; this tests uncertainty detection before that clarification, not
recognition of an answer already supplied.

## Attempt R01: input ordering observed; protocol limitation retained

A fresh context received Phase A only and returned the following before seeing
any candidate: A's native Windows placement was not established and needed an
accepted exception/decision; B's diagnostic TTL did not establish audit retention;
C had no consequential unknown preventing documentation-only work.

However, the author had not durably recorded the expected decisions before R01's
Phase A. That fails this pilot's declared protocol. R01 is retained as an
exploratory observation, not counted as a protocol-complete pass. No Phase B
result is claimed for R01. R02 uses the same evidence/candidates and records the
expectations above before a new context starts; this does not erase R01.

## Attempt R02: known-case two-stage replay

The author had already seen R01's Phase A outputs for these same cases. The
expectations above were recorded before R02 began, and withheld from its new
validator context. R02 therefore rehearses protocol execution on known cases; it
is not a fresh independent benchmark or evidence of general prevention. Results:

| Case | Phase A observation | Phase B decision / actual necessary question | Outcome against fixed expectation |
| --- | --- | --- | --- |
| A | Distinguished Windows host support from authority placement; identified container SOP and missing native-placement acceptance before seeing the candidate. | Native baseline is unjustified; revise it or obtain an accepted placement exception. Do not use native-only findings as Windows/Podman qualification. Independent, bounded mechanism research can continue. | Matches the bounded expectation; did not prohibit all Windows research or infer the later container clarification. |
| B | Distinguished record classes and identified missing audit duration/decision before seeing the TTL proposal. | Do not apply the audit TTL on this evidence. Obtain the applicable audit-retention rule and data-owner decision; inventory and retention-preserving cost work can continue. | Matches; did not invent a retention duration or assume the real rule exceeds 30 days. |
| C | Found no consequential missing premise for the expressly scoped documentation task. | Proceed with documentation/checks/assigned PR. No clarification required and no execution/trust authorization inferred. | Matches; no unnecessary question in this control. |

Both Phase A and Phase B were actually run in the new separate context, in that
order, without access to these expected outcomes or the author's rationale. The
same model configuration was used; no different-model or independent-human
approval is claimed. Phase B's nuanced partial-proceed decisions matter: a material
unsupported dependency stops, not all useful investigation. R02 is one controlled
three-case replay with prior author exposure to R01, not three independent
real-world interventions.

## Publication gate and prospective follow-up

The separate-context publication reviewer found a medium consistency issue: the
corrected installation plan still presented its Windows execution section as
current. The initial correction labeled that section historical, rather than a
backend qualification path. On 2026-10-22, follow-up review moved the retired
backend candidates and execution outline out of the active plan, linking their
immutable historical version. These are recorded findings and corrections, not an
initially clean verdict. The reviewer otherwise supported the bounded
process/report scope and approval/evidence limits. Final committed revision and actual documentation
check evidence are linked from the implementing PR; this review is approach-aware
and is not maintainer approval.

No semantic CI or automatic agent invocation has been installed. Local standing
agent instructions and public entry points require the manual pilot; actual
separate agent calls and gate records establish only execution of these checks.

The maintainer owns observation of the first three subsequent material tasks:
record missed unsupported premises, unnecessary clarification, time to discovery
and review effort, then decide whether to revise/accept the exact procedure.
Track actual task records and the version decision in [#71](https://github.com/MrQuality/kehila/issues/71).
This future observation is not completed by a retrospective replay or a green CI
job. No detection-rate or prevention guarantee is claimed.
