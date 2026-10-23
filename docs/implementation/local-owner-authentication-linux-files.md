# A0: Linux server-key file-admission API review

Status: Research / Proposed adapter outline; no dependency or storage adoption.
Owner: project maintainer. Recorded: 2026-10-23. Related #40/#49;
[container plan](local-owner-authentication-container-plan.md),
[Proposed ADR-106](../architecture/ADR-106-local-container-transport.md),
[dependency policy](../engineering/DEPENDENCIES.md).

## Outcome, authority and boundaries

After PR #75 merged, the maintainer requested continuation. This independent
readiness slice examines Linux file operations for server-key admission, comparing
the two storage alternatives already recorded in ADR-106. Baseline:
`d01c25b4f78ec0b5a66f01e79867dda308954ecc`. Windows is the MVP client platform;
the backend remains a Linux container on same-host Podman/WSL 2. No native Windows
server-key model, new provisioning actor or host authority is selected.

The serving process reads one explicitly selected generation before binding. It
receives no engine socket, issuer key or provisioning credential. Creation/import,
intent records, replacement and removal are separate operator responsibilities;
this review does not choose their process, host/guest placement or installation API.

| Consequential premise | Status / evidence | Consequence and resolver |
| --- | --- | --- |
| Linux key/file review can proceed while resume is unresolved | Independent API prerequisite in the container plan. | Do not claim executable readiness; keep the full resume gate unchanged. |
| A path, owner/mode or read-only mount proves immutable content | Unsupported by file/mount semantics. | Require protected topology and alias/writer evidence; stop if untrusted mutation remains possible. |
| A safe Rust signature establishes the complete security adapter | Unsupported: API shape and policy are different. | Review the whole admission sequence and actual provider failure cases before qualification. |
| Documentation or a merged proposal selects storage/dependencies | Not authorized. | ADR-106/storage-risk and dependency decisions retain their separate review gates. |

## The two storage alternatives

| Boundary | Proposed Podman mounted secret | Linux-owned generation files alternative |
| --- | --- | --- |
| Creation and copies | Engine imports one unique generation and supplies a mounted copy; inventory staging, engine storage and container copy. | Operator creates/protects/publishes complete generations; inventory staging and mounted files. |
| Serving reader | Validate the opened mounted object and read once. Engine labels or a mount declaration do not replace admission. | Same reader obligations, plus evidence that the selected generation cannot change through another path or writer. |
| Publication and renewal | New secret ID and new container; no replace/ignore or hot-update assumption. | Explicit unique generation publication; no in-place live key replacement or guessed newest generation. |
| Additional file work | Engine owns secret-store internals; the operator still needs protected intent and cleanup reconciliation. | More application/operator-owned creation, permissions, publication, durability and cleanup work. |
| Remaining risk | File-driver read-protection is not encryption; trusted engine/VM administration and retained copies remain exposed. | Permissions and read-only serving mounts also do not provide copied-key protection, secure erasure or a hostile-administrator boundary. |

[Podman 5.8.3 secret creation](https://docs.podman.io/en/v5.8.3/markdown/podman-secret-create.1.html)
describes file-driver read-protection and replacement scope;
[run](https://docs.podman.io/en/v5.8.3/markdown/podman-run.1.html) documents mounted
secret UID/GID/mode options. Those are interfaces to verify on the actual rootless
mapping, not measured ownership or safe-opening guarantees. Both alternatives
need admission evidence; neither is selected by this comparison.

## Candidate safe API surface

Inspected candidate: rustix **1.1.5**, tag dereferenced to
`287214b889865d8e1406a0ee71cc409b6f6191c8`. Its
[openat2 wrapper](https://github.com/bytecodealliance/rustix/blob/287214b889865d8e1406a0ee71cc409b6f6191c8/src/fs/openat2.rs)
has a safe Rust interface returning an owned descriptor; the
[descriptor operations](https://github.com/bytecodealliance/rustix/blob/287214b889865d8e1406a0ee71cc409b6f6191c8/src/fs/fd.rs)
include safe `fstat` and `fsync`, and
[relative operations](https://github.com/bytecodealliance/rustix/blob/287214b889865d8e1406a0ee71cc409b6f6191c8/src/fs/at.rs)
include `renameat_with`. Application unsafe platform code is unnecessary for this
API shape; this is not a soundness audit of library internals or an implemented
Kehila adapter. Owned-descriptor lifetime must remain single-owner through checks
and reads, including failures; no raw-descriptor conversion is proposed.

The package manifest declares Rust 1.65 and alternative license grants. Relevant
wrapper source was inspected; metadata alone is not a license review. No complete selected graph,
notices, advisory/reachability review or minimum-version build is qualified.
No crate is added or selected for adoption. Review exact features, platform backend,
transitives and distribution obligations under the dependency policy before use.
The [standard Unix OpenOptions extension](https://doc.rust-lang.org/std/os/unix/fs/trait.OpenOptionsExt.html)
can set open flags, but a final-component NOFOLLOW flag alone does not enforce
the all-component resolution policy below. This is not proof that every other
wrapper or a separately reviewed component-by-component design is unsuitable.

## Proposed serving-reader sequence

1. **Establish trusted topology first.** Review root-descriptor acquisition,
   protected ancestors, exact generation path, intended mounts, runtime UID/GID
   mappings and ordinary callers' write access. Hold a directory descriptor rooted
   in the intended container filesystem; use a fixed relative generation path,
   never request input or the current working directory as authority. The serving
   root and parents must not be replaceable by untrusted writers. Trusted engine/
   VM administrators remain an explicit residual risk.
2. **Constrain the lookup.** Candidate Linux `openat2` policy uses BENEATH,
   NO_SYMLINKS and NO_MAGICLINKS, read-only access, CLOEXEC, NOFOLLOW and NONBLOCK;
   no create/truncate flags. [Kernel resolution semantics](https://man7.org/linux/man-pages/man2/openat2.2.html)
   distinguish all-component restrictions from final-component NOFOLLOW.
   Do not blindly add NO_XDEV: it rejects bind-mount crossings, including a possible
   intended secret mount. Permitted mount topology needs separate positive evidence;
   these flags alone do not authenticate a mount or exclude hard links.
   Missing/denied syscalls or unsupported flags fail startup; no permissive fallback,
   flag stripping, new capability or seccomp relaxation is implied.
3. **Inspect the returned object.** Use `fstat` on that descriptor, require a regular
   file and the reviewed container-side owner/group and mode. For the Proposed
   `0400` policy, compare all permission/special bits, not just group/other access.
   Rootless host IDs must not be equated with container IDs. No path-stat followed
   by reopen or canonicalize-then-open sequence proves same-object admission.
4. **Load once through the same descriptor.** Handle short reads and interruptions
   under a total cap of 64 KiB plus one
   overflow byte; reject over-limit input before parsing and close on every outcome.
   Metadata size is only an observation, not the allocation/read limit. Valid
   exactly-64-KiB input remains admissible. Apply the existing strict version-1
   PEM/DER, key-match, SAN and validity rules before any listener bind. No parser
   or TLS credential-handling change is implemented here.

The [open semantics](https://man7.org/linux/man-pages/man2/open.2.html) explain why
this is still conditional: a descriptor survives pathname replacement, but another
writer can change its contents. NONBLOCK avoids a FIFO-open wait; it does not
bound regular-file I/O or make opening a device harmless. A regular-file check
occurs after open, so this is not a sandbox for hostile device nodes or a hostile
mount namespace. Trusted topology and rejection of unexpected object sources are
prerequisites. No cancellable filesystem-I/O deadline is established by this review.

A read-only mount applies to that access path; another writable mount, hard link
or already-open writable descriptor may still exist. The owner can ordinarily
change permissions. Mode, link count, fingerprints or before/after metadata cannot
alone prove absence of writers or consistent immutable input. The provider/operator
must establish the missing topology/ownership premise. Retaining the opened descriptor
prevents name substitution, not mutable-file races. Input-buffer cleanup also does
not prove that every parser/TLS/library copy or retained disk copy was erased.

Do not assume newer manual features exist in the reported kernel. The inspected
[6.18 source-candidate header](https://github.com/microsoft/WSL2-Linux-Kernel/blob/d504d40ab83839cb61942a812a99d183c42227b9/include/uapi/linux/openat2.h)
contains the resolution flags above; it is not evidence of a running kernel or
its seccomp policy. No newer regular-file-only open flag is selected.

## Writer and intent durability obligations

For a future Linux-owned-file adapter, use a positively owned protected directory,
exclusive creation of a new private staging object, complete write, final owner/
mode establishment, file synchronization, unique-name publication and directory
synchronization. A candidate same-directory `renameat_with(NOREPLACE)` avoids
replacing an existing generation; its
[filesystem support must be verified](https://man7.org/linux/man-pages/man2/rename.2.html).
Check final metadata, every write result, publication and synchronization errors.
Writer close/error-reporting semantics remain an API-review item; RAII cleanup
alone is not a durable-result proof. An unknown outcome requires reconciliation,
not blind retry or success.
Check final data/metadata before the final file sync. A cross-directory design
would need both directory durability obligations reviewed separately.

[File fsync alone does not ensure directory-entry durability](https://man7.org/linux/man-pages/man2/fsync.2.html).
Neither rename atomicity nor an error-free process exit qualifies crash recovery
through WSL/VM/storage layers. Before engine mutation, the separate protected intent
record must itself have a reviewed durable publication contract. A file record and
engine import/container creation are not one atomic transaction. Retain the existing
owned-ID inventory, explicit generation selection, two-generation bound and
ambiguity-stop reconciliation; no installer or intent-record location is selected.

## Disposition and next qualification gates

**API shape supported; adapter and provider behavior unqualified.** rustix's
explicit Linux resolution/descriptor APIs are a candidate for later dependency and
adapter review. This finding does not select production storage, accept ADR-106,
prove no writable alias, or discharge the independent unresolved resume gate.

A future bounded Linux file-admission proof needs reviewed code and actual cases:

| Case | Required oracle |
| --- | --- |
| Positive secret mount and Linux-owned generation | Exact selected source, mapped owner/mode, protected ancestors/mounts, valid same-object bounded read before bind; qualify each alternative separately. |
| Resolution and object errors | Final and parent symlink, magic-link/escape, wrong owner/group/mode/special bits, FIFO and unexpected type rejected without fallback; no unowned device experiments. |
| Mutation and aliases | Path replacement cannot switch the already-open object; any permitted untrusted writer/alias blocks immutable-generation qualification. Metadata stability alone cannot pass. |
| Resource and parser boundaries | Missing, zero/malformed, valid exact limit and over-limit/growing input; descriptor cleanup on all failures, strict grammar rejection before bind; no unmeasured I/O deadline claim. |
| Unsupported API/policy | Kernel/seccomp/flag/filesystem failures stop rather than weaken constraints. |
| Publication and crashes | Existing generation collision, partial write, owner/mode or sync error, interruption before/after rename/intent/engine mutation; select complete explicit state or stop for reconciliation. |

No runtime code, dependency graph, test fixture or kernel setting changed; no
machine/container start, build, key generation, trust change or filesystem failure
experiment was performed. Source/API review and proposed tests are not passing
provider evidence. Separate exact-code, dependency/license/advisory, architecture/
storage-risk and applicable execution permissions remain required. A0/T01-T16,
A1-A5, ADR-103 and #65 retain their gates; #40's separate production-dependency
review deadline remains 2026-11-18.
