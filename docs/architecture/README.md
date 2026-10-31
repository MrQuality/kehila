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
| [ADR-104: Project-creation storage codecs](ADR-104-project-create-codecs.md) | Accepted by explicit maintainer approval on 2026-10-13; pure codec plus mandatory ingress, actor/scope, JSONB snapshot and legacy migration contracts. Downstream adapters/evidence remain required under [#49](https://github.com/MrQuality/kehila/issues/49), successor to closed #26; #47 supplied the merged pure codec scope. |
| [ADR-105: Local-owner authentication](ADR-105-local-owner-authentication.md) | Accepted by explicit maintainer approval on 2026-10-19 under #40/#49 W2: Rust authority boundary, separate owner password, bounded browser/CLI sessions and actor-preserving recovery. Single active runtime and volatile sessions are local-MVP-only; [#65](https://github.com/MrQuality/kehila/issues/65) owns replicated deployment/migration. A0 investigation is authorized; subsequent implementation and unexecuted verification retain separate gates. ADR-103 remains Proposed. |
| [ADR-106: Local container transport and disposable server-key delivery](ADR-106-local-container-transport.md) | Accepted 2026-10-23 for a bounded disposable container proof: specific listener behind Windows loopback, Rust TLS and disposable file-secret residual risk. The 2026-10-24 amendment invalidates interrupted diagnostic runs; the explicitly accepted 2026-10-27 extension permits synthetic disposable authentication tests with fixture discard/fresh enrollment after interruption. The accepted 2026-10-30 amendment permits untrusted same-PC guest callers to reach the application only for those tests, retaining external-ingress exclusion and independent authentication/key/database controls. No live-test/trust permission, supported installation/suspend, production adoption or runtime qualification follows from acceptance. |

Future cross-record coordination, compaction/rebuild, CDC schema, event envelope
and storage upgrade decisions require further evidence and ADRs. In particular,
[Q-019](../product/open-questions.md#q-019) remains unresolved; ADR-102 records
the transaction direction but does not settle individual
command isolation/locking, large-job visibility, or resource/cost protocols.

## Post-MVP design considerations

[D-037](../product/decisions.md#d-037) schedules broader disaster recovery,
multilingual support and AI-agent task execution after MVP. Current MVP and
future designs must account for these needs. For new or materially revised
product/technical designs, record the relevant impact, assumptions and future
compatibility/migration work, or a reason a topic does not apply:

- [Recovery, B-024/#60](../product/backlog.md#b-024): authoritative versus
  rebuildable state, retained identities/replay/tombstones, recovery dependencies,
  stale-client reconciliation and interrupted migrations.
- [Languages, B-030/#61](../product/backlog.md#b-030): stable IDs/canonical values
  versus display text/locale formatting, user content, layout direction and
  locale-sensitive search/sort assumptions.
- [Agent execution, B-031/#62](../product/backlog.md#b-031): human-only identity
  assumptions, future machine credentials/delegation, authority and attribution,
  assignment/execution, approvals and uncertain outcomes.

Use the ADR template's consequences section or the equivalent review section in
a product design. Record unresolved choices and owning backlog/issue links;
do not implement speculative infrastructure to satisfy the checklist. Existing
durability/real-data gates and the one-owner MVP scope remain applicable. This
review obligation accepts no detailed technical mechanism; material choices
still require their own Proposed ADR and actual maintainer acceptance.
