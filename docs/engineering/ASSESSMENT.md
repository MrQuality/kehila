# Engineering baseline assessment

Date: 2026-10-02. Scope: engineering framework change on
`engineering/maturity-standard`, branched from local main. This is source/tooling
evidence, not maintainer release approval. No supported artifact is produced.

## Maturity disposition

E0 Experimental remains the defensible level. Documentation/metadata checks do
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

The maintainer subsequently reported that local endpoint security can suspend
test processes for inspection and proposed this as the cause of the delay/failure.
Host-security interference is a plausible hypothesis; no captured process or
security-tool logs confirm it for this run. Preserve the failed result and
existing deadlines. If it recurs, capture process exit diagnostics and correlate
them with host-security events before attributing the failure or changing bounds.

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

## Subsequent verification and current PR coding scope

The pre-review coding-standard revision
[`d7a15087586b7f6726a107a6afb28f90590ee168`](https://github.com/MrQuality/kehila/commit/d7a15087586b7f6726a107a6afb28f90590ee168)
passed [hosted full verification](https://github.com/MrQuality/kehila/actions/runs/37006315943).
This resolves the historical pending-CI observation for that source only. Review
updates need their own final-source verification record in
[PR #35](https://github.com/MrQuality/kehila/pull/35), following the
[evidence-record protocol](QUALIFICATION.md#qualification-evidence-records).
No maintainer approval or engineering gate advancement is recorded here.

The executable scope is engineering-register validation/rendering, Go module
scope and formatting/vet/test orchestration, staged-check selection and their
Python regressions. Runtime task/API/storage behavior is unchanged.

| Coding rule / impact | Applicability and disposition |
| --- | --- |
| Inputs, types and explicit errors | Register schema, obligations, gate IDs, references and evidence/backlog destinations fail closed. Workspace/source inventory mismatches fail before Go checks. Regression fixtures use real temporary files for filesystem behavior and test doubles for orchestration outcomes. |
| Boundaries and resource ownership | Tools operate on reviewed repository metadata, fixed output paths and an explicit module inventory. This is not a network payload/parser service. Each subprocess has a fixed argument list, working directory, checked outcome and timeout. Existing temporary test-directory cleanup remains owned by the Go runner. |
| Import safety and side effects | Operational execution uses guarded main entry points; imports define helpers/constants. Only the explicit regeneration command writes the fixed generated document; its authored preamble is preserved and invalid input prevents writing. Go's JSON workspace view does not rewrite go.work. |
| Failure and diagnostics | Required failures propagate; formatting failure prevents vet and inventory failure prevents checking/testing a reduced source scope. The existing task-path failure-log retention gap remains B-023 work; it is not introduced or corrected by this PR. |
| Tests and compatibility | Tests cover invalid metadata, obligation/gate semantics, source/reference scope, repeatable regeneration and failure propagation. The unpublished validator/register now uses E* gates and explicit obligation metadata; product M* IDs and application wire/storage contracts retain their meaning. |
| Domain, mutation and persistence rules | Not applicable to these tooling changes: no task mutation, driver cancellation, event projection or authoritative persistence logic changes. Existing domain and real-service tests remain in the full suite. |
| Performance, telemetry, migration and release artifacts | No supported runtime path, storage migration or artifact publication changes. Bounded tool subprocess deadlines apply; no application performance, observability, migration or supply-chain level is claimed. |
| Style and remaining qualification | New inventory/regeneration helpers use standard-library dependencies and explicit failures. No repository-wide Python formatting/typing conformance is claimed; some existing validator/test lines remain longer than PEP 8's recommendation. Broad Python static, TypeScript consumer and minimum-Rust-version qualification remain B-019/B-022/B-028 work. |

This is the implementation author's scoped technical assessment for review, not
the maintainer's merge assessment. Final results must identify the actual tested
source; no unbounded retry or test assertion weakening is authorized by this record.
