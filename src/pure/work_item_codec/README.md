# WorkItem storage codecs

Pure version-one representations for accepted ADR-111 under #27/#49. No I/O,
caller authentication, schema installation or command execution occurs here.
This packet does not deliver native WorkItem routes or qualify their transactions.

## Request identity

`Request::Create` wraps the domain creation command with project scope.
`Request::Edit` wraps the edit command with project and item scope. The stored
operation core supplies independently authenticated actor, family and target;
the adapter must compare that core with these representations. Client tokens
and payload identities never confer authority.

Version-one JSON has exactly `codec_version`, `kind`, `value`. Request kinds are
`item_create_request` and `item_edit_request`. Every command field is present,
including explicit null optional workflow/estimate values, expected versions,
ordered field entries and distinct Keep/Set/Clear actions. All u64 and signed
numeric field values use canonical decimal strings; estimates are exact integer
thousandths. Dates use `YYYY-MM-DD`. No normalization or float conversion occurs.

`encode_request` recursively sorts object keys by Rust string ordering, preserves
array order and writes compact JSON. This ordering is explicit and independent
of serde_json map features. `fingerprint` hashes these complete canonical bytes.
Equivalent incoming object order/escape spellings decode to the same typed
request and canonical identity. The adapter must hash canonical typed content,
not arbitrary wire bytes. `verify_request` requires retained bytes themselves to
be canonical and verifies the permanent digest. Frozen byte/digest fixtures cover
both families. Existing ProjectCreate binary bytes and JSON shapes are unchanged.

## Results and permanent history

`SavedResult::{Create,Edit}` retain complete typed domain results with distinct
`item_create_result`/`item_edit_result` envelope kinds. They reuse the frozen
version-one Project mapping through the existing codec composition boundary.
All five typed field-value kinds and their order are preserved; no current
configuration lookup or current domain acceptance runs during reconstruction.

`Revision` uses kind `item_revision` and retains actor, configuration revision,
project operational-state version, previous project operation and the
complete saved result. Its Project carries resulting sequence/estimate facts.
These are historical assertions, not verified relational facts: the future
adapter/SQL must enforce project/item/core/actor/configuration attribution and
head/transition coherence. Replay expiry must never remove this product history.
The fresh operation-row ID belongs to the relational revision row, not the JSON
payload: PostgreSQL allocates it only during the write transaction, after payload
admission. The adapter joins and verifies that relational attribution on reload.
No guessed/reserved ID or mutation of already admitted JSON is permitted.

## Resource and native admission

Every raw decode uses the existing original-byte duplicate-aware JSON parser:
1 MiB records, 64 KiB decoded keys/strings, depth 16 (root zero), 1,024 entries
per collection and 16,384 value nodes. Missing/unknown fields, wrong tags/kinds/
versions and malformed canonical numbers/dates fail explicitly.

Typed encoding preflights aggregate text/collections before constructing a tree;
the shared composition encoder then checks the full envelope and bounded output.
`admit_request/result/revision` also reject NUL and reserve PostgreSQL 16 JSONB
structural spacing. Their private `NativeRecord` bytes establish these representation
checks only. They do not establish current domain eligibility, SQL scalar widths,
foreign keys, grants, attribution, accepted state or committed execution.
The public composition helper cannot prevent a caller allocating its input Value
first; typed WorkItem APIs perform their own preflight before tree construction.

Inside the owned verification container, focused pure checks are:

```text
cargo test --locked -p work_item_codec -p project_create_codec
```

The workspace and pure verification matrix include this crate. Native JSONB
round trips, actual-role constraints and application journeys remain later
evidence. Current pure results are not native qualification.
Windows commits use the required exported-source Podman gate described in
`docs/implementation/container-verification.md`; do not use host-compiled tests
as a substitute for that lane.

## Dependencies

Use the already locked task/project codec crates and serde_json 1.0.154. SHA-256
and the bounded raw parser/writer remain owned by project_create_codec; no copied
parser, writer, Project mapping or external package/version upgrade is introduced.
