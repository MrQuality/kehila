# Exact edit recovery in a browser profile

Status: implementation and development verification in progress. Real IndexedDB,
Chrome loss/restart and affected native integration qualification remain pending.
This record does not establish supported crash durability from simulated tests.

## Selected behavior

An uncertain direct edit keeps its original operation token, exact request bytes,
expected item/configuration versions and historical-response validation facts.
Normal Chrome on Windows can restore that record in the same profile and HTTPS
origin after reload, close/reopen or browser-process crash/restart. Restoration
does not execute it: the owner can inspect the retained changes, check the recorded
outcome, explicitly retry the exact request, read current state separately, or
stop tracking it. No automatic fresh token, rebase or resend is supplied.

The journal has one tracked edit slot across tabs and projects. It accepts at most
1 MiB of UTF-8 request wire plus 2 KiB of identity/control metadata. These limits
do not bound JavaScript heap, serialized storage overhead or physical disk use.
Eligibility is seven days from original preparation under an honest local clock;
reading, retries and sign-in do not renew it. Known clock rollback is reported.
The server's independent replay-retention and permanent-identity rules remain in
force; local expiry does not undo an accepted write or make its identity new.

The scope excludes another browser/device, private or ephemeral profiles,
deleted profiles, cleared/evicted browser data, OS/disk failure and compromised
same-origin code or operating systems. No permission prompt, certificate trust
installation or application encryption key is part of recovery.

## Storage and ownership

`edit_recovery.js` supplies a closed versioned codec, journal session and IndexedDB
adapter. The adapter uses one database/object-store key and strict readwrite
durability. Request success is insufficient: the transaction must complete before
the caller can send. Missing storage, unsupported strict durability, blocked open,
abort, quota or timeout is visible and prevents an unconfirmed first send.

The bounded root contains a generation, observed clock high-water mark, optional
edit entry and SHA-256 checksum. The checksum covers the encoded closed payload
and detects accidental corruption; it is not authentication against an attacker
who can modify same-origin storage and scripts. The codec validates UTF-8,
metadata/wire identity agreement, bounds, owner/installation, versions, field
actions and lifecycle before use. No cookie, password, bearer, CSRF token, headers
or surrounding page/session object is retained.

Hashing uses an immutable snapshot outside the transaction. A readwrite
compare-and-swap checks the exact prior serialized root and synchronously checks
live clock/eligibility at actual admission, then waits for completion. A contended
write fails visibly rather than reconstructing or automatically retrying work.
An admission made before concurrent invalidation may still reach the server
afterward; browser storage and HTTP are not one atomic transaction.

The generation remains when the edit slot is empty. A page captures it before
fresh owner binding, so a delayed binding or preparation cannot reopen a session
invalidated by another tab's signout. Settlement, abandon and clearing rotate the
generation. Reply adoption and settlement check the matching journal identity;
late success cannot delete a replacement. A separate page epoch invalidates local
callbacks across signout, binding changes and disposal.
Visibility changes and page restoration hide recovery content and disable its
actions until fresh binding and journal validation finish. A single queued
refresh handles resumption while another page action is still completing;
resumption never automatically transmits a mutation. Unchanged-owner revalidation
preserves current selections and unrelated unsaved forms. A confirmed changed
owner clears those old selections/forms instead of exposing them to the new owner.

## Privacy and lifecycle

Edit text is readable to same-origin scripts and software/people with access to
the browser profile. Normal browser/OS profile isolation is the selected boundary,
without application-level encryption. The UI does not expose restored payloads
before fresh matching authentication.

Routine project/view changes preserve the journal. Explicit signout first
invalidates local recovery actions and attempts matching-owner journal clearing;
the server logout request must still be attempted if storage clearing fails.
Successful or uncertain password-change signout and confirmed owner/installation
change use the same local clearing boundary. A rejected password change or a
transient auth/network failure alone does not discard durable tracking.

Clearing, expiry and abandon mean logical removal from the application journal,
not cancellation, server revocation or forensic erasure. Expiry cleanup runs on
the next authenticated application visit, subject to storage errors. Generation,
clock and checksum control metadata can remain without edit content. Closed or
suspended browsers cannot guarantee immediate cleanup; browser/OS remnants and
backups have no promised erasure deadline. Failed durable invalidation blocks
the detecting context and is visible; another context may still use the old
identity until durable invalidation succeeds or server authority rejects it.
The detecting page keeps a cleanup-failure latch until page disposal; automatic
session refresh or sign-in callbacks cannot silently clear it. Reload permits a
fresh authenticated inspection of whatever exact record actually remains. It
does not execute that record or establish that the previous cleanup succeeded.
Suspension invalidates stale callbacks but does not erase knowledge of a cleanup
failure reported while the page was hidden.

## Verification boundaries

| Contract | Development evidence | Remaining qualification |
| --- | --- | --- |
| Closed codec, identity, bounds, expiry, high-water clock | Valid/rejection and asynchronous digest/transaction race controls | Composition with actual browser storage |
| Commit before send | Controlled completion/abort adapter events and page transmission barrier | Native IndexedDB completion and strict durability |
| One slot, logout/abandon generations and late replies | Transactional-port and simulated page controls | Actual cross-tab storage ordering and auth integration |
| Exact historical check/retry and current-state separation | Page and existing native contract source/tests | Affected PostgreSQL/HTTP/TLS execution and independent state/history witnesses |
| Reload, close/reopen and process loss | Prepared contract only | Owned real Chrome profile/process campaign, original bytes/token and no duplicate native writes |

Development policy/page tests use a transactional port or controlled API events;
they are not a browser implementation or durable-storage proof. Required local
and hosted checks, reviewed harnesses, final exact-head review and accurate PR
readiness remain part of delivery. A failed attempt remains failed even if a
later repaired check passes.
