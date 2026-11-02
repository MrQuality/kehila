# ADR-109 - Durable native event delivery

Status: Proposed, initial draft, 2026-11-02. Owner: project maintainer.
Acceptance: pending; outbox versus CDC and first-journey event scope remain open.
Related: ADR-101/102/103/104; #49, B-022/B-023, B-024/#60;
[integration questions](../implementation/authenticated-project-integration.md#native-adoption-questions-in-progress).

## Context

ADR-102 makes PostgreSQL acceptance independent of synchronous broker/search
responses. A process can commit creation and fail before publishing an event;
separate database commit and NATS publish cannot supply an atomic outcome.
The native integration has no delivered `ProjectCreated` publisher. That name
here describes the requested scenario, not an accepted event schema or API.

## Proposed decision and unresolved choice

If native event publication is required, authoritative commit must leave durable,
recoverable publication work. Recommend evaluating a transactional outbox first:
insert a stable versioned event/publication intent in the same transaction as
creation, then dispatch asynchronously. CDC from the committed native records is
the existing alternative; its event reconstruction, positions and recovery need
an equally explicit contract. Neither mechanism is selected by this draft.

Both candidates must preserve these obligations:

- A known rollback produces no committed creation event; a committed creation
  cannot lose its required handoff solely because the broker/process is unavailable.
- Never report a committed project as rolled back because publication failed.
  Authoritative reads and creation replay stay independent of projection health.
- Stable event identity survives publication retries. Uncertain acknowledgements
  permit duplicate delivery; consumer effects must be duplicate-safe and enforce
  applicable version/order rules. No cross-database/broker exactly-once claim.
- Recovery must resume durable pending work after dispatcher or broker restart.
  Specify confirmed handoff and retention/reclamation rules rather than deleting
  pending work merely because a send was attempted.
- Define bounded dispatch concurrency, retries/backoff, payload materialization,
  backlog/disk limits, lag visibility and admission when storage cannot safely
  retain more required work. No indefinite outage can imply unlimited capacity.

If JetStream is chosen, its publication acknowledgement can confirm stream storage;
it does not make the PostgreSQL transaction atomic with that acknowledgement.
Core fire-and-forget publishing alone supplies no durable recovery contract.
Broker acknowledgement/retention and consumer-effect commit semantics require
their own reviewed configuration and failure evidence.

For an outbox candidate, generated event envelopes must satisfy ADR-104 resource
admission before their persistence attempt, alongside the other generated records.
For CDC, define admitted event reconstruction and retained decoder compatibility.
Do not introduce an unbounded event serializer as an escape from codec limits.

**Open product decision:** does the first authenticated create/read journey require
an event or consumer? Do not silently make the publisher a prerequisite when no
consumer is needed, or defer it while claiming an event-dependent feature works.
Resolve this before final delivery sizing or enabling dependent consumers.

## Alternatives

- Commit then publish once: loses required publication when the process/broker
  fails in the gap.
- Publish before commit: can expose an event for a transaction that rolls back.
- Hold SQL locks while waiting on NATS: violates accepted broker-independent
  authoritative acceptance and introduces an external wait into the transaction.
- Transactional outbox: explicit atomic handoff; adds storage, dispatcher and
  retention ownership that must be reviewed and qualified.
- Native CDC: reuses committed-state capture direction; must prove consistent
  event reconstruction, replay positions, schema compatibility and outage recovery.

## Consequences and impacts

Consumers receive no authority merely from event content; define trusted producer,
scope, sensitive-data minimization and current authorization for exposed reads.
Retained events/backlogs add storage and restore obligations. Reconcile restored
producer/consumer positions and identities without regression or accidental repeated
external effects; event delivery does not itself solve disaster recovery.
Freeze canonical versioned values separately from localized display. Future agent
consumers need explicit credentials/delegation, never owner privilege by transport.

Changing an accepted event schema or capture mechanism needs a compatibility and
recovery plan; disabling a dispatcher does not undo already observed consumer effects.

## Verification and follow-up

Before enabling native events, test process loss after commit/before publish,
broker outage, lost publication acknowledgement, duplicate/redelivered messages,
consumer failure around its effect commit, producer/consumer restart, ordering,
schema-version mismatch, backlog admission and supported restore. Observe actual
database/broker/consumer state. These are planned cases, not qualified behavior.
No new broker dependency, publisher, consumer or execution is introduced here.
