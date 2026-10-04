# Initial Kehila threat model

Baseline: 2026-10-02. Status: initial model; no production security qualification.
Owner: project maintainer. Review on new authentication, storage, event, deployment
or UI boundaries. [B-020](../product/backlog.md#b-020) owns security qualification;
[B-007](../product/backlog.md#b-007) owns integrated access control.

Assets include authoritative task/configuration/history data, operation/replay
records and tombstones, future grants/sessions, secrets, backups, CDC checkpoints,
event stream and search projection. Adversaries include hostile browser origins,
untrusted local programs/users, unauthorized authenticated users, network peers,
malformed input producers and compromised dependencies/services. Workstation
trust is a development assumption, not an authentication control.

## Trust boundaries and current evidence

| Boundary | Current implementation | Planned control / verification destination |
| --- | --- | --- |
| Browser/client → Go API | No UI/authentication; mutation loopback Host/origin checks and 4096-byte bound | Owner session, authenticated CLI, CSRF/origin/read protection and project/object grants; B-007/B-020/B-022 |
| Go API → Rust worker | Fresh upstream requests; worker loopback binding, Host/browser-context rejection and bounded execution | Service identity/least privilege and deployment boundary contract; B-020/B-027 |
| Worker → FerretDB/PostgreSQL | Immutable successful-operation records, version/operation keys, bounded driver/pool work | Provisioning/serving privilege separation, secure credentials/networking, storage integrity/recovery; B-005/B-024 |
| PostgreSQL → CDC → NATS | SP-001 Python-adapter/CDC evidence only; no production pipeline | Authenticated capture/publish, minimal publication scope, checkpoints/WAL budgets, event schemas; B-022/B-023/B-027 |
| NATS → indexer → OpenSearch | Experimental indexer only; development search authentication disabled | Producer/consumer permissions, schema/size/version validation, quarantine and rebuild; B-020/B-022/B-023 |
| Search → API → client | Go minimum-version check; no real client merge behavior | Authorized search scope and monotonic authoritative client state; B-007/B-022/B-023 |
| Source/dependencies → CI/release | Read-only CI permissions, no persisted checkout credentials, locked Rust dependencies | Scanners, immutable inputs, reviewed build identity, signing and SBOM; B-021/B-028 |
| Backup/export → restore | No supported mechanism | Encrypted/restricted copies, off-domain storage, replay/tombstone preservation and restore authorization; B-024 |

Development Compose exposes host ports with default credentials and disabled
OpenSearch authentication. Follow SECURITY.md trusted/firewalled/disposable scope.
Neither the Go origin checks nor pure grant booleans protect supported real data.

## Abuse cases, controls and residual risks

ASVS references below use 5.0.0 chapter areas (not claims that every control in an
area is met). B-020 must build exact per-control L1/L2 applicability/evidence maps.

| Threat | Required mitigation / negative evidence | Current residual risk / work | ASVS area |
| --- | --- | --- | --- |
| Authentication bypass, session theft | Protect all routes; onboarding/expiry/recovery, secure session transport/storage, revoked/expired credentials tests | No integrated identity; B-007/B-020 | V6 Authentication, V7 Session Management, V9 Self-contained Tokens where used |
| Authorization bypass, IDOR, cross-project access | Authoritative object/project grants for reads/writes/search; both endpoint grants; denied and guessed-ID tests | Pure checks do not enforce transport/storage access; B-007 | V8 Authorization |
| Replay and operation-ID abuse | Scope identity, exact intent, current original grants before replay, bounded tokens, expiry/tombstones, restore safety | Legacy worker differs from future scoped model; B-005/B-007/B-022 | V2 Validation and Business Logic, V8 |
| Event spoofing/poisoning | Trusted publishers/subjects, validated bounded schema, aggregate versions, quarantine with replay/repair policy | Production event security absent; B-020/B-022/B-023 | V2, V4 API and Web Service, V13 Configuration |
| Delayed/reordered/missing events | Nondecreasing projection/client version, gap detection and authoritative rebuild; future schemas fail safely | Prototype evidence only; B-022/B-023 | V2, V4 |
| Query abuse/DoS/resource exhaustion | Query complexity/length, count/body/event limits, bounded workers/pools/backlogs, timeouts and admission policy tests | Some worker/model bounds; full query/queue limits absent; B-022/B-025 | V1 Encoding and Sanitization, V2, V4 |
| Malformed payload/injection | Typed validation, parameterized queries, output encoding; Unicode/escaping/size fuzz corpus | Simple parser not a full compiler/UI; B-023 | V1, V2 |
| Compromised dependency/build | Inventory, advisory/secrets/SAST checks, restricted CI, pinned release inputs and verified provenance | No comprehensive scanners/releases; B-021/B-028 | V13 Configuration, V15 Secure Coding and Architecture |
| Compromised worker/service | Separate serving/setup principals, minimal DB/broker/search privileges, protected secret delivery/rotation | Development privilege model unqualified; B-020/B-027 | V13, V15 |
| Search/index exposure | Restricted network/authentication; permission-aware projections/queries, no sensitive fields without justification | Dev search unauthenticated; hidden fields are not authorization; B-007/B-020/B-022 | V8, V14 Data Protection |
| Logs/backups leak secrets or data | Redaction, bounded telemetry cardinality, access/retention rules, protected off-domain backup | Not implemented; B-024/B-026 | V14, V16 Security Logging and Error Handling |

Severity is not assigned mechanically here: exposure and impact must be assessed
for the actual supported deployment. These gaps forbid interpreting this model
as ASVS evidence or production readiness. Tests must cover denied behavior as well
as valid paths, and compromised-service recovery must not assume the index is
authoritative. Security findings follow SECURITY.md; avoid publishing exploit or
credential details in PRs or test logs.
