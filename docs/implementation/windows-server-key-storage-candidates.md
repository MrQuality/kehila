# Windows storage candidates: source and adoption gaps

Status: Read-only candidate assessment; no adoption. Recorded: 2026-10-20.
Owner: project maintainer. Related: #40/#49 W2 and the
[comparison/recommendation](windows-server-key-storage-research.md).

## Evidence scope

Inspected published source archives for the ten package/version pairs below and
matched their SHA-256 checksums against registry version metadata. Source links
identify the release's recorded VCS revision. Inspected relevant call paths and
manifest/license-file presence, not every unsafe block or every transitive source.
Repository push dates are activity observations, not a maintenance-quality score.
No candidate was compiled or tested on the target; API/source evidence is not
runtime qualification, an independent audit or minimum-toolchain qualification.

| Candidate | Publication/activity observed | License/adoption scope |
| --- | --- | --- |
| `windows-dpapi` 0.2.0 | Published 2026-03-12; repository last push 2026-03-12, not archived. | MIT OR Apache-2.0; both texts packaged. No selection/adoption. |
| `windows-permissions` 0.2.4 | Published 2021-06-29; repository last push 2023-06-07, not archived. | Manifest declares MIT; no license/copying file found in the published archive. Distribution/provenance review remains unresolved. |
| `windows-acl` 0.3.0 | Published 2021-01-11; repository last push 2023-06-09, not archived. | Manifest declares MIT; packaged MIT and additional nested license files require complete distribution review. |
| `cap-std`, `cap-primitives`, `cap-fs-ext` 4.0.3 | Published 2026-08-20; shared repository last push 2026-10-19, not archived. | Apache-2.0/MIT alternatives and LLVM-exception text packaged. Prefer evaluating the plain Apache-2.0 alternative for these packages; mandatory Windows `winx` remains a separate license gate. |
| `winx` 0.36.0 | Mandatory Windows dependency in the inspected `cap-primitives` manifest; repository last push 2024-12-04, not archived. | Apache-2.0 WITH LLVM-exception only; text packaged. Explicit compatibility/distribution review required by project policy before adoption. |
| `atomic-write-file` 0.3.1 | Published 2026-08-11; repository last push 2026-08-11, not archived. | BSD-3-Clause text packaged; manifest declares Rust 1.85, not a qualified graph minimum. |
| `tempfile` 3.27.0 | Published 2026-03-11; repository last push 2026-10-16, not archived. | MIT/Apache-2.0 texts packaged; manifest declares Rust 1.63, not a qualified graph minimum. |
| `winsafe` 0.0.29 | Published 2026-09-01; repository last push 2026-10-12, not archived. | MIT text packaged; manifest declares Rust 1.87, not a qualified graph minimum. |

These are candidates, not new version pins. Before adding any package, apply the
[dependency policy](../engineering/DEPENDENCIES.md): actual selected license/notice
review, resolved features and full graph, provenance, supported toolchain, owned
maintenance/update strategy and target tests. No installed or locked graph was
resolved here. A point-in-time OSV package/version query returned no entries for
these ten pairs; it does not cover their unselected transitive graph or establish
absence of source defects. #40's existing worker advisory follow-up is unchanged.

## DPAPI wrapper: useful API, unmet application contract

In [`windows-dpapi` 0.2.0 source](https://github.com/sheridans/windows-dpapi/blob/4ac261cc789ab046dd8bac3e914f49b26b8b7d77/src/lib.rs):

- `encrypt_data` exposes User/Machine scope; the User branch passes flags zero.
  Neither protect nor unprotect offers `CRYPTPROTECT_UI_FORBIDDEN`. Passing no
  prompt structure is not an exposed guarantee that forbidden UI fails closed.
- `decrypt_data` copies the Windows output to `Vec<u8>`, calls `LocalFree`, and
  returns the copy. It does not clear the original plaintext allocation; wrapping
  the returned Vec in zeroization cannot retroactively clear that buffer.
- The output allocation has no RAII owner across the copy. `to_blob` casts slice
  length to DWORD without checked conversion; application admission bounds would
  prevent oversized input on our path, but are not supplied by this wrapper.
- Its decryption `Scope` parameter is not evidence that the input was protected
  with that scope. Windows' documented unprotect flags do not provide a
  user-versus-Machine enforcement switch. Require user-only protection on creation,
  provenance/admission rules and actual another-user denial evidence; do not parse
  opaque DPAPI internals or invent a scope-verification guarantee.

Disposition: do not adopt unchanged. Investigate an upstream improvement or a
different reviewed safe wrapper with explicit flags, bounded conversion and
RAII/clearing of native plaintext. This does not authorize a fork or application
unsafe code. The candidate search also considered broader safe bindings and
published DPAPI search results; it is not an exhaustive ecosystem survey or
justification for adding a large credential framework to store a TLS key.

## ACL candidates: handle operations are necessary but incomplete

[`windows-permissions` 0.2.4](https://github.com/danieldulaney/windows-permissions-rs/blob/8740e4efbd88dd01046ad9c169894f3a52eb6e2c/src/windows_secure.rs)
provides handle-based security operations and explicit low-level wrappers.
However, its handle `set_multiple` implementation derives the DACL and SACL
selection flags from `group` instead of the corresponding arguments. This source
path cannot be assumed to apply an independently supplied DACL correctly. Direct
wrappers differ from that convenience method, but no complete creation/inspection
adapter was qualified. Stale release activity and missing packaged license text
add review work. Disposition: no unchanged convenience-path adoption.

[`windows-acl` 0.3.0](https://github.com/trailofbits/windows-acl/blob/09f87952649080c38b085e9759e84aa88ccbb2e2/src/acl.rs)
does expose `from_file_handle`; it is not exclusively path-based. Its stored raw
handle does not encode the external file owner's lifetime. It is a possible ACL
inspection component with disciplined lifetime ownership, not an atomic protected
file-creation or path-containment solution. No recommendation to rely on its
path-based alternative after a separate path check; complete owner/DACL policy
and maintenance review remains required.

[`winsafe` 0.0.29](https://github.com/rodrigocfd/winsafe/blob/71ed88c2a0d18b03ee452f6d4261f4c22f443483/src/kernel/handles/hfile.rs)
has safe `CreateFile` with security attributes, owned close guards and handle file
information; its kernel functions include directory creation and replacement.
No `CryptProtectData`/`CryptUnprotectData`, `GetSecurityInfo`/`SetSecurityInfo` or
`SetFileInformationByHandle` wrapper was found in the published source search.
It is worth evaluating for creation primitives, not an established one-package
solution. Safe signatures do not establish that the necessary descriptor policy
can be constructed and composed without missing APIs.

## Capability and replacement candidates

[`cap-primitives` 4.0.3 Windows opening](https://github.com/bytecodealliance/cap-std/blob/b7acf8e8807fe3fab991884d2208b7e03d35a409/cap-primitives/src/windows/fs/create_file_at_w.rs)
uses `NtCreateFile` with a root directory handle. `cap-fs-ext` exposes final
no-follow and directory no-follow helpers. This is the leading opening-path
candidate, but sandbox containment is a different policy from rejecting every
symlink/reparse redirection. Component handling, root acquisition, opened-object
attributes and descriptor creation/inspection still need qualification.

Crucially, the same release's
[`rename_unchecked`](https://github.com/bytecodealliance/cap-std/blob/b7acf8e8807fe3fab991884d2208b7e03d35a409/cap-primitives/src/windows/fs/rename_unchecked.rs)
concatenates paths derived from directory handles and calls `std::fs::rename`.
Do not infer a fully handle-relative Windows publication operation from the
opening implementation or from Unix behavior. Directory pinning/share policy or
another replacement primitive would need a separate race argument and target
evidence. The `winx` license gate also remains.

[`atomic-write-file` 0.3.1 generic backend](https://github.com/andreacorbellini/rust-atomic-write-file/blob/e1ee3a5f75717532e5275261af45de0f8d4980e6/src/imp/generic.rs)
creates a path-named temporary file with `create_new`, then path-renames it.
Its directory-descriptor robustness is Unix-specific; the Windows path is not
a protected-parent/handle-replacement proof. The retry loop for temporary-name
collisions is unbounded in this implementation. Disposition: not a drop-in match
for our complete bounded Windows contract.

[`tempfile` 3.27.0 Windows persistence](https://github.com/Stebalien/tempfile/blob/5c8fa12eb584931b4f1bccfde87eb72fbfa7dc61/src/file/imp/windows.rs)
uses path-based `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING` on overwrite; that
path does not add `MOVEFILE_WRITE_THROUGH` or freeze ancestors. A private temporary
file API still needs the approved parent, inherited ACL, retained-object identity,
bounded cleanup and flush/replacement policy. Disposition: useful ownership/temp
primitive, not an end-to-end secure publication guarantee.

## Why the dependency gate remains open

Each inspected family solves part of the problem. None was shown to compose all
required creation, handle validation, no-redirection, replacement and buffer
ownership guarantees under the current no-application-unsafe rule. A source
finding is not a measured exploit or a target test. Recommend the focused
[adapter-feasibility proof](windows-server-key-storage-research.md#candidate-source-assessment-and-next-decision)
with explicit stop conditions before selecting the actual dependency graph.
