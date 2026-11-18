# Persistent owner current work

This capability is under qualification. The contract is accepted in
[ADR-112](../architecture/ADR-112-current-work-selection.md); source presence and
component tests do not establish complete native/browser qualification.

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
