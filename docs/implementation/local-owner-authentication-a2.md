# A2: credential core and bounded admission

Status: implementation in progress, 2026-10-28. This is an internal part of the
[owner-login delivery](local-owner-authentication-delivery.md), not a completed
login capability or production authentication implementation.

## Implemented primitive boundary

The standalone experiment owns password normalization, fixed-profile Argon2id
hashing and random-secret encoding. Password input is at most 1,024 UTF-8 bytes;
NFC output must contain 15–128 Unicode scalars. There is no trimming, case folding
or compatibility normalization. Policy one pins Unicode 18.0.0 tables; upgrading
those tables requires compatibility review.

Stored verifiers must use policy one, Argon2id/v19, the exact canonical parameter
string `m=65536,t=3,p=1`, a 16-byte salt and 32-byte output, within 512 ASCII bytes.
Unsupported storage fails unavailable before workspace allocation. The hasher
constructs its costs independently; parsed costs never control allocation.
The caller owns a 64-MiB workspace, wipes its blocks after each computation and
retains its allocation for sequential verification and replacement.

Random secrets use 32 bytes from the operating system and exactly 43 canonical,
unpadded Base64url bytes. Stored comparisons use fixed-size SHA-256 digests and
constant-time equality. Random-source failure returns no credential and has no
fallback. Secret wrappers expose no formatting, cloning or serialization API.
This does not yet establish registry behavior or authorize storing raw bearers.

Owned input/output/workspace buffers are wiped. Library temporaries, allocator
history, caller copies and process termination remain outside a universal
physical-erasure guarantee. The [dependency review](../../experiments/AUTH-001/DEPENDENCIES.md#credential-primitive-selection-2026-10-28)
records the selected APIs, licenses, parser pitfalls and advisory scope.

## Observed primitive verification

On Windows with Rust 1.93, the restored standalone source passed 56 unit and
20 serial transport tests. Credential checks include a full PHC known answer
independently computed by the Argon2 C implementation, normalization/size limits,
duplicate PHC parameter rejection, canonical token decoding and random-source
failure. Clippy passed with warnings denied.

Four targeted mutations were detected: removing NFC, admitting duplicate PHC
parameters, changing the actual Argon2 time cost while retaining its label, and
omitting workspace wiping. Source was restored and the suite rerun. These are
primitive observations, not completed authentication cases T01–T16.

## Owned hash worker

One owned thread runs at a time, with no waiting queue. Dropping a request marks
only its job cancelled; the slot remains occupied until the actual thread is
joined. Completed work is joined before reuse. Shutdown stops admission, marks
cancellation and joins outside request execution, shared locks and SQL fences.
Thread-spawn failure drops its captured inputs; a panic is reported and joined.

Password replacement verifies the current password, obtains a fresh salt and
hashes the replacement sequentially with one workspace. Cancellation/deadline
checks between stages prevent starting later work. The receiving coordinator
must apply its absolute timeout and drop the pending future; an active Argon2
call cannot be interrupted. Dropping the future never detaches the worker.
The caller must admit a new password against the blocklist before this job;
that typed integration remains unfinished.

Thirteen worker tests passed, including real verification/replacement, wrong
password, spawn failure, panic, shutdown, retained capacity after abandonment
and cancellation between the actual replacement stages. Removing either
inter-stage cancellation check failed its targeted regression; restored tests
passed. This does not yet qualify a live request coordinator.
The restored worker source passed 69 unit and 20 serial transport tests, plus
Clippy with warnings denied. An interrupted final run had no completed result;
these totals come from its completed resumed run.

For the worker implementation committed as `4b95901`, five Windows/Rust 1.93
release samples measured the real verify-plus-replacement
job through workspace cleanup and worker join: 188.700–195.864 ms, median
189.390 ms. Observed process peak working set was 69.54–69.72 MiB. Peak counters
were polled every 20 ms; the process included the test harness and a subsequent
verification, so this is an observed process high-water mark, not an allocation
trace or Linux/container RSS. The fixed workspace remains 64 MiB. Test-process
wall times were 326–401 ms; these are not full HTTP/database request latencies.
The initial measurement test passed but its harness could not obtain exit
status; the process harness was corrected before collecting these five samples.
No cost or deadline was relaxed. Linux measurements and real-request budget
qualification remain pending.
The subsequent typed blocklist boundary and shared salted-stage extraction have
not been remeasured; these numbers remain a source-specific component baseline.
The measurement binary also compiled unconnected blocklist mechanics tests;
only the named worker case ran in each sample.

## External new-password blocklist, policy one

The blocklist module admits new passwords before worker scheduling. Its opaque
admitted-password type is required by new-verifier and replacement jobs;
existing-password verification takes normalized input directly. Changing a list
therefore does not silently deny an existing password. This boundary still has
no HTTP, enrollment or recovery coordinator caller.

Policy one selects the historical
[Probable-Wordlists source](https://github.com/berzerk0/Probable-Wordlists/blob/2df55facf06c7742f2038a8f6607ea9071596128/Real-Passwords/Top304Thousand-probable-v2.txt).
Its source SHA-256 is
`0c6d3eec0406f02a47f3b32ded1548db9d02440d6243b216bd952724bbec6af7`.
All 303,872 LF-delimited rows are ASCII. Preserve exact bytes, select complete
15–128-character rows, SHA-256 each whole row, deduplicate, sort the 32-byte
digests lexicographically and concatenate without a header. ASCII is unchanged
by the pinned NFC policy. The result has 2,097 entries, 67,104 bytes and SHA-256
`4a2ec43e1b737bfae06eb3e4aec46993344c28dd8aa959f5011c378bad7c2fc8`.
Two independent computations reproduced these identities.

This is limited historical common-password coverage based on cross-list
occurrences. It does not establish recent or comprehensive compromised-password
coverage. The source and derived asset retain their
[CC BY-SA 4.0 obligations](https://github.com/berzerk0/Probable-Wordlists/blob/2df55facf06c7742f2038a8f6607ea9071596128/License.txt),
distinct from application code. Attribution, full supplied grant and transformation
provenance accompany the external local-test asset. Neither corpus nor derived
asset is committed or bundled with the application; supported distribution and
installation remain unqualified. An additional license-website download returned
HTTP 403; the full upstream-supplied legal text was already retained.

The decoder caps all profiles at 4,096 entries, checks exact size/count/hash and
rejects unsorted or duplicate digests. Production policy-one construction pins
the expected count/hash in code. Matching is an exact normalized whole-password
lookup; no trimming, case folding or substring matching. Submitted digest scratch
is zeroized. Missing/invalid configured data must fail startup closed; no
allow-all fallback exists. The bounded production file reader is not connected
yet, so file admission and startup failure remain integration gates.

The opt-in `selected_external_corpus_matches_pin_and_admits_by_exact_password`
test reads at most 131,073 bytes from `AUTH001_BLOCKLIST_FIXTURE`, verifies the
actual pinned asset and tests listed/unlisted choices. Run it explicitly with
`--include-ignored` and a separately provisioned external asset. Ordinary CI
ignores this asset test; its synthetic tests qualify mechanics only. Local
execution against the real asset passed all five blocklist tests. Lookup and
asset-checksum mutations were detected and source restored.
The typed boundary additionally tests new-verifier creation and existing-password
verification, including a deliberately blocked current password that still
verifies. Creation and replacement share the guarded salt-to-hash stage;
cancelling during randomness prevents either from starting its next hash. Its
targeted guard mutation was detected. Final restored local verification, with
the external asset explicitly selected, passed 77 unit and 20 serial transport
tests, formatting and Clippy with warnings denied. Ordinary CI runs the synthetic
subset and leaves the one external-asset test ignored.

## Password-attempt admission engine

The pure admission state has separate login and authenticated-management buckets,
each refilling at five attempts per minute with burst five. Integer nanosecond
credit preserves partial refill, caps idle accumulation and rejects backwards
time without rewinding its observation. No client-keyed map or waiting queue is
allocated. A two-token burst uses the same arithmetic for later pre-session
allocation, whose registry/atomic allocation integration is still unfinished.

Four controlled-time tests cover exact burst/refill boundaries, capped idle
credit, backwards time and allowance independence. The threshold mutation failed
its regression; restored source passed. The actual server must own these buckets
once per installation and charge at the accepted ordering points. This engine
does not yet qualify session/CSRF validation before management charging, busy
worker handling, HTTP 429 or concurrent request admission.

Final selected-asset verification passed 81 unit and 20 serial transport tests,
formatting and Clippy on Windows/Rust 1.93. A
[hosted run at the earlier blocklist source](https://github.com/MrQuality/kehila/actions/runs/40634853337)
failed Rust 1.99 Clippy's constant `chunks_exact` lint. The decoder now uses safe
[fixed-array chunks](https://doc.rust-lang.org/stable/std/primitive.slice.html#method.as_chunks),
available since Rust 1.88; exact prior length admission ensures no remainder.
[Hosted verification of correction `a891c61`](https://github.com/MrQuality/kehila/actions/runs/40635696198)
passed. The earlier failed run remains historical evidence. Local success and
API availability do not qualify execution on the minimum compiler.

## Session candidate registry

The unconnected registry owns at most 64 unpublished, active or retained session
slots. Each entry binds installation, owner, runtime/auth generations, kind and
public ID to a bearer digest; browser entries retain an independent CSRF secret.
Lookup returns a candidate reference, never a permission verdict. Publication
requires a still-registered unpublished entry and a current matching snapshot.
The coordinator must obtain that snapshot from the real fenced database read;
the Rust value cannot establish its provenance, grants or connection ownership.

Captured references share phase/activity state through a per-entry mutex.
Revocation and retirement never return to active. Admission scans at most 64
entry states for expiry, duplicates and the oldest audit-confirmed logout;
active or unconfirmed entries are not evicted. Retirement precedes slot removal,
and reclamation orders by revocation time then public ID. Unpublished entries
cannot be looked up, and failed issuance cleanup denies later publication.

Monotonic inactivity/absolute deadlines are 30 minutes/12 hours. Only the explicit
successful-business transition renews still-active state. CSRF bootstrap returns
the existing browser value without renewal, including eligible own-logout retry
state. Its actual HTTP metadata/cookie checks remain unimplemented. The current
revocation method models own logout only; revoke-other batching and retry rights
remain separate unfinished work.

Explicit stop closes a shared runtime latch before releasing entries. Source
review found that owner destruction initially left captured references locally
eligible; a regression failed before the fix. Registry destruction now performs
the same irreversible stop. This is local lifetime enforcement, not evidence of
actual PostgreSQL advisory-loss detection or output suppression.

Fourteen controlled-time/state tests cover publication and failed issuance,
kind/duplicate rejection, exact expiry and activity, binding/generation changes,
revocation/retained references, bounded capacity, confirmed-logout ordering,
CSRF retrieval, stop and owner destruction. Revocation, audit-dependent capacity,
expiry and runtime-latch mutations were detected; restored final verification
passed 95 unit and 20 serial transport tests with the external corpus selected,
formatting and Clippy. The 64-slot bound does not bound every captured allocation:
real coordinators must also bound reference retention and owned task lifetime.
No registry guard spans an await in this pure component.

## Remaining integration gates

Qualify the actual coordinator's timeout and retained worker capacity, and
measure the Linux/container and complete request budgets. Connect blocklist
file admission and enrollment/recovery callers, then complete bounded
pre-sessions and actual registry/rate coordinators. Login verification must not
reapply a newer blocklist to an existing password.

Connect credentials to the reviewed PostgreSQL fences and runtime generation,
then qualify actual browser/CLI behavior. No HTTP caller or production worker
currently uses these primitives. Minimum Rust 1.88 execution, Linux credential
measurements and integrated container/client evidence remain unqualified.
