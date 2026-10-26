# A0: Linux server-key file-admission API review

Status: Research / Proposed adapter outline; no dependency or production storage adoption.
Owner: project maintainer. Recorded: 2026-10-23. Related #40/#49;
[container plan](local-owner-authentication-container-plan.md),
[ADR-106: bounded acceptance](../architecture/ADR-106-local-container-transport.md),
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
| Linux key/file review could proceed while resume was unresolved at the 2026-10-23 review | Independent API prerequisite in the container plan. | Historical full-resume gate; the [2026-10-24 amendment](../architecture/ADR-106-local-container-transport.md#development-proof-interruption-amendment-2026-10-24) now requires invalidation/rerun for interrupted disposable diagnostics. No executable or supported-authentication readiness is established. |
| A path, owner/mode or read-only mount proves immutable content | Unsupported by file/mount semantics. | Require protected topology and alias/writer evidence; stop if untrusted mutation remains possible. |
| A safe Rust signature establishes the complete security adapter | Unsupported: API shape and policy are different. | Review the whole admission sequence and actual provider failure cases before qualification. |
| Documentation or a merged proposal selects storage/dependencies | Not authorized by merging alone. | Separate explicit ADR-106 acceptance on 2026-10-23 covers disposable mounted secrets only; production storage and dependency adoption remain gated. |

## The two storage alternatives

| Boundary | Accepted disposable Podman mounted secret | Linux-owned generation files alternative |
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

The initial API review inspected wrapper source, not a resolved dependency graph
or package-license/advisory assessment. The
[2026-10-26 dependency research](#dependency-research-2026-10-26) below now records
that bounded candidate review and its limitations. No crate is added or selected
for runtime adoption; minimum-version execution remains unqualified.
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
   file and the reviewed container-side owner/group and mode. For the accepted diagnostic
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
adapter review. The maintainer separately accepted ADR-106 for the bounded proof
on 2026-10-23. This finding does not select production storage, prove no writable
alias. The full resume gate was unresolved at that review date; the
[2026-10-24 amendment](../architecture/ADR-106-local-container-transport.md#development-proof-interruption-amendment-2026-10-24)
changes only the disposable diagnostic requirement, not supported authentication.

The [focused reader-slice proposal](local-owner-authentication-linux-reader-slice.md)
makes the next private implementation boundary and test oracles concrete. It
retains separate dependency/execution review and does not implement the reader
or select the Linux-owned-file provisioning alternative.

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

## Development-proof lifecycle update, 2026-10-24

[ADR-106 now records the accepted interruption/rerun concession](../architecture/ADR-106-local-container-transport.md#development-proof-interruption-amendment-2026-10-24).
Any observed or suspected host/VM pause invalidates the complete disposable run;
stop/reconcile and rerun from a fresh instance. Earlier references in this dated
research/result to the full diagnostic resume barrier describe the prior gate.
This amendment supplies no automatic pause detection, safe-file evidence,
supported authentication/suspend qualification or integrated-test permission.

## Dependency research, 2026-10-26

**Recommendation for review:** use exact `rustix = "=1.1.5"`, defaults disabled,
features `fs` and `std`, as a Linux-target-only dependency of AUTH-001. This is a
source/metadata recommendation, not a manifest change or permission to implement,
execute or distribute the reader. Baseline: `a773b38931a67d62d44464b2112c5401937b1853`.

The [published package](https://crates.io/crates/rustix/1.1.5) was the registry's
latest stable non-yanked release when checked, published 2026-09-16. Its archive
checksum matched registry metadata and its packaged VCS record points to the
already inspected `287214b889865d8e1406a0ee71cc409b6f6191c8` source. This establishes
artifact correspondence, not a complete source/soundness audit.

### Feature and graph result

An external metadata-only candidate copied the existing experimental manifest
and lock, added the proposed target dependency, and let Cargo resolve it. Cargo
1.93.0 metadata/tree commands ran; no dependency build script, compiler build,
Linux kernel test or service was executed. Source placeholders affected no
dependency declarations; the checkout's manifests and locks remain unchanged.

| Scope | Observed result |
| --- | --- |
| Complete registry lock inventory | 94 to 98 entries; four additions below, no existing version removed or replaced. |
| `x86_64-unknown-linux-gnu` normal/build package tree | 49 to 52 unique registry packages; adds rustix, bitflags and linux-raw-sys. This excludes development-only edges. |
| rustix effective features | `alloc`, `fs`, `std`; `std` supplies owned-descriptor/std interoperability and implies allocation support. |
| linux-raw-sys effective features | `auxvec`, `elf`, `errno`, `general`, `ioctl`, `no_std`. These come from rustix, not extra Kehila requests. |
| errno | Added to the cross-target resolved inventory; absent from the inspected default x86_64 Linux raw normal/build tree. |
| `x86_64-pc-windows-msvc` filtered metadata | No active rustix node through the Linux-only root dependency. This is resolution evidence, not a Windows build. |

The [manifest and build selector](https://github.com/bytecodealliance/rustix/blob/287214b889865d8e1406a0ee71cc409b6f6191c8/build.rs)
choose the Linux raw backend for this ordinary x86_64 Linux configuration.
`use-libc`, explicit backend cfgs, Miri, architectures and feature unification can
change that selection. No backend-override Rust flags were present during this
metadata run, but the build selector itself was not executed. Cargo's filtered
metadata can retain cfg/optional alternatives; use the target tree and selector
source together, not every metadata node as proof of compiled reachability.
Do not enable `all-apis`, `time`, `thread`, `net` or experimental/runtime features
for the reader without another feature, license and reachability review.

| New registry package | Declared Rust floor | Archive SHA-256 |
| --- | --- | --- |
| rustix 1.1.5 | 1.65 | `891efababe418670775f199f0d233d84843c227a0949a883ce15b37c78d6629d` |
| bitflags 2.13.2 | 1.56.0 | `3ded4057c258ba199e2d26386d3af3780957ecaee6c4ef4041c6b4b8b97c0b06` |
| linux-raw-sys 0.12.1 | 1.63 | `32a66949e030da00e8c7d4434b251670a91556f4144941d37452769c25d58a53` |
| errno 0.3.14 | 1.56 | `39cab71617ae0d63f51a36d69f866391735b51691dbda63cf6f96d042b63efeb` |

All four archive hashes matched their registry records. The inspected candidate
metadata declares no package floor above AUTH-001's Rust 1.88, but some packages
declare no floor; this is not a minimum-version build. Rustix documents a 1.77
minimum when `std` is disabled, distinct from its package's 1.65 declaration.
The production workspace's separate Rust floor is not changed or qualified.

### Actual license and distribution review

The four packaged plain `LICENSE-APACHE` texts were read and are byte-identical
(SHA-256 `a60eea817514531668d7e00765731449fe14d059d3249e0bc93b36de45f759f2`).
Recommend that plain Apache-2.0 grant for this scoped use; rustix/linux-raw-sys
explicitly offer it independently of their LLVM-exception alternative, while
bitflags/errno offer Apache-2.0 or MIT. Retain package attribution, applicable
copyright/source notices and the selected license in the adoption/distribution
artifacts. No separate packaged NOTICE was found; rustix/linux-raw-sys supply
COPYRIGHT files, and bitflags/errno source headers retain Rust Project attribution.
The existing runtime inventory/notices are unchanged because nothing is adopted.

**Source exception:** rustix's
[vDSO parser](https://github.com/bytecodealliance/rustix/blob/287214b889865d8e1406a0ee71cc409b6f6191c8/src/backend/linux_raw/vdso.rs)
identifies Linux-derived CC0-1.0 code. Its
[module cfg](https://github.com/bytecodealliance/rustix/blob/287214b889865d8e1406a0ee71cc409b6f6191c8/src/backend/linux_raw/mod.rs)
includes it only for `time`, `thread` or 32-bit `x86`, excluding this x86_64
`fs`/`std` candidate. Do not label all packaged source exclusively Apache-2.0.
The [CC0 legal text](https://creativecommons.org/publicdomain/zero/1.0/legalcode.en)
was also reviewed: its copyright waiver/fallback imposes no attribution or
share-alike condition, but supplies no patent/trademark grant or warranty.
That distinction does not change the excluded file into Apache-licensed code.
Broader features/architectures or source redistribution must retain its provenance
and revisit distribution coverage; no packaged CC0 text was present. Include the
CC0 text with that provenance in any broader source-notice bundle before shipment.

### Advisory, maintenance and reviewer findings

An OSV package/version batch on 2026-10-26 covered all **98** candidate registry
entries. No finding matched the four additions, but the full query returned
**RUSTSEC-2026-0386 for existing Tokio 1.53.1**. Both the experimental and production
locks contain that version. The
[RustSec record](https://rustsec.org/advisories/RUSTSEC-2026-0386.html) describes
`AsyncFd` constructor I/O unsoundness with possible memory corruption, and marks
1.53.3 or later patched on this release line. It supplies no CVSS/severity rating.
Direct `src/` and AUTH-001 source inspection found no AsyncFd call; the probe uses
Tokio TCP types. This is not a transitive reachability audit or proof of no
exposure. #40 owns explicit triage and a separately reviewed compatible patch;
do not call this graph clean, silently accept the risk, or replace the proposed
owned regular-file reader with AsyncFd registration. The
[upstream fix](https://github.com/tokio-rs/tokio/pull/8587) changes registration's
ownership contract and is not an incidental workaround under application
`unsafe_code = "forbid"`. The pre-existing rustls follow-up remains separate.

Bounded triage scope at the research baseline: the experiment enables Tokio's
`net` feature. In Tokio 1.53.1's ordinary Unix configuration, that makes the
AsyncFd API available; API availability is not evidence of calling an affected
constructor. Immediate reverse dependency edges in the committed lockfiles are:

- **AUTH-001 (4):** kehila_local_tls_probe 0.1.0, hyper 1.11.1, hyper-util 0.1.21
  and tokio-rustls 0.26.6.
- **Root workspace (10):** task_worker 0.1.0, axum 0.8.9, hyper 1.11.1,
  hyper-util 0.1.21, mongodb 2.8.2, tokio-rustls 0.24.1, tokio-util 0.7.19,
  tower 0.5.3, trust-dns-proto 0.21.2 and trust-dns-resolver 0.21.2.

These counts include the local packages and describe lockfile adjacency, not an
executed or target-filtered transitive call graph. Use them to scope #40's source/
feature review and separately verified patch. Constructor invocation, descriptor
ownership/lifetime and exposure still require assessment; P1 remains triage
priority, with upstream severity unrated and project severity pending triage.

The four upstream repositories were not archived/disabled. Latest inspected
default-branch commits were rustix 2026-10-15, bitflags 2026-10-17,
linux-raw-sys 2026-03-30 and errno 2026-08-06. These are bounded maintenance
observations, not support/security guarantees; keep monthly and advisory-triggered
review under the dependency policy. Re-query advisories before actual adoption.

Self-review and a separate code-review agent checked artifact/graph/license/API
claims. The agent identified the vDSO exception and an allocation boundary worth
preserving: `openat2` converts its
[path argument](https://github.com/bytecodealliance/rustix/blob/287214b889865d8e1406a0ee71cc409b6f6191c8/src/path/arg.rs)
before the syscall. String/path conversion can allocate separately from the
fallible read buffer. A borrowed, setup-validated CStr avoids that conversion;
review that input ownership deliberately without inventing a new path grammar.

**Disposition:** the bounded dependency research is complete enough to review
this exact candidate. Adoption still needs maintainer disposition, retained
notices/inventory updates, the Tokio finding's disposition, and permission for
single-job builds, minimum-version/platform checks and actual Linux reader
oracles. No passing provider, descriptor, immutable-generation or authentication
qualification follows from metadata or this review.
