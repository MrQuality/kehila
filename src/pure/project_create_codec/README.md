# Project-creation storage codecs

Pure versioned representations for the future native PostgreSQL adapter.
This crate performs no I/O and exposes no route. Related: #47, #39, #26/B-005.

`NativeRequest::new` owns checked new-write admission: shared creation validation
plus the native ProjectId byte/NUL bounds. It preserves exact identifier bytes.
The future adapter must accept this checked type rather than trusting public
String-backed wrappers. The legacy worker and public domain acceptance remain
unchanged. Actor authentication, authority and stable allocation are external.

`encode_request`/`decode_request` preserve the frozen version-one binary format;
`verify_request` additionally checks the permanent SHA-256 fingerprint. Stored
versions dispatch explicitly. These reconstruction functions deliberately do
not validate historical content against today's creation rules.

Codec ceilings are resource admission limits, not product policy. Individual
strings are limited to 64 KiB and records to 1 MiB; rejection never truncates or
normalizes input. The caller must bound transport reads before collecting a
record and treat an over-limit record as failure. A fingerprint is an integrity
comparison, not authentication or a replacement for exact typed comparison.

`parse_json` is the raw first-parse boundary for both future ingress and stored
records. It rejects duplicate decoded keys, trailing input, depth over 16
(root at zero), more than 1,024 entries per array/object and more than 16,384
JSON values, including containers. Keys are bounded strings, not value nodes.
These guarantees cannot recover duplicate keys already collapsed by JSONB or
another parser; future ingress must use this boundary on original bytes.

`encode_result`/`decode_result` and `encode_snapshot`/`decode_snapshot` preserve
complete saved successes and configuration snapshots, using stored-version
and embedded-kind/version checks. Typed preflight bounds aggregate collection
entries/string bytes before allocating a tree; exact node/depth checks and a
bounded memory writer also limit escaped JSON output. They do not authenticate
valid-shaped stored data or prove cross-record semantic agreement. The adapter
must check core/request/result attribution and persist one trusted domain result.
A project-only snapshot envelope remains adapter work; this crate does not
invent a format absent from the frozen version-one fixtures.

## Dependencies

| Direct package | Locked version | Purpose and provenance |
| --- | --- | --- |
| serde | 1.0.229 | JSON deserialization traits; [serde-rs](https://github.com/serde-rs/serde) |
| serde_json | 1.0.151 | JSON storage envelopes; [serde-rs/json](https://github.com/serde-rs/json) |
| sha2 | 0.10.9 | Direct SHA-256 ownership; [RustCrypto/hashes](https://github.com/RustCrypto/hashes/tree/sha2-v0.10.9/sha2) |

These versions already exist in the workspace lockfile; this crate adds direct
ownership without upgrading their external dependency graphs or obtaining
hashing through the worker's MongoDB dependency. The downloaded package archives'
actual MIT and Apache-2.0 license texts were inspected; select MIT for these
dependencies and retain upstream copyright/permission notices in distributed
artifacts. Their upstream projects remain active; pinning an existing version
does not assert it is the newest or free of advisories. No new advisory scan or
release qualification is claimed. Review monthly and on upstream security
notices under the repository dependency policy; assess upgrades separately.

Package-declared compiler floors fit Rust 1.75 for serde/serde_json. sha2 and its
locked transitive graph still require actual minimum-toolchain qualification;
stable-toolchain checks do not establish a supported minimum-version claim.
