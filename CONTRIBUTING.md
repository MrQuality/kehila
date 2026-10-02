# Contributing to YAJA

Keep changes focused and explain the problem they solve. Discuss architectural
changes in an issue before implementing them. Follow the
[Code of Conduct](CODE_OF_CONDUCT.md).

The [project procedure index](docs/procedures/README.md) lists SOPs, their scope,
approval status, and enforcement coverage. Follow approved procedures applicable
to your change. Proposed procedures remain proposals until explicitly approved.

## Setup and testing

Follow the [development setup](README.md#development-setup), then run:

```text
python scripts/verify.py
```

Use `python scripts/verify.py --pure` when working on pure logic without the
development services. Integration changes need the full suite against running
services. See [the testing guide](docs/TESTING.md) for coverage and limitations.

Use the [test strategy](docs/TEST_STRATEGY.md) to identify the evidence relevant
to a change. It is proposed; [the testing guide](docs/TESTING.md) describes the
checks currently available. Add tests for changed behavior and relevant failure
cases. Update documentation when commands, contracts, or supported behavior
change. Run `cargo fmt --all`
and `gofmt` on changed Rust and Go files.

Pure Rust code lives in `src/pure/` and adapters in `src/io/`. Go uses `go/pure/`
and `go/io/`. Add new Go modules to `go.work` and the test runner.

## Repository roles

The **project maintainer** owns repository decisions, reviews changes, and
records the assessment required before merging. The current GitHub account for
that role is listed in [CODEOWNERS](.github/CODEOWNERS). During the
sole-contributor phase, the project maintainer also owns the proposed
[test strategy](docs/TEST_STRATEGY.md); no separate **quality owner** has been
assigned. When these responsibilities are delegated, update this section and
the relevant ownership records.

CODEOWNERS routes review requests. It does not grant permissions inside YAJA,
identify an independent reviewer for an author's PR, or by itself require
code-owner approval. Product project administrators and their grants are
separate roles defined by the product contracts.

## Pull requests and review

Apply the [Definition of Done](docs/engineering/DEFINITION-OF-DONE.md), recording
applicable evidence and justified not-applicable criteria. Link existing
requirement/decision/backlog IDs and engineering IDs where relevant. Material
technical decisions use [ADRs](docs/architecture/README.md); product decisions
remain D-* records. Follow the [dependency policy](docs/engineering/DEPENDENCIES.md)
for dependency changes. The [engineering standard](docs/engineering/ENGINEERING-STANDARD.md)
defines release qualification, not automatic compliance from passing metadata checks.

Describe the resulting behavior, tests run, and any remaining limitations.
Include relevant service versions and results for integration changes. Keep
credentials, local environment files, and unrelated changes out of the PR.

The local commit hook tests staged source files in an isolated directory. Stage
test fixes before retrying a failed commit. CI runs the full suite for pull
requests and pushes to `main`.

Changes to `main` require passing CI. During the sole-contributor phase, an
independent approving PR review is optional. Before merging, the maintainer
inspects the final change, test evidence, and applicable procedure requirements
and records that assessment in the PR. Give particular attention to architecture,
dependencies, CI, and test coverage. Self-assessment is not an independent GitHub
approval. Ownership is recorded in [CODEOWNERS](.github/CODEOWNERS); code-owner approval is
not a required merge condition in this phase. Reconsider mandatory independent
review when another reviewer is available.

## License and attribution

Contributions are licensed under Apache-2.0. Retain third-party license notices
and credit contributors using their preferred public name or handle. Report
vulnerabilities privately through [SECURITY.md](SECURITY.md).

Use YAJA's [naming and third-party reference policy](docs/BRANDING.md). Submit
original work or material whose license permits its use here, identify its
source and license in the contribution, and preserve required notices. This
applies to code, grammar definitions, examples, documentation, and visual assets.
Keep third-party product references factual and compatibility claims limited to
documented, tested behavior. Do not introduce third-party branding into YAJA's
name, module names, logo, or visual identity.
