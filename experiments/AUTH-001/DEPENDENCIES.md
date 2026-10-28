# Standalone probe dependency review

Owner: project maintainer. Initial review 2026-10-19; Tokio patch review 2026-10-26; Linux reader review 2026-10-27; credential primitive review 2026-10-28.
Scope: the locked AUTH-001 experiment. The matching root-workspace Tokio patch is
recorded in the canonical A0 disposition below.

## Purpose and selection

TLS: rustls 0.23.46 with the explicit ring provider; Tokio/Tokio-rustls own asynchronous sockets and deadlines; Hyper/Hyper-util and http-body-util provide bounded HTTP/1 handling. Bytes is the body representation. Test-only rcgen/time generate disposable certificates, including renewal and invalid-certificate fixtures. All direct versions are exact; Cargo.lock fixes the complete registry graph.

These are maintained upstream Rust TLS/runtime/HTTP projects with published source and release histories. Pinning does not establish security: review monthly and immediately after upstream security notices under [DEP-001](../../docs/engineering/DEPENDENCIES.md). The maintainer owns requalification. The minimum declared package requirement reaches Rust 1.88; initial execution used 1.93.0 and the dated Linux reader tests used 1.95.0, not a qualified full-graph minimum-version build.

## License selection and distribution

Actual packaged license texts were inspected. Apache-2.0 is selected where offered, MIT for memchr/wasi, and the sole grant where no alternative exists. Ring retains its Apache/ISC and bundled once_cell/fiat notices. The packaged asn1-rs-impl 0.2.0 lacks its license files: its .cargo_vcs_info.json identifies upstream commit a20e5f7319c896737ad0f2557037817b91ad854f; the [root Apache text](https://github.com/rusticata/asn1-rs/blob/a20e5f7319c896737ad0f2557037817b91ad854f/LICENSE-APACHE) supplies the selected grant.

Explicit Unicode-3.0 compatibility/distribution review: unicode-ident combines a code-license alternative with the mandatory Unicode data license. Apache-2.0 AND Unicode-3.0 is selected. The inspected Unicode grant permits use, modification and redistribution, requires retention of its copyright/permission notice, and restricts advertising use of copyright-holder names. No third-party trademark is used as Kehila branding; the full Unicode grant is retained. This permits this experimental distribution under those obligations; it does not relicense the Unicode data. [Unicode policy](https://www.unicode.org/policies/licensing_policy.html).

[THIRD-PARTY-NOTICES.txt](THIRD-PARTY-NOTICES.txt) retains the selected texts and supplied notices, grouping byte-identical LF-normalized text once with package/source attribution. No third-party source is vendored. Include these notices if distributing probe binaries; re-review graph/license changes before distribution.

## Advisory scope

**Tokio patch, 2026-10-26:** the experiment now pins Tokio 1.53.3, a patched
release for [RUSTSEC-2026-0386](https://rustsec.org/advisories/RUSTSEC-2026-0386.html).
The separate root lock is patched in the same focused change. See the
[canonical A0 disposition](../../docs/implementation/local-owner-authentication-a0.md#advisory-follow-up)
for source review, verification and remaining limits. The 2026-10-23 query in
`advisories.json` remains historical; its new `tokio_patch` record covers only
Tokio 1.53.3, not a fresh whole-graph scan.

The [1.53.2/1.53.3 release notes](https://github.com/tokio-rs/tokio/blob/tokio-1.53.3/tokio/CHANGELOG.md)
were reviewed: AsyncFd constructor deprecation/new unsafe registration APIs,
shutdown and Windows handle lifetime fixes, synchronization/waker fixes, and
paused-time behavior are relevant compatibility review areas. No project API or
feature change is required. The downloaded archive SHA-256 matches both locks
(`ce3335fa71841cda333a58d7615b03901380ecf09d59b3296d21f8bbac0dde4e`);
the MIT license text is byte-identical to 1.53.1 after LF normalization, so only
the notice's version/source attribution changes. Existing monthly and immediate
security-notice review remains the update policy.

The 2026-10-19 package/version OSV query covered the then-locked 85 registry packages, including platform-specific and test-only entries. The subsequent profile adoption review recorded the then-94-package graph; the Linux reader update below brings the selected inventory to 98. It is a point-in-time package advisory query, not a complete source/reachability audit or a qualification of the existing worker. The pre-existing worker rustls 0.21.12 follow-up remains under [#40](https://github.com/MrQuality/kehila/issues/40) and the [A0 plan](../../docs/implementation/local-owner-authentication-a0.md#advisory-follow-up).

## Linux reader dependency selection, 2026-10-27

The private reader selects exact **rustix 1.1.6**, defaults disabled, Linux-only
`fs`/`std` (`alloc` is transitively enabled). It supplies safe owned-descriptor
`openat2`, `fstat` and bounded `read` APIs without application unsafe code. The
root workspace is unchanged. Cargo adds rustix, bitflags 2.13.2, linux-raw-sys
0.12.1 and errno 0.3.14: **94 to 98 registry entries**. Linux metadata includes
rustix `alloc/fs/std`; Windows metadata has no rustix node. Lock inventory is not
target execution evidence, and declared package Rust floors (rustix 1.65,
bitflags/errno 1.56, linux-raw-sys 1.63) do not qualify a minimum-version build.

The [upstream 1.1.5-to-1.1.6 comparison](https://github.com/bytecodealliance/rustix/compare/v1.1.5...v1.1.6)
and actual published source were reviewed, including filesystem/backend fixes;
the new adoption uses 1.1.6 rather than freezing the earlier 1.1.5 research
candidate. Its packaged source identity is `4a563ad698b2c20ff848515821023d2265a788d0`.
The APIs used by this reader retain their reviewed signatures. No general
source/reachability audit or observed runtime performance follows from that review.

Apache-2.0 is selected from each addition's actual packaged alternatives; the
identical grant text (SHA-256 `a60eea817514531668d7e00765731449fe14d059d3249e0bc93b36de45f759f2`)
is retained once with all four source attributions in the notices. Archive SHA-256
values were checked against Cargo's lock checksums. The rustix vDSO parser retains
its [Linux-derived CC0 provenance](https://github.com/bytecodealliance/rustix/blob/4a563ad698b2c20ff848515821023d2265a788d0/src/backend/linux_raw/vdso.rs).
Its [module cfg](https://github.com/bytecodealliance/rustix/blob/4a563ad698b2c20ff848515821023d2265a788d0/src/backend/linux_raw/mod.rs)
excludes it for the reviewed x86_64 Linux `fs/std` selection. This is not a claim
that all packaged source is exclusively Apache-2.0. Broader features/architectures
or source redistribution require renewed distribution review and the provenance/
CC0 notice coverage described in the [research record](../../docs/implementation/local-owner-authentication-linux-files.md#dependency-research-2026-10-26).

The [new package/version observation](advisories-reader.json) queried all 98
selected registry entries on 2026-10-27 and returned no findings. It binds the
exact LF-normalized lock hash; it is not a root graph scan, platform/provider
qualification or guarantee against future advisories. Older `advisories.json`
records remain historical. Review monthly and immediately after security notices.
Compilation, actual Linux descriptor tests and minimum-version qualification
remain distinct evidence gates; none is claimed by these metadata results.

## Credential primitive selection, 2026-10-28

Argon2 0.6.0 requests `alloc/password-hash/zeroize` with defaults disabled;
unicode-normalization 0.1.26 requests no features. Existing ring 0.17.14 is
promoted with `std` for OS randomness/SHA-256, and subtle 2.6.1 with defaults
disabled for fixed-size comparison. The resolved graph has 112 registry packages:
14 additions and no removed or changed existing versions. Defaults disabled on
an edge do not disable transitively unified features; ring retains default/std/
alloc. No production manifest/lock changes.

All fourteen new published archive hashes match Cargo's generated lock. Actual
supplied grants select Apache-2.0, plus Unicode-3.0 for normalization. The packaged
Unicode grant was explicitly reviewed: retain its copyright/permission text,
avoid endorsement, and retain the code grant as well. These obligations permit
the experimental distribution; Unicode data is not relicensed. Actual selected
text variants and package attribution are retained in THIRD-PARTY-NOTICES.txt.
None of the fourteen supplied a separate top-level NOTICE file.

[The credential graph query](advisories-credential.json) returned no OSV findings
for all 112 package/versions on 2026-10-28, including test/target entries. It is
bound to the lock hash, not a source/reachability audit or production disposition.
Root advisory obligations remain separate. Inspected upstream repositories are
not archived/disabled. RustCrypto and normalization show October 2026 activity;
tinyvec September, typenum May, ring July. Subtle's latest observed push is
August 2024: a mature low-churn component, not proof of active support or
abandonment. Re-review monthly and on security notices under DEP-001.

New declared Rust minima fit the existing 1.88 floor: ctutils reaches 1.87;
the RustCrypto additions generally declare 1.85. Tinyvec declares no manifest
minimum; its README states 1.47. A full minimum-compiler build is still
unqualified. Current compiler execution must not be described as that build.

Two source/API pitfalls affect the implementation:

- Argon2's allocating hash path deallocates its workspace without wiping it,
  including with its optional `zeroize` feature. Use caller-owned pre-sized
  `Zeroizing<Vec<Block>>` and the with-memory API; explicitly wipe each block
  after computation and retain the owner through actual completion. PHC's old
  serialization helper is deprecated; use its supported string formatting.
- PHC 0.6.1 permits duplicate parameter names: lookup takes the first while
  Argon2 conversion takes the last. Version one admits only the exact canonical
  `m=65536,t=3,p=1` string, Argon2id/v19, salt16/output32 and bounded ASCII encoding.
  Hashing always uses independently constructed fixed parameters. Unsupported
  storage is unavailable, never a wrong-password verdict or unbounded hash job.

The fixed profile has an independent full-PHC known answer from the Argon2 C
implementation through argon2-cffi/bindings 25.1.0. It uses an explicitly public
synthetic password/salt, not an owner credential. Those reference tools are not
runtime dependencies. The OS-random primitive has a separate SHA-256 known answer;
no weak/randomness fallback is allowed. Ring permits a blocking first random
fill, so error handling alone cannot establish request deadlines.

Credential policy one pins Unicode 18.0.0 tables. Unicode stability excludes
then-unassigned scalars; table changes require credential-policy compatibility
review rather than silently changing existing passwords. Input/output and owned
hash workspace are wiped, but Unicode iterators retain internal char buffers and
PHC/provider types can make copies. No universal physical-erasure guarantee is
claimed. Independent worker ownership, blocklist/registry and actual HTTP/CLI
qualification remain distinct from these primitive/API observations.

Primary source: [Argon2 workspace](https://github.com/RustCrypto/password-hashes/blob/argon2-v0.6.0/argon2/src/block.rs),
[PHC parameters](https://docs.rs/phc/0.6.1/phc/struct.ParamsString.html),
[Argon2 parameters](https://docs.rs/argon2/0.6.0/argon2/struct.Params.html),
[Unicode stability](https://www.unicode.org/policies/stability_policy.html),
[reference API](https://argon2-cffi.readthedocs.io/en/stable/api.html#argon2.low_level.hash_secret).

## Linux PostgreSQL bridge selection, 2026-10-28

Linux-only direct pins are tokio-postgres 0.7.18 (`runtime`, defaults disabled)
and tokio-postgres-rustls 0.14.0 (`ring`, defaults disabled). The adapter reuses
the existing rustls/ring stack without native/system roots or another provider.
No production manifest/lock changes. The generated lock has 170 registry entries:
58 additions, no removed or changed existing versions. The actual Linux
normal/build graph selects 37 additions; Windows does not select this backend.

All 58 new archive checksums matched the generated lock. The 37 selected Linux
packages supply actual MIT grants; MIT is explicitly selected, including the
dual-licensed packages. Their grant bodies agree with the standard MIT terms;
individual copyright/source attribution and supplied COPYRIGHT notices remain
retained in THIRD-PARTY-NOTICES.txt. This is scoped dependency review, not a
complete transitive-source or operating-system image audit.

The other 21 entries are unselected platform metadata, not distribution-cleared
code. In particular the [Apple binding upstream statement](https://github.com/madsmtm/objc2/blob/7b1abfd750a2cacaea71d6a56ecfb83cb7de560b/LICENSE.md)
identifies unresolved SDK-derived distribution implications. No Apple backend,
vendored source or uploaded Cargo-source cache is part of this profile. Preserve
the target exclusion in actual build/artifact evidence before qualification;
platform activation, source redistribution or changed packaging requires a new
compatibility review. A lockfile name does not grant distribution rights.

[The 170-version OSV query](advisories-postgres.json) returned no findings on
2026-10-28. It covers package/version metadata, including inactive targets, not
source reachability or license clearance. New declared compiler requirements fit
the existing Rust 1.88 floor; minimum-compiler execution remains unqualified.
The driver and adapter upstream repositories were not archived/disabled and
showed October/May 2026 pushes respectively. These are maintenance observations,
not support guarantees. Re-review monthly and on security notices under DEP-001.

Ordinary explicit-root TLS and authenticated runtime login remain required;
the existing isolated trust/psql fixture does not qualify that transport. The
connection future must remain owned and polled independently of query results.
Dropping a transaction/query future is not proof of completed rollback. Native
connection-loss/cancellation and fenced write/uncertain-outcome qualification
are separate obligations before HTTP use.
See the [driver API](https://docs.rs/tokio-postgres/0.7.18/tokio_postgres/) and
[TLS adapter](https://docs.rs/tokio-postgres-rustls/0.14.0/tokio_postgres_rustls/).

## Locked registry inventory

| Package | Version | Selected grant |
| --- | --- | --- |
| [argon2](https://crates.io/crates/argon2/0.6.0) | 0.6.0 | Apache-2.0 |
| [asn1-rs-derive](https://crates.io/crates/asn1-rs-derive/0.6.0) | 0.6.0 | Apache-2.0 |
| [asn1-rs-impl](https://crates.io/crates/asn1-rs-impl/0.2.0) | 0.2.0 | Apache-2.0 |
| [asn1-rs](https://crates.io/crates/asn1-rs/0.7.2) | 0.7.2 | Apache-2.0 |
| [async-trait](https://crates.io/crates/async-trait/0.1.92) | 0.1.92 | MIT |
| [atomic-waker](https://crates.io/crates/atomic-waker/1.1.2) | 1.1.2 | Apache-2.0 |
| [autocfg](https://crates.io/crates/autocfg/1.5.1) | 1.5.1 | Apache-2.0 |
| [base64](https://crates.io/crates/base64/0.22.1) | 0.22.1 | MIT |
| [base64ct](https://crates.io/crates/base64ct/1.8.3) | 1.8.3 | Apache-2.0 |
| [bit-vec](https://crates.io/crates/bit-vec/0.9.1) | 0.9.1 | Apache-2.0 |
| [bitflags](https://crates.io/crates/bitflags/2.13.2) | 2.13.2 | Apache-2.0 |
| [blake2](https://crates.io/crates/blake2/0.11.0) | 0.11.0 | Apache-2.0 |
| [block-buffer](https://crates.io/crates/block-buffer/0.12.1) | 0.12.1 | Apache-2.0 |
| [bumpalo](https://crates.io/crates/bumpalo/3.20.3) | 3.20.3 | Unselected platform metadata; not distribution-cleared |
| [byteorder](https://crates.io/crates/byteorder/1.5.0) | 1.5.0 | MIT |
| [bytes](https://crates.io/crates/bytes/1.11.1) | 1.11.1 | MIT |
| [cc](https://crates.io/crates/cc/1.9.0) | 1.9.0 | Apache-2.0 |
| [cfg-if](https://crates.io/crates/cfg-if/1.0.5) | 1.0.5 | Apache-2.0 |
| [chacha20](https://crates.io/crates/chacha20/0.10.2) | 0.10.2 | MIT |
| [cmov](https://crates.io/crates/cmov/0.5.4) | 0.5.4 | Apache-2.0 |
| [const-oid](https://crates.io/crates/const-oid/0.10.2) | 0.10.2 | Apache-2.0 |
| [const-oid](https://crates.io/crates/const-oid/0.9.6) | 0.9.6 | MIT |
| [cpufeatures](https://crates.io/crates/cpufeatures/0.3.1) | 0.3.1 | Apache-2.0 |
| [crypto-common](https://crates.io/crates/crypto-common/0.2.2) | 0.2.2 | Apache-2.0 |
| [ctutils](https://crates.io/crates/ctutils/0.4.6) | 0.4.6 | Apache-2.0 |
| [data-encoding](https://crates.io/crates/data-encoding/2.11.1) | 2.11.1 | MIT |
| [der-parser](https://crates.io/crates/der-parser/10.0.0) | 10.0.0 | Apache-2.0 |
| [der](https://crates.io/crates/der/0.7.10) | 0.7.10 | MIT |
| [der](https://crates.io/crates/der/0.8.2) | 0.8.2 | Apache-2.0 |
| [der_derive](https://crates.io/crates/der_derive/0.7.3) | 0.7.3 | MIT |
| [der_derive](https://crates.io/crates/der_derive/0.8.0) | 0.8.0 | Apache-2.0 |
| [deranged](https://crates.io/crates/deranged/0.5.9) | 0.5.9 | Apache-2.0 |
| [digest](https://crates.io/crates/digest/0.11.3) | 0.11.3 | Apache-2.0 |
| [displaydoc](https://crates.io/crates/displaydoc/0.2.7) | 0.2.7 | Apache-2.0 |
| [errno](https://crates.io/crates/errno/0.3.14) | 0.3.14 | Apache-2.0 |
| [fallible-iterator](https://crates.io/crates/fallible-iterator/0.2.0) | 0.2.0 | MIT |
| [find-msvc-tools](https://crates.io/crates/find-msvc-tools/0.1.15) | 0.1.15 | Apache-2.0 |
| [flagset](https://crates.io/crates/flagset/0.4.7) | 0.4.7 | Apache-2.0 |
| [futures-channel](https://crates.io/crates/futures-channel/0.3.34) | 0.3.34 | Apache-2.0 |
| [futures-core](https://crates.io/crates/futures-core/0.3.34) | 0.3.34 | Apache-2.0 |
| [futures-sink](https://crates.io/crates/futures-sink/0.3.34) | 0.3.34 | MIT |
| [futures-task](https://crates.io/crates/futures-task/0.3.34) | 0.3.34 | MIT |
| [futures-util](https://crates.io/crates/futures-util/0.3.34) | 0.3.34 | MIT |
| [getrandom](https://crates.io/crates/getrandom/0.2.17) | 0.2.17 | Apache-2.0 |
| [getrandom](https://crates.io/crates/getrandom/0.4.3) | 0.4.3 | MIT |
| [hmac](https://crates.io/crates/hmac/0.13.0) | 0.13.0 | MIT |
| [http-body-util](https://crates.io/crates/http-body-util/0.1.3) | 0.1.3 | MIT |
| [http-body](https://crates.io/crates/http-body/1.1.0) | 1.1.0 | MIT |
| [http](https://crates.io/crates/http/1.5.0) | 1.5.0 | Apache-2.0 |
| [httparse](https://crates.io/crates/httparse/1.10.1) | 1.10.1 | Apache-2.0 |
| [httpdate](https://crates.io/crates/httpdate/1.0.3) | 1.0.3 | Apache-2.0 |
| [hybrid-array](https://crates.io/crates/hybrid-array/0.4.15) | 0.4.15 | Apache-2.0 |
| [hyper-util](https://crates.io/crates/hyper-util/0.1.21) | 0.1.21 | MIT |
| [hyper](https://crates.io/crates/hyper/1.11.1) | 1.11.1 | MIT |
| [itoa](https://crates.io/crates/itoa/1.0.18) | 1.0.18 | Apache-2.0 |
| [js-sys](https://crates.io/crates/js-sys/0.3.106) | 0.3.106 | Unselected platform metadata; not distribution-cleared |
| [lazy_static](https://crates.io/crates/lazy_static/1.5.1) | 1.5.1 | Apache-2.0 |
| [libc](https://crates.io/crates/libc/0.2.191) | 0.2.191 | Apache-2.0 |
| [libredox](https://crates.io/crates/libredox/0.1.25) | 0.1.25 | Unselected platform metadata; not distribution-cleared |
| [linux-raw-sys](https://crates.io/crates/linux-raw-sys/0.12.1) | 0.12.1 | Apache-2.0 |
| [lock_api](https://crates.io/crates/lock_api/0.4.14) | 0.4.14 | MIT |
| [log](https://crates.io/crates/log/0.4.34) | 0.4.34 | MIT |
| [md-5](https://crates.io/crates/md-5/0.11.0) | 0.11.0 | MIT |
| [memchr](https://crates.io/crates/memchr/2.8.3) | 2.8.3 | MIT |
| [minimal-lexical](https://crates.io/crates/minimal-lexical/0.2.1) | 0.2.1 | Apache-2.0 |
| [mio](https://crates.io/crates/mio/1.2.4) | 1.2.4 | MIT |
| [nom](https://crates.io/crates/nom/7.1.3) | 7.1.3 | MIT |
| [num-bigint](https://crates.io/crates/num-bigint/0.4.8) | 0.4.8 | Apache-2.0 |
| [num-conv](https://crates.io/crates/num-conv/0.2.2) | 0.2.2 | Apache-2.0 |
| [num-integer](https://crates.io/crates/num-integer/0.1.47) | 0.1.47 | Apache-2.0 |
| [num-traits](https://crates.io/crates/num-traits/0.2.19) | 0.2.19 | Apache-2.0 |
| [objc2-core-foundation](https://crates.io/crates/objc2-core-foundation/0.3.2) | 0.3.2 | Unselected platform metadata; not distribution-cleared |
| [objc2-system-configuration](https://crates.io/crates/objc2-system-configuration/0.3.2) | 0.3.2 | Unselected platform metadata; not distribution-cleared |
| [oid-registry](https://crates.io/crates/oid-registry/0.8.1) | 0.8.1 | Apache-2.0 |
| [once_cell](https://crates.io/crates/once_cell/1.21.4) | 1.21.4 | Apache-2.0 |
| [parking_lot](https://crates.io/crates/parking_lot/0.12.5) | 0.12.5 | MIT |
| [parking_lot_core](https://crates.io/crates/parking_lot_core/0.9.12) | 0.9.12 | MIT |
| [password-hash](https://crates.io/crates/password-hash/0.6.1) | 0.6.1 | Apache-2.0 |
| [percent-encoding](https://crates.io/crates/percent-encoding/2.3.2) | 2.3.2 | MIT |
| [phc](https://crates.io/crates/phc/0.6.1) | 0.6.1 | Apache-2.0 |
| [phf](https://crates.io/crates/phf/0.13.1) | 0.13.1 | MIT |
| [phf_shared](https://crates.io/crates/phf_shared/0.13.1) | 0.13.1 | MIT |
| [pin-project-lite](https://crates.io/crates/pin-project-lite/0.2.17) | 0.2.17 | Apache-2.0 |
| [pkcs8](https://crates.io/crates/pkcs8/0.11.0) | 0.11.0 | Apache-2.0 |
| [postgres-protocol](https://crates.io/crates/postgres-protocol/0.6.12) | 0.6.12 | MIT |
| [postgres-types](https://crates.io/crates/postgres-types/0.2.14) | 0.2.14 | MIT |
| [powerfmt](https://crates.io/crates/powerfmt/0.2.1) | 0.2.1 | Apache-2.0 |
| [proc-macro2](https://crates.io/crates/proc-macro2/1.0.107) | 1.0.107 | Apache-2.0 |
| [quote](https://crates.io/crates/quote/1.0.47) | 1.0.47 | Apache-2.0 |
| [r-efi](https://crates.io/crates/r-efi/6.0.0) | 6.0.0 | Unselected platform metadata; not distribution-cleared |
| [rand](https://crates.io/crates/rand/0.10.3) | 0.10.3 | MIT |
| [rand_core](https://crates.io/crates/rand_core/0.10.1) | 0.10.1 | MIT |
| [rcgen](https://crates.io/crates/rcgen/0.14.10) | 0.14.10 | Apache-2.0 |
| [redox_syscall](https://crates.io/crates/redox_syscall/0.5.18) | 0.5.18 | Unselected platform metadata; not distribution-cleared |
| [ring](https://crates.io/crates/ring/0.17.14) | 0.17.14 | Apache-2.0 AND ISC; bundled notices retained |
| [rusticata-macros](https://crates.io/crates/rusticata-macros/4.1.0) | 4.1.0 | Apache-2.0 |
| [rustix](https://crates.io/crates/rustix/1.1.6) | 1.1.6 | Apache-2.0 |
| [rustls-pki-types](https://crates.io/crates/rustls-pki-types/1.15.1) | 1.15.1 | Apache-2.0 |
| [rustls-webpki](https://crates.io/crates/rustls-webpki/0.103.16) | 0.103.16 | ISC |
| [rustls](https://crates.io/crates/rustls/0.23.46) | 0.23.46 | Apache-2.0 |
| [rustversion](https://crates.io/crates/rustversion/1.0.23) | 1.0.23 | Unselected platform metadata; not distribution-cleared |
| [scopeguard](https://crates.io/crates/scopeguard/1.2.0) | 1.2.0 | MIT |
| [sec1](https://crates.io/crates/sec1/0.8.0) | 0.8.0 | Apache-2.0 |
| [serde](https://crates.io/crates/serde/1.0.229) | 1.0.229 | Apache-2.0 |
| [serde_core](https://crates.io/crates/serde_core/1.0.229) | 1.0.229 | Apache-2.0 |
| [serde_derive](https://crates.io/crates/serde_derive/1.0.229) | 1.0.229 | Apache-2.0 |
| [sha2](https://crates.io/crates/sha2/0.11.1) | 0.11.1 | MIT |
| [shlex](https://crates.io/crates/shlex/2.0.1) | 2.0.1 | Apache-2.0 |
| [siphasher](https://crates.io/crates/siphasher/1.0.4) | 1.0.4 | MIT |
| [slab](https://crates.io/crates/slab/0.4.12) | 0.4.12 | Unselected platform metadata; not distribution-cleared |
| [smallvec](https://crates.io/crates/smallvec/1.16.3) | 1.16.3 | Apache-2.0 |
| [socket2](https://crates.io/crates/socket2/0.6.5) | 0.6.5 | Apache-2.0 |
| [spki](https://crates.io/crates/spki/0.7.3) | 0.7.3 | MIT |
| [spki](https://crates.io/crates/spki/0.8.1) | 0.8.1 | Apache-2.0 |
| [stringprep](https://crates.io/crates/stringprep/0.1.5) | 0.1.5 | MIT |
| [subtle](https://crates.io/crates/subtle/2.6.1) | 2.6.1 | BSD-3-Clause |
| [syn](https://crates.io/crates/syn/2.0.119) | 2.0.119 | Apache-2.0 |
| [syn](https://crates.io/crates/syn/3.0.9) | 3.0.9 | Apache-2.0 |
| [synstructure](https://crates.io/crates/synstructure/0.13.2) | 0.13.2 | MIT |
| [thiserror-impl](https://crates.io/crates/thiserror-impl/2.0.21) | 2.0.21 | Apache-2.0 |
| [thiserror](https://crates.io/crates/thiserror/2.0.21) | 2.0.21 | Apache-2.0 |
| [time-core](https://crates.io/crates/time-core/0.1.9) | 0.1.9 | Apache-2.0 |
| [time-macros](https://crates.io/crates/time-macros/0.2.32) | 0.2.32 | Apache-2.0 |
| [time](https://crates.io/crates/time/0.3.55) | 0.3.55 | Apache-2.0 |
| [tinyvec](https://crates.io/crates/tinyvec/1.13.3) | 1.13.3 | Apache-2.0 |
| [tls_codec](https://crates.io/crates/tls_codec/0.4.2) | 0.4.2 | Unselected platform metadata; not distribution-cleared |
| [tls_codec_derive](https://crates.io/crates/tls_codec_derive/0.4.2) | 0.4.2 | Unselected platform metadata; not distribution-cleared |
| [tokio-macros](https://crates.io/crates/tokio-macros/2.7.2) | 2.7.2 | MIT |
| [tokio-postgres-rustls](https://crates.io/crates/tokio-postgres-rustls/0.14.0) | 0.14.0 | MIT |
| [tokio-postgres](https://crates.io/crates/tokio-postgres/0.7.18) | 0.7.18 | MIT |
| [tokio-rustls](https://crates.io/crates/tokio-rustls/0.26.6) | 0.26.6 | Apache-2.0 |
| [tokio-util](https://crates.io/crates/tokio-util/0.7.20) | 0.7.20 | MIT |
| [tokio](https://crates.io/crates/tokio/1.53.3) | 1.53.3 | MIT |
| [try-lock](https://crates.io/crates/try-lock/0.2.5) | 0.2.5 | MIT |
| [typenum](https://crates.io/crates/typenum/1.20.1) | 1.20.1 | Apache-2.0 |
| [unicode-bidi](https://crates.io/crates/unicode-bidi/0.3.18) | 0.3.18 | MIT |
| [unicode-ident](https://crates.io/crates/unicode-ident/1.0.27) | 1.0.27 | Apache-2.0 AND Unicode-3.0 |
| [unicode-normalization](https://crates.io/crates/unicode-normalization/0.1.26) | 0.1.26 | Apache-2.0 AND Unicode-3.0 |
| [unicode-properties](https://crates.io/crates/unicode-properties/0.1.4) | 0.1.4 | MIT |
| [untrusted](https://crates.io/crates/untrusted/0.9.0) | 0.9.0 | ISC |
| [want](https://crates.io/crates/want/0.3.2) | 0.3.2 | MIT |
| [wasi](https://crates.io/crates/wasi/0.11.1+wasi-snapshot-preview1) | 0.11.1+wasi-snapshot-preview1 | MIT |
| [wasi](https://crates.io/crates/wasi/0.14.7+wasi-0.2.4) | 0.14.7+wasi-0.2.4 | Unselected platform metadata; not distribution-cleared |
| [wasip2](https://crates.io/crates/wasip2/1.0.4+wasi-0.2.12) | 1.0.4+wasi-0.2.12 | Unselected platform metadata; not distribution-cleared |
| [wasite](https://crates.io/crates/wasite/1.0.2) | 1.0.2 | Unselected platform metadata; not distribution-cleared |
| [wasm-bindgen-macro-support](https://crates.io/crates/wasm-bindgen-macro-support/0.2.129) | 0.2.129 | Unselected platform metadata; not distribution-cleared |
| [wasm-bindgen-macro](https://crates.io/crates/wasm-bindgen-macro/0.2.129) | 0.2.129 | Unselected platform metadata; not distribution-cleared |
| [wasm-bindgen-shared](https://crates.io/crates/wasm-bindgen-shared/0.2.129) | 0.2.129 | Unselected platform metadata; not distribution-cleared |
| [wasm-bindgen](https://crates.io/crates/wasm-bindgen/0.2.129) | 0.2.129 | Unselected platform metadata; not distribution-cleared |
| [web-sys](https://crates.io/crates/web-sys/0.3.106) | 0.3.106 | Unselected platform metadata; not distribution-cleared |
| [whoami](https://crates.io/crates/whoami/2.1.3) | 2.1.3 | MIT |
| [windows-link](https://crates.io/crates/windows-link/0.2.1) | 0.2.1 | Apache-2.0 |
| [windows-sys](https://crates.io/crates/windows-sys/0.52.0) | 0.52.0 | Apache-2.0 |
| [windows-sys](https://crates.io/crates/windows-sys/0.61.2) | 0.61.2 | Apache-2.0 |
| [windows-targets](https://crates.io/crates/windows-targets/0.52.6) | 0.52.6 | Apache-2.0 |
| [windows_aarch64_gnullvm](https://crates.io/crates/windows_aarch64_gnullvm/0.52.6) | 0.52.6 | Apache-2.0 |
| [windows_aarch64_msvc](https://crates.io/crates/windows_aarch64_msvc/0.52.6) | 0.52.6 | Apache-2.0 |
| [windows_i686_gnu](https://crates.io/crates/windows_i686_gnu/0.52.6) | 0.52.6 | Apache-2.0 |
| [windows_i686_gnullvm](https://crates.io/crates/windows_i686_gnullvm/0.52.6) | 0.52.6 | Apache-2.0 |
| [windows_i686_msvc](https://crates.io/crates/windows_i686_msvc/0.52.6) | 0.52.6 | Apache-2.0 |
| [windows_x86_64_gnu](https://crates.io/crates/windows_x86_64_gnu/0.52.6) | 0.52.6 | Apache-2.0 |
| [windows_x86_64_gnullvm](https://crates.io/crates/windows_x86_64_gnullvm/0.52.6) | 0.52.6 | Apache-2.0 |
| [windows_x86_64_msvc](https://crates.io/crates/windows_x86_64_msvc/0.52.6) | 0.52.6 | Apache-2.0 |
| [wit-bindgen](https://crates.io/crates/wit-bindgen/0.57.1) | 0.57.1 | Unselected platform metadata; not distribution-cleared |
| [x509-cert](https://crates.io/crates/x509-cert/0.2.5) | 0.2.5 | MIT |
| [x509-cert](https://crates.io/crates/x509-cert/0.3.0) | 0.3.0 | Apache-2.0 |
| [x509-parser](https://crates.io/crates/x509-parser/0.18.1) | 0.18.1 | Apache-2.0 |
| [yasna](https://crates.io/crates/yasna/0.6.0) | 0.6.0 | Apache-2.0 |
| [zeroize](https://crates.io/crates/zeroize/1.9.1) | 1.9.1 | Apache-2.0 |
| [zeroize_derive](https://crates.io/crates/zeroize_derive/1.5.0) | 1.5.0 | Unselected platform metadata; not distribution-cleared |

## 2026-10-23 verifier advisory follow-up

The refreshed 85-package query found [RUSTSEC-2026-0382](https://rustsec.org/advisories/RUSTSEC-2026-0382.html)
in previously locked rustls-webpki 0.103.15. Its certificate-path verifier may
accept distrusted emSign keys if configured trust anchors permit such a path.
The approved disposable profile uses its own explicit private root; tests start
with an empty root store and add a generated fixture root. This narrows observed
reachability, but is not a general absence-of-exposure proof for the configuration
helper or other graphs. No upstream CVSS rating was supplied.

The experimental lock now selects patched 0.103.16; no other registry package,
feature declaration or production lock changed. Reviewed the exact old/new
published archive diff and the [upstream fix](https://github.com/rustls/webpki/pull/546):
path construction checks a fixed distrusted-public-key set, including leaf and
anchor positions, and adds a typed rejection error. Upstream tests cover path
positions and alternate/cross-signed paths; source review is not local execution
of those upstream tests. Dependency declarations, Rust minimum and ISC license
text are unchanged; attribution now names the patched version. The patched
85-package query returned no findings, a point-in-time package check only.
Local validation passed: three unit and 14 transport tests, formatting and
Clippy with warnings denied. Normal shared gates remain required before publication. The 2026-10-19 clean result remains historical; #40's separate
production rustls disposition and 2026-11-18 review deadline are unchanged.

## 2026-10-23 decoder direct-use review

The text-only decoder directly pins data-encoding 2.11.1 with defaults disabled
and zeroize 1.9.1 with defaults disabled plus `alloc`. Metadata and normal/build
feature-tree review found 85 registry packages unchanged in count, 52 to 53
normal/build packages (data-encoding is newly reachable), and no additional
registry version or all-target unified feature change beyond the separately
reviewed webpki patch. Runtime-only data-encoding requests no features; zeroize
already has default/alloc through existing TLS dependencies. Test-inclusive
resolution additionally unifies data-encoding default/std/alloc. Defaults disabled
on our edge do not disable features requested elsewhere. Cargo generated the
two new root dependency edges. No production dependency is adopted.

Both locked archive checksums and actual selected license texts were checked:
MIT for data-encoding, Apache-2.0 for zeroize; their existing retained texts cover
this use. Their published manifests add no normal dependencies under these
requested features. Upstream path histories showed changes on 2026-10-14 for
[data-encoding](https://github.com/ia0/data-encoding) and
[zeroize](https://github.com/RustCrypto/utils/tree/master/zeroize); this is a
maintenance observation, not a security guarantee. The current patched package
query above covers these exact versions. Review monthly and on security notices.

Source/API review covered base64 padding/partial-write behavior and zeroizing
buffer ownership. Version 1.9.1's `Zeroizing` Debug is opaque, correcting the
proposal's broader statement; the bundle still exposes no Debug/Clone/serialization
interface. Allocator/process termination, caller copies and historical allocations
remain outside the erasure guarantee. This is bounded API review, not a complete
source/reachability audit. x509-parser is not promoted; full credential admission
and provider qualification remain separate.

## 2026-10-23 credential-admission schema adoption

Direct pins are x509-cert 0.3.0 and pkcs8 0.11.0 with defaults disabled, plus
sec1 0.8.0 with defaults disabled and `der`. The research-only declaration lacked
SEC1's required decoder feature; actual compilation exposed that setup gap.
Review of its manifest confirms `der` enables SEC1/DER zeroization, without
point/PEM/serde support or new package versions. The actual 94-package/version
set matches the candidate query, which returned no OSV findings on 2026-10-23.
This remains package/version evidence, not absence of reachable source defects.

Nine registry entries are new; existing package versions are unchanged. All nine
archive hashes match Cargo's locked checksums. Actual packaged grants were
reviewed: select Apache-2.0 for the eight dual-licensed RustCrypto packages and
flagset's sole Apache-2.0 grant. Retained notices add package attribution and the
exact selected text variants; none of these archives supplied a separate NOTICE.
The graph declares no Rust requirement above the existing 1.88 floor; actual
minimum-version execution remains unqualified. No production graph is modified.

Upstream path histories show 2026 activity for x509-cert, pkcs8, sec1, der and
base64ct. Flagset's latest inspected source change is 2025-03-02: treat it as
a mature, low-churn dependency, not evidence of recent active support or proof
of abandonment. It has no normal dependency graph and implements bit-flag values
used by typed extension decoding. Review it with the rest of the graph monthly
and on security notices; any abandonment/advisory finding needs an owned
disposition under the dependency policy. No support guarantee is inferred.

Source review covers typed nested completion, PKCS#8 discarded-field handling,
SEC1 decoding, extension access and the fixed ring loader. Schema-owned buffers
and provider internals can allocate/copy; no universal allocator-failure or
physical-erasure guarantee follows. Public APIs and controlled test construction
use library DER primitives; no application TLV decoder, crypto implementation,
unsafe code, installer or trust-store access is introduced.
