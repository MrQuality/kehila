# YAJA Coding Standard

Version 1. Owner: the [project maintainer](../../CONTRIBUTING.md#repository-roles).
Applies to new and materially changed code. This is the implementation/review
baseline under CODE-001 in the [Engineering Standard](ENGINEERING-STANDARD.md).
It does not claim that all existing code conforms or that planned checks run today.

MUST and SHALL are mandatory; SHOULD requires a recorded reason for departure;
MAY is optional. Existing requirement, decision, test and evidence records remain
the authority for behavior. This standard does not change accepted domain rules.

## Adoption and change scope

A material change alters behavior, an interface, validation, persistence,
concurrency, resource ownership, cancellation, security or failure handling.
New code and the materially changed operation/module MUST satisfy applicable rules
below. Review the affected dependency boundary and callers even when only a few
lines changed. Formatting, comment-only edits and mechanical renames do not by
themselves require redesign of the containing file.

The author MUST identify the affected scope, applicable rules, evidence and
remaining gaps in the PR. The maintainer MUST assess that scope before merging,
using the existing [Definition of Done](DEFINITION-OF-DONE.md). An explicit
not-applicable entry needs a reason. The sole-contributor review policy remains
unchanged; this process does not require self-approval as an independent review.

Existing untouched code MAY remain while gaps have an implementation destination.
Do not use that allowance to ignore a discovered security or data-integrity defect:
triage through [SECURITY.md](../../SECURITY.md) or the relevant integrity backlog.
If a material change cannot meet a rule, record a narrow exception with scope,
owner, reason, mitigation, approving maintainer, follow-up and expiry/review date.
An exception does not satisfy a mandatory release requirement. Do not attach an
unrelated repository-wide rewrite to a bounded change merely to improve style.

## Design and implementation rules

| Area | Required rule | Review evidence |
| --- | --- | --- |
| Pure boundaries | Domain code in `src/pure/` and `go/pure/` MUST NOT perform network, filesystem, database, environment, clock, random-source or other external I/O access. Supply time, identity, configuration, randomness and complete state observations explicitly where needed. I/O/runtime dependencies belong in adapters. | Dependency/API review and deterministic pure tests; purity is not established by a directory name alone. |
| Domain ownership | Business rules MUST have an identified authoritative implementation. Adapters MAY validate transport shape and bounds, but MUST NOT independently reinterpret authorization, replay, lifecycle or accounting rules. | Shared domain decisions and cross-language contract tests; identify legitimate adapter-only validation. |
| Types and invariants | Use meaningful types where confusing identities, versions, units or states could violate an invariant. Commands/outcomes SHOULD be explicit variants. Validate external data before trusting it; distinguish missing, null, empty, zero and invalid according to the accepted contract. | Type/API review and negative/boundary tests. A wrapper type alone does not validate its value. |
| Errors | Expected invalid input, conflicts, overload and dependency failure MUST produce explicit outcomes. Preserve domain codes through adapters. Ignored errors MUST have a reason; distinguish infallible serialization from I/O that can fail. Internal details and secrets MUST NOT become client errors. | Error mappings, negative tests and documented deliberate discards. Do not log and return the same failure redundantly at every layer. |
| Ownership and concurrency | Every task, goroutine, queue, subscription and shared mutable resource MUST have an owner, lifetime, capacity policy and failure/cleanup behavior. Detached work needs explicit supervision or a justified completion owner. | Lifetime/capacity design, contention, termination and failure tests. |
| Cancellation and unknown outcomes | Distinguish a cancelled caller from cancelled underlying work. A timeout MUST NOT imply that a mutation did not commit. Capacity MUST account for admitted work that outlives its caller; reconciliation MUST preserve operation identity. | Driver/protocol contract, uncertain-outcome and saturation/recovery tests. |
| Bounds | Externally controlled input and admitted work MUST have explicit bounds and over-limit behavior. Bound encoded bytes as well as logical values where relevant. A bounded read MUST detect overflow rather than silently treating truncation as a complete successful payload. | Maximum/over-limit tests; name units explicitly, such as bytes, entries or milliseconds. |
| Representations and evolution | Domain, transport and storage representations SHOULD be separate when their validation/evolution rules differ. Wire/storage changes MUST assess readers, writers, event schemas and all affected languages. | Contract/compatibility evidence and migration impact; avoid exposing database records accidentally as public DTOs. |
| Persistence and replay | Preserve accepted version, replay and authority invariants. Cross-record operations MUST NOT imply atomic acceptance without an evidenced protocol. Authority MUST remain independent of search; clients/projections MUST NOT regress aggregate versions. | Relevant D-* and ADR links, actual-storage evidence and explicit Q-019 boundaries. |
| Maintainability | Modules SHOULD organize around domain concepts and ownership. Abstractions SHOULD address an existing invariant, dependency boundary or reuse need. Keep public APIs minimal and control flow understandable; do not add indirection solely to satisfy a pattern. | Maintainer review of responsibilities and callers; no universal function/file length quota. |
| Documentation | Important public APIs MUST explain inputs, outcomes, invariants and relevant failure/replay/cancellation behavior. Comments SHOULD explain constraints and reasoning. Material technical changes MUST follow ADR governance. | API docs, decision links and updated evidence boundaries. |
| Tests and diagnostics | Tests MUST verify observable behavior/invariants with explicit oracles. Real-service claims need real-service evidence. Async tests MUST use bounded observable conditions or deterministic barriers. Failure paths SHOULD preserve bounded sanitized diagnostics before cleanup; retain the failed result rather than retry until a pass. | Existing shared verification, relevant qualification campaigns and diagnostic retention. |

These rules refine existing engineering requirements; they do not introduce a
second event, error, security or consistency contract. Use
[QUALIFICATION.md](QUALIFICATION.md) for the corresponding invariant and failure
oracles and [DEPENDENCIES.md](DEPENDENCIES.md) for dependency/security exceptions.

## Language conventions

### Rust

- MUST inherit the workspace unsafe-code prohibition and retain rustfmt/Clippy
  checks. A new crate MUST join the appropriate workspace verification path.
- MUST use explicit outcomes for expected user/dependency failures. Production
  `unwrap`, `expect`, indexing or panic paths need a defensible invariant; document
  the reasoning when it is not evident from types/control flow. Test assertions
  MAY use `unwrap`/`expect` without manufacturing production error handling.
- SHOULD use enums/newtypes and narrow constructors for meaningful domain states,
  with fallible validation at trust boundaries. Broad string/JSON bags are not a
  substitute for a domain contract.
- SHOULD borrow where it makes ownership clear. Copies, clones, allocation and
  synchronization in important paths need a purpose; measure consequential costs.
  Avoid imposing allocation tricks without a measured need.
- MUST keep async/runtime/database responsibilities in adapters. Do not hold a
  lock across an await unless its lifetime, contention and cancellation behavior
  are explicitly justified. Driver cancellation behavior must be investigated.
- MUST assess the affected crate's declared minimum Rust version when introducing
  APIs/dependencies. Change the declaration deliberately when required. A
  supported minimum-version claim needs actual compiler/dependency qualification;
  current stable CI alone does not establish that claim.

Use the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) as design
references for predictable, typed, documented APIs; apply recommendations to the
actual crate rather than adding every suggested trait mechanically.

### Go

- MUST use `gofmt` and pass the shared `go vet` checks. Exported/package names and
  documentation SHOULD follow Go conventions.
- MUST pass context through I/O and define deadline/cancellation ownership.
  Goroutines require termination and cleanup paths; background admission is bounded.
- SHOULD use explicit typed request/response structures for known contracts and
  small consumer-facing interfaces at real dependency boundaries.
- MUST handle returned errors or justify discarding them. After a response starts,
  a write error may be observable/recordable without being replaceable by a new
  HTTP status; do not pretend a second error response repairs a partial write.
- SHOULD use structured control flow and explicit errors rather than panics for
  expected service failures. Keep the authoritative domain rule in its owning
  implementation while protecting transport boundaries.

[Effective Go](https://go.dev/doc/effective_go) is a core-language idiom reference;
it is not a complete guide to the current module, generic or library ecosystem.

### TypeScript contracts and future runtime code

- Declarations MUST match the actual wire contract; changing a declaration alone
  does not change or validate the server. Specify numeric range/precision and
  absent/null semantics across languages when they matter.
- Known protocol outcomes SHOULD use discriminated unions. Unvalidated external
  values SHOULD start as `unknown`; deliberate `any` usage requires a reason.
- Runtime boundaries MUST validate received values before treating them as trusted
  domain data. Compile-time types do not validate JSON at runtime.
- A future compiler/test setup MUST enable [strict checking](https://www.typescriptlang.org/tsconfig/strict.html)
  and exercise consumer fixtures for shared declarations. Assess additional compiler
  flags individually. No runtime/UI scaffolding is required for today's declarations.

### Python verification and integration tooling

- SHOULD follow [PEP 8](https://peps.python.org/pep-0008/) and use type annotations
  for significant tooling interfaces/structured data. Avoid blanket annotations
  or rewrites that add little clarity to small existing scripts.
- New/materially changed command-line tools MUST put operational execution behind
  a `main()` entry point and guard; importing their helpers must not run services,
  mutate files, read required CLI-only environment or start subprocesses.
- MUST invoke subprocesses using argument lists with intentional working directory,
  environment, timeout and exit handling. Do not build shell commands from external
  input. Shell-specific orchestration needs explicit quoting and trust boundaries.
- MUST clean up only verified, owned temporary/service resources, including partial
  startup failures. Preserve bounded sanitized failure diagnostics before deleting
  fixtures; do not keep secrets or unlimited logs as evidence.
- MUST fail explicitly on missing tools, unavailable required services, skipped
  required checks and test failures. An advisory/network assessment outage is not
  a clean result. Use standard library tooling unless a dependency has a justified
  maintenance/testing benefit under the dependency policy.

## Current enforcement and existing gaps

Inspection baseline: 2026-10-02. This is a scoped inventory, not a complete code
audit or certification. The listed destinations extend existing backlog work.

| Area | Current evidence / gap | Destination |
| --- | --- | --- |
| Pure design and typed contracts | Pure task/query crates and Go synchronization rules have tests; boundary/type design is manually reviewed. No automated architecture-boundary guard exists. | [B-019](../product/backlog.md#b-019), [B-023](../product/backlog.md#b-023) |
| Rust formatting/safety/lint | Workspace unsafe prohibition, rustfmt and Clippy are configured. Minimum compiler versions differ (workspace 1.75, worker 1.88) and are not qualified by stable-only CI. | [B-019](../product/backlog.md#b-019), [B-028](../product/backlog.md#b-028) |
| Go error/response handling | Formatting/vet checks exist. `writeJSON` and response forwarding discard write/encoding errors without documented disposition; `forward` caps upstream response bytes without explicit overflow detection. Assess these when changing that boundary; this inventory does not claim a demonstrated integrity failure for today's bounded fixtures. | [B-004](../product/backlog.md#b-004), [B-022](../product/backlog.md#b-022) |
| Cancellation and capacity | The worker documents driver completion ownership after caller timeout and retains its permit until completion; tests cover bounded ownership and stalls. This is scoped evidence, not full application cancellation qualification. | [B-023](../product/backlog.md#b-023), [B-025](../product/backlog.md#b-025) |
| Python tooling and diagnostics | Python unit tests/shared execution exist; no dedicated formatter, linter, typing or import-safety audit is claimed. Task-path cleanup can remove startup process logs after failure, as recorded in ASSESSMENT.md. | [B-019](../product/backlog.md#b-019), [B-023](../product/backlog.md#b-023) |
| TypeScript | Shared declarations exist; no compiler/consumer fixture or runtime validation baseline exists. Add contract checking before claiming typed cross-language conformance. | [B-022](../product/backlog.md#b-022) |
| Review evidence | Contribution policy and Definition of Done require final maintainer assessment. The PR template prompts coding scope, applicable rules and gaps; a prompt is not proof that assessment occurred. | [B-019](../product/backlog.md#b-019) |

Sources for the inventory are [workspace configuration](../../Cargo.toml),
[worker configuration](../../src/io/task_worker/Cargo.toml),
[worker execution ownership](../../src/io/task_worker/src/lib.rs),
[Go adapter](../../go/io/task_api/server.go),
[task-path harness](../../tests/integration/task_path.py),
[contracts package](../../packages/contracts/package.json),
[CI](../../.github/workflows/ci.yml) and [assessment](ASSESSMENT.md).

## PR assessment record

Use the existing PR description/maintainer assessment, not another approval system.
Record the changed scope, applicable rules and decisions, tests/checks and their
limits, and any existing gap/exception with its backlog destination. Pure logic,
adapter, contract and tooling changes have different applicability; an explicitly
justified not-applicable entry is preferable to an empty checklist.

Automation currently enforces the stated formatting/lint and engineering-register
structure. Semantic coding rules require review and behavioral evidence. B-019
owns incremental qualification of future checks; no new language tooling,
repository-wide conformance sweep or arbitrary size/coverage quota is introduced
by adopting this scope.
