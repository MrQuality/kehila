# Native qualification boundaries and remaining evidence

Status: documentation and planning map for the existing bounded development
profile. The source mapping uses [`27f7794`](https://github.com/MrQuality/kehila/commit/27f779446ccb4a5f4c3defde2c3e457d45c1b3f4)
and the dated observations linked below. This map records existing qualification
boundaries; it selects no implementation, storage provider or new test campaign.
Recorded results retain their original source, fixture, platform and failure
dispositions. Documentation does not turn an unexecuted scenario into a pass.

## Storage-provider support gate

Owner: [#40](https://github.com/MrQuality/kehila/issues/40), with the existing
[provider/file evidence requirements](local-owner-authentication-linux-files.md#disposition-and-next-qualification-gates).
Apply these checks to a specifically proposed supported profile, rather than
assuming support for every host filesystem or mount type.

The [protected-startup observation](local-owner-authentication-linux-reader-slice.md#protected-authentication-startup-2026-10-29)
admitted four actual Podman secret-mounted assets as UID/GID 10001, with the
selected root and exact file modes. The [current Linux reader](../../src/io/kehila_authority/src/protected_file.rs)
checks the opened object, rejects unsupported/failed opening and metadata checks,
and bounds the same-object read. Its startup-asset mode is exactly `0400`;
the separate CLI credential-storage profile is a different contract.

Those observations qualify the inspected disposable fixture. They do not prove
immutable provisioning or absence of writable aliases. The
[writable-alias control](../../src/io/kehila_authority/src/protected_file/tests.rs)
demonstrates that an already-open writer can change the object after its mode
becomes read-only. A mode bit or read-only serving mount alone is insufficient.

Before declaring a proposed storage/deployment profile supported, its existing
admission and provisioning obligations need the following evidence:

| Boundary | Evidence required for that profile |
| --- | --- |
| Source and identity | Identify the actual host/engine/container copies and mount topology; observe effective owner/group mapping and exact object/root modes. |
| Protected opening | Establish trusted ancestors/root acquisition and actual same-object admission, including relevant resolution/type/error rejection without fallback. |
| Mutation exclusion | Establish which principals, paths, open writers or aliases can modify the admitted generation; any permitted untrusted writer prevents immutable-generation qualification. |
| Publication and renewal | Identify complete generation selection and interruption behavior across publication, intent and engine operations; uncertain state stops for reconciliation. |
| Boundaries and ownership | Record actual byte/parser limits, owned descriptor cleanup and unsupported kernel/filesystem/policy behavior. Byte caps are not filesystem-I/O deadlines. |

These are the existing [provider oracles](local-owner-authentication-linux-files.md#disposition-and-next-qualification-gates),
not newly passed checks. The operator/deployment author supplies profile-specific
evidence; the maintainer decides its supported scope. Existing engine/VM
administration and retained secret-copy risks remain as documented in the
[storage alternatives](local-owner-authentication-linux-files.md#the-two-storage-alternatives).

## Crash and recovery evidence boundaries

Owners: [#49](https://github.com/MrQuality/kehila/issues/49) for native command
identity, reconciliation and client uncertainty; [#40](https://github.com/MrQuality/kehila/issues/40)
for credential/runtime recovery boundaries; [#60](https://github.com/MrQuality/kehila/issues/60)
for the separately scheduled supported recovery/restore work. Their existing
scope and real-data safety gates remain unchanged.

| Retained evidence | What it establishes and what it does not |
| --- | --- |
| [Native item identity/rollback and withheld-confirmation control](../../src/io/kehila_authority/src/coordinator/jobs/tests/project/item_writes.rs) | SQL really commits before confirmation is withheld; subsequent database admission is refused and independent observation finds the saved item. This is an unknown-outcome control, not an abrupt coordinator-process kill. |
| [Owned transaction commit](../../src/io/kehila_authority/src/postgres/transaction.rs) and [scoped operation uniqueness](../../storage/postgresql/operations.sql) | Failed/expired commit confirmation is `UnknownCommit`, not proof of rollback. The scoped identity constrains a retry of the same operation; it does not identify a newly tokened request as the same user intention. |
| [Measured blocked-write stop](local-owner-authentication-listener.md#measured-blocked-write-stop-2026-10-31) | The observed normal-stop schedule preserved verifier/audit state and released owned database resources. It does not prove every write/cancellation ordering or abrupt process death. |
| [SP-002 recovery observations](../spikes/SP-002-postgresql-project-create.md) | The frozen experiment records database-container SIGKILL/restart and clean logical-restore subsets. Preserve its original failure/blocked counts and baseline/fixture limits; this is not qualification of the current native coordinator or supported restore orchestration. |
| [Fresh foundation installation](../../storage/postgresql/README.md#installation-and-ownership) | Installation is one transaction and failure rolls back its changes. Reinstallation, populated upgrades and repair are not supported by that command. A post-commit corruption/restore contract is separate from installer rollback. |

The following broader scenarios remain unqualified by the evidence above. These
are applicability limits and inputs to later scoped work, not newly scheduled
tests or claims of a demonstrated corruption/duplication defect:

| Scenario requiring a later scope/evidence decision | Owning boundary |
| --- | --- |
| Abrupt termination of the current native coordinator around SQL execution, commit and confirmation, followed by fenced restart and same-identity reconciliation | #49; retain independent durable-state and operation/replay evidence rather than interpreting timeout as rollback. |
| Database, host or storage failure outside the frozen experimental profile, including physical power-loss behavior and supported persistent-volume recovery | #49's existing supported-use gates and #60's separately scheduled recovery scope. |
| Client request-history loss or recovery beyond the already observed pending/replay paths | #49; preserve original scope/identity where available and distinguish an uncertain result from a new intention. |
| Restored ledgers with lost post-backup operations, stale retained requests and credential recovery | #49/#40/#60 under the [accepted restore boundary](m1-uncertain-save-reconciliation.md#supported-restore-and-retained-browser-requests); ordinary reauthentication or matching record versions alone does not establish request ancestry. |

The [uncertain-save document](m1-uncertain-save-reconciliation.md) is a proposal
whose original status remains intact. This map neither selects its journal design
nor adds a restore-generation field, new error code, migration or rollback script.
The [native WorkItem decision](../architecture/ADR-111-native-work-items.md)
remains bounded to a fresh synthetic development database. Broader restore stays
separately scheduled; that scheduling does not waive existing real-data gates.
