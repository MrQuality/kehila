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
| [ADR-102: Native PostgreSQL and bounded transactions](ADR-102-postgresql-transactions.md) | Accepted direction for new M1 storage; [local schema foundation](../../storage/postgresql/README.md) implemented, application adapter pending. |
| [ADR-103: Native creation storage foundation](ADR-103-native-creation-foundation.md) | Proposed for production adoption; implemented and tested locally for review. |

Future cross-record coordination, compaction/rebuild, CDC schema, event envelope
and storage upgrade decisions require further evidence and ADRs. In particular,
[Q-019](../product/open-questions.md#q-019) remains unresolved; ADR-102 records
the transaction direction but does not settle individual
command isolation/locking, large-job visibility, or resource/cost protocols.
