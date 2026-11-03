# Authenticated project delivery retrospective

| Field | Value |
| --- | --- |
| Delivery | PR #92, original implementation interval `3ad275d..10a6829` |
| Owner | Project maintainer |
| Authority | Maintainer accepted the retrospective recommendations on 2026-11-03 and added their implementation to PR #92; requested recurrence at milestone/large-goal closeout |
| Sources | Commit history, [authority observations](../implementation/project-authority-verification.md), [container record](../implementation/container-verification.md), [CI](https://github.com/MrQuality/kehila/actions/runs/41625125167) and reviewed gate durations |
| Result | Revised native/API goal delivered; actual Chrome/Edge execution explicitly deferred and unqualified; no merge |

## Observations and dispositions

The implementation connected product-owned authentication to native project
creation, server-owned pending recovery, cancellation/reconciliation and fresh
authorized reads. Independent exact-head review and three CI checks passed at
`10a6829`. This does not qualify supported installation/restore, real-data use,
multi-actor HTTP or declared-minimum Rust execution. The process amendment has its
own later head and checks; the old CI run does not qualify that later head.

| Priority | Supported observation | Accepted improvement |
| --- | --- | --- |
| P1 | Contract commit at 01:11:54+02; first implementation commit at 11:13:16+02, approximately ten hours later | Plan coherent checkpoints and publish after their required gates, rather than accumulating implementation locally |
| P1 | Native CI rejected a cache option on Podman 4.9.3 before native execution; a later local probe exposed another ownership serialization | Exercise the smallest relevant path on actual target tools/versions early; preserve closed admission and real affected-environment checks |
| P1 | Harness failures included encoding/copy, permissions, SQL observation, error classification, stream ordering, debug stack size and retained-reply scheduling assumptions | Review uncertain harness mechanisms and use focused controls before expensive integrated campaigns |
| P2 | Layered reviews need explicit coverage and validity to avoid duplicate work or gaps | One review map; scoped component reviews, affected-delta revalidation and final exact-head composition review |
| P2 | Available records do not support a complete duration attribution; some parent stage labels were stale | Actual-stage events, separate activity categories, unknown/overlapping time and append-only corrections |

Qualification also reproduced a meaningful audit-uncertainty response-delivery
defect before its correction. Preserve testing of these behavior contracts. Failed
campaigns remained failed; affected reruns retained unaffected passing evidence.
These are strengths, not waste to remove. Six measured mandatory gates totalled
2,419.704 seconds (about 40 minutes 20 seconds), only a subset of all verification.
Commit timestamps do not establish when code was authored, time spent reviewing,
or wasted hours. Failed-run duration is not automatically avoidable cost. No
saved-hours or process-effectiveness claim follows from this retrospective.

The sanitized duration inputs are below. These are different source checkpoints,
not six qualifications of the final product or a total for all testing.

| Checkpoint | Local gate | Seconds through cleanup |
| --- | --- | --- |
| `18db288` | Full | 382.219 |
| `dd7ed44` | Pure | 169.844 |
| `8e4250d` | Full | 338.375 |
| `666bf68` | Full | 502.969 |
| `149fd69` | Full | 509.938 |
| `10a6829` | Full | 516.359 |

Historical CI identity: run `41625125167`, head
`10a68295c126cf72c9f0a45074f4c2988db3db5c`; `native-authority`, `verify` and
`windows-owner-client` passed. The last is the experimental CLI check, not browser
qualification. The independent source review was recorded in the
[PR closeout](https://github.com/MrQuality/kehila/pull/92#issuecomment-6602530825),
not a GitHub approving review. Later reviewers must distinguish inspecting that
record from independently executing those checks.

## Implementation and evaluation

[SOP-003](../procedures/engineering-delivery.md) and its
[record fields](../procedures/engineering-delivery-record.md) implement the accepted
controls: source-first risk challenge, layered review, early environment/harness
admission, publication checkpoints, evidence retention and measured closeout.
The [template](../procedures/delivery-retrospective-template.md) makes recurrence
usable without creating another platform. Contributor entry points link the rules.

These are manual controls and reusable records, not semantic CI enforcement or
proof of effectiveness. The amendment report/PR records actual validation and
skill availability. At the next applicable closeout, examine time to discovery,
checkpoint delay, repeated review coverage and verification rework using observed
events. Keep unknowns visible and adjust only on demonstrated problems. Exact
SOP codification and any material future revision need explicit version acceptance.
