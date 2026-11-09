# Native history storage measurements

This finite synthetic campaign measures the existing permanent full-history
representation. It provides a conditional planning estimate, not qualification
of a 1,000-item installation. The capacity/history disposition is **pending
maintainer acceptance**. No history representation or retention policy changed.

## Workload and method

The planning scenario is 1,000 items, each created and edited 20 times over one
year: 900 compact items, 90 large items and 10 near-boundary items. The database
expectation is 4 GiB; WAL, backups and build outputs are separate. Entropy and
edit-breadth proportions are unspecified, so the projection below uses the
observed minimum and maximum within each size class instead of assuming a mix.

Each of 24 pairs contains one native item written through the actual HTTP/TLS
route and one separately constructed historical item. Both have 21 revisions.
The matrix crosses three sizes, repeated text versus deterministic SHA256 hex
text, one-field versus all-field edits, and two repetitions. Compact fields are
4 × 64 bytes; large fields are 128 × 1,024 bytes. Near-boundary fields are
128 × 7,600 bytes; every observed history's PostgreSQL text rendering is checked
against the admitted 943,719–1,048,576-byte interval. SHA256 hex is a synthetic
lower-compressibility contrast, not arbitrary random binary or real user data.

The native runtime is joined before independent final-state observation. The
historical fixture constructs expired operation origins at insertion, with
normal database guards enabled. It then explicitly retires replay payloads and
prunes expired pending payloads in separate transactions. This exercises those
storage contracts; it does not establish when production maintenance runs.
Historical project setup is minimal and differs from native project creation.
Compare each role with its own baseline; do not subtract their absolute totals
to estimate retirement savings.

Read-only censuses run at baseline, creation, and edits 1, 5, 10 and 20, then
after historical replay retirement and pending-payload pruning. Independent
observers verify exact histories, request hashes, saved results, operation
identity/attribution, tombstones, item and project heads, and field-use witnesses.
The final exports validate all 24 pairs: 1,008 successful item revisions and
1,008 exact model reconstructions across 48 history chains of 21 revisions each.

PostgreSQL 16.15 uses 8 KiB pages, default TOAST compression `pglz`, extended
storage for history and replay payload columns, WAL compression off, full-page
writes on and autovacuum on. Each role uses a fresh database in a disposable
cluster. The runner has 2 GiB memory and one CPU; PostgreSQL has one CPU.
Twenty pairs use 256 MiB database memory and a 256 MiB data tmpfs. The four
near-boundary SHA256-text pairs use 512 MiB for both after a controlled allocation
guard stopped a 256 MiB attempt. Other settings and storage contracts match;
the fixture's changes are limited to resource admission and headroom checks.
These are separate source/resource cohorts. Their union covers the matrix,
but cannot establish a homogeneous-profile or resource-independent causal
comparison. The large repeated/SHA256 comparisons share the 256 MiB profile.

The campaign's accepted cap was 1,058 revisions, including 50 revisions from
two failed attempts. Failed evidence is retained separately: one historical
setup SQL defect and one controlled data-allocation stop. Earlier image-store
selection and compilation failures produced no item revisions. Successful
reruns do not erase those failures. There was no host-fill experiment.

## Observed growth

The following values are native baseline-to-edit-20 allocation increases,
rounded to MiB. History is the complete history relation, including its indexes
and TOAST. Database growth includes all measured database relations and their
page/index/TOAST overhead. Values represent one item per fresh fixture, not a
steady-state marginal cost in a populated database.

| Fields | Text | Edit breadth | History growth MiB | Database growth MiB | Generated WAL MiB |
| --- | --- | --- | ---: | ---: | ---: |
| Compact | Repeated | One field | 0.109 | 0.328 | 0.131 |
| Compact | Repeated | All fields | 0.109 | 0.320 | 0.128 |
| Compact | SHA256 hex | One field | 0.109 | 0.328 | 0.132 |
| Compact | SHA256 hex | All fields | 0.109 | 0.328 | 0.134 |
| Large | Repeated | One field | 0.141 | 0.500 | 0.272 |
| Large | Repeated | All fields | 0.141 | 0.500 | 0.297 |
| Large | SHA256 hex | One field | 3.070 | 6.648 | 6.541 |
| Large | SHA256 hex | All fields | 3.070 | 9.445 | 9.421 |
| Near boundary | Repeated | One field | 0.531 | 1.320 | 1.089 |
| Near boundary | Repeated | All fields | 0.531 | 1.758 | 1.534 |
| Near boundary | SHA256 hex, 512 MiB profile | One field | 20.562 | 43.414 | 44.482 |
| Near boundary | SHA256 hex, 512 MiB profile | All fields | 20.562 | 62.719 | 64.400 |

Repetitions agree on these allocation totals except the compact repeated/all
case retains the smaller 0.320 MiB observation in both repetitions. Small WAL
differences remain visible in the CSV. WAL is the cluster insert-LSN delta
between settled checkpoints and includes background activity; it is not an
isolated causal byte count for each command. The WAL directory allocates whole
segments and is a different measurement from generated WAL or archived WAL.

Per-edit allocation is stepwise. For edits 11–20, mean native relation growth
ranges from 4.0–4.8 KiB/edit for compact items; 7.2–8.0 KiB for large repeated
text; 292.8–436.8 KiB for large SHA256 text; 47.2–68.8 KiB for near-boundary
repeated text; and 2,004.8–2,993.6 KiB for near-boundary SHA256 text in the
512 MiB cohort. These are checkpoint-interval averages, not individual-edit
peak measurements. [allocations.csv](allocations.csv) contains every checkpoint,
logical payload byte totals, database allocation, heap/auxiliary/index/TOAST
components, and interval WAL and growth values in bytes. The components
reconcile to each relation total before reporting.

Logical full-history text totals are independent of entropy for these equal-size
fixtures. Physical allocation differs greatly. On the same 256 MiB profile,
large history growth is 0.141 MiB for repeated text and 3.070 MiB for SHA256 text.
This demonstrates that logical limits alone cannot predict physical storage.
It is consistent with compression and TOAST/page effects under the recorded
settings; per-row compression methods and an isolated compressor contribution
were not measured.

## Replay retirement and retained history

All 21 histories, permanent operation identities/request hashes/attribution,
retired markers, item heads and field-use witnesses survive historical replay
retirement. The 21 item replay payloads are removed. Pruning clears the expired
pending creation payload while retaining its identity. The historical project
fixture's unrelated one-byte request and two-byte result remain; zero total
payload bytes would be an incorrect expectation.

History bytes and history-relation allocation do not decrease. Database files
also do not shrink during this campaign: deletion makes content unavailable
without returning those allocated pages to the filesystem. No vacuum/compaction
experiment or future reuse benefit is claimed. Historical baseline-to-pruned
allocation is slightly larger than native baseline-to-edit-20 allocation, due
to its own lifecycle and page effects. It is not evidence that deleting payloads
increases live data or that history was deleted.

## Conditional planning projection

For each size class, multiply its observed per-fixture baseline-to-final
allocation by the planned item count, then sum. Minima/maxima range over entropy,
edit breadth and repetitions; they are scenario envelopes, not confidence
intervals. One-year timing does not add an annual multiplier: 20 edits is the
total per item in that year. More years/edits require a new estimate.

| Quantity | Native, replay present GiB | Historical, after pruning GiB |
| --- | ---: | ---: |
| Full-history logical PostgreSQL text | 0.462 | 0.462 |
| History-relation allocation increase | 0.114–0.567 | 0.114–0.567 |
| Whole database allocation increase | 0.338–1.731 | 0.370–1.763 |

For example the upper historical estimate is
`900 × 0.359375 + 90 × 9.4921875 + 10 × 62.765625 = 1,805.390625 MiB`,
or 1.763 GiB. This is below the 4 GiB expectation for the stated synthetic
scenario. It provides a reason to consider retaining full history, not proof
that a deployed database will remain below 4 GiB.

The calculation repeats one-item fresh-database deltas. It does not measure
1,000 items in a shared project/database, project/actor/configuration growth,
index packing at scale, concurrency, long-lived dead tuples/autovacuum, real
field distributions, other workloads or sustained capacity. Fresh database
baseline allocation must be added once per actual database; project/setup costs
may be counted differently in a real deployment. No statistical uncertainty
or operational reserve has been established. WAL retention/archiving, backups,
replication, filesystem overhead outside PostgreSQL's database-size observation
and temporary/build space are outside this database projection. The 4 GiB
expectation is therefore a planning target to monitor, not an enforced limit or
capacity guarantee. No automatic replay-retirement schedule is assumed.

## Full history versus checkpoint/delta comparison

The concrete alternative model stores full histories at versions 1, 11 and 21.
Other versions retain complete non-field metadata, field order, changed field
entries and a SHA256 digest of the reconstructed history. Removed/absent fields
and order are reconstructed exactly. Every version of every observed chain is
reconstructed and compared with the independently validated original.

Both variants use the same canonical Python JSON encoding for this comparison.
That encoding differs from PostgreSQL JSONB storage and `history::text` output.
Ratios therefore compare logical model encodings, never physical PostgreSQL
savings. [history-model.csv](history-model.csv) records both roles and repetitions.

| Size | One-field edits: model/full bytes | All-field edits: model/full bytes |
| --- | ---: | ---: |
| Compact | 0.914 | 1.186 |
| Large | 0.170 | 1.016 |
| Near boundary | 0.152 | 1.002 |

The current format reads one full historical record for an arbitrary revision.
The model reconstructs from 1–10 records (mean 5.286 over these 21 versions),
checking the reconstructed history digest at each step. For near-boundary
chains this checks up to 9,813,073 encoded history bytes for one lookup; that is
hash input volume, not disk bytes or a measured CPU time. Compression of repeated
full histories may reduce or reverse the apparent logical savings. No physical
alternative layout, migration, supported codec, indexing/access benchmark,
backup/restore or malformed-input error contract is qualified by this model.
The digest detects accidental mismatch in this experiment; it is not
authentication. Permanent operation identities, request hashes, replay
tombstones and field-use relations remain outside the comparison model.

Retaining the current representation preserves direct per-version reads and
the existing storage/decode/restore contracts. A checkpoint/delta representation
offers substantial logical savings for sparse edits to large items, at the cost
of dependent reconstruction and additional compatibility/error-containment
work; it can be larger for broad edits. Selection requires an explicit
maintainer decision. If a new representation is selected, this report and
comparison model do not implement or qualify that capability.

## Reproduction and evidence scope

See [the testing guide](../../TESTING.md) for the opt-in finite sampler and
reporter. [provenance.json](provenance.json) records public source-inventory and
analysis hashes, pinned images, actual PostgreSQL settings/column storage and
the two execution profiles. Full inventories are bound separately; the shared
contract fingerprint is a review aid, not automated semantic-equivalence proof.
Raw histories, operation identities and execution diagnostics are intentionally
excluded from these published tables.

The measurement fixtures, model and focused orchestration checks have executed;
final required qualification, hosted CI, exact-head review and maintainer
capacity/history acceptance remain separate delivery gates. This document does
not claim their completion.
