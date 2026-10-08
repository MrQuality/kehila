# Kehila spike procedure and register

Status: approved procedure, version 1, 2026-09-25. The
[project maintainer](../../CONTRIBUTING.md#repository-roles) approved this
version on 2026-09-25 for initial use with manual checks. Use the
first spike to assess the template and propose revisions if needed. This
approval does not approve architecture changes.

Procedure ID: **SOP-001**. Owner and approver: project maintainer. Approval
record: SOP-001 version 1 approved for initial use on 2026-09-25, with manual
assessment while automation is built. See the
[project procedure index](../procedures/README.md) for enforcement status. The
procedure-wide spike validator is not implemented; individual experiment runners
may implement their own preflight. Independent
PR approval is optional during the sole-contributor phase; record the
maintainer's assessment instead.

A spike answers a bounded uncertainty that prevents a design or delivery
decision. Its useful output is a supported conclusion, including an inconclusive
result when appropriate. Completing a spike does not mean shipping its prototype.
Repository requirements remain in the [contribution guide](../../CONTRIBUTING.md)
and [testing guide](../TESTING.md).

## Register

| ID | Investigation | Status | Linked work | Outcome |
| --- | --- | --- | --- | --- |
| [SP-001](SP-001-task-path.md) | First task create/update/read path | Concluded | [B-002](../product/backlog.md#b-002) | R04: bounded Python-adapter C01-C06 and full suite passed locally; 3.125 GiB container ceilings; first-increment direction accepted in [D-016](../product/decisions.md#d-016) |
| [SP-002](SP-002-postgresql-project-create.md) | Native PostgreSQL project-creation feasibility | Concluded | [#26](https://github.com/MrQuality/kehila/issues/26), B-005 | R04: original gap retained; separate corrected guard cases pass; six product checks remain |
| [SP-003](SP-003-replay-codecs.md) | Replay codecs and identifier representation | Concluded | [#39](https://github.com/MrQuality/kehila/issues/39), parent #26 | R08: exact prototype/database round trips pass; codec/ID recommendation awaits maintainer assessment |
| [SP-004](SP-004-actor-authority.md) | Trusted actor binding and initial-owner authority | Planned | [#40](https://github.com/MrQuality/kehila/issues/40), parents #11/#32 | No execution; trust/permission boundary needs a decision |
| [SP-005](SP-005-project-target-allocation.md) | Stable project-target allocation across retries | Planned | [#41](https://github.com/MrQuality/kehila/issues/41), parent #27 | No execution; allocation/reconciliation protocol needed |

Use sequential `SP-NNN` IDs, distinct from the existing acceptance scenario IDs.
Create each record from [TEMPLATE.md](TEMPLATE.md). Keep its ID and links stable.

## Where information belongs

| Information | Home | Purpose |
| --- | --- | --- |
| Question, scope, cases, concise results, limitations, next action | `docs/spikes/SP-NNN-topic.md` | One record someone can resume without the original conversation |
| Repeatable experimental code, fixtures, container definitions | `experiments/SP-NNN/` when needed | Versioned reproduction alongside the record; mark prototype code clearly |
| Accepted choice and rationale | Existing [decision log](../product/decisions.md) | Link the spike as evidence; preserve existing D-IDs |
| Unresolved choice and delivery follow-up | Existing [questions](../product/open-questions.md) and [backlog](../product/backlog.md) | Preserve Q-IDs and B-IDs; avoid duplicate task lists |
| Shipped behavior and retained regression tests | [Implementation status](../IMPLEMENTATION.md) and normal test locations | Record only integrated behavior |

Commit small, sanitized result summaries with the record. They must remain
understandable if external CI artifacts expire. Raw local output can use the
already ignored `.yaja/spikes/SP-NNN/` directory; do not depend on it as the only
evidence for a published conclusion. Exclude credentials, personal host details,
and real user data from shared artifacts. Use synthetic fixtures.

## Opening a spike

Search the register and related decisions first. Reuse applicable evidence or
state the changed condition that makes another experiment necessary. Record:

- The question and decision it will inform; linked B-, Q-, R-, and D-IDs.
- An owner, assessment responsibility (the maintainer may assess their own work
  during the sole-contributor phase), effort/time limit, and stopping condition.
  Leave unknown values explicit and settle them before execution. Reaching the
  limit permits an inconclusive report; scope expansion needs a recorded reason.
- Assumptions, alternatives worth testing, and excluded work. An engine comparison
  is appropriate only when engine choice is the uncertainty.
- A case table: variable, fixed conditions, expected observation, measurable
  threshold where needed, and the conclusion that observation could support.

For small investigations, brief answers in the template are sufficient. Mark
irrelevant fields with a reason instead of adding infrastructure for its own sake.

## Preflight and execution environment

Perform and record a fresh preflight for each run. Inspect resources and installed
software before starting the experiment's services. Check available RAM, CPU,
host and container-storage disk space against the actual planned workload; the
current development stack's roughly 4 GiB guidance is not a measured budget for
an expanded CDC stack. Record required tools, versions, image versions/digests,
available ports, image access, and relevant Linux settings.

Use Podman for service-based spikes, with application and supporting services in
containers and test commands on the host/CI runner. Check the actual Compose
provider and select `podman-compose` to avoid a Docker dependency; do not rely on
automatic provider selection. Podman's documentation explains that
[`podman compose` delegates to an external provider](https://docs.podman.io/en/latest/markdown/podman-compose.1.html).
Verify compatibility with the installed Podman version rather than assuming it.

On Windows, identify the chosen WSL environment and Podman machine. After static
resource checks pass, starting that machine and bounded disposable connectivity
probes is part of preflight. Verify the container connection and, for OpenSearch,
`vm.max_map_count`. Diagnose firewall/VPN interference when observed. Proceed to
the experiment workload only after its prerequisites pass; record blocked checks
as blocked rather than successful. On Linux CI, mark Windows-specific checks N/A.

Require only tools used by the chosen host test runner or builds. Record where
builds run so containerized builds do not acquire accidental host dependencies.
Give each run an isolated project/data namespace and clean up only its resources.
Tests needing recovery must preserve the intended data across process restarts;
removing a volume is a separate data-loss case.

## Reproduction and results

Before execution, provide exact commands or a script for preflight, setup, cases,
and cleanup. Make timeouts, readiness, fixtures, and cleanup explicit. Record the
source revision and any tested uncommitted changes needed to reproduce the run.
If automation is not yet implemented, label proposed commands as proposed.

Each run gets an ID such as `SP-001-R01`, a date with timezone, environment,
preflight outcome, commands, and case outcomes: pass, fail, blocked, or not run.
Keep observed values distinct from expected values and interpretation. Preserve
failed or inconclusive attempts when adding a successful rerun. Record resource
peaks and elapsed time when relevant; do not invent performance thresholds.

For storage/event boundaries, use real services as required by the testing guide.
State exactly what was established: connection readiness, saved data, durable
delivery, replay, and query visibility are different claims. A host resource
failure does not establish an architecture failure. A single successful run does
not establish general reliability or production capacity.

Repeatable scripts should report nonzero failure, isolate fixtures, retain useful
diagnostics, and clean up on failure. Local and CI execution should use the same
cases. Record which environments actually passed; do not infer Windows results
from Linux results. Experimental jobs may run on demand. Promote relevant cases
to normal regression coverage when their behavior becomes a supported contract.

## Closing, deciding, and reusing

Use statuses Planned, Active, Blocked, Concluded, and Superseded. Concluded records
also state an outcome: supported, rejected, or inconclusive. Record a recommendation,
alternatives, limitations, and remaining risks. A negative result is reusable.

Architecture changes follow the contribution guide's issue and review requirements.
When a choice is accepted, link its D-ID, rationale, tradeoffs, and review evidence
from the spike; update affected Q- and B-records. Keep proposed recommendations
distinct from accepted decisions. This use of context, alternatives, consequences,
and preserved decision history follows the
[ADR guidance](https://learn.microsoft.com/en-us/azure/well-architected/architect-role/architecture-decision-record).

Every conclusion states its reuse boundary: tested versions, configuration,
runtime, workload, and failure modes. Changes to these, a contradicting result,
or a new requirement trigger a targeted rerun. Reference the old spike instead
of copying its conclusion without context. Append new run summaries; when a
decision or investigation is superseded, link both records and retain the old result.

At closure, state what happens to each prototype: retained for reproduction,
promoted into supported code with tests, or removed with its tested revision
recorded. Keep executable artifacts while another contributor needs them to
reproduce an active recommendation.

Before ending any work session, update the record's handoff: latest run and
result, blockers, exact next action, and open choices. Before marking Concluded,
ensure someone can trace the question to cases, observations, conclusion, and
follow-up without reading a chat transcript. Update this register at the same time.
