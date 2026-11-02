# ADR-109 - Durable native event delivery

Status: Proposed, initial draft, 2026-11-02. Owner: project maintainer.
Acceptance: pending; outbox versus CDC remains open. The first accepted integration journey excludes lifecycle-event consumers.
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

**Original scope question, now resolved for the first journey:** does it require
an event or consumer? Do not silently make the publisher a prerequisite when no
consumer is needed, or defer it while claiming an event-dependent feature works.
The subsequent explicit 2026-11-02 integration decision excludes lifecycle-event consumers from the first journey. Delivery selection remains required before enabling later dependent consumers.

## Additional lifecycle-event questions under review

These are proposed design obligations, not a selected event envelope, dispatcher,
consumer topology, broker configuration or implementation commitment.

**Crash consistency:** for the outbox candidate, project acceptance and publication
intent commit together or both roll back. A crash after commit leaves recoverable
work; a crash after broker storage but before marking handoff confirmed can cause
duplicate publication. Use stable event identity and duplicate-safe consumer effects;
never claim one atomic commit across PostgreSQL, broker and arbitrary sinks. A CDC
candidate must prove equivalent committed-source recovery through durable positions.

**Historical rebuilds:** event schema version, embedded snapshot codec version,
domain rules and database schema version are distinct. Retain explicit decoders
and golden fixtures for every supported historical version; dispatch by stored
tags without live domain validation. Unknown versions and malformed records fail
explicitly rather than acquiring today's defaults. Define the authoritative rebuild
source, retention and replay positions: an expiring broker stream is not automatically
a complete history archive. Preserve ADR-104 legacy isolation and permanent operation
identity independently of broker retention. Event transformations need reviewed
deterministic semantics; no unapproved upcaster or codec is selected here.

**Poison messages:** bound bytes, decoding and attempts; classify invalid/unsupported
input separately from transient dependency failure. Decode errors are typed failures,
not panics. Define a durable, access-controlled quarantine/disposition record and
replay path before acknowledging or terminating unprocessable delivery. If quarantine
cannot persist, retain bounded unresolved work and signal degradation rather than
silently lose it. Retry limits alone do not implement a dead-letter workflow. In
JetStream, max-delivery handling needs explicit application ownership; it supplies
no built-in dead-letter queue. Isolate affected causal scopes so unrelated projects
can progress, while dependents of the missing event remain blocked/recovering.
Do not both skip an essential delta and claim that project's projection is complete.

**Causal and monotonic application:** define ordering scope before promising strict
order. Recommend committed per-project ordering metadata, stable event identity and
consumer progress that advances atomically with its effect where the sink permits.
Duplicate events have no repeated effect; stale events cannot regress state. A gap
needs bounded buffering or durable recovery, not unbounded memory. Complete snapshots
and deltas need distinct gap rules; do not skip required predecessors of a delta.
External sinks need their own atomic conditional/versioned apply and recovery proof.
Broker arrival time, wall-clock milliseconds and stream sequence alone do not define
business causality across publishers. Cross-project dependencies need an explicit
dependency/coordination contract under Q-019; no global total order is selected.

**Backpressure:** for an outbox candidate, reserve/check durable backlog capacity
atomically with creation admission so concurrent requests cannot all pass a stale
capacity check. Define byte/count/age budgets and storage safety headroom, bounded
dispatcher queues and retry/backoff. Reject before commit when required handoff
cannot safely fit; never drop an already committed required event to make space.
Release SQL locks before broker waits. CDC alternatives need WAL/retention and
connector-outage controls too; stopping a consumer does not stop WAL growth.
The numeric limits, overload response and recovery thresholds remain open.

**Publisher authority:** authenticate distinct service principals and allow only
required lifecycle publish subjects to the trusted publisher/capture boundary.
Consumers, gateways and browser callers must not inherit lifecycle-publish rights.
Grant only required subscribe, acknowledgement/reply and broker API subjects;
broad stream-management rights can defeat apparently narrow data-subject rules.
Verify effective denies for unauthorized publish, wildcard subjects and management
operations, including reconnect/rotation. Subject names and payload actor fields
are not credentials. A compromised authorized publisher remains inside the trusted
producer boundary; ACLs alone cannot prove an event reflects a committed SQL action.
Review committed-source provenance and consumer validation as separate obligations.

References: [NATS acknowledgement/redelivery](https://docs.nats.io/learn/jetstream/acknowledgment),
[NATS subject authorization](https://docs.nats.io/learn/security/authorization).
Before consumer implementation, qualify old-version rebuild, malformed/unsupported
quarantine and failure, redelivery, reorder/gap recovery, scaled concurrent effects,
backlog exhaustion and actual-principal publish/subscribe/API denial on the selected
graph/profile. These are targeted gates when applicable, not authority to execute.

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
