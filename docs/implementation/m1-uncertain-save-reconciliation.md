# M1 uncertain-save reconciliation proposal

Status: Proposed for maintainer review, 2026-10-24. No client/API implementation.
Related: B-006/M1-08, #49, D-032/D-033/D-034,
[authority and replay](../architecture/ADR-101-task-authority.md).

## Problem

A creation may commit while its response is lost. A current-record GET can return
absence before that in-flight transaction commits, or show later edits that hide
an earlier success. Replacing the operation token after that GET can create a
second effect. A timeout, authentication failure or expired replay is not proof
that the first request rolled back. Deleting a browser retry record cancels no
server transaction.

## Recommended flow

1. Before dispatch, freeze and persist the bounded business request envelope:
   stable authenticated actor, installation, command family, typed target,
   operation token, codec/version, expected revisions/versions and exact original
   request bytes. Preserve collection order and exact values. Store no password,
   bearer, session cookie or CSRF token in the envelope. Reuse current valid CSRF
   separately when sending. A local digest can detect accidental changes; it is
   not proof of server authenticity or authorization.
2. Require the local envelope write to succeed before first dispatch. If browser
   storage is disabled/full, show a clear error and keep the unsent draft; do not
   send a mutation with a promise of reload recovery that cannot be met. The
   proposed first profile covers normal reload of the same tab. Tab closure,
   erased storage or browser data loss can still destroy a sessionStorage journal;
   they must remain explicit limits, not automatic failed-save classifications.
   A same-origin auxiliary window can initially receive a copy of the journal;
   the copies then have independent storage. Clearing or abandoning one copy does
   not clear another. See the [HTML session-storage rules](https://html.spec.whatwg.org/multipage/webstorage.html#the-sessionstorage-attribute)
   and [storage-cloning algorithm](https://storage.spec.whatwg.org/#storage-sheds).
   This is a lifecycle case to test, not a broader persistence guarantee.
3. On a lost response, retain an `OutcomeUnknown` state. Offer **Retry same save**
   using the exact retained request and original token. Reauthenticate as needed,
   verify the original actor/installation, then use current credentials. Never
   silently rebind to another actor, generate a new token or replace expected
   versions under the old token. Unrelated reads/work may continue if authorized;
   this is not a global application lock.
4. The server uses the existing scoped command/replay contract, not a current
   object GET: validate ingress; acquire the relevant authority/operation fences;
   check current original mutation grants; resolve the original scoped operation
   before mutable state/version rejection. A concurrent original attempt must
   finish or roll back through that serialization point before the retry decides.
   A recorded success returns its saved result; otherwise only this same intent
   may execute under its original identity. No separate unauthenticated lookup
   endpoint, new cancellation API or second reservation table is proposed.
5. Clear/retire the envelope after a confirmed success or a coherent server
   rejection that establishes no success for this request. Distinguish outcome
   from access: `401`, `403`, transient `503`, transport loss and timeouts cannot
   establish non-commit. Keep the unresolved classification when the outcome
   cannot be disclosed under current grants. A reused-token/content mismatch is
   an integrity/conflict state; do not repair it by silently allocating a new ID.
6. A retained success tombstone / `replay_expired` means the earlier accepted
   operation cannot execute again. Full historical results may be unavailable;
   refresh current state for orientation, without calling that original failure.
   Restore, lost local journals and unknown historical outcomes need explicit
   review/recovery behavior rather than automatic resubmission under a new token.
7. Creating a new intention is a separate user action. While the earlier outcome
   is unknown, default to reconciliation. An explicit **Stop retrying** option
   must state: the earlier save may already have happened or may still complete.
   It stops local retry only. Do not automatically follow it with a replacement
   creation; show current state as information and preserve the uncertainty.

### Supported restore and retained browser requests

[ADR-105's accepted restore security contract](../architecture/ADR-105-local-owner-authentication.md#consequences-and-impacts)
requires credential invalidation, removal of the restored password verifier and
an authentication-blocked API until local owner recovery completes, while
preserving actor/operation/tombstone identities. Signing in again alone does not
establish that a retained request belongs to the restored history.

The [backup/restore acceptance case](m1-backup-freshness-storage-budget.md#proposed-acceptance-cases)
must distinguish retained successes/tombstones from post-backup operations lost
from the restored ledger. Test a tab whose expected version is ahead of the
restored server and an original or cloned journal retrying a lost-ledger token
after reauthentication. Also test a creation with no existing-item version
precondition and an edit whose expected base version matches the restored state,
but whose later success record was lost. Version comparison alone cannot detect
every request crossing the restore boundary. Preserve the draft/exact request and
explain the recovery uncertainty; do not silently change versions/tokens or
execute the request as a fresh intention. Review the restore/reconciliation
design before implementation and prove its fence before supported use. This outcome selects no
restore-generation field, token format or new error code, and qualifies no
browser/restore behavior.

## Trade-offs

| Choice | Benefit | Cost / limitation |
| --- | --- | --- |
| Exact retained request + original-token retry | Uses the accepted replay ledger and avoids duplicate effects from ordinary lost responses | Depends on current authorization and retained replay information; unavailable servers can leave the UI pending |
| Refuse dispatch if journal storage fails | Fails before a server effect instead of pretending recovery is available | Browser storage becomes a prerequisite for this supported mutation flow; show an actionable error |
| Same-tab reload journal initially | Small implementation surface; no new backend journal service | Does not guarantee recovery after closing/erasing the tab or losing browser data; broader persistence needs a separate privacy/cleanup contract |
| Keep unknown distinct from failed | Honest user-visible state and safer creation behavior | The user cannot always get an immediate yes/no; retries may require signing in again |
| Explicit abandonment without cancellation | Allows the user to stop local retries without claiming to undo a server action | It cannot guarantee no earlier success, and must not be a hidden duplicate-create path |

## Required review and evidence before implementation

Review journal bounds/cleanup, supported browser persistence and user wording.
Test response loss before/after COMMIT, the original request still running, exact
replay after reload and reauthentication, revoked grants, replay expiry/tombstones,
storage-disabled/quota failures, journal loss and supported restore. Apply native
command/authority integration and actual browser checks at their existing gates.
No T01-T16 case or supported UI behavior is qualified by this proposal.

Include two contexts reconciling the same cloned envelope concurrently, including
an original attempt still running, one context clearing its copy, and later
reauthentication. Both must preserve the original request/token and actor/
installation checks; local clearing cancels neither the other context's retries
nor server work. Verify supported-browser tab duplication and opener/no-opener
cases explicitly rather than assuming every browser UI operation copies storage.
These remain planned browser cases; no journal or multi-tab UI is implemented.
