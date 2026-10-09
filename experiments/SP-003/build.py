"""Build the isolated oracle against exact Cargo-reported existing dependencies."""
from contextlib import contextmanager
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

SOURCE = Path(__file__).resolve().parent
REPO = SOURCE.parents[1]


def command(args, *, input=None, timeout=120):
    result = subprocess.run(args, cwd=REPO, input=input, capture_output=True, timeout=timeout)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors="replace"))
    return result.stdout


def remove_build_directory(root):
    """Allow a bounded Windows executable-lock release; never ignore failures."""
    deadline = time.monotonic() + 5
    while True:
        try:
            shutil.rmtree(root)
            return
        except OSError as error:
            if (sys.platform != "win32" or getattr(error, "winerror", None) not in {32, 33}
                    or time.monotonic() >= deadline):
                raise
            time.sleep(0.1)


@contextmanager
def build():
    """Keep unique compiler outputs alive only while the caller uses the oracle."""
    messages = command(["cargo", "build", "--locked", "--offline", "-p", "task_worker",
                        "--lib", "--message-format=json"], timeout=450)
    artifacts = {name: set() for name in ("task_contract", "serde_json", "sha2")}
    for line in messages.splitlines():
        record = json.loads(line)
        target = record.get("target", {})
        name = target.get("name")
        if record.get("reason") == "compiler-artifact" and name in artifacts:
            artifacts[name].update(Path(p) for p in record["filenames"] if p.endswith(".rlib"))
    if any(len(paths) != 1 for paths in artifacts.values()):
        raise RuntimeError("Cargo must report one library for each exact dependency")
    libraries = {name: paths.pop() for name, paths in artifacts.items()}
    parent = libraries["task_contract"].parent / "sp003"
    parent.mkdir(exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix="run-", dir=parent))
    try:
        suffix = ".exe" if sys.platform == "win32" else ""
        args = ["rustc", "--edition=2021", "-D", "warnings", str(SOURCE / "main.rs")]
        for name, library in libraries.items():
            args += ["--extern", name + "=" + str(library)]
        args += ["-L", "dependency=" + str(libraries["task_contract"].parent)]
        tests = root / ("codec_tests" + suffix)
        command([*args, "--test", "-o", str(tests)])
        output = command([str(tests)])
        binary = root / ("codec_oracle" + suffix)
        command([*args, "-o", str(binary)])
        yield binary, output.decode()
    finally:
        remove_build_directory(root)


if __name__ == "__main__":
    with build() as (_, output):
        print(output, end="")
