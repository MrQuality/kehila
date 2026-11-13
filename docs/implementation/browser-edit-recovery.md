# Exact edit recovery in a browser profile

Status: implementation and the selected real Chrome recovery campaign passed.
The observations below cover actual IndexedDB, three browser-loss boundaries and
the native PostgreSQL/HTTP/TLS path. Final repository qualification and delivery
review remain separate gates. This is not a general browser or storage guarantee.

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

| Contract | Evidence | Coverage boundary |
| --- | --- | --- |
| Closed codec, identity, bounds, expiry, high-water clock | Valid/rejection and asynchronous digest/transaction race controls; real storage in the selected campaign | No seven-day elapsed wait or maximum-size browser stress campaign |
| Commit before send | Controlled completion/abort events and actual strict IndexedDB transaction completion before each selected fetch | Selected Windows Chrome environment; no OS, disk or power-loss claim |
| One slot, logout/abandon generations and late replies | Transactional-port and simulated page controls; matching generation and explicit settlement in Chrome | No actual cross-tab contention campaign |
| Exact historical check/retry and current-state separation | Real native PostgreSQL/HTTP/TLS execution, rendered history/current state and independent final SQL | Synthetic fixture bypasses protected startup and provider publication |
| Reload, close/reopen and process loss | Three real Chrome loss/restoration cases in one owned persistent profile, preserving original bytes/token and write identity | Selected installed Chrome on Windows, not other browsers, devices or storage-failure classes |

Development policy/page tests use a transactional port or controlled API events;
they are not a browser implementation or durable-storage proof. Required local
and hosted checks, reviewed harnesses, final exact-head review and accurate PR
readiness remain part of delivery. A failed attempt remains failed even if a
later repaired check passes.

### Observed Chrome recovery campaign, 2026-11-13

Installed, headed Chrome 157.0.8103.78 on Windows completed three fresh edit
intents in one owned synthetic persistent profile. Each edit reached the native
pre-COMMIT barrier after its exact wire and identity had completed a strict
IndexedDB transaction. Reload, clean close/reopen and whole-browser process
crash/restart each restored the same retained record. Fresh and idle restoration
observations contained no edit resend. Explicit outcome checks settled the reload
and crash cases; explicit exact-request retry settled the clean-close case.
The selected fault sequences took 4.203, 3.875 and 3.656 seconds respectively,
within the unchanged five-second window. Reload evidence proves loss of the
receiving document before release; it does not establish TCP cancellation.

A fourth distinct native edit committed version 5 while the third browser edit
remained retained. Its later historical response and rendered receipt stayed at
version 4; a separate current read and rendered field showed the exact version-5
value. Independent final SQL checked all 22 tables against the expected state,
including permanent operation/replay and revision history. It also observed zero
runtime sessions, runtime transactions and prepared transactions after normal
native shutdown, together with advisory-lock and row-fence release witnesses.

All owned Chrome Jobs joined with zero active processes, including the intentional
whole-browser crash. Native listener/coordinator, relay and helper cleanup
completed, ownership remained held through provider shutdown, and the provider
finished stopped. Transfer accounting retains 85 completed and 23 failed
transfers, including 22 unselected resets; their admitted physical release does
not make them successful transfers. Earlier failed attempts remain failed and
are not replaced by this passing campaign.

The campaign took 220.828 seconds through cleanup. Sampled aggregate working sets
peaked at 5.0823 GiB, with minimum available memory of 1.5360 GiB. One 2 GiB reserve
warning occurred; neither the 6 GiB aggregate stop nor Windows low-memory stop
triggered. The maximum sample gap was 3.4402 seconds, and one bounded retry followed
a changing Job-membership observation. These are sampled observations, not a hard
peak-memory bound or continuous measurement. This campaign does not qualify
cross-tab contention, seven days of elapsed retention, maximum-size browser
stress, machine failure or supported real-data recovery.

### Native backend for browser qualification

The ignored `native_browser_recovery_fixture` test supplies a finite backend for
a separately controlled browser campaign. It uses the shipped HTML/scripts and
the actual listener connection, HTTP admission, authentication, coordinator and
PostgreSQL path. It bypasses protected startup and provider publication; those
contracts retain their separate tests. Running ordinary library tests compiles
this fixture without starting it.

`native_fixture_environment.py` owns the shared PostgreSQL preparation and
evidence lifetime. The ordinary native verifier retains its existing selectors,
finite journeys and SQL qualification oracles. A separately controlled backend
can use that same resource owner, then stop and join its child and finish its
independent state observations before leaving the context. The environment
removes PostgreSQL before the namespace-owning runner and verifies retained
evidence before optional asset reset. Its receipt records resource/evidence
handling. Browser qualification requires the caller's outcome oracles. Default
admission still rejects attached networks and published ports; a Windows-facing route requires
its own reviewed configuration and execution admission.

Long-lived controllers can provide `release_dependents` to enforce worker release
before database cleanup, including after a partial acquisition or work deadline.
The callback must use bounded control operations, allow for no worker having
started, and raise if release is unproved. A release failure preserves the primary
error and withholds database removal, evidence export and asset reset. The outer
runner owner must independently refuse removal while the database remains or its
absence cannot be established. This failure path leaves unresolved resources
visible; it supplies no qualification.

The fixture requires an explicitly selected fresh synthetic mutation database,
external DER certificate/key files, and an owned controller. Its public origin is
`https://127.0.0.1:16291`; its listen address is supplied separately. The
certificate must be accepted through ordinary browser verification. The fixture
does not install trust, bypass certificate errors, open a host forwarding route,
launch Chrome or provision a database.

After seeding one project and item, it publishes a ready record with their
identifiers and an independent SQL baseline. The external control directory uses
atomically published, consecutively numbered `command-NNNN.json` inputs and
`reply-NNNN.json` outputs. Command objects reject unknown fields. The finite
protocol supports:

- `arm` with the original edit wire: canonicalize through the real edit decoder
  and select that complete command, including its original operation token,
  target, expected versions and changes. The acknowledgement records separate
  original-wire and canonical-command SHA-256 digests.
- `release`: permitted after the matching `arrived-NNNN.json` observation. Arrival
  means the selected new write reached its pre-COMMIT barrier; it does not prove
  commit. Internal repreparation retains the selected barrier. Replay, rejection
  and unrelated commands cannot manufacture matching arrival.
- `snapshot`: wait for all serving admission permits and hold them while an
  independent administrative connection observes permanent operation, item,
  revision and replay state. This prevents a concurrent mutation from creating a
  mixed snapshot.
- `stop`: end admission and join the listener and coordinator, then independently
  observe backend release and final durable state.

At most one barrier is outstanding, with 64 commands and a 15-minute controller
lifetime. The application's five-second request/work deadlines are unchanged.
The controller must perform any crash/join/release sequence automatically within
that deadline. Dropping a gate can release work, so controller failure, stop and
release retain an unknown mutation outcome until SQL observation. Cleanup records
forced connection cancellation, task failures and unavailable witnesses as
failures, separately from normal shutdown. Failed evidence is never overwritten.

The selected campaign above exercised this backend with ordinary verified TLS,
real IndexedDB observations, owned full-process crash/relaunch and independent
native witnesses. The fixture alone, or a successful default unit run, does not
establish those browser outcomes. Its startup/publication exclusions and the
campaign's stated environment and coverage limits remain in force.
