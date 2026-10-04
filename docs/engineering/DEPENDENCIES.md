# Dependency and vulnerability policy

This is the review baseline for new/changed dependencies; complete inventories and
automated checks are pending [B-021](../product/backlog.md#b-021). It covers direct
and transitive language packages, images, actions, tools and bundled assets.

Apache-2.0, MIT, BSD-2-Clause, BSD-3-Clause, ISC and 0BSD dependencies MAY be used
after verifying actual license texts, provenance and notices. Other licenses,
including weak/strong copyleft, non-code/data licenses or ambiguous/dual licensing,
MUST receive explicit compatibility/distribution review before adoption. Choose
and record the applicable license in a dual-licensed dependency. An SPDX label is
not proof that all files have that license. Preserve attribution and required
notices in artifacts. No silent assumption that an image's top-level license
covers every transitive package.

Dependency changes MUST record purpose, version/source, maintenance health,
license, direct/transitive risks and update strategy. Prefer maintained packages
and small dependency surfaces. Lock supported dependency graphs and release
toolchains; pin release actions/images by immutable digest. Rust's existing
`--locked` enforcement remains. Version tags in development Compose are not
immutable release input pins. Investigate abandoned or compromised dependencies,
assign a replacement/isolation owner and block new supported adoption if risk
cannot be mitigated. Review updates monthly and on upstream security notices;
security updates take priority over the routine cadence.

All advisory findings MUST have a disposition that considers severity,
exploitability, deployment exposure and reachability, including transitive
dependencies. Confirmed critical/high handling retains the response targets in
[SECURITY](../../SECURITY.md): 3-business-day acknowledgment, 7-calendar-day triage,
14-calendar-day mitigation plan and weekly high-impact updates. Confirmed
exploitable critical/high issues block supported release. Medium/low findings
MUST have an owned fix or documented time-bounded risk acceptance; review medium
within 30 days and low within 90 days. These review intervals are triage policy,
not a promise to ship a fix before investigation. A severity downgrade needs
recorded evidence; scanner suppression alone is not a disposition.

Every exception MUST include affected package/version/advisory or license, owner,
reason, reachability evidence, mitigation, approving maintainer, expiry and next
review date. Expired exceptions reopen the finding and block qualification.
Permanent wildcard ignores are prohibited. A supported release with a mandatory
gap remains below its claimed gate; exceptions do not manufacture compliance.

Planned scanner qualification MUST cover Rust advisories, Go vulnerabilities,
future TypeScript runtime dependencies, image dependencies and secrets. Evaluate
`cargo audit`, `govulncheck`, repository secret scanning and CodeQL/equivalent
against the actual manifests, hosted access and detection fixtures. Pin tool
versions, define advisory-database update/failure behavior, and separate network
assessment outages from deterministic PR metadata/lint tests. Do not report an
unavailable database as a clean scan. Dependency review and Scorecard complement
these checks; neither establishes absence of vulnerabilities.
