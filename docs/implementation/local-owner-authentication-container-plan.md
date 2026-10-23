# Remaining A0: bounded Linux-container proof plan

Status: ADR-106 architecture accepted for the bounded disposable proof; executable
proposal and API/provider qualification remain pending. No execution permission.
Owner: project maintainer. Recorded: 2026-10-22. Related #40/#49 W2;
[Accepted bounded ADR-106](../architecture/ADR-106-local-container-transport.md),
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
| Literal-loopback origin; bounded diagnostic listener amendment | ADR-105 with explicitly Accepted ADR-106, 2026-10-23. | One specific container listener is permitted for the disposable proof; actual ingress and all executable-readiness gates remain unqualified. |
| Runtime/transport investigation is full authentication qualification | Contradicted: AUTH-001 and A0-A5 plan. | Keep identity, schema, cookies/CSRF, actor and real-data claims gated. |
| Same-host Podman/WSL 2 is the first-proof topology | Selected by the maintainer on 2026-10-22; Windows client, Linux-container backend. ADR-106's bounded rootless-bridge design was explicitly accepted on 2026-10-23; isolation evidence remains outstanding. | Verify actual provider, mappings and prerequisites before execution; selection supplies no networking or resume guarantee. |
| Restricted file secret meets copied-key protection | Unsupported: Podman documents read-protection, not encryption. | Disposable-only read-protection/copy risk explicitly accepted in ADR-106 on 2026-10-23; no production storage or copied-key protection is established. |
| Documentation proves host/VM/container isolation or safe file operations | Unsupported. | Executor needs the path/ownership/crash matrix below; reviewers cannot substitute a verdict for actual evidence. |

The accepted diagnostic path and alternatives are in ADR-106. Independent relevance review
receives original requirements before this candidate; record its sources, gaps
and dispositions in the PR. Exact SOP-002 version approval remains separate.

The maintainer selected Podman with WSL 2 on the Windows client host for the first
proof on 2026-10-22. WSL is infrastructure for the Linux-container backend, not
frontend code. The [read-only feasibility result](local-owner-authentication-podman-feasibility.md)
records stopped-state observations and an unresolved resume mechanism. Its
[pinned source follow-up](local-owner-authentication-podman-feasibility.md#pinned-provider-source-follow-up)
examines queued correction, cached PTP samples and lifecycle paths without
establishing the required adapter. This
selection alone did not accept ADR-106 or storage risk. The maintainer separately
accepted those bounded choices on 2026-10-23; executable work remains unapproved.

## Review follow-up and executable readiness

The design review identified these requirements; specifying them does not prove
the runtime. ADR-106 now has separate explicit bounded acceptance. #40/#49 retain the unexecuted evidence gates.

| Item | Design disposition | Evidence still required before executable approval |
| --- | --- | --- |
| CR-01, Medium / P2 | Pause-inclusive expiry/lifetime; suspension ends the instance, with a barrier before application continuation. | [Read-only feasibility remains unresolved](local-owner-authentication-podman-feasibility.md#disposition-and-next-evidence). Review a dependable adapter and actual-provider qualification method; missing semantics block approval, and injected clocks alone are insufficient. |
| CR-02, Low / P2 | One required argv interface with canonical address/origin grammar and no fallback sources. | Review launcher/process validation APIs and negative-test oracles; later execution must prove actual publication. |
| CR-03, Low / P2 | Version-1 certificate-then-PKCS#8 PEM grammar, explicit whitespace and complete DER consumption. | Select/review parser APIs and rejection-test oracles; later execution must prove strict admission. |
| WSL clarification, Low / P3 | Capture effective mode/options/versions separately from configured values; qualify by ingress evidence. | Review read-only preflight method; later execution must establish effective state and full allowed/denied paths. |

Executable approval requires these methods and APIs to be reviewable, not a claim
that an unexecuted matrix already passed. Actual provider pause/ingress tests
require their own applicable execution permissions and remain qualification gates.

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

### Trusted network configuration

For this separately selected diagnostic profile, require exactly one occurrence
of each argv pair: `--listen-addr V:P` and
`--public-origin https://127.0.0.1:P`. These are the sole inputs for these two
fields: no default, environment/config-file override, alias or request-derived
fallback. Missing, repeated, conflicting or malformed values fail before binding.
Other profile arguments have their own reviewed interface; these names do not
change AUTH-001's existing host-only CLI.

`V` is canonical dotted-decimal IPv4: four decimal octets 0..255, no leading zeros
except `0` itself. It must be a usable unicast address assigned to the container
on the selected owned bridge, not unspecified, loopback, multicast or broadcast.
`P` is canonical decimal 1024..65535, without signs, leading zeros or whitespace;
both arguments must use the same port. Public origin is exactly the ASCII form
shown: no credentials, DNS alias, IPv6, path (including `/`), query, fragment,
percent encoding or surrounding whitespace. Host admission uses `127.0.0.1:P`;
Origin admission uses the exact configured public-origin bytes, preserving the
existing protocol's required/absent Origin rules.

The operator-controlled container definition supplies these nonsecret arguments
and the publication from one reviewed configuration. Launcher preflight validates
owned network/address/publication agreement; the Rust process checks grammar,
port agreement and actual local address assignment before bind. The process gets
no engine socket and cannot prove host publication by parsing its own arguments.
Tests reject omitted/duplicate fields, unequal ports, URI extras, noncanonical
IPv4/ports, unassigned addresses and request-header attempts to configure either
field; topology tests independently verify the launcher/publication contract.

### Effective WSL networking preflight

Record configured and effective networking mode separately, with WSL/kernel,
Windows, Podman client/server and VM-provider versions. Capture relevant
localhost forwarding, mode-specific host-loopback options, Windows/Hyper-V
firewall state and any provider fallback, using read-only inspection.
[Microsoft's WSL configuration reference](https://learn.microsoft.com/en-us/windows/wsl/wsl-config)
documents mode-dependent options and NAT fallback behavior; configuration text
alone is not proof of the active path. Missing effective-state evidence leaves
the profile unqualified. Neither NAT nor mirrored mode is automatically accepted
or rejected by its name: exercise the full ingress matrix on the exact recorded
profile. No mode, forwarding or firewall change is authorized to obtain a pass.

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

Use one non-root serving UID/GID and one mounted secret generation as Accepted ADR-106
specifies. Verify guest engine ownership, UID mapping, parent/mount protections,
opened-object owner/type/mode and no writable alias before reading. Read at most
64 KiB plus one overflow byte before parsing; reject symlinks/redirection, wrong
owner/mode, unknown/extra objects and mismatched key/SAN/validity before listening.
Exactly one leaf and PKCS#8 key are permitted; trust roots are separate inputs.
Do not infer safe opening from path text, mount mode or a zeroizing input buffer.
The serving process gets no issuer key, engine socket or provisioning credentials.

### Diagnostic bundle format, version 1

One secret contains ASCII PEM in this order: exactly one `CERTIFICATE` block,
then exactly one unencrypted PKCS#8 `PRIVATE KEY` block. Version 1 is selected by
the diagnostic profile, not guessed from input; no in-file version prefix is
permitted. Trust roots and chains are not part of this bundle. The 64 KiB limit
covers the complete encoded file, including permitted whitespace.

Require exact `-----BEGIN LABEL-----` / `-----END LABEL-----` lines with the labels
above, and LF or CRLF line endings (no lone CR). Begin lines and base64 lines end
with a line ending; an end line may terminate at EOF. Between/before/after blocks,
only SP (`0x20`), HT (`0x09`), CR (`0x0d`) and LF (`0x0a`) are permitted, with CR
only as part of CRLF. Delimiters begin at a line boundary, without indentation or
trailing spaces. Each nonempty base64 line has 1..64 ASCII characters; concatenated
payload must be canonical padded RFC 4648 base64, with padding only at its end
and no internal whitespace. Decode exactly one DER object per block and consume
all decoded bytes; require the matching certificate/key and the agreed certificate
profile. Line wrapping may vary within that grammar; the parser must not silently
normalize other input into acceptance.

Reject BOMs, comments, headers, non-ASCII bytes, blank lines inside a block,
reordered/duplicate/extra blocks, encrypted or algorithm-specific key labels,
trailing DER bytes and any non-whitespace tail. API selection must demonstrate
this exact grammar (including overflow-before-parse and DER consumption) rather
than rely on a general PEM parser's defaults. Positive LF/CRLF and boundary cases,
plus each rejection class, need reviewed test cases/oracles before executable
approval and actual passing parser tests before qualification.

The [bundle-parser API review](local-owner-authentication-bundle-parser.md)
identifies broader PEM acceptance, canonical-base64 and positive key-match checks.
It supplies candidate composition and rejection oracles, not an adopted dependency
or passing strict parser. Nested DER/profile behavior remains a review/test gate.
The [first adapter result](local-owner-authentication-parser-adapter-plan.md#first-slice-result-2026-10-23)
records implemented pure framing/decoding and passing tests without listener
integration. It does not satisfy this plan's complete credential-admission
requirement; decoded payloads remain unvalidated.
The [credential-admission comparison](local-owner-authentication-credential-admission.md)
shows why an empty outer remainder is insufficient and outlines a schema route.
Its additional key restriction is Proposed; no runtime dependency, admission
implementation or readiness result follows from that research.

The operator owns issuer generation/signing and confidential staging outside the
checkout, and verifies the secret-import channel and absence of echo/logging.
Serving keys, staging copies, engine storage and mounted copies have a positively
owned inventory. The serving container cannot write its generation. Reject reuse
of an existing secret name/ID; no replace/ignore flags or in-place multi-file swap.
Safe wrapper/open semantics, strict bundle parsing and owned durable intent-record
creation/replacement remain API/dependency-review gates before implementation;
no safe adapter, new crate, raw unsafe code or installer is selected by this plan.
The [Linux file-admission API review](local-owner-authentication-linux-files.md)
compares both storage alternatives and outlines a descriptor-based reader. API
shape is supported; topology/alias evidence, dependency adoption and actual
file/durability behavior remain unqualified. This is independent of the unresolved
resume gate and does not authorize executable work.

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

### Clock and suspend/resume contract

Check UTC validity before listening and each new handshake. Derive an elapsed-time
expiry budget at startup; backward UTC jumps must never extend that budget or the
180-second server lifetime. Forward jumps may end service early; once stopped,
no rewind reopens admission. Both budgets include host suspend/hibernate and VM
suspension, freeze or restore. These events end the diagnostic instance; transparent
continuation is unsupported. Ordinary task descheduling is not a resume event.

[Rust `Instant`](https://doc.rust-lang.org/stable/std/time/struct.Instant.html)
does not guarantee whether suspension counts. Linux
[timekeeping](https://docs.kernel.org/core-api/timekeeping.html) distinguishes
`CLOCK_MONOTONIC`, which stops during system suspend, from `CLOCK_BOOTTIME`, which
includes it. Neither name alone proves the selected VM/host pause behavior.
A provider-specific elapsed-time/resume adapter and admission barrier need
independently reviewed API guarantees and a qualification method before executable
approval. Actual separately authorized tests must prove that, after a resume,
the barrier latches stop
before application work can admit a request or continue owned pending responses.
Abort pending work; bytes already handed to the kernel/client cannot be retracted.
If dependable detection/barrier semantics cannot be established, this profile
cannot execute or qualify. No host supervisor or time-sync service is selected here.

While runnable, poll validity at most every second, stop admission within one
second of expiry/invalid clock state, and drain/abort owned work within five
seconds. Scheduling overruns fail the runnable-case oracle. No deadline execution
is promised while the VM is paused; separately prove the resume barrier before
application continuation. Restart requires a new instance and operator-validated
UTC; stale or uncertain UTC fails closed.

Test exact boundaries, expired/not-yet-valid startup, forward/backward jumps and
resume with stale UTC and a paused elapsed clock. Process-local injected clocks
exercise logic but cannot qualify a provider. Separately authorized actual host/VM
pause tests need an independent elapsed-time observer and evidence of the barrier,
including pending requests. Measure short-lived certificate expiry and shutdown
with separate server/client oracles. Use an isolated adversarial TLS fixture for
client rejection, never a bypass of server startup guards. No OS clock mutation,
suspend test, new host service or trust change is authorized by this document.

| Case group | Specific oracle; all results currently unexecuted |
| --- | --- |
| Topology and publication | Full allowed/denied path matrix above; strict bind/origin separation, no alternate listener, occupied port/subnet conflict fails closed. |
| Certificate/browser/CLI | Actual Chrome and CLI success with agreed disposable issuer; typed untrusted, wrong-SAN, expired and removed-trust failures. No generic reset as certificate proof. |
| Secret admission | Accept a valid bundle exactly 64 KiB; reject larger inputs before parsing/listening. Missing, format-v1 grammar/DER errors (including boundary-sized inputs), wrong-key/SAN, wrong-owner/mode and redirected inputs fail before listening; owned other-UID test cannot read. |
| File/mount safety | Same-object checks, immutable mounted generation, concurrent/replacement attempts, no writable alias; parser and safe-open wrappers have source review and real failure evidence. |
| Renewal and interruption | Interrupt at every lifecycle row; accept only complete explicit generations or stopped state, bound inventory to two, verify fresh leaf, refuse invalid rollback. |
| Resource/expiry/shutdown | Preserve stalled TLS/header/body, task-count and listener-release assertions; exercise numeric expiry/drain, clock-jump and suspend/resume cases under container limits; injected logic and actual provider qualification are distinct. |
| Trust/cleanup | Exact separately approved per-user trust and explicit CLI roots; no issuer in server; remove owned trust/material and prove absence of listener/owned leftovers. |

ADR-106 and its disposable-only storage risk were explicitly accepted on
2026-10-23. Before executable work, review exact images/digests, issuer tooling, APIs, versioned dependencies/notices/
advisories, intent-file and clock/resume adapters, admission barrier and failure
oracles. Missing dependable resume API semantics block executable approval;
actual provider evidence gates qualification even if clock-injection tests pass. #40 retains the independent
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
