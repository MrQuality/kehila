# A0: proposed Linux credential-reader implementation slice

Status: **Proposed for maintainer review**, 2026-10-25. Related #40/#49 W2.
This makes the next step in the [credential-admission handoff](local-owner-authentication-credential-admission.md)
concrete using the [Linux file API review](local-owner-authentication-linux-files.md).
No code, dependency, production storage or execution permission is adopted here.

## Outcome and scope

Implement one private Linux-only reader in the standalone AUTH-001 experiment:
constrained descriptor-relative open, metadata admission and bounded same-object
read, followed by the existing strict bundle decoder/profile checker. Keep the
public diagnostic examples and production worker unchanged. A reader success
means a bounded profile/key-pair snapshot under explicit topology premises; it
does not prove immutable storage, issuer trust or ongoing certificate validity.

The accepted first-proof storage direction is a unique disposable Podman mounted
secret under [ADR-106](../architecture/ADR-106-local-container-transport.md).
Synthetic Linux files can exercise reader mechanics without qualifying that
mount. Linux-owned generation creation/publication, intent durability, engine
operations, renewal, listeners, system-clock acquisition and integrated browser
TLS remain separate work. Windows is the MVP client platform; this adapter runs
in the Linux backend, not on a Windows server.

## Inputs, ownership and result

The proposed internal boundary accepts a borrowed directory descriptor, one
explicitly selected relative generation path and the reviewed container-side
UID/GID plus exact `0400` policy. These are trusted setup inputs, never request
fields, ambient working-directory authority or a guessed newest generation.
The caller owns the root descriptor throughout the call. Root acquisition,
protected ancestors, mount identity and absence of untrusted writable aliases
must have separate positive evidence before integrated admission is claimed.
The function cannot establish those premises from file metadata alone.

Place the reader below the private bundle module, with Linux-specific code
behind `cfg(target_os = "linux")`. Keep visibility limited to the internal
coordinator/tests; do not export a public admission API or add a serving caller.
The coordinator supplies time to the existing profile check, without obtaining
wall time in this slice. Review the minimum parent-module visibility necessary
for composition instead of exposing parser types outside the private boundary.

The reader owns the returned file descriptor once, borrows it for `fstat` and
reads, and releases it on every return path. Do not clone descriptors, convert
raw ownership, reopen by path or retain the descriptor in the admitted result.
Return a zeroizing encoded buffer to the private coordinator; drop it after
decode/profile admission. Retained TLS/library copies remain subject to their
existing limits; buffer cleanup is not a claim of complete secret erasure.

Use typed failure categories for open/resolution, stat, object/owner/group/mode,
read, size and allocation, preserving the existing decode/profile categories.
Diagnostics identify the failed stage without printing key bytes, bundle text
or sensitive configured paths. Errors return no usable credential result.

## Opening and bounded reading

Carry forward the exact [serving-reader sequence](local-owner-authentication-linux-files.md#proposed-serving-reader-sequence):
`openat2` with BENEATH/NO_SYMLINKS/NO_MAGICLINKS and read-only,
CLOEXEC/NOFOLLOW/NONBLOCK, followed by descriptor metadata checks. Compare all
permission/special bits and exact configured container UID/GID; require a regular
file. Do not add NO_XDEV blindly, because an intended secret bind mount can cross
a mount boundary. Missing/denied APIs or unsupported policy fail closed with no
fallback, stripped flags or changed seccomp/capabilities.

Reserve at most 65,537 encoded bytes fallibly before reading: 65,536 bytes plus
one overflow byte. Track bytes actually read, not metadata size. Handle short
reads and EINTR, stop at EOF or overflow, and reject overflow before decoding.
Exactly 65,536 bytes remain admissible to the unchanged grammar/profile checks;
zero bytes must fail admission. Never grow the input buffer beyond the cap.
The byte bound does not provide a filesystem-I/O deadline. NONBLOCK avoids a
FIFO-open wait but neither bounds regular-file I/O nor makes hostile device
opens safe; trusted topology remains a prerequisite.

The pinned [rustix wrapper](https://github.com/bytecodealliance/rustix/blob/287214b889865d8e1406a0ee71cc409b6f6191c8/src/fs/openat2.rs)
returns an owned descriptor through a safe API. Rust's
[OwnedFd contract](https://doc.rust-lang.org/std/os/fd/struct.OwnedFd.html)
supplies scoped descriptor ownership; Linux documents the
[resolution restrictions](https://man7.org/linux/man-pages/man2/openat2.2.html)
and [short-read/EINTR behavior](https://man7.org/linux/man-pages/man2/read.2.html).
These source contracts are not observations of the actual provider/kernel policy.

## Dependency gate and focused delivery

1. Review exact rustix candidate/feature/backend selection and the resulting
   Linux target graph before adoption, including actual licenses/notices,
   maintenance, advisory dispositions and compatibility with the experiment's
   Rust floor. The prior 1.1.5 wrapper review is a candidate, not a completed
   package/graph audit or a minimum-version build. Prefer the smallest applicable
   target-specific feature surface; retain application `unsafe_code = "forbid"`.
2. Add the private reader and negative/boundary tests in a focused implementation
   commit after review. Reuse the decoder's 64-KiB ceiling rather than maintain
   an independent policy constant. Do not hand-edit the lockfile or production
   manifests. Record actual dependency changes separately from reader behavior.
3. Qualify real Linux descriptor behavior and targeted mutations in an approved
   execution phase. Ordinary Windows checks cannot pass Linux-only cases by
   skipping them. Record supported kernel/target and which properties were
   observed; mocked reads prove only bounded-loop/error handling.

## Required oracles before reader qualification

All cases below are **unexecuted**; they define the next implementation's checks.

| Case | Required result |
| --- | --- |
| Positive protected regular file | Exact selected file and UID/GID/`0400`, bounded bytes, unchanged decoder/profile outcome. Root remains caller-owned; opened-file descriptor is released. |
| Resolution/object policy | Missing file, parent/final symlink, escape/magic-link, directory/FIFO, wrong UID/GID or any permission/special-bit mismatch fails without permissive fallback. No hostile-device experiment. |
| Byte/read errors | Short reads and injected EINTR preserve byte order; zero/malformed, 65,536-byte grammar-valid, 65,537-byte and growing-input cases enforce the real read cap; I/O/allocation failure yields no credential. |
| Same-object ownership | Controlled path replacement after open cannot switch the selected descriptor's contents; check cleanup on success and each failure. A writable alias demonstrates the remaining mutability risk, never immutable-generation qualification. |
| Denied/unsupported API | Real Linux API success plus deterministic failure-path checks distinguish unsupported/denied lookup; no flag stripping or fallback. Actual provider seccomp/mount behavior stays a separate gate. |
| Composition | File, decode or profile failure cannot produce a checked generation; valid input preserves existing certificate/key profile checks and snapshot-only limits. No listener is introduced to claim pre-bind integration evidence. |

Targeted mutations should remove a resolution restriction, weaken exact mode
checking, bypass the overflow byte and substitute a reopened path. Each needs
an independent behavior oracle, followed by a restored-source passing rerun.

## Execution boundary and remaining decisions

This proposal needs maintainer review before the code/dependency slice. Builds,
actual Linux/provider fixtures and mandatory executable commit gates need fresh
resource authorization and RAM checks; run serially with one build job and
external artifacts. A source-only proposal needs no such heavy phase.

Review exact dependency/feature changes and test setup before execution. Provider
mount topology, root acquisition, writable-alias exclusion, provisioning/intent
durability, ingress, expiry and integrated TLS remain open. Any observed or
suspected diagnostic pause invalidates the run under ADR-106's accepted fresh
rerun rule. No trust/security setting changes or A1-A5 implementation are included.
A0/T01-T16, ADR-103 and #65 remain unqualified; #40's separate production
dependency disposition and 2026-11-18 review deadline remain unchanged.
