#!/usr/bin/env python3
"""Run real Go tests; retry only temporary-directory cleanup on Windows locks."""
import os
import subprocess
import sys
from go_inventory import GO_PACKAGES, ROOT, go_files
from temp_work import owned_directory


def main():
    go_files()
    with owned_directory("kehila-go-") as work:
        environment = dict(os.environ, GOTMPDIR=str(work))
        # -work delegates deletion to us; it does not skip compilation or tests.
        command = ["go", "test", "-count=1", "-work", *(sys.argv[1:] or GO_PACKAGES)]
        result = subprocess.run(command, cwd=ROOT, env=environment, timeout=300, check=False)
        return result.returncode


if __name__ == "__main__":
    sys.exit(main())
