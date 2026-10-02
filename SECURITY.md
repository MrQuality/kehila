# Security policy

## Supported scope

Only the current main branch receives fixes. No supported production
release exists yet. Development Compose publishes the explicitly requested host
ports with default PostgreSQL credentials and disabled OpenSearch authentication.
Run it only on a trusted, firewalled workstation or disposable CI runner. Do not
use this configuration for Internet-facing or production data. Containers are
ephemeral; removing them can remove data.

## Private reporting

Once the project is published, use its GitHub **Security → Report a vulnerability**
private reporting feature. Maintainers must enable this feature before soliciting
public reports. Before publication, contact the repository owner through the
private channel already used to collaborate on the project. If no private channel
is available, open a public issue asking only for a private contact method; do not
include vulnerability details, credentials, or a working exploit there.

Include affected revision, environment, prerequisites, reproduction, impact, and
proposed mitigation. Share only the minimum sanitized evidence needed to reproduce.

## Response targets (SLA policy)

Maintainers target acknowledgment within 3 business days, initial triage within
7 calendar days, and a mitigation plan within 14 calendar days for confirmed
critical/high issues. These are volunteer project service targets, not a paid
contractual guarantee. Give weekly updates while a confirmed high-impact issue
remains open. Agree on disclosure timing with the reporter, normally within
90 days; do not publish unpatched details without coordinated assessment.

Before release, review supported FerretDB/backend versions and all image pins;
configure TLS, credentials, least privilege, backups, resource limits, tenancy,
dependency scanning, signed releases, and protected required CI. Never commit
tokens or `.env` secrets. The local hook is a developer aid, not a security sandbox.

The [initial threat model](docs/engineering/THREAT-MODEL.md) distinguishes current
transport controls from planned identity and service security. Follow the
[dependency policy](docs/engineering/DEPENDENCIES.md) for advisories and expiring
exceptions. The [engineering standard](docs/engineering/ENGINEERING-STANDARD.md)
requires scoped ASVS/OSPS evidence before supported adoption; no compliance or
production security is claimed today.

For future supported releases, private reports must lead to documented severity
and exposure triage, remediation, verified patched artifacts, an advisory and
CVE/GHSA where applicable, coordinated disclosure, and a postmortem. Retain the
response targets above. Enable and exercise the private reporting and patch/advisory
workflow before claiming Production readiness; track this work in B-020/B-028.
