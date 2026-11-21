# Bounded IndexedDB component checks

The fixture exercises the shipped recovery store/journal in two browser target
contexts using synthetic databases with a unique test prefix. It checks observed
write-transaction contention, completion before compare settlement, rollback after
an injected `QuotaExceededError`, and cross-context generation invalidation with
a positive admission control. The abort is a real IndexedDB transaction event;
the quota error is injected, not evidence of actual browser quota exhaustion.

The test client requires Node 22 or later with built-in WebSocket/fetch. It adds
no package dependency. Run the client controls without launching a browser:

```text
node --test tests/browser/protocol.test.cjs tests/browser/cleanup.test.cjs
```

For a real component run, the caller must first own a disposable Chrome/Chromium
process and fresh profile, with an explicit loopback DevTools endpoint and normal
browser security settings. Do not connect to a profile containing application
credentials, personal data or an existing recovery journal. Keep profiles,
process logs and raw results outside the checkout. The browser owner supplies
memory/time limits, process-tree ownership and final cleanup; the test client
does not grant those permissions or launch/stop a browser implicitly.

```text
node tests/browser/journal.cjs http://127.0.0.1:<owned-port>
```

The harness loads the local fixture and production scripts from the same source
export. Each run creates its own synthetic database names. The contention witness
holds the first real readwrite transaction alive for at most two seconds, observes
the second transaction queued without a successful read, then releases the first
and checks both results. A missing/expired witness fails the case. Protocol calls,
database cleanup and target release have separate finite allowances.

Both database deletions are attempted even if one fails. A passing result is
printed only after the known targets and databases are released. A lost target
creation reply can leave an unknown target; caller-owned browser shutdown and
profile cleanup remain the final reconciliation boundary on every outcome.
The protocol response-size check is after WebSocket materialization, so process
resource containment is still necessary.

The controller retains the synthetic database identities before initialization.
Cleanup passes those identities explicitly and tries a surviving context if the
first context fails; successful receipts count recovered cleanup failures. If all
known contexts fail, their errors are retained together with any primary test
failure. Caller-owned profile shutdown/deletion remains necessary on every outcome.

These are selected file-origin storage-component checks. They do not qualify the
deployed HTTPS origin, cookie/TLS policy, visible owner journey, profile eviction,
actual quota exhaustion, power loss or a supported browser matrix. Existing
native/visible-browser evidence retains its original source and environment
limits. Component success cannot replace it or establish wider release readiness.
