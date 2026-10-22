# Remaining A0: proposed Linux-container boundary and disposable proof

Status: Proposed design and probe plan; no implementation/execution permission.
Owner: project maintainer. Recorded: 2026-10-22. Related #40/#49 W2;
[ADR-106 proposal](../architecture/ADR-106-local-container-transport.md),
[Accepted ADR-105](../architecture/ADR-105-local-owner-authentication.md),
[installation/lifecycle criteria](local-owner-authentication-installation-plan.md).

## Framing and decisions

Original outcome: prepare a reviewable design PR for container connection/TLS,
server-key ownership/lifecycle and a bounded next proof. The maintainer authorized
that design work after PR #72, not a network amendment, storage adoption or tests.
Baseline: `8b13fd34625b799824a8422b5c24db799955243c`. This existing plan is the brief;
the PR records candidate/final reviewed revisions, actual findings and checks.

| Consequential premise | Status / source | Consequence and resolver / next action |
| --- | --- | --- |
| Backend in a Linux container; Windows MVP clients | Established by maintainer clarification and the installation plan. Linux clients remain a separate target. | Design those boundaries; do not resume native Windows server-key research. |
| Literal-loopback origin AND Rust listener | Established: ADR-105 section 1. | Direct container binding conflicts; maintainer must explicitly decide Proposed ADR-106 before dependent implementation. |
| Runtime/transport investigation is full authentication qualification | Contradicted: AUTH-001 and A0-A5 plan. | Keep identity, schema, cookies/CSRF, actor and real-data claims gated. |
| Windows/WSL rootless bridge is the first proof target | Proposed, not a universal Windows/Podman guarantee. | Maintainer reviews scope; executor verifies actual provider, mappings and prerequisites before execution. Stop on mismatch. |
| Restricted file secret meets copied-key protection | Unsupported: Podman documents read-protection, not encryption. | Maintainer explicitly accepts disposable-only residual risk or requests a different storage contract. |
| Documentation proves host/VM/container isolation or safe file operations | Unsupported. | Executor needs the path/ownership/crash matrix below; reviewers cannot substitute a verdict for actual evidence. |

The proposed path and alternatives are in ADR-106. Independent relevance review
receives original requirements before this candidate; record its sources, gaps
and dispositions in the PR. Exact SOP-002 version approval remains separate.

## Source evidence and its limits

Primary API references checked 2026-10-22, pinned to Podman 5.8.3 documentation
for reproducible source review, not engine installation/adoption approval:

- [Run: publication, network, user and secret options](https://docs.podman.io/en/v5.8.3/markdown/podman-run.1.html):
  explicit host address, bridge/private networking and mounted-secret ownership
  controls are documented; rootless IDs require mapping, not numeric equality.
- [Secret creation and drivers](https://docs.podman.io/en/v5.8.3/markdown/podman-secret-create.1.html):
  file storage is described as read-protected; pass uses GPG encryption.
- [Network creation](https://docs.podman.io/en/v5.8.3/markdown/podman-network-create.1.html):
  bridge/internal options do not prove isolation from every VM or Windows path.
- [Windows tutorial](https://github.com/podman-container-tools/podman/blob/15c9ca130ffb3d12619817a842cc52e3fc738b4a/docs/tutorials/podman-for-windows.md):
  host forwarding is provider-dependent evidence to verify against exact versions.

There is an upstream documentation inconsistency: the run secret paragraph says
later changes affect the container; secret-create's replace option says existing
containers are unchanged. No hot-update guarantee is inferred. Use immutable IDs,
recreate containers and test the resulting generation. Freeze the executed
client/server/provider/image versions and digests in evidence; source inspection
alone does not select a secure version or complete license/advisory review.

## Proposed connection and ingress contract

Keep public origin `127.0.0.1:P` distinct from the exact assigned listen address
`V:P`. Pin one network/subnet/address after conflict checks; do not infer topology
from a default network. Require an owned, otherwise unused rootless bridge, no
other attached workload and one explicit IPv4 publication; no publish-all, wildcard
listener, host networking, shared namespaces or TLS proxy. Do not enable/change
provider networking, firewall or security settings to make the proof pass.

| Source/path | Required observation |
| --- | --- |
| Windows browser/CLI to configured loopback port | Positive TLS and diagnostic request; exact public Host/Origin policy. |
| Windows other IPs, IPv6 loopback and external owned LAN peer | No diagnostic listener reachable. Inspect actual bindings and exercise connections; failed unrelated network setup is not a pass. |
| Unrelated container on another network / untrusted guest namespace | No direct path to `V:P` or an alternate VM publication. Record topology and distinguish lack of route from successful unintended ingress. |
| Trusted engine/VM administrator | Can defeat network/key restrictions; explicit residual risk, not an isolation success claim. |
| Ordinary local caller to Windows loopback | May reach TLS; this is not caller authentication. Future routes retain ADR-105 identity checks. |

Here, untrusted guest sources include ordinary nonadministrative guest users/
processes and unrelated containers, excluding only the explicitly trusted engine/
VM administrator. Do not choose a convenient isolated namespace as a substitute
for other reachable guest paths. List actual Windows, VM and container interfaces,
routes and publications before and after tests. A Windows self-connection to its LAN address does not prove remote
LAN refusal. An unavailable owned peer/namespace leaves its case unqualified;
do not create accounts or use external machines implicitly. Any unintended ingress
blocks qualification. `--internal` or a declaration alone cannot pass this matrix;
select compatible actual bridge/forwarding controls in the reviewed executable
proposal, without widening this contract if they fail.

## Key, issuer and file-operation contract

Propose one non-root serving UID/GID and one mounted secret generation as ADR-106
specifies. Verify guest engine ownership, UID mapping, parent/mount protections,
opened-object owner/type/mode and no writable alias before reading. Read at most
64 KiB plus one overflow byte before parsing; reject symlinks/redirection, wrong
owner/mode, unknown/extra objects and mismatched key/SAN/validity before listening.
Exactly one leaf and PKCS#8 key are permitted; trust roots are separate inputs.
Do not infer safe opening from path text, mount mode or a zeroizing input buffer.
The serving process gets no issuer key, engine socket or provisioning credentials.

The operator owns issuer generation/signing and confidential staging outside the
checkout, and verifies the secret-import channel and absence of echo/logging.
Serving keys, staging copies, engine storage and mounted copies have a positively
owned inventory. The serving container cannot write its generation. Reject reuse
of an existing secret name/ID; no replace/ignore flags or in-place multi-file swap.
Safe wrapper/open semantics, strict bundle parsing and owned durable intent-record
creation/replacement remain API/dependency-review gates before implementation;
no safe adapter, new crate, raw unsafe code or installer is selected by this plan.

| Lifecycle / interruption point | Required state and reconciliation |
| --- | --- |
| Prepare / before import | Write protected operation intent with exact origin, certificate fingerprints, generation and owned resource IDs before mutation. Select positive-owned baseline only; no inferred cleanup by name. |
| Import creates a generation / crash afterward | Record actual secret ID; incomplete intent/inventory requires operator reconciliation. Never guess the newest generation or overwrite an existing secret. |
| Old process stopped / container removed | No serving instance until an explicit complete generation is selected. Keep old material only within the owned inventory for valid rollback. |
| New container created / startup fails or outcome unknown | Inspect exact container/secret IDs and selected generation. Stop ambiguous serving instances; validate the intended pair before another start. No parallel serving or automatic restart loop. |
| Renewal succeeds / crash before retirement | New fingerprint must be observed in fresh browser/CLI clients before retiring the old generation. At most two generations; do not infer leaf revocation from replacement. |
| Failure / rollback | Recreate with an explicitly selected still-valid previous generation, or remain stopped. Expired/compromised material cannot be a rollback target. |
| Removal / interrupted cleanup | With valid diagnostic service reachable, test exact trust removal in fresh clients, then stop/remove owned containers, secret IDs, staging keys and network. Reconcile before retry; preserve unrelated resources. |

A protected intent record needs an actual durable-file contract; no crash-safe
installer is claimed until its adapter and failure points are tested. Metadata
must not contain a private key or bearer. Key deletion is not secure erasure of
VM storage/backups, and a copied old key/leaf is not revoked while still trusted.
Issuer-key retention/removal follows the existing approved-scope inventory rules.

## Bounded next probe proposal

This is a diagnostic extension, not an auth server. Preserve AUTH-001's host-only
profile/tests. A separately reviewed container profile must separate public origin
from binding without relaxing global host checks. Reuse its closed GET / and
POST /probe protocol, TLS policy and bounded ownership; no database, broker,
search service, login, actor or command execution is needed.

Propose one diagnostic container: 256 MiB memory, 0.5 CPU, 64 PIDs, read-only root,
all Linux capabilities dropped, no new privileges and no automatic restart. Permit only needed
bounded tmpfs (16 MiB total) and the secret mount; verify actual mount/limit state.
Inherit 16 tasks including completed/unjoined tasks, 32 headers, 8 KiB HTTP buffer,
two body bytes, 2-second TLS/header deadlines, 5-second connection lifetime and
180-second server lifetime. These are proposed test ceilings, not capacity claims.

Propose UTC validity checks before listening and each new handshake, plus a
monotonic expiry deadline derived at startup so backward wall-clock jumps cannot
extend validity. Poll validity at most every second; after expiry/invalid clock
state stop admission within one second and drain/abort owned work within five
seconds. A forward clock jump can trigger earlier shutdown; no later rewind
reopens admission. Test exact boundaries, startup with expired/not-yet-valid
material and forward/backward jumps; both server and client need distinct oracles.
Use a reviewed process-local injectable clock for jump cases; no Windows/VM clock
change is authorized. Measure real expiry/drain with short-lived fixtures and
monotonic elapsed time independently. Client rejection of invalid leaves uses an
isolated adversarial TLS fixture, not an override of the proposed server startup
guard. Scheduling overruns fail rather than silently widening the bound.

| Case group | Specific oracle; all results currently unexecuted |
| --- | --- |
| Topology and publication | Full allowed/denied path matrix above; strict bind/origin separation, no alternate listener, occupied port/subnet conflict fails closed. |
| Certificate/browser/CLI | Actual Chrome and CLI success with agreed disposable issuer; typed untrusted, wrong-SAN, expired and removed-trust failures. No generic reset as certificate proof. |
| Secret admission | Missing, 64 KiB/over-limit, malformed, extra-object, wrong-key/SAN, wrong-owner/mode and redirected inputs fail before listening; owned other-UID test cannot read. |
| File/mount safety | Same-object checks, immutable mounted generation, concurrent/replacement attempts, no writable alias; parser and safe-open wrappers have source review and real failure evidence. |
| Renewal and interruption | Interrupt at every lifecycle row; accept only complete explicit generations or stopped state, bound inventory to two, verify fresh leaf, refuse invalid rollback. |
| Resource/expiry/shutdown | Preserve stalled TLS/header/body, task-count and listener-release assertions; exercise numeric expiry/drain and clock-jump cases under container limits. |
| Trust/cleanup | Exact separately approved per-user trust and explicit CLI roots; no issuer in server; remove owned trust/material and prove absence of listener/owned leftovers. |

Before executable work: maintainer decides ADR-106 and disposable-storage risk;
review exact images/digests, issuer tooling, APIs, versioned dependencies/notices/
advisories, intent-file adapter and failure oracles. #40 retains the independent
production dependency disposition and 2026-11-18 review deadline.
Obtain fresh permission for heavy work/VM/services and exact trust mutations.
Check actual RAM before VM start, after start and before compilation; retain the
[public native-check minimum](../spikes/SP-004-actor-authority.md) of 3 GiB remaining
headroom, not permission to start a VM that consumes it. Use
one Cargo job, external build/temp/evidence storage and no overlapping heavy jobs.
No provider switch, new identity, certificate trust or security-setting change is
implicit. Preserve existing unrelated resources and stopped/running baselines.

Retain a 90-minute active investigation budget for one bounded run, excluding
fetch/build, required gates and permission waits. Prioritize topology/ingress and
a complete-generation positive CLI check first, then secret admission and lifecycle
interruptions, then remaining browser/clock/resource cases if time permits. A
failed or unreached group remains open for a separately reviewed follow-up scope
and applicable fresh permissions; no repeated run or budget extension is implicit.
Stop at budget, unsafe/missing API guarantees,
unintended reachability, ambiguous resource ownership or failed prerequisites.
Report incomplete cases and required follow-up rather than weakening assertions.

Exit only with exact-source reviewed implementation, actual matrix/clock/resource
results and confirmed owned cleanup. Partial evidence remains partial. This proof
does not complete A0's wider dependency/installation/client work or any T01-T16
case, authorize A1-A5, adopt ADR-103 or qualify production/replicated deployment.
Record reached subset and remaining gates in #40/#49 and the implementing PR.
