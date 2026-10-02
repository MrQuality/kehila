# Engineering baseline assessment

Date: 2026-10-02. Scope: engineering framework change on
`engineering/maturity-standard`, branched from local main. This is source/tooling
evidence, not maintainer release approval. No supported artifact is produced.

## Maturity disposition

M0 Experimental remains the defensible level. Documentation/metadata checks do
not implement a usable authenticated product, durable supported installation,
clean restore, production event path or qualified release. ARC-001 and DOC-001
are satisfied within their limited record/documentation scope. Other obligations
remain partial or planned as specified in the register. In particular, GOV-001,
GOV-002 and TEST-001 still need final PR/qualification evidence; passing local
checks does not manufacture maintainer approval or hosted CI results.

## Remote governance inspection

Read-only inspection of the GitHub `main` branch protection on this date observed:

- Required `verify` status check with strict/up-to-date evaluation.
- Pull-request review configuration with zero required approving reviews,
  no required code-owner or last-push approval, and stale-review dismissal.
- Administrator enforcement enabled; force-push and deletion disabled.
- Conversation resolution required.

These settings align with current sole-contributor contribution policy. The
inspection does not change settings, establish a future release-branch policy,
or substitute for a final maintainer PR assessment. Reinspect before qualification;
remote settings can change independently of Git history.

## Verification evidence and limits

The framework regression tests verify unique IDs, normative metadata, valid
evidence/backlog/source references, stale generated-document rejection,
cumulative gap rejection and shared runner integration. Go static tests verify
format failures and vet exit propagation. Rust formatting/Clippy and pure
verification passed. Documentation review checked local links and stable anchors.

The first full attempt stopped with unavailable services. After starting the
documented disposable stack, a full run passed unit/static checks, real NATS,
Rust/Go tests, 48 two-worker contention pairs, worker boundary and database
stall/recovery tests, then failed because an API process exited before readiness.
That run's temporary process logs were removed by existing cleanup. A diagnostic
run of the unchanged task-path test subsequently passed all cases, including
Go-to-Rust save/replay/read with search unavailable. The earlier startup exit
was not reproduced or explained; it remains a test-evidence limitation, not a
proven product defect or a reason to weaken assertions. Do not report the failed
run as successful or interpret diagnostic success as a fix.

Working-tree full verification passed: 28 Python tests, the Rust workspace
including real NATS handshake, Go tests/static checks, and all task-path contention,
boundary, database recovery and search-outage cases. This successful run coexists
with the unexplained earlier startup failure; it does not erase it. Final staged
snapshot verification also passed, with 29 Python tests including the added
ambiguous-anchor regression, and all Rust/Go/real-service task-path checks.
Hosted CI was pending at this baseline assessment; inspect the final PR/check
result separately. SP-001 C01–C06
were not rerun: their adapters, mapping and event experiment were unchanged.
No ASVS/OSPS control qualification, SLSA level, security scan, production recovery,
performance envelope or accessibility result is claimed.
