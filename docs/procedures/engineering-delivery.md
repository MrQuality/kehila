# Engineering delivery, review and risk assessment

| Field | Value |
| --- | --- |
| ID / version | SOP-003 / 2 |
| Status | Proposed codification; accepted controls in manual use |
| Authority | Maintainer accepted the PR #92 post-mortem recommendations and milestone/large-goal recurrence on 2026-11-03. This records that authorization, not acceptance of this exact SOP version. |
| Revision | General verification practices accepted 2026-11-07; this revision incorporates them without accepting the exact codification or adding semantic CI enforcement. |
| Owner | Project maintainer; delivery author maintains the task record |
| First application | PR #92 process amendment; [retrospective](../postmortems/2026-11-03-authenticated-project-delivery.md) |
| Enforcement | Human/agent assessment and existing required checks; no semantic CI enforcement or branch-protection change |

## Scope and operating record

Apply these controls to material engineering deliveries. Use the existing
[framing brief](work-framing-template.md), task or PR as the single record;
the [record fields](engineering-delivery-record.md) extend it rather than require
another registry. Small routine edits do not require every layer or another risk
agent. Preserve [contribution policy](../../CONTRIBUTING.md), accepted decisions,
execution permissions and confidentiality rules. No review verdict grants access,
accepts architecture, waives tests or authorizes merging.

Before implementation, name the outcome, included/excluded work, completion
criteria, coherent commit boundaries and consequential uncertainties. A checkpoint
is ready when its contract is coherent and its required review/checks pass; commit
and publish it promptly. Adjacent focused commits may share a push to avoid
redundant CI; record the bounded hold and next publication checkpoint. Do not
accumulate the entire implementation awaiting a
perfect final campaign. Do not commit broken scaffolding to manufacture progress.
Where a push would knowingly repeat an already diagnosed CI failure, document the
short hold and publish the focused repair with the held checkpoint. Avoid arbitrary
line/time quotas. Large exceptions need a component map and coupling justification;
distinguish production code, adopted source, generated locks/notices, tests and prose.

## Independent risk challenge

Optional agent entry points are the versioned
[engineering risk skill](../skills/engineering-risk-assessment/SKILL.md) and
[Kehila review overlay](../skills/kehila-engineering-review/SKILL.md). They express
these same rules for agent use; contributors can follow this procedure directly.
Installing a skill or passing metadata validation does not enforce it or prove
effective behavior. Preserve user and project policy over conflicting generic rules.

Integrate risk assessment into SOP-002's existing source-first Phase A/B validation.
Before dependent work, give the challenger the original request, accepted decisions,
source identities, environment and operating constraints, including contrary
evidence. Preserve its framing before supplying the candidate. The challenger
identifies material failure mechanisms and the cheapest decisive observation; it
does not replace the author's plan or manufacture completion gates.

For each material risk, record consequence/affected invariant, evidence and confidence
(including unknown likelihood), existing controls, proposed action/probe and its
cost, residual risk, owner, acceptance authority and invalidation condition. Keep
likelihood and severity separate from urgency. Qualitative rankings aid judgment;
an arithmetic score cannot accept a severe risk. A hypothesis is not a confirmed
defect. Distinguish product failures, harness failures, environment incompatibility
and process uncertainty.

The author converts substantiated risks into ordered work or a bounded probe;
the maintainer resolves nondelegated scope/security/architecture decisions. Record
rejected claims and their evidence-backed disposition in the existing record.
Refresh only affected risks at a material contract, deployment, actor/data or
permission change, or contradictory observation. Before qualification, check that
material risks have meaningful controls/oracles. At closeout, identify residual
risks and owners. No per-commit risk ceremony or universal rehearsal campaign.

## Layered review

| Layer | Trigger and responsibility | Output |
| --- | --- | --- |
| Mechanical | During development; author uses applicable format/lint/static/source checks | Actual command/results and revision; no semantic-correctness claim |
| Intent and risk | Before dependent material work; independent source-first challenger | Supported outcome, material premises/risks, dispositions and decisions |
| Component | Stable or materially changed contracts; scoped reviewer | Boundary/invariant coverage and concrete findings for codecs, storage/authority, lifetimes, transport or interface |
| Integration and harness | Before costly qualification or user participation | Producer/consumer compatibility, cancellation/failure composition, environment admission and oracle/cleanup validity |
| Final exact head | Before review readiness; independent reviewer | Whole-outcome composition, unresolved findings, evidence applicability and exact committed head |

Assign overlapping boundaries explicitly so that multiple reviewers do not each
repeat a whole-repository review or leave a shared contract unowned. Read-only
component reviews may run concurrently within resource permissions. Heavy phases
remain governed by the selected execution profile. Use separate author/reviewer
contexts and direct source inspection; model diversity is optional and is not
proof of independence. Use only available, authorized models/tools.

Start with the original outcome and authoritative contract, then invariants/tests
and implementation. Tests do not establish missing user intent. Every review names
base/head or tree identity, covered contracts/files, source assumptions, checks
actually executed, findings by severity and priority, exclusions and disposition.

Reuse a review only after inspecting the intervening delta against its recorded
validity conditions. A changed shared contract, caller, environment or cancellation
path can invalidate a review without changing the reviewed file. Mechanical prose
corrections do not restart all layers. Final exact-head review still assesses
composition; component passes or a matching tree alone do not replace that work.

Mutation experiments are selective: use them when they can settle a material test
oracle question, only in an isolated export with the necessary execution authority
and bounded cleanup. Never mutate the shared working tree or run a new suite merely
because a generic skill says so. A surviving mutant requires reachability/equivalence
analysis before concluding a test is missing.

Remove newly introduced unused code within authorized scope once unused status is
established. Check API compatibility, migration and retained-evidence obligations
before removing older code; ask only where that consequential uncertainty remains.
Do not promote optional cleanup into an unrelated delivery blocker. Missing optional
review skills must be disclosed; use a bounded direct assessment without claiming
that the missing skill ran. Preserve mandatory specialist checks when applicable.

## Contract-based verification

The maintainer accepted these general practices on 2026-11-07. The delivery
author applies them with the relevant component/integration reviewer, using the
existing risk and review record. They refine risk-based testing rather than add
a universal matrix or a separate approval ceremony.

1. Test permitted and prohibited outcomes against the current contract. Isolate
   the property under test, preserve valid unrelated inputs and use positive
   controls where needed. A rejection must be attributable to the intended
   boundary; an unrelated parse error, timeout or crash cannot satisfy it.
2. Trace changed contracts through producers, consumers, permissions, invariants
   and existing tests, including callers and negative assertions outside edited
   files. Select affected checks and reassess retained evidence from that trace.
3. Verify meaningful composition boundaries with the smallest representative
   integration check. Component success does not establish behavior under a
   different role, real storage/transport, implicit locking or cancellation path.
4. Treat failure and interruption as lifecycle behavior. Select consequential
   fault points by risk; observe fault activation, permitted outcomes, durable
   state where relevant, ownership, cancellation, cleanup and recovery. Normal
   cleanup alone is not evidence of interrupted cleanup.
5. Keep one authoritative current summary of source, execution/evidence scope,
   remaining work, blockers/decisions, live workloads and resource availability.
   Distinguish prepared, executed, passed, failed, stopped, deferred, superseded
   and excluded work. Label historical checkpoints and preserve their evidence.

Use the [testing guide](../TESTING.md#risk-based-selection-and-fault-testing) for
selection and fault-experiment guidance. Cheap relevant mechanical checks should
precede expensive integrated work where practical. Run selected boundary/fault
checks during development; reserve full applicable qualification for the completed
goal. Required backstops remain mandatory and do not infer semantic risk coverage.
No automatic risk engine, chaos platform or expanded execution authority follows
from these practices. Evaluate their use and limits at the next existing closeout.

## Environment and harness admission

Exercise the smallest representative path on the actual target early enough to
change implementation: pinned tools/image, supported engine/version/options,
nonroot file/volume ownership, basic connectivity and owned cleanup as applicable.
Queue CI at the first coherent checkpoint containing its prerequisites. Local
success does not prove another engine/version works. Reuse valid compatibility
evidence; do not add another always-running environment campaign.

For new harness mechanisms, inspect protocol framing/output ordering, typed error
classification, permissions/copy semantics, deadline/permit ownership, independent
final-state oracle and failure cleanup. Use focused positive/negative controls for
uncertain mechanisms before the integrated campaign, not a universal matrix.
Capture consumed protocol packets as well as subprocess diagnostics, with bounds
and redaction, so a classification failure remains diagnosable.

Preserve failed attempts and perform owned cleanup before diagnosis. Do not relax
product limits, silently retry or substitute a host workload to make a test pass.
Honor the actual failure/retry authorization. Retain unaffected passing evidence
only after checking source, environment, fixture and oracle premises; name affected
reruns and necessary controls. Required hook/CI checks remain mandatory. The actual
path classifier determines scope; prose or a skill's description is not an exemption.

## Milestone and large-goal closeout

At completion, cancellation or material suspension of each milestone or large goal,
the author prepares a proportional [retrospective](delivery-retrospective-template.md).
An independent context challenges causal and effectiveness claims using original
outcomes and decisive evidence. Reuse the delivery's existing records; do not start
a new qualification campaign for the retrospective. A single report may cover
coincident milestone/goal boundaries. Routine commits do not require retrospectives.
Editing a retrospective or implementing its routine improvements does not itself
recursively trigger another report; record that work in the existing closeout.

Record outcome versus original/revised scope; commit/publication and decision
timeline; product, harness, environment and process findings; valid evidence and
limits; measured time/resource observations; proposed improvements, owner, priority
and acceptance/implementation status. Unknown time is unknown; failed-run duration
is not automatically waste. Preserve earlier failures and explicit deferrals.

Use actual phase events for preparation, compilation, tests, cleanup, review,
waiting and other/unknown work. Track UTC boundaries and process-local monotonic
durations when available. Concurrent spans are not additive wall time. A failure
records its actual command/oracle stage separately from a parent/controller label.
Never rewrite historical raw evidence to correct classification; append a sourced
correction. Mark gaps rather than reconstructing precise hours from commit dates.

At the next applicable closeout, evaluate time to discovery, checkpoint delay,
repeated review coverage and verification rework against observed records. Do not
promise savings, invent numerical targets or make future effectiveness proof a
condition for adopting these already accepted controls. Material procedure revisions
return for explicit approval; exact SOP-002/SOP-003 codification remains separate.
