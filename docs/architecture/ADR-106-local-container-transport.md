# ADR-106 - Local container transport and disposable server-key delivery

Status: Accepted for the bounded disposable proof. Owner: project maintainer.
Created: 2026-10-22. Explicit maintainer acceptance: 2026-10-23.
The maintainer accepted the listener amendment and disposable-only file-secret
residual risk by an explicit decision, separately from merging PRs #73-#76.
The 2026-10-24 development-proof interruption amendment below replaces the
original diagnostic resume gate. Live-test permission and production storage
adoption remain separate. APIs, isolation, dependencies and runtime qualification
remain gated. Acceptance does not turn unexecuted criteria into evidence.
Related: [ADR-105](ADR-105-local-owner-authentication.md),
[B-007](../product/backlog.md#b-007), [#40](https://github.com/MrQuality/kehila/issues/40),
[#49](https://github.com/MrQuality/kehila/issues/49), D-036/D-037;
[design and proposed probe](../implementation/local-owner-authentication-container-plan.md).

## Context and authority

The maintainer requested a reviewable remaining-A0 design for the Linux-container
backend, Windows MVP client path, server-key lifecycle and a bounded next proof.
The [framing record](../implementation/local-owner-authentication-container-plan.md#framing-and-decisions)
separates that authorization from these proposed choices and execution permission.
The accepted source baseline is `8b13fd34625b799824a8422b5c24db799955243c`.

On 2026-10-22 the maintainer selected same-host Podman with WSL 2 as the first-proof
topology: Windows client, Linux-container backend, WSL infrastructure. This selects
the research/proof target, not this listener amendment or storage risk. The
[read-only feasibility result](../implementation/local-owner-authentication-podman-feasibility.md)
did not establish the original full resume barrier. The development-proof
interruption amendment below changes that proof requirement; it supplies no
provider resume qualification or integrated-test execution permission.

ADR-105 requires HTTPS termination and authority in one Rust process, and says
both the client origin and listener are literal `127.0.0.1`. Host and container
loopback are different boundaries. A host port declaration alone does not prove
access to a container-loopback listener or confinement at the Windows/VM boundary.
AUTH-001's existing host-only evidence and bind checks remain valid for that scope.

## Accepted decision and exact amendment

Use an additional, explicitly selected diagnostic container profile. This
amends only ADR-105 section 1's listener-address rule for the bounded disposable
proof, not production authentication or a replicated deployment:

- Client origin remains exactly `https://127.0.0.1:P`, for one configured port
  above 1023. The certificate retains the agreed exact IP SAN, TLS 1.3 and normal
  verification. No container IP, DNS alias or LAN origin becomes a client identity.
- Publish only Windows `127.0.0.1:P` through the qualified Podman/VM path to one
  configured container IPv4 address `V:P` on a dedicated rootless bridge. Rust
  binds that specific assigned address, never a wildcard. A conflicting subnet,
  unexpected interface/address or additional publication fails preflight.
- Separate trusted `PublicOrigin` from `ListenAddress`; request headers cannot
  configure either. The probe plan defines required, unique `--listen-addr` and
  `--public-origin` arguments, canonical grammar and pre-bind validation, without
  fallback configuration sources. Exact Host/Origin admission uses the former, not `V` or a
  forwarded header. The Rust process terminates TLS; forwarding carries opaque
  TLS bytes and receives no TLS key or authority role. No HTTP or TLS-terminating
  proxy is introduced. Source IP is not caller identity.
- Host-network mode, shared pod/network namespaces, unrelated peers on the bridge,
  automatic forwarding changes and privileged containers are outside the profile.
  Engine/VM administration is trusted and can defeat these restrictions; this is
  not a hostile-engine boundary. Ordinary local callers still require future
  authentication, even though they can reach the Windows loopback endpoint.

```mermaid
flowchart LR
    W[Windows browser or CLI] -->|TLS for 127.0.0.1:P| H[Windows loopback publication]
    H -->|Opaque TLS: provider-specific path to prove| N[Dedicated Linux container network]
    N -->|V:P: exact listener| R[Single Rust TLS authority]
    S[One immutable key and leaf generation] -->|Owner-restricted file mount| R
```

This is an explicitly accepted amendment to diagnostic reachability, not a
reinterpretation of ADR-105's original host-only rule. The allowed/denied path
matrix in the probe plan must pass
on the selected Windows provider before any qualification. A failed direct-access
isolation test blocks this profile; it does not authorize broader VM/LAN exposure.
ADR-105 remains Accepted with this bounded diagnostic amendment; all other
transport/authentication requirements and the original host-only evidence retain
their scope.

## Accepted key delivery: disposable proof only

Use a uniquely named Podman mounted secret containing one complete leaf and
PKCS#8 key generation, read once with a 64 KiB maximum. Require exactly one leaf
and matching key in the plan's exact version-1 ASCII PEM grammar: certificate
first, unencrypted PKCS#8 key second, complete DER consumption. Reject extra
objects, unknown material and non-whitespace tails; parser API selection and
negative tests remain gates, not implemented guarantees.
Rootless engine ownership and container UID/GID mappings must be measured.
Use container UID/GID `10001:10001`, secret mode `0400`, read-only root,
all Linux capabilities dropped, no new privileges and no engine socket in the
container.
These settings are acceptance criteria, not proof of safe opening or protection.

The maintainer accepted the file secret driver for disposable values only, with
this residual risk: its documented read-protection is not at-rest encryption.
Engine owner, VM/host administrators and retained disk/backup copies can expose
keys. No copied-record protection, secure erasure or production storage adoption
is claimed. If that threat must be covered by this proof, stop and review an
encrypted storage/unlock contract instead. Windows DPAPI is not this backend's
storage model; the existing Windows CLI contract remains distinct.

Keep the issuer signing key with the operator, outside the serving container and
its secret store. Import the reviewed bundle through an approved local engine
channel with no secret in arguments, environment, image, build context or logs.
Use a new secret ID per generation; do not overwrite a live record or rely on
hot-update semantics. Stop/remove the old container, then create the new one with
one explicitly selected generation. Persist owned operation intent before mutation;
ambiguous recovery stops for reconciliation. The plan defines each crash point.

## Alternatives and reasons

| Option | Assessment for this proposal |
| --- | --- |
| Container-loopback listener plus ordinary port publication | Preserves the literal bind rule, but the source material does not establish a complete reachable Windows path. Not a qualified baseline. |
| Opaque TCP relay to container loopback | Keeps Rust TLS termination; needs another listener/process and bounded buffering, admission, shutdown and keyless relay review. Reconsider if the direct profile cannot meet isolation criteria; no automatic fallback. |
| VM host network / shared pod | Adds shared-network reachability and interference; not the selected isolated profile. |
| TLS-terminating reverse proxy | Changes the accepted Rust TLS/authority boundary and adds authenticated forwarding/trust work. No demonstrated need justifies it here. |
| Linux-owned generation files on a volume | Viable storage alternative; needs protected creation, safe opened-object checks, atomic generation selection and crash/durability evidence. More application-owned file operations for this proof. |
| Windows bind-mounted key / GPG-backed secret | Windows filesystem translation or encrypted-store unlock adds a separate contract. Neither is selected merely for convenience. |

## Development-proof interruption amendment, 2026-10-24

The maintainer explicitly accepted restarting development tests after a pause,
with the aim of avoiding disproportionate development-station infrastructure.
This amendment applies only to the disposable A0 diagnostic proof:

- Execute each run as one uninterrupted host/VM/container interval. Any observed
  or suspected sleep, hibernation, freeze, restore or interruption invalidates
  the **whole current run**, including partial successes. Preserve prior completed
  runs separately; do not splice evidence across the interrupted interval.
- After interruption, stop the owned diagnostic instance, reconcile owned
  resources, verify usable UTC and begin a fresh instance and full run. An
  interrupted request has an unknown transport outcome; this proof executes no
  business mutations. Do not resume the old process or continue its results.
- This is an operator/test-evidence rule. It claims no automatic pause detection,
  prevention, pause-inclusive guest clock guarantee, timeout-before-continuation
  guarantee or suspend/resume qualification. Unknown interruption means rerun.
- Keep normal runnable-case validity checks, bounded task/connection lifetimes,
  failure diagnostics and owned shutdown. Their actual implementation and
  provider evidence remain required. A suspected pause is not a passed timeout
  or security case. No host watchdog, power-setting change or new privilege is
  required by this amendment.

ADR-105's supported authentication expiry, revocation, runtime-generation and
read/command authority requirements remain unchanged. Supported operation across
host sleep/VM suspension requires a separate reviewed contract and evidence;
a successful uninterrupted diagnostic run does not establish it. All remaining
key/file, ingress, dependency, trust and execution gates retain their scope.

## Consequences and follow-up

A direct profile avoids an extra application relay but enlarges the Rust socket's
network scope. The maintainer accepted that bounded trade-off; actual isolation
evidence remains required before qualification. Downtime during generation
replacement is allowed
for this diagnostic proof. Later authenticated restart loses sessions under
ADR-105; no transparent renewal/session continuity is claimed. Apply the
development-proof interruption rule below instead of requiring an automatic
pre-continuation barrier for this disposable diagnostic run.

The proof does not qualify Linux-host clients, Windows CLI bearer storage,
supported installation, A1-A5, the production graph, ADR-103 or replicas (#65).
The container profile must be explicit: no global weakening of AUTH-001's host
loopback guard or anonymous prototype routes. Production integration remains gated.
Recovery (#60) needs stable identity and fresh enrollment after supported restore;
a TLS bundle is not actor identity or an identity backup. Languages (#61) do not
change canonical origin/identifiers. Agent execution (#62) needs future authority
contracts; diagnostic reachability grants no agent execution capability.

Verification: no new runtime/probe has been executed. Review sources, findings,
actual documentation checks and final revision in the implementing PR. The
[probe plan](../implementation/local-owner-authentication-container-plan.md) owns
numerical oracles, unresolved prerequisites and evidence limits under #40/#49.
