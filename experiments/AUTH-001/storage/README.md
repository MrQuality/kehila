# Disposable A1 storage proof

This opt-in proof implements the disposable SQL portion of the
[A1 contract](../../../docs/implementation/local-owner-authentication-a1.md).
It does not install a supported authentication database, serve an HTTP login,
replace the prototype storage, or qualify the rest of A1–A3.

Current state: enrollment, startup, post-wait fencing, credential management and
session-audit confirmation SQL are implemented. The small-workload native run
passed eleven oracle groups. Rust connection/hash/session ownership and actual
browser/CLI integration remain outside this subset.

The default Compose profile uses the native foundation's pinned PostgreSQL 16
image and strict container validator: no networking or published ports, no host
or persistent mounts, 256 MiB memory, one CPU, 128 processes and a 256 MiB
ephemeral data filesystem. A fresh owned container receives the unchanged public
foundation installer followed by the disposable overlay. The proof never resets
an existing database. Resource-limit enforcement is a distinct qualification
from SQL enrollment behavior.

Run only with sufficient host memory, an initially stopped exclusively owned
Podman machine and externally configured temporary/log storage:

```text
python experiments/AUTH-001/storage/run.py --podman-machine <owned-machine> --exclusive-podman-machine
```

The explicitly accepted alternate is selected with `--profile small-workload`.
It retains the pinned image, network isolation, 256 MiB ephemeral data filesystem,
fixed fixtures, work deadline and ownership checks, but does not request per-
container cgroup caps. A separate validator checks that profile; the strict
foundation validator is unchanged. There is no automatic fallback after failure.
A small-workload pass cannot qualify container CPU/memory limits or the native
foundation's full verification suite.

The runner checks RAM before startup, after machine startup and after container
creation, and has a 600-second work budget. Each native participant has its own
bounded lifetime and explicit backend/process cleanup. The machine and Compose
group are owned resources; teardown failures remain failures. Raw service/session
output belongs in external evidence storage, not in the checkout or public logs.

Fixtures contain only fixed synthetic identifiers and a bounded placeholder
verifier. The placeholder is deliberately not a password hash: these tests
qualify storage behavior only. Password policy, PHC validation and hashing require
separate actual Rust tests before serving integration.

The enrollment cases require atomic rollback; one winner after an observed lock
wait between competing initializers; reconciliation after a lost response without
password replacement; runtime/admin capability denials; immutable owner binding;
bounded verifier/counters; and append-only audit history. Credential-management
cases add post-actor-wait stale proof, competing replacements, generation-only
revocation, inactive-owner recovery with structural history preservation, overflow
and a distinctive injected audit failure. Session-audit cases add fixed-identity
confirmation, conflicting attributes, concurrent logout, unchanged timestamps,
privilege denials, rollback, a distinct audit fault and stale authority. These
calls do not establish real credential eligibility or local revocation. Startup uses the caller's dedicated session
advisory acquisition. SQL checks the exact calling backend/database/lock object;
`pg_locks` does not prove session duration. Native acquisition survives both
commit and rollback and excludes a different backend. Production Rust ownership,
loss observation and admission/output suppression remain unimplemented.

## Observations, 2026-10-27

The historical [sanitized enrollment receipt](results.enrollment.json), from
commit `4b4ba4dd18cc7f19a3bd24d7032015cd86b2e0f3`, records the exact
executed command/profile, LF-normalized source hashes, unchanged foundation/shared
helper baseline, actual PostgreSQL 16.15, Podman 5.8.3, Python 3.10.11 and Windows
host observations. The provider was Podman/WSL 2. The final receipt is emitted only
after owned container and machine cleanup completes. Its harness hash identifies
that enrollment checkpoint, rather than the later authority runner.

The historical [authority receipt](results.authority.json), from
`1ce7fc6757ac26d3cf8ff6ac339e2e2690571df4`, identifies the six-group
source and restored native result. Moving the authority read ahead of the locks
failed its queued stale-runtime oracle; the restored source passed all six groups.
It includes the same actual tool/provider versions and confirmed owned cleanup.

The historical [credential-management receipt](results.management.json), from
`aa2e484789083038b8af1bf23ef2b5a5ca7f9cb0`, identifies the
nine-group source and restored native result. Ignoring auth generation allowed
a stale proof after global revocation; advancing verifier revision on global
revocation broke its independent oracle. Both changes were restored, and all
nine groups passed. The distinct `P0A01` audit fault proves the write reached the
injected audit boundary before rollback. History rows are structural fixtures,
not valid domain codec/command evidence. Global revocation is separate from
revoke-other-sessions: the latter must retain the current session and individually
revoke other registry entries without advancing auth generation.

The [audit receipt](results.audit.json) identifies the eleven-group SQL boundary.
Session-audit confirmation compares every fixed attribute and preserves the
original timestamp. A changed credential kind, initiating session or auth
generation cannot silently confirm a different event. Neutralizing the kind
comparison failed the conflict oracle; restored source passed all eleven groups.
`P0A02` identifies the injected session-audit write failure and intact retry.
Only narrow runtime wrappers are granted. No token, digest, password, verifier or
CSRF enters these event APIs. Confirmation returns a boolean, not a bearer, and
cannot reconstruct a registry entry. Rust must enforce actual lookup/retention,
same-logout eligibility and irreversible local revocation before calling SQL.

| Phase | Actual observation |
| --- | --- |
| Capped-profile attempt | Provider could not apply its cgroup memory limit; PostgreSQL did not start and zero enrollment cases ran. |
| First small-workload attempt | Fixture omitted the initialization password required by the image entrypoint; PostgreSQL exited before tests. Corrected with a fixed synthetic bootstrap password. No authentication security conclusion follows from this fixture failure. |
| Before overlay implementation | After foundation installation, the native call failed with SQLSTATE `3F000` because the `auth001` schema did not exist. No enrollment invariant was qualified. |
| Enrollment implementation | Four oracle groups passed: atomic/competing enrollment; simulated lost-response reconciliation; restricted capabilities; binding/verifier/audit constraints. The contention oracle observed a real blocking relation before releasing the winning transaction. |
| Targeted regressions | Removing the explicit enrollment row lock failed the competing-enrollment SQLSTATE oracle. Disabling immutable-binding protection failed its constraint oracle. Both source changes were restored; the restored native run passed all four groups. These are two targeted checks, not a mutation score. |
| Before authority implementation | New native authority calls failed with expected `55000` versus observed `42883` (missing function). This was not a passing rejection oracle. |
| Startup/takeover implementation | Six groups passed, including the four enrollment groups. Dedicated session-lock survival, startup rollback and reused-attempt rejection, takeover waiting behind fenced work, queued old-runtime denial, inactive/blocked/stale-revision denial and runtime overflow rollback were exercised. |
| Pre-wait snapshot regression | Moving the authority read before the locks caused the queued takeover oracle to accept stale authority and fail. Original SQL was restored and all six groups passed again. |
| Credential/recovery implementation | Nine groups passed, including existing enrollment/authority groups. Post-actor-wait rejection, one winner from identical old password metadata, generation-only revocation, inactive actor/permissions/history preservation, credential-counter overflow and audit-failure rollback were exercised. |
| Credential regressions | Ignoring auth generation failed the post-global stale-proof oracle; changing the global-revocation revision increment failed its counter oracle. Both restored; final nine-group run passed. |
| Session audit implementation | Eleven groups passed, including the earlier nine. Identical/concurrent confirmations retained one original event; valid but conflicting kind/initiator/auth attributes failed with `XX001`; `P0A02` write rejection, rollback and stale generations left no session-audit residue. |
| Audit comparison regression | Replacing the stored kind with the proposed kind in the fixed-attribute comparison incorrectly accepted a conflict and failed its oracle. Restored SQL passed all eleven groups. |

Response loss is simulated by discarding the successful enrollment result and
reconciling its fixed claim; this is not an injected network disconnect. The
runner's selected profile is part of the evidence. A pass under `small-workload`
does not qualify CPU/memory-limit enforcement, supported installation, production
storage adoption, password cryptography, or complete A1–A3/T01–T16 behavior.
