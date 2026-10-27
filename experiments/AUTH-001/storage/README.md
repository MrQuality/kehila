# Disposable A1 storage proof

This opt-in proof implements the enrollment portion of the
[A1 contract](../../../docs/implementation/local-owner-authentication-a1.md).
It does not install a supported authentication database, serve an HTTP login,
replace the prototype storage, or qualify the rest of A1–A3.

Current state: the enrollment overlay, roles and native oracles are implemented.
The small-workload native run passed four oracle groups. Runtime startup,
takeover, recovery and session management are not implemented by this subset.

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
bounded verifier/counters; and append-only audit history. Startup, recovery,
generation races, session audit and remaining native failure cases are not covered
by this enrollment subset.

## Observations, 2026-10-27

The [sanitized enrollment receipt](results.enrollment.json) records the exact
executed command/profile, LF-normalized source hashes, unchanged foundation/shared
helper baseline, actual PostgreSQL 16.15, Podman 5.8.3, Python 3.10.11 and Windows
host observations. The provider was Podman/WSL 2. The final receipt is emitted only
after owned container and machine cleanup completes.

| Phase | Actual observation |
| --- | --- |
| Capped-profile attempt | Provider could not apply its cgroup memory limit; PostgreSQL did not start and zero enrollment cases ran. |
| First small-workload attempt | Fixture omitted the initialization password required by the image entrypoint; PostgreSQL exited before tests. Corrected with a fixed synthetic bootstrap password. No authentication security conclusion follows from this fixture failure. |
| Before overlay implementation | After foundation installation, the native call failed with SQLSTATE `3F000` because the `auth001` schema did not exist. No enrollment invariant was qualified. |
| Enrollment implementation | Four oracle groups passed: atomic/competing enrollment; simulated lost-response reconciliation; restricted capabilities; binding/verifier/audit constraints. The contention oracle observed a real blocking relation before releasing the winning transaction. |
| Targeted regressions | Removing the explicit enrollment row lock failed the competing-enrollment SQLSTATE oracle. Disabling immutable-binding protection failed its constraint oracle. Both source changes were restored; the restored native run passed all four groups. These are two targeted checks, not a mutation score. |

Response loss is simulated by discarding the successful enrollment result and
reconciling its fixed claim; this is not an injected network disconnect. The
runner's selected profile is part of the evidence. A pass under `small-workload`
does not qualify CPU/memory-limit enforcement, supported installation, production
storage adoption, password cryptography, or complete A1–A3/T01–T16 behavior.
