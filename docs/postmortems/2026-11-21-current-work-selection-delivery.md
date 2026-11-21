# Persistent current-work selection delivery retrospective

| Field | Value |
| --- | --- |
| Delivery | [PR #104](https://github.com/MrQuality/kehila/pull/104), implementation interval `3c61b0b..771d5bdb` |
| Owner | Project maintainer |
| Outcome | Authenticated owner set/clear, persisted current selection and original-operation recovery under [ADR-112](../architecture/ADR-112-current-work-selection.md) |
| Evidence | [Qualification and limits](../implementation/current-work-selection.md#qualification-on-2026-11-21), commit history and PR checks/review record |
| Status boundary | Bounded disposable-owner qualification; review readiness is recorded in the PR, and merging remains with the maintainer |

## Result and scope

The implementation connects versioned codecs, fresh schema v4, permanent
operation/event attribution, native authority/HTTP and the browser recovery
journal. The final browser case distinguishes a lost reply's original B/version5
receipt from a later current A/version6 choice. Independent same-database SQL
confirms six accepted operations and unchanged unrelated state after normal
release. Temporary trust and successful profiles were removed and absence was
checked.

This is an increment toward M1-02, with applicable access/native integration. It
does not complete parent issues #27, #32 or #49, add populated migration or general
archive/restore, qualify supported real-data use, or change deployment/security
policy. No next delivery is activated by this retrospective.

## Timeline and evidence validity

The accepted contract precedes implementation. Six focused commits on 2026-11-18
cover the codec/contract, fresh storage, native/API/faults, interface/recovery,
expiry-response repair and completed-fixture lifetime repair. This preserves one
coherent capability PR without treating its coupled contracts as independent
deliveries.

An earlier full local attempt failed while provisioning a restored synthetic
database. After the focused fixture-lifetime repair, full local qualification
passed in 620.297 seconds including cleanup. Required full hosted qualification
passed at the implementation head. Later browser attempts failed prerequisites,
evidence admission or diagnostic bounds; those attempts are not reclassified by
the final pass.

On 2026-11-21 the complete selected browser, independent SQL and Root-cleanup
command passed in 394.203 seconds. Separate exact certificate cleanup took 6.485
seconds. Final documentation-only changes can retain these runtime observations
because they do not change executable, asset, storage, caller or oracle contracts;
their own publication checks and exact-head review remain required.

These are individual measured command spans. Preparation, review, user waits and
other investigations are partly uninstrumented or overlap. Commit timestamps and
failed-run durations do not establish total labor, avoidable cost or saved hours.

## Observations and dispositions

| Observation | Consequence and supported interpretation | Disposition / owner / remaining limit |
| --- | --- | --- |
| Native and interface components passed before complete actual-browser qualification | Component success did not establish actual browser storage/lifecycle behavior | Retain the selected real-browser and independent SQL boundary. Maintainer owns future supported-platform scope; no exhaustive browser matrix is claimed |
| Some integrated attempts stopped at harness/prerequisite checks | A harness failure is not automatically a product defect; a later pass cannot supply a missing old observation | Preserve failures and classify the failed stage. Apply the existing delivery/evidence controls proportionally rather than add a generic verification campaign |
| Auxiliary diagnostic work displaced attention from the feature outcome | More instrumentation introduced its own cost and failure conditions | The maintainer selected a simpler qualification path retaining the original behavioral, authority, storage and cleanup criteria. This is a delivery-specific disposition, not a new reusable framework or global policy |
| The final result includes independent changed-state SQL and normal process release | Neither a browser receipt nor exit 0 alone proves durable acceptance | Keep those distinct oracles. Their agreement supports the bounded result, not disaster-recovery or every browser-lifecycle combination |
| Sampled aggregate memory reached 5.999596 GiB under 6 GiB; available memory fell to 0.276310 GiB | The run recorded one reserve warning and no resource alerts; this left almost no sampled aggregate margin | Resource-pressure reduction remains a maintainer-owned follow-up proposal before expanding workloads. These observations do not qualify greater workloads or another deployment profile |
| Earlier visibility/resource problems were not all causally resolved | A successful run does not prove why every earlier run failed or that process improvements are effective generally | Retain the limitations and reassess at the next applicable milestone/large-goal review; do not create another campaign solely to claim process effectiveness |

The independent source/evidence challenge checked the original outcome, retained
runtime observations and scope boundaries. It required the memory warning and
test-level distinctions to remain explicit. PR #104 tracks final exact-head
composition review and the corresponding check results; they remain required
before review readiness.

## Follow-up

Use the existing [engineering delivery controls](../procedures/engineering-delivery.md)
and [testing guide](../TESTING.md) at the next material delivery. Reuse valid
evidence after checking source, shared callers, environment, fixtures and oracles;
repeat affected checks when a premise changes. Treat machine resources and user
attention as limited, and keep optional diagnostics proportional to the actual
uncertainty.

The resource-pressure cause, broader platform/lifecycle coverage and supported
installation/restore remain scoped follow-up work. Recommendations here do not
approve exact SOP codification, automated semantic enforcement or shared-skill
changes. Longer-term effectiveness has not been established.
