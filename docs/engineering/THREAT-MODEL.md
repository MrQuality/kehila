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

## Current native boundary references

The active owner interface uses the authenticated Rust authority and PostgreSQL;
the Go API/worker/FerretDB rows below describe the October prototype. Current
contracts and bounded evidence are recorded in
[native WorkItems](../implementation/native-work-items.md),
[browser edit recovery](../implementation/browser-edit-recovery.md) and
[current-work selection](../implementation/current-work-selection.md).
The selection-specific threat update below describes its additional surfaces.
These records do not establish supported real-data deployment or qualify the
future event, search, backup and remote-deployment paths.

## Historical prototype trust boundaries — 2026-10-02

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

## Accepted local-MVP authentication design; controls unqualified

[ADR-105](../architecture/ADR-105-local-owner-authentication.md) selects a separate
owner password, one Rust authority for future native routes, HTTPS and bounded
opaque browser/CLI sessions. It removes identity forwarding from that proposed
path; the current Go/Rust prototype and gaps above remain unchanged. Architecture
acceptance is recorded in ADR-105; the
[verification gates](../implementation/local-owner-authentication-verification.md)
are pending, including certificate installation, restricted enrollment/recovery,
password resource limits, CSRF, queued revocation, singleton takeover and restore
re-enrollment. No mitigation in the table becomes implemented by this proposal.
The [review follow-up](../implementation/local-owner-authentication-verification.md#pr-64-review-follow-up)
adds installation takeover fencing and bounded admission/retry contracts. Separate
rate buckets do not prevent shared-worker/transport denial by local callers;
expired-slot reclamation does not stop repeated allocation traffic. Failed logout
audits can be lost after abandonment/expiry/crash while revocation remains final.
These declared residual risks were included in local-MVP acceptance; controls remain unqualified.
The maintainer explicitly accepted [ADR-106](../architecture/ADR-106-local-container-transport.md)
on 2026-10-23 for the disposable container proof only: a specific listener behind
Windows loopback forwarding and read-protected file secrets. Trusted engine/VM
administration and retained disk/backup copies can expose keys. Actual ingress,
file/alias and resume controls remain unqualified; no production storage or
execution permission follows.
The [2026-10-30 local-caller amendment](../architecture/ADR-106-local-container-transport.md#accepted-local-caller-amendment-2026-10-30)
permits ordinary same-PC WSL processes and containers to reach the existing
application listener for disposable authenticated tests, like ordinary Windows
callers. They remain adversaries, not trusted peers. This increases exposure to
TLS/HTTP flaws, password guessing, pre-session allocation and shared-admission
denial of service. Credentials and current authority remain mandatory; browser
headers do not authenticate native callers. Keys, PostgreSQL and administration
remain independently protected. External LAN ingress is forbidden; a cooperating
local relay is outside that confinement guarantee. No supported-installation or
real-data boundary is accepted by this test concession. The original failed
guest-isolation observation remains a failure under its original criterion.
Confirmed logout entries can be reclaimed without reactivating captured tokens;
later retries can return 401 without contradicting a committed audit. Reload-safe
CSRF retrieval exposes only the existing independent CSRF value through bounded,
same-origin browser bootstrap, with no bearer exposure or lifetime renewal.
Real-browser and capacity/retry qualification remain unexecuted T15/T16 gates.

The single active runtime and its local session registry apply only to the
local MVP. [B-032/#65](../product/backlog.md#b-032) owns cross-replica credentials,
revocation/CSRF, aggregate limits, actor contention, remote proxy/peer trust,
secret rotation and shared-backend failure domains. Multiple Rust pods alone
would not qualify any of these controls; local proof cannot qualify remote or
multi-user/tenant deployment. See ADR-105's
[remaining obligations](../architecture/ADR-105-local-owner-authentication.md#remaining-scale-and-security-obligations).

Under this accepted design, OS/authority administrator compromise still permits credential reset
or data access; encrypted per-user storage cannot contain malware controlling the
same account. Throttle saturation can deny local login availability. Browser XSS
can act with a session despite HttpOnly. Keep these residual risks and prove the
specific controls rather than describing loopback or framework defaults as trust.

## Current-work selection boundary update — 2026-11-21

This update covers [ADR-112](../architecture/ADR-112-current-work-selection.md)
and the bounded native/browser current-work capability. The initial tables and
authentication-stage notes above retain their historical scope. This section does
not refresh every older surface or establish supported real-data deployment,
ASVS compliance, a performance envelope or general recovery qualification.

| Surface / threat | Implemented boundary and control | Residual risk / limit |
| --- | --- | --- |
| Authenticated `POST /selection/read`, `/selection/set`, `/selection/reconcile`; forged owner or target access | Existing session, browser origin/client marker and CSRF admission; subject comes from the installed owner, never a caller actor field. Current original-target authority precedes replay, expiry or reused-identity disclosure; clear needs independent self authority | Local callers remain adversaries. Shared admission can deny availability; loopback and browser headers are not authority |
| Fresh schema v4 selection head, permanent success events and operation bindings; duplicate or inconsistent acceptance | Atomic operation/payload/event/head agreement, immutable original attribution, version-conflict checks, no-op success history and exact-identity reconciliation | Every success, including unchanged choices, adds permanent history and retains target identifiers after payload retirement. No populated upgrade, compaction or general archive route is introduced |
| Same-profile IndexedDB selection recovery journal; lost replies, stale callbacks or replacement by another tab | Strict transaction completion before send; installation/owner/family binding; bounded exact wire and metadata; generation comparison; seven-day eligibility; no automatic execution on restore; mutation cleanup failures remain latched | The unkeyed SHA-256 checksum detects corruption, not malicious rewriting. Normal OS/profile and same-origin isolation apply; XSS or malware controlling that profile can read or alter metadata. No application encryption or forensic-erasure guarantee |
| Current-read availability during journal failure | A current read independently checks fresh owner binding and lifecycle identity; journal failure does not authorize mutation, retry, reconciliation or forgetting | Reading does not settle an unknown operation, prove an empty journal, cancel a request or clear an unconfirmed-cleanup latch |
| Restore, archival and resource exhaustion | Authoritative selection/events/operations must remain coherent. Future archive exposure requires atomic affected-user selection clearing; future supported restore must preserve history and identity | Those broader mechanisms remain outside this increment. Aggregate qualification memory approached its configured bound; the cause and a larger supported workload remain unqualified |

The [qualification record](../implementation/current-work-selection.md#qualification-on-2026-11-21)
distinguishes actual native, visible-browser and independent SQL observations from
transactional-port tests. Historical failures, the reserve warning and the narrow
sampled memory margin remain recorded. The measurement covers the combined test
workload, not the authority process's RSS; it does not identify a cause or justify
changing worker pools, validation, keep-alive, retention or security policy.
[Issue #105](https://github.com/MrQuality/kehila/issues/105) tracks the bounded
resource-attribution follow-up under B-025; it does not activate a capacity test.
