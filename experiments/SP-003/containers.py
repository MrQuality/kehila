"""Owned disposable Podman resources for the isolated codec experiment."""
import json
import subprocess

from build import command


def cleanup(container):
    """Remove only this run's labelled container and its anonymous volumes."""
    try:
        inspection = subprocess.run(["podman", "inspect", container], capture_output=True, timeout=15)
        if inspection.returncode:
            exists = subprocess.run(["podman", "container", "exists", container], capture_output=True, timeout=15)
            return [] if exists.returncode == 1 else ["Unable to prove owned container absence"]
        details = json.loads(inspection.stdout)[0]
        labels = details["Config"]["Labels"]
        if not isinstance(labels, dict) or labels.get("purpose") != "kehila-sp003":
            return ["Unexpected container ownership label; cleanup refused"]
        removal = subprocess.run(["podman", "rm", "--force", "--volumes", container], capture_output=True, timeout=30)
        return [] if removal.returncode == 0 else ["Owned container removal failed"]
    except (OSError, ValueError, KeyError, IndexError, TypeError, subprocess.SubprocessError) as error:
        return ["Cleanup failed: " + type(error).__name__]


def run_probe(image, name, arguments, cleanup_errors, *, timeout=120):
    """Reconcile the named remote resource even when the local client times out.

    The caller retains cleanup diagnostics separately so they cannot mask the
    primary launch/deadline error. A cleanup failure also blocks successful runs.
    """
    try:
        output = command(["podman", "run", "--name", name, "--label", "purpose=kehila-sp003",
                          "--rm", "--network=none", "--memory=128m", "--cpus=1",
                          "--pids-limit=64", image, *arguments], timeout=timeout)
    finally:
        errors = cleanup(name)
        cleanup_errors.extend(errors)
    if errors:
        raise RuntimeError("Probe cleanup failed: " + "; ".join(errors))
    return output
