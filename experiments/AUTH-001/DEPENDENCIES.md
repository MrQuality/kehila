# Standalone probe dependency review

Owner: project maintainer. Reviewed 2026-10-19. Scope: the locked AUTH-001 experiment only; no production graph upgrade.

## Purpose and selection

TLS: rustls 0.23.46 with the explicit ring provider; Tokio/Tokio-rustls own asynchronous sockets and deadlines; Hyper/Hyper-util and http-body-util provide bounded HTTP/1 handling. Bytes is the body representation. Test-only rcgen/time generate disposable certificates, including renewal and invalid-certificate fixtures. All direct versions are exact; Cargo.lock fixes the complete registry graph.

These are maintained upstream Rust TLS/runtime/HTTP projects with published source and release histories. Pinning does not establish security: review monthly and immediately after upstream security notices under [DEP-001](../../docs/engineering/DEPENDENCIES.md). The maintainer owns requalification. The minimum declared package requirement reaches Rust 1.88; execution here used 1.93.0, not a qualified full-graph minimum-version build.

## License selection and distribution

Actual packaged license texts were inspected. Apache-2.0 is selected where offered, MIT for memchr/wasi, and the sole grant where no alternative exists. Ring retains its Apache/ISC and bundled once_cell/fiat notices. The packaged asn1-rs-impl 0.2.0 lacks its license files: its .cargo_vcs_info.json identifies upstream commit a20e5f7319c896737ad0f2557037817b91ad854f; the [root Apache text](https://github.com/rusticata/asn1-rs/blob/a20e5f7319c896737ad0f2557037817b91ad854f/LICENSE-APACHE) supplies the selected grant.

Explicit Unicode-3.0 compatibility/distribution review: unicode-ident combines a code-license alternative with the mandatory Unicode data license. Apache-2.0 AND Unicode-3.0 is selected. The inspected Unicode grant permits use, modification and redistribution, requires retention of its copyright/permission notice, and restricts advertising use of copyright-holder names. No third-party trademark is used as Kehila branding; the full Unicode grant is retained. This permits this experimental distribution under those obligations; it does not relicense the Unicode data. [Unicode policy](https://www.unicode.org/policies/licensing_policy.html).

[THIRD-PARTY-NOTICES.txt](THIRD-PARTY-NOTICES.txt) retains the selected texts and supplied notices, grouping byte-identical LF-normalized text once with package/source attribution. No third-party source is vendored. Include these notices if distributing probe binaries; re-review graph/license changes before distribution.

## Advisory scope

The separately recorded package/version OSV query covers the 85 locked registry packages, including platform-specific and test-only entries. It is a point-in-time package advisory query, not a complete source/reachability audit or a qualification of the existing worker. The pre-existing worker rustls 0.21.12 follow-up remains under [#40](https://github.com/MrQuality/kehila/issues/40) and the [A0 plan](../../docs/implementation/local-owner-authentication-a0.md#advisory-follow-up).

## Locked registry inventory

| Package | Version | Selected grant |
| --- | --- | --- |
| [asn1-rs](https://crates.io/crates/asn1-rs/0.7.2) | 0.7.2 | Apache-2.0 |
| [asn1-rs-derive](https://crates.io/crates/asn1-rs-derive/0.6.0) | 0.6.0 | Apache-2.0 |
| [asn1-rs-impl](https://crates.io/crates/asn1-rs-impl/0.2.0) | 0.2.0 | Apache-2.0 |
| [atomic-waker](https://crates.io/crates/atomic-waker/1.1.2) | 1.1.2 | Apache-2.0 |
| [autocfg](https://crates.io/crates/autocfg/1.5.1) | 1.5.1 | Apache-2.0 |
| [bit-vec](https://crates.io/crates/bit-vec/0.9.1) | 0.9.1 | Apache-2.0 |
| [bytes](https://crates.io/crates/bytes/1.11.1) | 1.11.1 | MIT |
| [cc](https://crates.io/crates/cc/1.9.0) | 1.9.0 | Apache-2.0 |
| [cfg-if](https://crates.io/crates/cfg-if/1.0.5) | 1.0.5 | Apache-2.0 |
| [data-encoding](https://crates.io/crates/data-encoding/2.11.1) | 2.11.1 | MIT |
| [der-parser](https://crates.io/crates/der-parser/10.0.0) | 10.0.0 | Apache-2.0 |
| [deranged](https://crates.io/crates/deranged/0.5.9) | 0.5.9 | Apache-2.0 |
| [displaydoc](https://crates.io/crates/displaydoc/0.2.7) | 0.2.7 | Apache-2.0 |
| [find-msvc-tools](https://crates.io/crates/find-msvc-tools/0.1.15) | 0.1.15 | Apache-2.0 |
| [futures-channel](https://crates.io/crates/futures-channel/0.3.34) | 0.3.34 | Apache-2.0 |
| [futures-core](https://crates.io/crates/futures-core/0.3.34) | 0.3.34 | Apache-2.0 |
| [getrandom](https://crates.io/crates/getrandom/0.2.17) | 0.2.17 | Apache-2.0 |
| [http](https://crates.io/crates/http/1.5.0) | 1.5.0 | Apache-2.0 |
| [http-body](https://crates.io/crates/http-body/1.1.0) | 1.1.0 | MIT |
| [http-body-util](https://crates.io/crates/http-body-util/0.1.3) | 0.1.3 | MIT |
| [httparse](https://crates.io/crates/httparse/1.10.1) | 1.10.1 | Apache-2.0 |
| [httpdate](https://crates.io/crates/httpdate/1.0.3) | 1.0.3 | Apache-2.0 |
| [hyper](https://crates.io/crates/hyper/1.11.1) | 1.11.1 | MIT |
| [hyper-util](https://crates.io/crates/hyper-util/0.1.21) | 0.1.21 | MIT |
| [itoa](https://crates.io/crates/itoa/1.0.18) | 1.0.18 | Apache-2.0 |
| [lazy_static](https://crates.io/crates/lazy_static/1.5.1) | 1.5.1 | Apache-2.0 |
| [libc](https://crates.io/crates/libc/0.2.191) | 0.2.191 | Apache-2.0 |
| [memchr](https://crates.io/crates/memchr/2.8.3) | 2.8.3 | MIT |
| [minimal-lexical](https://crates.io/crates/minimal-lexical/0.2.1) | 0.2.1 | Apache-2.0 |
| [mio](https://crates.io/crates/mio/1.2.4) | 1.2.4 | MIT |
| [nom](https://crates.io/crates/nom/7.1.3) | 7.1.3 | MIT |
| [num-bigint](https://crates.io/crates/num-bigint/0.4.8) | 0.4.8 | Apache-2.0 |
| [num-conv](https://crates.io/crates/num-conv/0.2.2) | 0.2.2 | Apache-2.0 |
| [num-integer](https://crates.io/crates/num-integer/0.1.47) | 0.1.47 | Apache-2.0 |
| [num-traits](https://crates.io/crates/num-traits/0.2.19) | 0.2.19 | Apache-2.0 |
| [oid-registry](https://crates.io/crates/oid-registry/0.8.1) | 0.8.1 | Apache-2.0 |
| [once_cell](https://crates.io/crates/once_cell/1.21.4) | 1.21.4 | Apache-2.0 |
| [pin-project-lite](https://crates.io/crates/pin-project-lite/0.2.17) | 0.2.17 | Apache-2.0 |
| [powerfmt](https://crates.io/crates/powerfmt/0.2.1) | 0.2.1 | Apache-2.0 |
| [proc-macro2](https://crates.io/crates/proc-macro2/1.0.107) | 1.0.107 | Apache-2.0 |
| [quote](https://crates.io/crates/quote/1.0.47) | 1.0.47 | Apache-2.0 |
| [rcgen](https://crates.io/crates/rcgen/0.14.10) | 0.14.10 | Apache-2.0 |
| [ring](https://crates.io/crates/ring/0.17.14) | 0.17.14 | Apache-2.0 AND ISC; bundled notices retained |
| [rusticata-macros](https://crates.io/crates/rusticata-macros/4.1.0) | 4.1.0 | Apache-2.0 |
| [rustls](https://crates.io/crates/rustls/0.23.46) | 0.23.46 | Apache-2.0 |
| [rustls-pki-types](https://crates.io/crates/rustls-pki-types/1.15.1) | 1.15.1 | Apache-2.0 |
| [rustls-webpki](https://crates.io/crates/rustls-webpki/0.103.15) | 0.103.15 | ISC |
| [serde](https://crates.io/crates/serde/1.0.229) | 1.0.229 | Apache-2.0 |
| [serde_core](https://crates.io/crates/serde_core/1.0.229) | 1.0.229 | Apache-2.0 |
| [serde_derive](https://crates.io/crates/serde_derive/1.0.229) | 1.0.229 | Apache-2.0 |
| [shlex](https://crates.io/crates/shlex/2.0.1) | 2.0.1 | Apache-2.0 |
| [smallvec](https://crates.io/crates/smallvec/1.16.3) | 1.16.3 | Apache-2.0 |
| [socket2](https://crates.io/crates/socket2/0.6.5) | 0.6.5 | Apache-2.0 |
| [subtle](https://crates.io/crates/subtle/2.6.1) | 2.6.1 | BSD-3-Clause |
| [syn](https://crates.io/crates/syn/2.0.119) | 2.0.119 | Apache-2.0 |
| [syn](https://crates.io/crates/syn/3.0.9) | 3.0.9 | Apache-2.0 |
| [synstructure](https://crates.io/crates/synstructure/0.13.2) | 0.13.2 | MIT |
| [thiserror](https://crates.io/crates/thiserror/2.0.21) | 2.0.21 | Apache-2.0 |
| [thiserror-impl](https://crates.io/crates/thiserror-impl/2.0.21) | 2.0.21 | Apache-2.0 |
| [time](https://crates.io/crates/time/0.3.55) | 0.3.55 | Apache-2.0 |
| [time-core](https://crates.io/crates/time-core/0.1.9) | 0.1.9 | Apache-2.0 |
| [time-macros](https://crates.io/crates/time-macros/0.2.32) | 0.2.32 | Apache-2.0 |
| [tokio](https://crates.io/crates/tokio/1.53.1) | 1.53.1 | MIT |
| [tokio-macros](https://crates.io/crates/tokio-macros/2.7.2) | 2.7.2 | MIT |
| [tokio-rustls](https://crates.io/crates/tokio-rustls/0.26.6) | 0.26.6 | Apache-2.0 |
| [try-lock](https://crates.io/crates/try-lock/0.2.5) | 0.2.5 | MIT |
| [unicode-ident](https://crates.io/crates/unicode-ident/1.0.27) | 1.0.27 | Apache-2.0 AND Unicode-3.0 |
| [untrusted](https://crates.io/crates/untrusted/0.9.0) | 0.9.0 | ISC |
| [want](https://crates.io/crates/want/0.3.2) | 0.3.2 | MIT |
| [wasi](https://crates.io/crates/wasi/0.11.1+wasi-snapshot-preview1) | 0.11.1+wasi-snapshot-preview1 | MIT |
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
| [x509-parser](https://crates.io/crates/x509-parser/0.18.1) | 0.18.1 | Apache-2.0 |
| [yasna](https://crates.io/crates/yasna/0.6.0) | 0.6.0 | Apache-2.0 |
| [zeroize](https://crates.io/crates/zeroize/1.9.1) | 1.9.1 | Apache-2.0 |
