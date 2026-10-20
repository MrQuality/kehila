# Windows server-key storage and file-operation assessment

Status: Research complete for the stated source/API scope; recommendation Proposed.
Owner: project maintainer. Recorded: 2026-10-20. Related: #40/#49 W2,
[ADR-105](../architecture/ADR-105-local-owner-authentication.md) and the
[A0 installation plan](local-owner-authentication-installation-plan.md).
No dependency, storage model or experiment is adopted by this assessment.

## Recommendation and its limits

Propose ACL-restricted plaintext as the baseline **disposable server-key** proof,
conditional on qualifying the common file adapter below. It has fewer startup
dependencies and directly tests the access/ownership boundary required by both
options. This recommendation accepts readable key bytes on disk; it is unsuitable
if protection of a copied key record without its Windows access controls is a
requirement. In that case prefer user-scoped DPAPI plus the same ACL/file adapter,
after qualifying a suitable wrapper. Do not silently fall back to plaintext.

DPAPI has a real additional benefit, not merely extra complexity: a copied blob
alone does not grant its plaintext to an unrelated account lacking the required
DPAPI context. It also introduces user-profile/recovery dependencies and extra
plaintext-buffer handling at startup. Neither option contains code controlling
the runtime OS user, administrator/OS compromise, or all process-memory copies.
Do not claim general protection against disk theft from DPAPI alone. Microsoft's
[protection API](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata)
documents user binding, Machine scope and roaming-profile exceptions.

The **common file adapter is the immediate engineering gate**. This investigation
did not establish a complete existing safe-Rust combination that satisfies
protected creation, same-object validation, all-component redirection refusal and
replacement under the current contract. That is a bounded research result, not
proof that no suitable library exists. Do not begin a key lifecycle implementation
by combining package names whose individual guarantees do not compose.

This does not change ADR-105's DPAPI-plus-ACL requirement for **CLI bearers**.
Issuer signing keys remain a separate lifecycle outside the serving process's
required access. The agreed zero-fee private-CA/TLS profile is unchanged. A1-A5,
ADR-103 and B-032/#65 retain their gates.

## Comparison against the accepted local-user boundary

| Question | ACL-restricted plaintext | User-scoped DPAPI plus ACL |
| --- | --- | --- |
| Ordinary other OS user | File/directory DACL is the confidentiality boundary. Broad inherited access or a copied plaintext record can expose the key. | Same DACL boundary, plus user-bound protection for the encrypted record. Machine scope is unacceptable. |
| Same OS user can execute code | Can read an accessible key or inspect the serving process. | Can invoke user-context decryption or target the process after decryption; no application-only isolation is established. |
| Startup | Bounded same-handle read and parse under the expected user. No decryption/profile availability step. | Same read/parse checks plus noninteractive unprotect under the provisioning user's usable Windows profile. Fail closed on decryption failure. |
| Memory | Plaintext read buffer and TLS loader/provider representations need ownership review. | Adds the Windows-allocated unprotect buffer and its Rust copy; clearing a later Rust buffer does not clear the earlier allocation. |
| Replacement | A complete new record and its permissions must be validated before publication. | Same requirement. DPAPI does not serialize writers or make certificate/key replacement atomic. |
| Recovery | Reissue disposable certificates after key loss; never imply file deletion erases all copies. | Reissue after loss of the relevant DPAPI context; do not promise portable blobs or a recoverable secret from an OS-account reset. |
| Cost/portability | No certificate fees; Windows DACL implementation remains platform-specific. | No certificate fees or separate application decryption password, but the Windows user/profile dependency does not transfer to a Linux container. |

DPAPI uses the OS context, not the Kehila password. Ordinary OS password changes
and administrative password resets are different events; Microsoft's
[DPAPI example](https://learn.microsoft.com/en-us/windows/win32/seccrypto/example-c-program-using-cryptprotectdata)
warns that reset can make old data unrecoverable. Test the chosen runtime/profile
configuration; do not generalize interactive-user success to a Windows service.

Unprotect success is not application-record admission. Enforce encoded and
decrypted bounds, record kind/version/origin, supported key/certificate formats,
pair matching and certificate validity afterward. Microsoft's
[unprotect contract](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptunprotectdata)
warns against relying on one corruption error code. Neither DACLs nor DPAPI supply
freshness or rollback detection for an older valid record.

## Required file-operation contract, common to both options

These are proposed adapter acceptance criteria, not implemented guarantees.
Start with explicitly approved local NTFS storage, excluding network, synchronized
and container-mounted paths until separately qualified. Treat path selection and
the initial trusted directory handle as security inputs.

1. **Protected creation:** create the root with its restrictive security descriptor
   in the creation operation, or prove an already protected parent's inheritance
   supplies the required descriptor from the first instant. Creating a permissive
   object and tightening it afterward is insufficient. Verify owner SID, DACL,
   inheritance and allowed rights; reject a NULL DACL and unsupported ACE forms.
   Microsoft's [file-security rules](https://learn.microsoft.com/en-us/windows/win32/fileio/file-security-and-access-rights)
   explain creation-time descriptors and parent inheritance. A readonly attribute
   is not a DACL check ([Rust Permissions](https://doc.rust-lang.org/std/fs/struct.Permissions.html)).
2. **Anchor and open:** resolve from an owned directory handle, with fixed relative
   single-component record names. Reject device/UNC paths, alternate streams,
   traversal and redirection in every traversed component. A final-component
   no-follow flag or a canonicalize-then-open sequence is not a complete proof.
   Inspect actual reparse attributes/tags; ordinary symlink detection is not a
   demonstrated refusal of every redirection mechanism. Handle-relative APIs
   exist ([NtCreateFile](https://learn.microsoft.com/en-us/windows/win32/api/winternl/nf-winternl-ntcreatefile));
   their presence alone does not qualify a Rust adapter.
3. **Validate the object used:** query owner/DACL, object type, reparse attributes,
   identity and link policy on the retained handle before secret I/O. Use that
   handle for the bounded read; never reopen by pathname after validation.
   [GetSecurityInfo](https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-getsecurityinfo)
   queries handle security, but does not freeze it against concurrent changes.
   Define trusted-writer assumptions and reject unknown ACL/identity outcomes.
4. **Own handles and writers:** define non-inheritable handles, lifetime ownership,
   writer serialization and explicit share modes. Existing handles can block new
   opens or renames ([CreateFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew)).
   Bound retries and report sharing violations; do not delete the old record first,
   terminate unrelated processes or weaken protection to make replacement succeed.
5. **Publish one generation:** prefer one bounded certificate/key record so readers
   cannot combine independent old/new files. Keep the issuer key separate. Prepare
   under the same protected root with create-new semantics, validate the complete
   record, flush, then publish via the reviewed replacement operation. A one-file
   shape removes the two-file consistency problem, not the need to prove atomic
   visibility, ACL behavior and interruption recovery. A corrupt published record
   denies startup; do not search old files and silently resurrect one.
6. **Reconcile failures:** identify old, pending and published artifacts after each
   reached failure boundary; cleanup only positively owned objects. Keep complete
   old/new visibility separate from power-loss durability. [FlushFileBuffers](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-flushfilebuffers)
   does not by itself prove that a later namespace update is durable.

Do not assume `ReplaceFileW` means every failed call leaves the old pathname
unchanged. Microsoft documents partial failure states and preservation/merging of
the replaced object's DACL; its write-through flag is unsupported. A restrictive
temporary file is therefore not sufficient if replacement preserves an unsafe
target ACL. [ReplaceFileW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew)
and [MoveFileExW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw)
must be assessed as distinct operations. This assessment supplies no blanket
Windows crash-atomicity or secure-erasure claim.

## Candidate source assessment and next decision

The [candidate audit](windows-server-key-storage-candidates.md) records exact
published versions, source findings, maintenance/license limitations and advisory
scope. It finds useful primitives, not a qualified end-to-end storage library.
In particular, `windows-dpapi` 0.2.0 needs contract changes before adoption;
`cap-std`/`cap-fs-ext` are the leading opening-path candidates, not proof of safe
Windows replacement. No production or experimental manifest/lockfile changed.

Next review the baseline choice and the missing safe adapter capabilities. Scope
one adapter-feasibility proof before the full renewal/browser lifecycle: show
protected creation, same-object ACL/read admission, parent/final redirection
refusal, writer contention, safe replacement and interrupted cleanup on actual
Windows/NTFS. If the candidate APIs cannot express those guarantees, stop and
bring back a narrowly specified upstream contribution or alternative safe wrapper.
The ban on handwritten application unsafe platform code remains; this is no
approval to fork a library, introduce FFI or replace handle checks with shell calls.

For DPAPI, additionally prove explicit UI suppression, expected-user success,
other-user denial, malformed/wrong-kind/oversized record rejection and native/Rust
buffer cleanup before selecting a wrapper. Use synthetic bytes, not a server key,
for that focused adapter test. ADR-105 still requires this gate before CLI adoption.
Any temporary identity, heavy execution or trust changes retains separate approval.

This proof is prerequisite work, not a passing installation, authentication or
restore case. A later executable scope must name numeric operation/retry limits,
failure barriers and cleanup ownership under the A0 budget; no experiment ran for
this assessment. #40/#49 own the remaining local qualification and library decision.
