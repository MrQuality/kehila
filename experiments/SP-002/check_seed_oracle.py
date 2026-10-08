"""Compile and check the actual QA oracle without database services."""

import json
from pathlib import Path
import subprocess
import sys
import uuid

from manage import cargo_library

SOURCE = Path(__file__).resolve().parent
REPO = SOURCE.parents[1]


def run(args):
    return subprocess.run(args, cwd=REPO, capture_output=True, timeout=120, check=True).stdout


def main():
    build = run(["cargo", "build", "--locked", "-p", "task_contract", "--lib", "--message-format=json"])
    library = cargo_library(build)
    # Keep compiler outputs with Cargo artifacts; Windows may retain executable
    # file handles briefly after exit, so test completion does not delete binaries.
    root = library.parent / "sp002-oracle-check" / uuid.uuid4().hex
    root.mkdir(parents=True)
    suffix = ".exe" if sys.platform == "win32" else ""
    binary = root / ("seed_oracle" + suffix)
    tests = root / ("seed_oracle_tests" + suffix)
    compile_args = ["rustc", "--edition=2021", str(SOURCE / "seed_oracle.rs"),
                    "--extern", "task_contract=" + str(library),
                    "-L", "dependency=" + str(library.parent)]
    run([*compile_args, "--test", "-o", str(tests)])
    print(run([str(tests)]).decode(), end="")
    run([*compile_args, "-o", str(binary)])
    args = [str(binary), "qa_project", "qa_token", 'QA "Project"', "QA"]
    for unit in ["hours", "points"]:
        result = json.loads(run([*args, unit]))
        if result["project"]["estimate_unit"] != unit or result["project"]["name"] != args[3]:
            raise RuntimeError("Oracle fixture differs from supplied valid command")
    for invalid in [[], args, [*args, "minutes"], [*args, "Hours"], [*args, "hours", "extra"]]:
        result = subprocess.run(
            invalid or [str(binary)], capture_output=True, timeout=15,
        )
        if result.returncode != 2 or result.stdout or not result.stderr:
            raise RuntimeError("Invalid oracle input must fail explicitly without a fixture")
    print("PASS: supported units, JSON escaping, invalid units and argument counts")


if __name__ == "__main__":
    main()
