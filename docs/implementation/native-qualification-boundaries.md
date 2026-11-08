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
