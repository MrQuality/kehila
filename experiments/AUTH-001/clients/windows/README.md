# Disposable Windows owner client

This is an experimental Windows **client** for the Linux AUTH-001 owner backend.
It does not select Windows server-key storage or provide a supported installation.
Source and local file/parser checks are prepared; real TLS, cancellation and
browser/container journeys remain qualification gates.

Use the installed .NET Framework 64-bit compiler through `build.ps1`, with a
pre-created protected output directory outside the checkout. `-Test` compiles
and runs local DPAPI/file and closed-response checks. It creates external
disposable credential fixtures and retains them for diagnosis; it never installs
trust, changes security-product settings or contacts a service.

The output path and its ancestors must be inspected and protected from untrusted
replacement. The script rejects reparse ancestors; textual path checks are not a
general atomic containment guarantee against administrator/path remapping.

The interrupted-write test creates a valid pending file with an explicit
current-user owner and protected DACL at creation, matching the cache contract.
It separately observes the ambient owner of a copied file and requires rejection
when that owner differs. No SIDs, paths or encrypted contents are printed.

```powershell
./build.ps1 -OutputDirectory $externalOutput -Test
```

Run `owner-client.exe` with one of `login`, `state`, `logout`, `password` or
`revoke-others`, followed by the exact `https://127.0.0.1:port` origin and uppercase
SHA-256 fingerprint of the selected issuer certificate. These arguments are
public configuration; **never put a password or bearer in arguments**. Passwords
use hidden console input. Explicit `--stdin` accepts bounded password lines for
controlled disposable automation; it does not enable credential output.

The transport requires TLS 1.3 and ordinary Windows certificate/IP validation,
then additionally pins the selected chain root. It disables redirects, proxy,
ambient credentials and cookies. A five-second request deadline retains owned
response disposal through body reads; actual stalled-peer behavior must still
be measured on the selected Windows profile. Headers and response bodies are
bounded. Only the backend's fixed canonical JSON response encoding is accepted;
the BCL parser/writer rejects unknown metadata, duplicate fields and trailing
bytes. This deliberately does not promise compatibility with arbitrary JSON
formatting from other servers.

For HTTP 400-599, the client also accepts the backend's exact canonical
`{"error":"request rejected"}` envelope and maps it to the existing typed status
handling. The same body on a success/redirect status, different text, mixed
success/error fields or trailing data is rejected. Server text is never displayed.
The parser regression reproduced rejection of the real error envelope before
this repair; local checks do not prove the observed live request's failure cause.

The bearer cache is bound to the origin and issuer fingerprint, protected with
DPAPI CurrentUser, and checked against owner-only file/directory ACLs. Ancestors
must have trusted ownership and exclude untrusted replacement rights. Reparse
objects and interrupted pending writes fail closed. This depends on those
verified ancestors remaining protected; it is not an atomic defense against
the same OS user or an administrator. No plaintext fallback or automatic ACL
repair is attempted. Managed strings and networking/crypto library copies are
not covered by a universal erasure claim; owned secret buffers are wiped.

Logout marks the cache as retiring before sending. A retained credential permits
only the same logout retry or explicit `owner-client.exe forget`; it never
probes a business request. Forget removes local state and proves no server audit
outcome. A failed replacement retains one fixed pending file for this explicit
recovery, rather than choosing or deleting an uncertain newest credential.
An uncertain password change clears the old local credential and requires a
fresh login using the chosen new password; it is never automatically repeated.
