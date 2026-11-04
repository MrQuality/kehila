# Disposable direct-SSH reachability probe

This SP-007 research fixture tests whether an admitted local SSH client can
reach a synthetic contained target on a standard GitHub-hosted Linux runner.
It is not a product backend, browser test, production session bootstrap or a
qualification of hosted-runner ingress support.

The operator first reviews and pushes the exact source, checks its required
gates, and obtains explicit admission for the temporary listener and encrypted
artifact exchange. The local owner requires a fresh external private storage
directory, GitHub CLI access to the selected repository, OpenSSH and OpenSSL.
It creates temporary source/ref/phase variables and a configuration secret;
existing entries with those names cause rejection. Private client and recipient
keys for the admitted operator remain local. A distinct ephemeral contained
control key is generated and removed inside the hosted job. No product credentials
or application trust are involved.
Temporary configuration requires a single operator. Journaled intent and remote
value/metadata checks reconcile ambiguous API outcomes, but GitHub's separate
read/write requests provide no atomic compare-and-swap guarantee. Conflicting
changes stop deletion and remain explicit cleanup failures for reconciliation.

The control-only tag runs the Linux encryption producer without exposing a
listener. The local owner verifies the authenticated artifact producer, archive
digest, successful authenticated decryption and exact session bindings before
creating a separate live tag at the same source commit. The tag trigger avoids
requiring the new workflow to be merged into the default branch. Neither phase
changes PR state.

The live job checks the effective SSH policy, actual contained SSH forwarding and
an eight-second expiry control with no published port before exposing one high
IPv4 port. Rootful bind-mount ownership is checked explicitly for StrictModes.
The SSH role requires the
admitted key and exact observed client source, denies sessions and reverse
forwarding, and permits only the synthetic loopback destination. VPN address
rotation causes rejection; it does not widen policy. The two-minute container
lease and host owner are separate controls, not proof of a hard remote deadline
under host compromise or platform failure. The ten-minute job ceiling is another
separate limit. An egress-derived endpoint is only a reachability hypothesis.

At most three local attempts use pinned host identity and five-second execution
deadlines, followed by the existing separately bounded descendant/pipe cleanup.
This is not a hard wall-clock guarantee under operating-system failure.
`ssh -W` tests direct TCP forwarding without creating a local
browser listener. Reachability requires both the expected fresh HTTP response
through authenticated SSH and matching runner-side synthetic response evidence.
Successful cleanup with zero responses is an unreachable experiment result,
not a passing path. A timeout alone cannot identify the upstream failure or
exhaust all local-to-runner possibilities.

The encrypted artifact is a temporary admitted return channel. Only an explicitly
selected DER envelope is uploaded, with one-day fallback retention. The local
owner deletes it after consumption; independent local and job-finally cleanup
delete and verify any remaining owned artifacts. Failures remain visible; no
guarantee is made that downloaded ciphertext copies can be retracted. A permanent
return channel remains a design question. Cleanup also removes only the owned
containers, tags, variables and secret, and deletes local private keys. Raw
addresses, keys and diagnostics stay outside the checkout and public output.
Final cleanup permanently closes container admission and serializes removal with
in-flight creation, so delayed setup cannot create another listener after final
absence is reported. A lost engine-create response with no observed resource
remains an explicit cleanup uncertainty; an empty container query alone cannot
prove the request will never complete. Artifact deletion remains independent.

The base image is digest-pinned; distro OpenSSH packages are resolved at build
time. This is a limitation of the research fixture, not a locked dependency or
release qualification claim. Revalidate changed package/image/engine versions,
policy, source, network path, recipient or workflow. The probe never configures
a local inbound receiver, VPN, firewall, router, browser trust, relay or paid
capacity. Runner-to-local fallback remains a separate decision after investigation.
