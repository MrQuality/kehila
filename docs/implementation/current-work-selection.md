# Persistent owner current work

The native capability and selected browser journey have passed bounded
qualification in the disposable local-owner profile. The contract is accepted in
[ADR-112](../architecture/ADR-112-current-work-selection.md).
[PR #104](https://github.com/MrQuality/kehila/pull/104) records source checks,
independent review and remaining delivery status; merging remains with the
maintainer. This does not qualify the full configurable M1 model or real-data use.

## Owner journey

Sign in, open an existing work item, and choose **Make this work item my current
work**. The current-work section displays the persisted choice. Open another
item to replace it, or choose **Clear my current choice**. This changes the
owner's personal context only. It does not change either item's status, phase,
estimate, resources or lifecycle.

The server stores this choice independently of the page and browser profile.
Reloading or signing in again reads that stored state. The current-work section
survives navigation between project views. A saved command receipt describes
that command's original result; **Refresh current work** reads the current
choice separately. Another tab may have changed it since the command succeeded.

If access to the selected item's project is revoked, a current read still returns
the owner's selection version, but hides both project and item identifiers. The
page reports that the chosen item is unavailable. This describes target access;
it does not mean the browser disconnected. The owner can still clear the choice.

## Unconfirmed requests

Before transmitting a set or clear, the page commits its exact operation token,
expected version and target to a separate IndexedDB journal. A storage failure
before transmission prevents the send. The journal stores no credentials.

If a valid reply cannot be confirmed, the request remains tracked. Closing the
tab or reloading restores its identity in the same browser profile and HTTPS
origin. Restoration sends nothing automatically.

- **Check this request's outcome** only reconciles the original identity. An
  absent result describes the lookup's ordering point; concurrent work may still
  commit.
- **Retry this exact request** explicitly resends the same bytes and expected
  version. It never substitutes a new token or rebases onto a newer selection.
- **Stop tracking this request** attempts local logical deletion. It does not
  cancel a request, undo a committed choice or establish that the server rejected
  it. A failed deletion is visible and blocks another choice until reload and
  inspection.

A version conflict reports the observed version and closes retry of that stale
command. Refresh the persisted choice before preparing a new action. Defined
missing-item and archived-target rejections also close retry of the tracked
command. An original replay
receipt is never presented as a new current-state read.

Journal eligibility lasts seven days from original preparation; retries do not
extend it. Deleting browser profile/site data loses that browser's tracking of an
unknown operation. Another browser cannot discover an unknown token. These limits
do not expire the server's current selection. Signout and password/binding changes
coordinate edit and selection journal cleanup, retaining independent identities
and visible cleanup failures. Page visibility changes suspend callbacks and
restore retained records when the page resumes; they do not silently resend them.

Recovery-storage failure blocks preparation, retry, reconciliation and forgetting
of selection commands until recovery is usable again or the page is reloaded as
instructed. It does not block reading the server's current selection. Refresh
still requires a fresh owner-binding check and rejects superseded callbacks or a
signed-out session. Reading current work neither changes the local journal nor
releases a failed-cleanup latch, including after a visibility change.

## Native boundary

All three routes require the existing browser session, origin, client marker and
CSRF admission. They expose no CLI equivalent and accept no caller user/actor.
The installed owner supplies the subject.

| Route | Exact body | Effect |
| --- | --- | --- |
| `POST /selection/read` | `{}` | Read own version and `none`, `available` target, or identifier-free `unavailable` target. |
| `POST /selection/set` | `operation_token`, `expected_version`, `requested` | Set or clear with an exact expected selection version. |
| `POST /selection/reconcile` | Same command shape as set | Lookup only; never execute an absent command. |

`requested` is `null` or exactly `{project_id, item_id}`. Tokens are 32 lowercase
hexadecimal characters generated from 128 random bits; versions are canonical
decimal u64 strings. IDs retain exact UTF-8 bytes, are nonempty, at most 128 bytes,
and exclude NUL. Raw requests and complete responses are bounded to 4 KiB.
Duplicate/extra fields and malformed values are rejected before application work.

Fresh non-null selection needs a coherent current project grant and a live target.
Clear needs self authority, including after former-target access is revoked.
Historical resolution checks the **original recorded target's** current grant
before reuse, expiry or receipt disclosure. A changed retry target cannot replace
that grant. Original replay does not reapply mutable target/version eligibility.

| Observed result | HTTP | Meaning |
| --- | --- | --- |
| `accepted` / `replayed` | 200 | Original successful result; independently read current state for display. |
| `absent` | 200 | No recorded operation at the authorized lookup's ordering point. |
| `selection_version_conflict` | 409 | Selection version changed; no new success for this command. |
| `operation_id_reused` | 409 | The retained token belongs to different command bytes. |
| `archived_target` | 409 | A new choice cannot select the archived target. |
| `invalid_reference` | 404 | The requested item was absent within the authorized project. |
| Generic forbidden | 403 | Required current authority is missing; disclose no recorded target details. |
| `replay_expired` | 410 | The identity remains permanent but its replay payload is ineligible. |
| `preparation_unavailable` | 503 | Three ordinal-stale preparations exhausted this request's allowance. |
| Unavailable / unknown outcome | 503 | No inference of rollback; retain and reconcile the original identity. |

## Storage and coordination

Fresh product schema version four commits the operation core, payload, immutable
event and current head together. An unchanged successful choice, including an
initial empty clear, creates an event without advancing the selection version.
Its independent event ordinal always advances. Permanent history retains the
original target after payload retirement. Live replay eligibility lasts ninety
days; expiry never makes a token executable again.

Bounded read-only repeatable-read preparation releases its transaction and database
lane before CPU validation and complete response admission. New writes consume a
sealed packet in a distinct fresh fenced transaction. The accepted branch checks
core/payload/event/head agreement and deferred constraints before commit. Unknown
commit stays unknown. An intervening no-op permits at most two additional
preparations of the identical command under the original deadline and serving
reservation. A changed selection version remains a conflict.

Historical replies are fully admitted before fresh replay authorization. The
final trusted-clock check follows all payload reads and exact identity checks.
Confirmed transaction cleanup and live session/deadline gates precede delivery.
No payload lookup, result reconstruction or response encoding follows that final
authorization.

The profile adds no serving DDL, populated migration, general archive/restore,
cross-user administration, new trust policy or provider. Existing native routes do
not expose archival. Future archival exposure must implement atomic affected-user
selection clears and their contention contract first.

## Evidence boundaries

The selected native fixture exercises real PostgreSQL, HTTP and verified TLS;
its separate SQL observer checks exact ordered identities, no-op versions,
head/event/payload agreement and unchanged unrelated state. Fault controls observe
their gates, waits or injected errors and own cleanup. The clock fixture changes
only a disposable database clock and requires restoration in its normal path;
outer disposable-environment cleanup remains the fallback after helper panic.

Node interface tests use ports and a simulated DOM. They do not establish real
IndexedDB durability, browser lifecycle or the visible owner journey. Actual
browser evidence, required checks and exact-head review remain distinct delivery
gates. Historical failures remain separate from later affected reruns.

## Qualification on 2026-11-21

The measured native/browser observations use implementation commit
`771d5bdb5c87b4c0155fdabf4d74c58205df0124`. Subsequent documentation-only closeout
changes retain that attribution; they do not turn old observations into a new
runtime execution. PR #104 tracks final publication checks and committed-head
review, which remain required before review readiness.

| Boundary | Actual observation | Limit |
| --- | --- | --- |
| Native authority, storage and transport | Real PostgreSQL 16.15, HTTP and verified TLS exercised set/clear/no-op, original replay, conflicts, rejected targets and revoked authority, rollback, uncertain commit, native runtime reconstruction within the test process and reconciliation, with independent state observers | These are selected disposable-owner cases, not OS-process restart, populated upgrade, general archival or disaster-recovery qualification |
| Visible browser journey | Windows Chrome 157.0.8103.80 selected A, retained A after reload and fresh login, selected B, cleared the choice and observed a competing authenticated client's version conflict | Two authenticated browser contexts exercise the server conflict; this is not an exhaustive real-browser multi-tab journal campaign |
| Unknown reply and persisted identity | A committed B/version5 reply was deliberately lost. Reload restored the original identity without automatically sending a selection request. After a later accepted A/version6 choice, explicit reconciliation returned original B/version5 while a separate read/display showed current A/version6 | Same-profile recovery only; this does not discover unknown operations in another profile or establish power-loss durability |
| Independent durable state | After normal native release, a separate SQL observation of all 24 tables in the same original database matched the final native observation: six accepted selection operations, no duplicate acceptance and unchanged unrelated state | A response or normal process exit alone would not establish these storage facts |
| Release and cleanup | Browser, action and native processes exited normally with joins and native EOF; owned runtime resources were removed, the provider stopped, temporary trust entries and successful test profiles were removed, and absence was checked | Failed evidence is retained separately; filesystem absence is not secure erasure |

The complete browser/SQL/Root-cleanup command took 394.203 seconds. Separate exact
certificate cleanup took 6.485 seconds. The earlier required full local check took
620.297 seconds including cleanup at the same implementation commit; its full
hosted qualification also passed. These command spans are not total engineering
effort or an end-user latency promise.

The browser run's sampled aggregate memory peak was 5.999596 GiB under the 6 GiB
limit. Available memory fell to 0.276310 GiB, producing one 2 GiB reserve warning;
there were no recorded resource alerts or sampled Windows low-memory signals.
This narrow margin does not establish comfortable headroom, exclude peaks between
samples or explain earlier resource-pressure failures.

Transactional port and controlled adapter tests cover additional replacement,
late-callback, binding/signout, transaction-completion and failure cases. They
remain tests at those boundaries, not claims that every such combination was
executed through a real browser. The selected browser observations establish the
strict transaction-completion and same-profile restoration path used above.

Earlier failed attempts remain failed. Their later corrections and the final
pass do not retrospectively supply missing original observations or establish the
cause of every earlier visibility/resource problem. The
[delivery retrospective](../postmortems/2026-11-21-current-work-selection-delivery.md)
records the proportionality and resource follow-up.

## Review follow-up on 2026-11-21

Current reads now remain available when browser recovery storage cannot be read
or cleanup cannot be confirmed. The resumed page establishes fresh owner binding
before restoring either journal. Neither a successful read nor that binding check
creates a recovery session, changes the journal or releases a failed-cleanup latch.
Mutations and recovery actions remain blocked until their own admission succeeds.

Historical replay and retained-event reads share the same structural validator.
Replay still independently compares its decoded result with recorded history;
retained-event verification still binds the owner and request fingerprint, without
constructing a discarded replay response. Read rollback handling reuses the
existing helpers with the same cleanup-before-release and fatal-failure behavior.
SQL authority, storage constraints, codec versions and transaction policy are unchanged.

| Selected follow-up check | Observation | Scope |
| --- | --- | --- |
| Controller and selection journal | 39 passed | Storage/lifecycle failure, fresh owner checks, blocked mutations and malformed replies at their ports |
| Page lifecycle with existing controllers | 41 passed | Includes both journals failing before their own owner lookup; an independently authorized current read remains reachable |
| Three isolated client guard controls | Each intended mutation failed its matching test; restored controller passed all 29 tests and copied-source equality | Archived offered item, unsupported absence codec and impossible version-zero non-null receipt; shared source unchanged by experiments |
| Authority history units | 10 passed | Original identity/result/transition predicates, retained fingerprint and structural rejection |
| Selected native selection/recovery fixture | Passed, with a separate SQL observation and completed evidence/reset receipt | Existing real PostgreSQL and HTTP/TLS selection case, including read composition; no new read-specific cleanup injection is claimed |

The availability regressions first failed on the old controller/page paths. An
earlier page rerun failed an old assertion requiring the entire authenticated
view to remain hidden; the corrected assertion preserves visible reload guidance,
the durable identity, disabled retry and no mutation/reconciliation. Those earlier
failures remain separate from later passes.

These selected observations remain applicable to the implementation tree committed at
`f980e331f26bd904678013135d82b1bb97b9f7db`. Normal commit hooks and final hosted results are
tracked in PR #104. Earlier actual-browser observations retain their original
`771d5bdb` attribution and limits; the new availability paths were checked through
controller/page tests, with no new actual-browser campaign. No throughput or
resource saving is inferred from the refactoring.

[Issue #105](https://github.com/MrQuality/kehila/issues/105) tracks the original
combined-workload memory concern under B-025. Smaller follow-up checks are different
workloads and do not demonstrate that the original pressure has been resolved.

The first hosted follow-up at `1ff103c` failed strict production-library lint:
`Historical.event` had lost its production consumer after the extraction. The
correction removes that unused private field and its redundant private-field test
assertion while retaining the exact receipt assertion and all validation checks.
Formatting, strict authority all-target Clippy with warnings denied and the ten
history tests passed on the correction. The same hosted run also failed while
creating pristine native test databases, before application cases; its database
container was no longer running and the available log does not establish why.
That failure remains separate from the passing local selected native fixture.
PR #104 tracks subsequent hosted results; no cause or full-suite pass follows
from these targeted checks alone.
