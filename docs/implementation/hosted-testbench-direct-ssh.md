# Brokerless SSH options for the hosted testbench

Status: Proposed research, 2026-11-04. Related:
[ADR-110](../architecture/ADR-110-hosted-verification-testbench.md),
[SSH mechanisms and risk plan](hosted-testbench-ssh.md),
[SP-007](../spikes/SP-007-hosted-testbench.md).

Further evaluation is deferred in [#95](https://github.com/MrQuality/kehila/issues/95)
for future contributors; no near-term work is planned. Local-to-runner A failed
for the selected external endpoint/path in R17; runner-to-local B remains
externally untested. See the [current handoff](../spikes/SP-007-hosted-testbench.md#current-result-and-contributor-handoff).

## Outcome and current recommendation

The maintainer requested evaluation of both direct connection directions to
reduce operated infrastructure and recurring cost. This replaces broker-first
research priority, not the underlying hosted-backend/local-browser outcome.
Neither complete bench topology nor network/security reconfiguration is accepted here.
Source baseline: `c1ddaaf674c4a616e0b916801973adf615dc8403`.

The original sequence evaluated direct local-to-runner SSH first: if ingress works, its server-side
destination restriction is simpler than a reverse forward. The maintainer selected
a bounded A-only synthetic experiment. The later R17 result and issue #95 preserve
runner-to-local SSH as the deferred next direction, without automatically starting
it or declaring every A path impossible. Both can avoid a rented
broker, but neither is currently qualified as reachable. Do not purchase a broker
merely because the first investigation used one as its topology example.

The source comparison is complete enough to define the next probes. The
[disposable research fixture](../../experiments/hosted_ssh_probe/README.md) prepares
one control-only encryption run followed by a separately admitted live SSH run.
The encrypted artifact return channel is explicitly temporary: delete owned
artifacts after consumption and again at closeout, verify absence, and retain
cleanup failures. Its admission does not select the permanent bootstrap channel.
R17 observed three external TCP connection timeouts after successful contained
controls. It does not establish a supported incoming mapping or identify the
upstream cause. Actual Windows/WSL session isolation and lifetime, provider
support and product integration remain unqualified. Machine-specific observations
guide local admission only; no machine addresses/configuration belong here.

## Compare the two directions

| Property | A: local machine initiates to runner | B: runner initiates to local machine |
| --- | --- | --- |
| SSH server | Disposable, contained server on the hosted runner | Separately admitted, restricted receiver on the local machine |
| Forwarding | Local `-L`: runner server opens the backend destination | Remote `-R`: runner client opens the backend destination |
| Public reachability needed | Runner-facing SSH endpoint and a usable per-job ingress mapping | Local-facing SSH endpoint through an ISP or supported VPN mapping |
| Browser endpoint | Retained local gate at the exact admitted loopback IP/port | Same gate, forwarding to a separate admitted reverse listener; no release/rebind shortcut |
| Destination enforcement | Runner `sshd` can constrain exact `PermitOpen` | Local `sshd` constrains `PermitListen`; it cannot independently constrain the runner client's ultimate destination |
| Local exposure | Outbound SSH only; no new internet-facing local server | Incoming SSH receiver; authenticated runner must gain no shell or unrelated local reach |
| Additional hosting cost | USD 0 if standard hosted ingress is usable under existing eligibility | USD 0 if existing ISP/VPN service already permits the necessary incoming path |
| Potential other cost | Special runner/network features may invalidate the free-capacity premise | ISP public-address fees, VPN plan changes, optional domain/DDNS and local resource use; unknown here |
| Maintenance | Per-job server/bootstrap/host-key lifecycle, ingress discovery and repeatability | Receiver patches/keys, NAT/firewall/VPN mapping, endpoint discovery and wake/reboot handling |
| Main unresolved gate | Standard-runner public inbound TCP is unestablished | No concrete internet-reachable receiver/mapping is qualified |
| Assessment | Best conditional design fit; investigate ingress before dependent implementation | Credible conditional alternative; savings depend on local-network prerequisites and receiver operation |

These are conditional estimates, not measured total-cost savings. Zero licence
cost and removal of a VM do not remove session admission or operations. Both
retain independent product TLS/authentication, exact Host/Origin, five-second
product deadlines and the unchanged local/CI gates. DNS names identify transport
endpoints only; the browser origin remains `https://127.0.0.1:P`.

### A: direct local-to-runner path

```text
Browser -> retained local gate -> local SSH client
        -> reachable runner SSH server -> exact authority-container IP:port
```

GitHub's [runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
describes dynamic outbound addresses, discourages broad hosted-IP allowlists and
documents Linux guest sudo. None establishes an inbound address/port mapping or
customer control of the cloud perimeter. A public-IP lookup observes egress;
it does not prove that incoming traffic reaches the job. Current eligible Linux
capacity can use multiple cloud providers, so do not assume an Azure-only path.

Larger-runner static-IP features and
[Azure private networking](https://docs.github.com/en/organizations/managing-organization-settings/about-azure-private-networking-for-github-hosted-runners-in-your-organization)
are separate offerings. Static egress does not prove ingress; Azure private
networking excludes standard runners. Do not silently substitute paid capacity,
a self-hosted runner or a relay and call it this free standard-runner option.

No inspected primary source established either supported standard-runner SSH
ingress or a categorical prohibition. The verdict is unestablished, not impossible.
Guest firewall changes alone cannot establish provider routing or NAT control.

The [OpenSSH client manual](https://man.openbsd.org/ssh.1) places a local forward's
destination connection on the server. Therefore runner policy can allow only
local TCP forwarding to the inspected authority IP/port, deny reverse forwarding,
and use `MaxSessions 0` to reject shell, command and subsystem sessions, as
specified by the [server configuration manual](https://man.openbsd.org/sshd_config.5). This
removes R13's specific reverse-target substitution mechanism; it does not constrain
a compromised runner administrator or independently authenticate the application.

### B: runner-to-local path

```text
Browser -> retained local gate -> admitted local reverse listener
        -> runner-initiated SSH connection -> runner client -> authority container
```

Connection establishment travels toward the local SSH receiver; browser requests
travel back through that connection toward the backend. Calling it an outbound
runner connection does not make the local receiver internet-reachable.

Distinguish three possible routes: an ISP public IPv4 mapping, compatible global
IPv6 with firewall admission, or a VPN provider's supported incoming mapping.
Ordinary home-router forwarding cannot by itself establish an upstream carrier
NAT mapping. [RFC 6598](https://www.rfc-editor.org/rfc/rfc6598.html) identifies
the non-globally-routable shared address space used for carrier NAT. An address
outside that range alone is not an end-to-end reachability proof either.

For a VPN route, verify protocol, public address, allocated port, destination
interface, mapping lifetime/renewal and return route. For an ISP route while a
VPN is active, establish compatible routing and explicit privacy/security scope;
do not disable the VPN, alter its kill switch or introduce split tunnelling
implicitly. Existing local firewall rules do not create a provider mapping.

Prefer a contained receiver where practical under the project execution boundary.
A native Windows receiver is a separate execution/security choice, not a silent
installation. Container/WSL reception may add local VM memory and publication
layers. [Microsoft's WSL networking guide](https://learn.microsoft.com/windows/wsl/networking)
distinguishes NAT, LAN publication and mirrored networking; local `localhost`
forwarding is not internet ingress. No global WSL mode or port proxy is changed.

The local server can restrict the permitted reverse listener and deny shell,
commands, subsystems, local forwarding and auxiliary features. R13 still applies:
an allowed reverse listener does not constrain the runner-side destination, and
an existing authenticated connection must not survive reuse into another session.
Enforce destination confinement at the runner and retain local endpoint ownership.

## Dynamic DNS, VPNs and stable addressing

Dynamic DNS provides a stable hostname whose A/AAAA records follow an address.
It neither assigns a static IP nor creates incoming connectivity. No-IP's
[DDNS explanation](https://www.noip.com/support/knowledgebase/what-is-dynamic-dns)
explicitly distinguishes DNS updates from port forwarding and CGNAT traversal.
Updating DNS to a VPN exit address is useful only if the provider maps an incoming
port to the intended receiver. A/AAAA records also do not encode a changing SSH
port; the session descriptor/configuration must carry that separately.

VPN capability must be established for the actual product, plan and OS; P2P
support or an advertised public exit address is not an incoming mapping contract.
Do not design a VPN-exit receiver on an assumed feature. Optional address rotation
also does not establish which mode a machine uses or whether an existing stream
survives. DDNS cannot migrate an established SSH TCP connection. A path/port change
may interrupt it; preserve the failure and require fresh admission rather than
automatic mutation replay.

For short test sessions, prefer investigating an authenticated per-session
endpoint descriptor before adding a persistent DDNS updater. It must bind the
address, port, SSH host identity, source/workflow/run/attempt, recipient and
deadline, with replay rejection and confidential handling. This is a proposed
bootstrap mechanism, not an operational channel or a way around the ingress gate.

If a stable name is useful, compare the actual recurring work. No-IP advertises
[one free hostname](https://www.noip.com/free), but its
[confirmation policy](https://www.noip.com/support/knowledgebase/confirm-my-hostname-free-account-support-question-day)
requires action every 30 days. Cloudflare documents an
[API updater or ddclient](https://developers.cloudflare.com/dns/manage-dns-records/how-to/managing-dynamic-ip-addresses/);
a suitable domain and protected update credential still need ownership and cost
assessment. Neither service is selected or configured here.

A public DNS-only record makes its resolved endpoint available to DNS queries.
Using an ISP-facing address instead of a VPN exit can change the intended privacy
boundary; a hostname does not conceal that address. Include this publication in
the selected route's approval rather than silently updating a public record.

Cloudflare DNS-only access is distinct from its ordinary web proxy or a tunnel.
Its [network ports reference](https://developers.cloudflare.com/fundamentals/reference/network-ports/)
do not make raw SSH an HTTP service, including when SSH uses port 443. A raw TCP
proxy/relay is another dependency with its own eligibility/cost; do not present
it as a free DDNS feature or silently introduce a broker under another name.

## Risk, lifetime and proposed decisive probes

The operator owns the probes; the maintainer accepts public ingress, exact
network/security/trust changes, costs and architectural residuals. Likelihood
and actual savings are unknown; the priorities below precede dependent live use.

| Risk / consequence | Impact / priority | Cheapest next evidence / residual |
| --- | --- | --- |
| D1 Egress mistaken for ingress; wrong service contacted | High / P1 | Inspect candidate mapping and a fresh, owned nonce canary at one exact address/port. TCP connect alone cannot establish the intended job. |
| D2 Incoming local receiver exposes unrelated access | High / P1 | Review exact host/container publication and no-shell forwarding policy before applying it; test forbidden local destinations and source isolation. No broad firewall exception. |
| D3 DNS/VPN changes attach a stale or wrong endpoint | High / P1 | Bind host key plus address/port/deadline to the session; reject changed/wrong/stale mappings. DNS and a raw key scan are not authentication. |
| D4 Broker removal loses an independent cutoff point | High / P1 | Map local and runner owners separately; observe active-stream closure, new-login rejection and backend death. Shared failure domains remain; no hard remote-stop claim. |
| D5 Zero hosting fee shifts cost into ISP/VPN/DNS and administration | Medium / P1 before selection | Confirm incoming-network eligibility and fees, receiver/updater operation and local RAM before claiming savings. |

For A, propose one standard `ubuntu-24.04` job with a ten-minute platform limit,
a separately owned two-minute canary budget, reviewed immutable workflow/source,
Podman-contained synthetic listener, one admitted port/source and no product
secrets or trust. Those are proposed experiment bounds, not qualification of
forced-cancel cleanup. First inspect address/publication/policy state;
label an egress-only address as a hypothesis, not an ingress mapping. If an
authorized candidate exists, make at most three five-second attempts to that exact
endpoint, with bounded synthetic payloads and a fresh session-specific canary
response delivered through the admitted confidential descriptor. Require matching
runner observations as well; a generic echo alone cannot identify the job. Verify owned
expiry/cleanup. A timeout without arriving packets cannot distinguish missing
mapping from upstream filtering. A pass qualifies that run/path only; new
sessions still need admission. Provider clarification can complement the probe;
no support ticket is sent by this research.

For B, first establish a supported receiver route and review its exact
publication, identity, source/port and rollback scope. Then use one bounded runner
job to authenticate that receiver and request the separately gated reverse path
to a synthetic contained target. Test denied shell/local destinations, substituted
runner target, stale connection/port reuse and interruption. Demonstrate actual
Windows and WSL paths separately after the contained mechanism is admitted.

Both probes need an admitted confidential endpoint/host-identity exchange before
execution. Raw addresses, private keys and diagnostics must not enter public
workflow inputs/logs/artifacts. For the selected bounded A experiment only, an
encrypted descriptor may be returned through a temporary artifact to a fresh
local-only recipient key. Encryption does not authenticate the producer: bind
the authenticated workflow/source/run/attempt and archive digest, then check the
recipient, nonce, expiry and host key. Public ciphertext copies may outlive artifact
deletion. The exception does not permit plaintext metadata, product credentials,
raw diagnostic uploads or a permanent artifact channel. Exact source, harness,
source restriction and cleanup review remain pre-use gates.

## Retained fixture limits and gates before reuse

The disposable A fixture remains research source, not a supported connector or
a ready-made implementation of B. The following source findings are retained for
future contributors in [#95](https://github.com/MrQuality/kehila/issues/95).
Severity describes consequence; priority names the point before which the work
is needed. These are unresolved reuse requirements, not fixes delivered here.

| Finding and evidence | Severity / priority | Required disposition before dependent reuse |
| --- | --- | --- |
| Shared repository configuration: `client.py` checks and writes fixed variable/secret names with separate API requests. Hosted job concurrency does not serialize local controllers, and ownership checks followed by deletion are not atomic. | High under concurrent use / P1 before concurrent or unattended reuse | Keep the existing single-operator restriction. Select session-scoped bootstrap state or serialization covering the entire controller mutation lifecycle; prove competing writers cannot overwrite or delete each other's state. Choosing OIDC or reversing SSH does not itself solve state ownership. |
| Forced termination recovery: the controller journals intended ownership, but has no journal-loading/reconciliation command; normal invocation requires fresh storage. Process termination or machine failure can bypass `finally`, leaving configuration, artifacts or local keys for manual recovery. | Medium / P2 before further operational reuse | Define and review recovery from retained ownership records, with bounded reconciliation and independent final-state checks. Preserve conflicts and ambiguous operations for explicit resolution. Do not infer cleanup from a journal or delete resources merely because their names match. |
| External egress lookup: client preparation queries Cloudflare HTTPS trace even for admission-only mode and the encryption control, although those stages need no public SSH source admission. The live runner also queries this external service. | Low / P3 if the fixture is reused | Consider moving source discovery to live preparation while preserving strict live-source validation. Review producer/consumer schemas together; environment properties or DNS alone do not establish an incoming mapping. |

The first two limits are distinct: serializing live controllers would not recover
from a dead owner, and a recovery command would not make shared API mutations
atomic. A future implementation needs focused competing-controller and
restart/recovery controls before its operational scope expands. No concurrency
failure or forced-termination recovery was exercised by this documentation update.

For an interrupted existing experiment, stop new launches and preserve its
external ownership journal and diagnostics. An operator must reconcile the exact
source/ref/run/attempt and resource ownership before removing owned configuration,
tags, artifacts and local private keys. Record conflicts, inaccessible APIs and
uncertain operations as unresolved; do not claim absence from a failed query.
The current CLI cannot resume that journal automatically. Ordinary Python
exceptions and normal interruption can execute `finally`; independent container
leases and job limits are separate controls, not proof of configuration/key cleanup.

The completed R17 run retains its independently observed cleanup result. These
failure mechanisms do not establish a leak in that run or invalidate unchanged
capacity, encryption or contained-control evidence. Reassess the selected
bootstrap, recovery and confinement contracts before using product credentials.

## Review, limits and next decision

Source-first independent framing separated user intent from broker assumptions.
Its challenge preserved direction-of-connection vs direction-of-data, hostname
vs address/port, direct-forward enforcement and failure-domain distinctions.
Research/review effort is unmeasured; no maintenance saving is demonstrated.

The source comparison itself created no listener or paid resource. Subsequent
R15-R17 experiments are recorded separately in SP-007, including R17's admitted
public listener and failed external TCP attempts. No router/firewall/VPN/DNS
change followed. R13 remains valid only for its contained mechanism/version/
configuration scope. Future contributor work in #95 must first establish a
supported receiver route and satisfy the admission, confinement and lifetime
gates; selecting a complete topology remains a maintainer decision.
