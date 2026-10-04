# Kehila naming and third-party references

## Project identity

Use **Kehila** as the project name and **Project and task management** as the
descriptor. The repository is `MrQuality/kehila`; code and package namespaces use
`kehila`, and configuration environment variables use `KEHILA_*`.

Kehila (קהילה) means community or congregation. The name expresses the project's
emphasis on shared purpose, mutual responsibility, and interconnected work.
It does not claim uniqueness, affiliation with another Kehila product, or
trademark clearance.

Use **Kehila query language** or **query filters** for the project's own grammar,
`kehila_query` for the Rust crate, and `CompiledQueryArtifact` for the shared
compiled-query contract. The proposed browser module is `kehila-query`.

The Rust crate name, import path, source directory, Go module names, and
TypeScript package scope change together. Consumers must update imports;
parser behavior, `CompiledQueryArtifact`, and the contract's fields are
unchanged. No legacy aliases are provided in this early-development repository.

## Rename and retained storage identities

The maintainer selected Kehila on 2026-10-04. The rename changes current project
branding, repository links, package/import names, test fixture prefixes, and
environment variables. Existing launch configurations must use `KEHILA_*` in
place of the former environment-variable prefix. This is a naming change, not
a domain, wire-format, or persistence migration.

Update consumer imports and launch configuration using this migration table:

| Former identifier | Current identifier |
| --- | --- |
| `YAJA_*` | `KEHILA_*` (same suffix) |
| `yaja_query` / `src/pure/yaja_query/` | `kehila_query` / `src/pure/kehila_query/` |
| `yaja/task_api` | `kehila/task_api` |
| `yaja.local/sync_contract` | `kehila.local/sync_contract` |
| `yaja` / `@yaja/contracts` | `kehila` / `@kehila/contracts` |
| `docs/reference/YAJA-v0.2.md` | `docs/reference/Kehila-v0.2.md` |
| `MrQuality/yaja` | `MrQuality/kehila` |

The environment prefix changes for the worker's storage, operation-policy and
listen settings, the API's worker/search URLs and listen setting, and integration
test URLs and NATS settings. Retained storage identities below remain unchanged.

Existing storage and machine identifiers intentionally remain unchanged:

- The development Compose project and database remain `yaja`. The worker's
  default database remains the same; `KEHILA_TASK_DB` can override it.
- The retained SP-001 reproduction uses `yaja-sp001` for its isolated Compose
  project and default Podman connection, its existing volume/image names, and
  `.yaja/spikes/SP-001/` for local evidence. No machine or volume is renamed.
- `.yaja/` remains ignored so existing local artifacts stay excluded. New local
  artifacts may use the also-ignored `.kehila/` directory.

Historical commits, comments, logs, and immutable verification evidence retain
the names that were actually tested. Repository redirects preserve old links;
current documentation uses the new repository URL. Renaming the local checkout
folder does not rename its databases or containers.

## Compatibility and references

The implemented parser accepts one equality filter. Jira Query Language (JQL)
compatibility is not a current feature or requirement. The compiler design is
for Kehila's own grammar. A future interoperability proposal must specify the
supported syntax and semantics, differences, independent implementation sources,
and compatibility tests before making public compatibility claims.

Third-party product names may appear in necessary, accurate comparison,
interoperability, source attribution, or license documentation. Do not imply
endorsement, sponsorship, or compatibility that has not been established. Keep
Kehila's name and visual identity distinct. Use original or appropriately licensed
artwork; do not copy another product's logos, screenshots, or fonts without the
necessary rights. Preserve required third-party notices rather than blindly
replacing product names in them.

Where Jira is discussed publicly, use this independence notice:

> Kehila is an independent project and is not affiliated with, sponsored by, or
> endorsed by Atlassian. Jira is a trademark of Atlassian.

A notice and a naming scan do not establish legal clearance. Relevant policy
references include [Atlassian's trademark guidelines](https://www.atlassian.com/legal/trademark)
and its [JQL documentation](https://support.atlassian.com/jira-software-cloud/docs/use-advanced-search-with-jira-query-language-jql/).

## Release review

Before publishing a release, maintainers should:

- Review the repository description, topics, website, domains, package listings,
  release text, social profiles, and downloadable artifacts for consistent
  current branding and accurate feature claims.
- Record the source and applicable license of third-party code, grammar files,
  examples, documentation, and visual assets; retain required notices. A local
  source scan alone cannot establish provenance.
- Obtain appropriate trademark clearance for Kehila and any logo in the intended
  markets. No clearance or trademark availability is asserted by this policy.
- Assess historical releases separately when necessary. Do not rewrite shared
  history merely to change historical naming.

The repository checks current naming, not external properties, historical
artifacts, legal rights, or the completeness of compatibility claims. External
brand review, provenance verification, and trademark clearance remain release
responsibilities.
