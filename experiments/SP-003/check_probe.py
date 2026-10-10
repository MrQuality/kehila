"""Real Podman checks for probe exit, deadline and owned-container cleanup."""
import argparse
import json
import os
import subprocess
import sys
import uuid

from build import REPO
from containers import cleanup, run_probe

sys.path.insert(0, str(REPO / "experiments/SP-002"))
from manage import IMAGE


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--podman-connection", required=True)
    args = parser.parse_args()
    if sys.flags.optimize or sys.platform not in {"win32", "linux"}:
        raise RuntimeError("Use unoptimized Python on Windows/Linux")
    os.environ["CONTAINER_CONNECTION"] = args.podman_connection
    records = []
    for case in ("success", "nonzero", "timeout"):
        name = "kehila-sp003-probe-" + uuid.uuid4().hex[:12]
        errors = []
        # Every command emits a marker from inside the actual container. On
        # deadline this proves the client reached a running remote process.
        script = "printf 'probe-ready\\n'; " + {
            "success": "df -Pk /", "nonzero": "echo deliberate-exit >&2; exit 7",
            "timeout": "exec sleep 60",
        }[case]
        try:
            try:
                output = run_probe(IMAGE, name, ["sh", "-c", script], errors,
                                   timeout=10 if case == "timeout" else 30)
            except subprocess.TimeoutExpired as error:
                require(case == "timeout" and b"probe-ready" in (error.stdout or b""),
                        "Expected deadline after actual container startup")
            except RuntimeError as error:
                require(case == "nonzero" and "deliberate-exit" in str(error),
                        "Primary container failure changed")
            else:
                require(case == "success" and b"probe-ready" in output and b"Filesystem" in output,
                        "Probe unexpectedly succeeded or lost actual output")
            exists = subprocess.run(["podman", "container", "exists", name],
                                    capture_output=True, timeout=15)
            require(exists.returncode == 1, case + " probe left its owned container behind")
            require(not errors, "Probe cleanup reported errors: " + repr(errors))
            records.append(dict(case=case, status="PASS"))
        finally:
            # Also protect the machine when a lifecycle regression is detected.
            recovery = cleanup(name)
            require(not recovery, "Verification recovery failed: " + repr(recovery))
    print(json.dumps(dict(cases=records, cleanup_errors=[]), indent=2))


if __name__ == "__main__":
    main()
