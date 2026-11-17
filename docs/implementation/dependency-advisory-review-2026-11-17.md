# Current Rust dependency advisory disposition

Reviewed 2026-11-17 against application source `8afabe1`, whose tree matches
`3ae7404`. This records the inherited November 18 rustls review under
[#40](https://github.com/MrQuality/kehila/issues/40) and
[#49](https://github.com/MrQuality/kehila/issues/49). Both issues retain their
broader unfinished work.

## Current roots and deployment boundary

The root workspace has seven members. The native
[`kehila_authority`](../../src/io/kehila_authority/Cargo.toml) owns application
TLS and native PostgreSQL transport. The separate
[`AUTH-001`](../../experiments/AUTH-001/Cargo.toml) workspace remains an
experimental regression root: the [CI workflow](../../.github/workflows/ci.yml)
still formats, lints and tests it when the selected scope requires those checks.
It must be included in dependency review even though it is not the product
authority.

Both committed locks select **rustls 0.23.46**, **rustls-webpki 0.103.16** and
**Tokio 1.53.3**. Neither contains MongoDB, rustls-pemfile or webpki-roots. The
[prototype retirement](ferretdb-retirement.md) removed the old worker and
forwarding API from active application builds, normal setup and required checks;
its source history and retained evidence remain preserved. Historical affected
versions are not evidence of a current executable dependency.

The review conservatively considers every registry entry in both locks,
including platform and development dependencies, rather than excluding findings
through an assumed runtime-only closure. Root `cargo metadata --locked --offline
--no-deps` confirmed the seven members and declared dependency features. Both
authority and experiment explicitly disable rustls defaults and request
`std`/`ring`; their Tokio-rustls and PostgreSQL adapter edges explicitly select
`ring`. The inspected authority PostgreSQL client uses TLS 1.3, an explicit
configured certificate root and ordinary certificate verification. Its listener
uses `TlsAcceptor`; this review changes no trust or transport policy.

The experiment's Linux-target offline feature-tree query succeeded and showed
rustls `ring`/`std`. The root Linux-target queries for rustls and webpki stopped
on an uncached `ctutils 0.4.8`; a complete resolved root feature check is **not**
claimed. This does not weaken the version-based dispositions below: the entire
lock contains patched versions, independently of which features are enabled.

## Per-advisory disposition

The current [rustls register](https://rustsec.org/packages/rustls.html) and
[rustls-webpki register](https://rustsec.org/packages/rustls-webpki.html), and
each listed advisory, were read on November 17. The result is **inapplicable to
the recorded locked versions** for each row below. Upstream severity and
conditions are retained as context; this is not an exploitability assessment
of a historically vulnerable deployment or a severity downgrade.

| Advisory | Upstream condition or impact | Current locked disposition |
| --- | --- | --- |
| [RUSTSEC-2026-0346](https://rustsec.org/advisories/RUSTSEC-2026-0346.html) | Medium; custom client session stores can disclose secrets by formatting session values with `Debug`. Built-in cache output is unaffected. Record modified November 9. | rustls 0.23.46 meets the patched `>=0.23.46` range in both roots. The inherited November 18 review does not require a reachability exception or another upgrade. |
| [RUSTSEC-2026-0347](https://rustsec.org/advisories/RUSTSEC-2026-0347.html) | Low; buffered client data after ECH rejection. | 0.23.46 is patched. |
| [RUSTSEC-2026-0348](https://rustsec.org/advisories/RUSTSEC-2026-0348.html) | Low; unauthenticated Brotli certificate decompression allocation with the optional feature. | 0.23.46 is patched; no feature-based exemption is needed. |
| [RUSTSEC-2026-0349](https://rustsec.org/advisories/RUSTSEC-2026-0349.html) | Low; unauthenticated key-share prediction can affect later client connections. | 0.23.46 is patched; no post-quantum protection claim is made. |
| [RUSTSEC-2026-0285](https://rustsec.org/advisories/RUSTSEC-2026-0285.html) | Medium; TLS 1.3 handshake encryption-level handling. | 0.23.46 is beyond the patched threshold 0.23.45. |
| [RUSTSEC-2024-0399](https://rustsec.org/advisories/RUSTSEC-2024-0399.html) | Network panic in `Acceptor::accept`; upstream distinguishes the lazy acceptor from `TlsAcceptor`. | 0.23.46 is beyond the patched threshold 0.23.18. |
| [RUSTSEC-2024-0336](https://rustsec.org/advisories/RUSTSEC-2024-0336.html) | High; synchronous `complete_io` can loop indefinitely. | 0.23.46 is beyond the patched 0.23 threshold 0.23.5. |
| [RUSTSEC-2026-0382](https://rustsec.org/advisories/RUSTSEC-2026-0382.html) | Distrusted emSign intermediate paths; upstream record has no CVSS rating. | webpki 0.103.16 is in the patched `>=0.103.16, <0.104.0-alpha.1` range. |
| [RUSTSEC-2026-0098](https://rustsec.org/advisories/RUSTSEC-2026-0098.html) / [0099](https://rustsec.org/advisories/RUSTSEC-2026-0099.html) | URI and wildcard name constraints; exploitation requires misissuance after signature verification. | 0.103.16 exceeds the stable-branch patched threshold 0.103.12. |
| [RUSTSEC-2026-0104](https://rustsec.org/advisories/RUSTSEC-2026-0104.html) | CRL parser panic before signature verification; applications without CRLs are unaffected. | 0.103.16 exceeds the stable-branch patched threshold 0.103.13. |
| [RUSTSEC-2026-0049](https://rustsec.org/advisories/RUSTSEC-2026-0049.html) | CRL distribution-point matching can mishandle revocation status. | 0.103.16 meets the patched `>=0.103.10` range. |
| [RUSTSEC-2023-0053](https://rustsec.org/advisories/RUSTSEC-2023-0053.html) | High; CPU denial of service in certificate path building. | 0.103.16 meets the patched `>=0.101.4` range. |
| [RUSTSEC-2026-0386](https://rustsec.org/advisories/RUSTSEC-2026-0386.html) | Tokio I/O-safety unsoundness; no upstream CVSS rating. | Tokio 1.53.3 meets the patched range in both roots; the earlier patch remains the mitigation. |

The inherited [webpki-roots emSign advisory](https://rustsec.org/advisories/RUSTSEC-2026-0381.html),
[rustls-pemfile maintenance notice](https://rustsec.org/advisories/RUSTSEC-2025-0134.html)
and [MongoDB TLS option advisory](https://osv.dev/vulnerability/GHSA-3p6w-gv5g-xjw9)
are **inapplicable to these current locks because the packages are absent**.
The MongoDB finding is rated High upstream; removal from the current application
graph does not prove the old URI option was unreachable. The pemfile notice is
an unmaintained-package notice with no patched version, not a demonstrated
exploitable vulnerability. This review neither restarts the retired prototype
nor declares its historical deployments safe.

## Package query, limits and follow-up

At **2026-11-17 09:52:19–09:52:23 UTC**, one
[OSV batch package/version query](https://google.github.io/osv.dev/post-v1-querybatch/)
covered the root lock's **180** registry entries and the experiment lock's
**172**, deduplicated to **201** package/version pairs. No findings were returned
for those current pairs. Two additional historical positive controls returned
the expected rustls 0.21.12 / RUSTSEC-2026-0346 and Tokio 1.53.1 /
RUSTSEC-2026-0386 findings. Those controls are query-detection observations, not
application exploit reproductions. This was an OSV query, not a cargo-audit run.

The query and per-advisory comparison establish this dated package disposition.
They do not establish absence of vulnerabilities, a complete source/call-graph
audit, image/action/tool/Go coverage, minimum-version compatibility, fresh runtime
qualification or supported installation and real-data readiness. Previously
recorded failures and broader authentication/recovery gates remain unchanged.
No dependency, application or security configuration change is selected by this
review, and no new confirmed code fix was identified within this assessment.

The maintainer retains ownership under the
[dependency policy](../engineering/DEPENDENCIES.md), #40 and B-021. The inherited
November 18 current-applicability review is recorded here; the existing routine
review date of **December 3, 2026** remains. Reopen affected dispositions earlier
for a new or revised advisory, changed lock/root/features, restored legacy
execution, changed trust configuration or a new supported deployment. A package
query outage must remain unknown rather than a clean result. This document
accepts no exception or historical exposure risk and closes neither umbrella
issue.
