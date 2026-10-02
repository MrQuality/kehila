# Product planning

The [engineering standard](../engineering/ENGINEERING-STANDARD.md) adds independent
quality/release gates E0–E5. Product milestone M0–M4 and M1 child IDs here keep their
delivery meaning. Technical decision governance lives in
[architecture records](../architecture/README.md); existing D-* records remain
the source of product decisions, including accepted architecture-facing behavior.

Baseline: 2026-09-23. These documents describe planned product behavior. See
[implementation status](../IMPLEMENTATION.md) for available components.

## Scope

YAJA is intended to support local project and task management, task knowledge,
sprint and Gantt planning, shared resources, scheduling, time tracking, and
resource costs. Local use is the initial deployment target; broader hosting
requirements remain open.

The accepted first increment includes the full configurable project/work-item
model, custom types and fields, workflows, conversion and migration, relationships,
delegated administration, estimates, knowledge, and durable local storage.
[D-019](decisions.md#d-019) records this scope; the
[M1 implementation slices](backlog.md#m1-configurable-model) divide its delivery.
The complete first-release scope
also includes resource management, both planning views, manual and automatic
scheduling, authorized overrides, manual entries and timers, and the specified
cost policies. Automatic knowledge capture is outside the first release;
automatic resource-usage reporting remains an open scope decision.

## Documents

The [project procedure index](../procedures/README.md) is the entry point for
SOPs, their approval status, and enforcement coverage.

| Document | Contents |
| --- | --- |
| [Requirements](requirements.md) | Product rules, terminology, and acceptance scenarios. |
| [Decisions](decisions.md) | Design choices and their rationale. |
| [Open questions](open-questions.md) | Unresolved choices and affected work. |
| [Backlog](backlog.md) | Proposed delivery order, dependencies, and requirement coverage. |
| [B-003 project and WorkItem contract](B-003-contract.md) | Accepted typed rules, pure-check evidence, acceptance scenarios, and implementation handoffs. |
| [Spike register and approved procedure](../spikes/README.md) | Bounded investigations, reproducible evidence, and reusable findings. |

**Confirmed** identifies requirements included in this planning baseline.
**Proposed** identifies design or delivery options still subject to review.
**Open** identifies unresolved choices. These labels describe planning status;
implemented behavior is recorded separately. Example names, dates, quantities,
and rates are illustrative, and the scenarios are specifications for future tests.

## Technical context

The [v0.2 design](../reference/YAJA-v0.2.md) remains the architecture reference.
It predates these requirements and does not fully define resource scheduling,
cost accounting, or the phase/status model. Conflicts between the product rules
and technical contracts need resolution before the affected work is implemented.

Current components include the Rust equality parser, NATS connection checks,
Go synchronization rules, an experimental Go API/Rust task worker, pure B-003
contract rules, TypeScript declarations, and Compose services. There is no
application UI or supported configurable-model API; development storage is
ephemeral. See [implementation status](../IMPLEMENTATION.md) for the exact
boundary.

## Updating the plan

Keep the existing requirement, decision, question, backlog, and scenario IDs
stable. When a product rule changes, update its rationale, acceptance scenarios,
and linked work. Record completed behavior in the implementation status page.
Resolve open questions when they affect an item; unrelated work can proceed.
Use the [testing guide](../TESTING.md) and
[contribution guide](../../CONTRIBUTING.md) for repository-wide requirements.
