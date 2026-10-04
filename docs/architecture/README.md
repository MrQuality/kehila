# Architecture decision records

Technical architecture choices use ADRs here. Product scope and accepted behavior
remain in [D-* decisions](../product/decisions.md). ADRs link those records rather
than migrating or overriding them. The reference design is a proposal except
where a later accepted decision explicitly adopts it. ADR-101 consolidates the
current accepted task path without approving the rest of that reference.

A material change to persistence, consistency, boundaries, protocols, deployment
or failure behavior MUST create or supersede an ADR using [the template](TEMPLATE.md).
Use stable sequential ADR IDs. Status is Proposed, Accepted, Superseded or Retired.
An Accepted record MUST identify the actual accepting decision or maintainer
assessment; writing it does not manufacture approval. Record superseding links
and retain history. Code evidence cannot silently resolve a product question.

| Record | Status and boundary |
| --- | --- |
| [ADR-101: Task authority, replay and projection](ADR-101-task-authority.md) | Accepted behavior restated from D-016/D-017; implementation evidence remains bounded. |

Future cross-record coordination, compaction/rebuild, CDC schema, event envelope
and storage upgrade decisions require further evidence and ADRs. In particular,
[Q-019](../product/open-questions.md#q-019) remains unresolved; this register does
not choose a transaction, saga or locking protocol for it.
