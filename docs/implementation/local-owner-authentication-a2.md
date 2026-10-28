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

## Remaining integration gates

Qualify owned hash-worker cancellation, deadlines, join and saturation; measure
release verification-plus-replacement latency and peak memory. Apply the reviewed
new-password blocklist before enrollment/replacement, then implement bounded
session/pre-session registries and rate admission. Login verification must not
reapply a newer blocklist to an existing password.

Connect credentials to the reviewed PostgreSQL fences and runtime generation,
then qualify actual browser/CLI behavior. No HTTP caller or production worker
currently uses these primitives. Minimum Rust 1.88 execution, Linux credential
measurements and integrated container/client evidence remain unqualified.
