"""Run the bounded PostgreSQL experiment on fresh, isolated Podman resources."""

import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import socket
import subprocess
import sys
import time
import uuid

BASELINE = "9f433f99b31d0a3e7378a6009fd319a6788640fe"
IMAGE = "docker.io/library/postgres@sha256:0ea6700a3b4f0ae6ce746519073558aed4d88a79d8d07622a9a644946c7319c4"
SOURCE = Path(__file__).resolve().parent
REPO = SOURCE.parents[1]


def command(args, *, cwd=None, timeout=120):
    result = subprocess.run(args, cwd=cwd, capture_output=True, timeout=timeout)
    if result.returncode:
        raise RuntimeError(
            f"Command failed: {args[:3]}: " + result.stderr.decode(errors="replace")
        )
    return result.stdout


def validate_runtime():
    if sys.platform not in {"win32", "linux"}:
        raise RuntimeError("SP-002 supports Windows and Linux hosts only")
    if sys.flags.optimize:
        raise RuntimeError(
            "SP-002 refuses optimized Python; unset PYTHONOPTIMIZE and omit -O"
        )


def validate_baseline():
    try:
        command(["git", "cat-file", "-e", BASELINE + "^{commit}"], cwd=REPO)
    except (OSError, RuntimeError) as error:
        raise RuntimeError(
            "A Git checkout containing the frozen baseline is required; fetch the baseline "
            + BASELINE
            + " (or unshallow the checkout) before running SP-002"
        ) from error


def cargo_library(messages):
    artifacts = set()
    for line in messages.splitlines():
        record = json.loads(line)
        target = record.get("target", {})
        if (
            record.get("reason") == "compiler-artifact"
            and target.get("name") == "task_contract"
            and target.get("kind") == ["lib"]
        ):
            artifacts.update(
                Path(name) for name in record["filenames"] if name.endswith(".rlib")
            )
    if len(artifacts) != 1:
        raise RuntimeError(
            "Cargo must report exactly one task_contract library artifact"
        )
    return artifacts.pop()


def run_phase(root, name, *, timeout=300, arguments=(), log_name=None):
    log = root / (log_name or name + ".log")
    try:
        result = subprocess.run(
            [sys.executable, str(root / name), *arguments],
            cwd=root,
            capture_output=True,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired as error:
        log.write_bytes(
            (error.stdout or b"") + (error.stderr or b"")
        )
        raise
    log.write_bytes(result.stdout + result.stderr)
    return result.returncode


def cleanup_resources(root, names):
    errors = []
    for name in names:
        try:
            inspection = subprocess.run(
                ["podman", "inspect", name], capture_output=True, timeout=15
            )
            if inspection.returncode:
                # Confirm absence separately; an inspect error is not proof of absence.
                exists = subprocess.run(
                    ["podman", "container", "exists", name],
                    capture_output=True,
                    timeout=15,
                )
                if exists.returncode == 1:
                    continue
                raise RuntimeError(inspection.stderr.decode(errors="replace"))
            details = json.loads(inspection.stdout)[0]
            if details["Config"]["Labels"].get("purpose") not in {
                "kehila-sp002",
                "kehila-sp002-restore",
            }:
                raise RuntimeError("Refusing cleanup: purpose label does not match")
            stopped = subprocess.run(
                ["podman", "stop", "--time", "15", name],
                capture_output=True,
                timeout=30,
            )
            if stopped.returncode:
                raise RuntimeError(stopped.stderr.decode(errors="replace"))
        except (
            OSError,
            ValueError,
            KeyError,
            subprocess.SubprocessError,
            RuntimeError,
        ) as error:
            errors.append(name + ": " + str(error))
    (root / "cleanup.json").write_text(
        json.dumps(dict(errors=errors), indent=2), encoding="utf-8"
    )
    return errors


def available_memory():
    validate_runtime()
    if os.name == "nt":

        class MemoryStatus(ctypes.Structure):
            _fields_ = [("length", ctypes.c_ulong), ("load", ctypes.c_ulong)] + [
                (name, ctypes.c_ulonglong)
                for name in (
                    "total",
                    "available",
                    "page_total",
                    "page_available",
                    "virtual_total",
                    "virtual_available",
                    "extended",
                )
            ]

        value = MemoryStatus()
        value.length = ctypes.sizeof(value)
        if not ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(value)):
            raise RuntimeError("Cannot read host memory")
        return value.available
    values = Path("/proc/meminfo").read_text().splitlines()
    return (
        int(
            next(line for line in values if line.startswith("MemAvailable:")).split()[1]
        )
        * 1024
    )


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def main():
    validate_runtime()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output", type=Path, help="New artifact directory; never overwritten"
    )
    args = parser.parse_args()
    validate_baseline()
    run_id = "sp002-" + uuid.uuid4().hex[:12]
    root = (args.output or REPO / ".kehila" / "spikes" / "SP-002" / run_id).resolve()
    root.mkdir(parents=True, exist_ok=False)
    resources = dict(
        available_memory_bytes=available_memory(),
        free_disk_bytes=shutil.disk_usage(root).free,
    )
    if (
        resources["available_memory_bytes"] < 3 * 1024**3
        or resources["free_disk_bytes"] < 10 * 1024**3
    ):
        raise RuntimeError(
            "Preflight requires 3 GiB available RAM and 10 GiB free artifact disk; inspect Podman storage separately"
        )
    command(["podman", "info", "--format", "{{.Host.CgroupsVersion}}"])
    if command(["git", "diff", BASELINE, "--", "src/pure/task_contract"], cwd=REPO):
        raise RuntimeError(
            "Pure contract differs from the frozen baseline; record a new experiment version"
        )
    versions = {}
    for tool, arguments in [
        ("podman", ["--version"]),
        ("rustc", ["--version"]),
        ("cargo", ["--version"]),
        ("go", ["version"]),
    ]:
        versions[tool] = command([tool, *arguments]).decode().strip()
    (root / "preflight.json").write_text(
        json.dumps(
            dict(
                resources=resources, versions=versions, baseline=BASELINE, image=IMAGE
            ),
            indent=2,
        )
    )
    for path in SOURCE.iterdir():
        if path.suffix in {".py", ".rs", ".sql"} and path.name != "manage.py":
            shutil.copyfile(path, root / path.name)
    sql_dir = root / "sql"
    sql_dir.mkdir()
    blocks = []
    for name in ["M1-project-create-storage.md", "M1-project-create-seed.md"]:
        data = command(
            ["git", "show", BASELINE + ":docs/architecture/" + name], cwd=REPO
        ).decode()
        blocks.extend(
            match.group(1).encode()
            for match in re.finditer(r"^```sql\n(.*?)^```", data, re.M | re.S)
        )
    if len(blocks) != 5:
        raise RuntimeError("Unexpected original SQL block count")
    schema = b"\n".join(blocks)
    (sql_dir / "baseline.sql").write_bytes(schema)
    (root / "schema-sha256.txt").write_text(hashlib.sha256(schema).hexdigest())
    (root / "image-digest.txt").write_text(IMAGE)
    password = secrets.token_urlsafe(32)
    source_port, restore_port = free_port(), free_port()
    while source_port == restore_port:
        restore_port = free_port()
    container = "kehila-" + run_id
    restore = container + "-restore"
    config = dict(
        host="127.0.0.1",
        port=source_port,
        dbname="qa_baseline",
        user="postgres",
        password=password,
    )
    (root / "connection.json").write_text(json.dumps(config))
    (root / "container.env").write_text(
        "POSTGRES_PASSWORD="
        + password
        + "\nPOSTGRES_DB=qa_baseline\nPOSTGRES_INITDB_ARGS=--encoding=UTF8 --locale=C\n"
    )
    for name in ["connection.json", "container.env"]:
        (root / name).chmod(0o600)
    (root / "runtime.json").write_text(
        json.dumps(
            dict(
                container=container,
                restore_container=restore,
                restore_port=restore_port,
            )
        )
    )
    oracle = command(
        [
            "cargo",
            "test",
            "--locked",
            "-p",
            "task_contract",
            "--test",
            "operation",
            "--test",
            "project_create",
        ],
        cwd=REPO,
        timeout=300,
    )
    (root / "pure-oracle.log").write_bytes(oracle)
    build = command(
        [
            "cargo",
            "build",
            "--locked",
            "-p",
            "task_contract",
            "--lib",
            "--message-format=json",
        ],
        cwd=REPO,
        timeout=300,
    )
    (root / "oracle-build.jsonl").write_bytes(build)
    library = cargo_library(build)
    oracle_name = "seed_oracle" + (".exe" if os.name == "nt" else "")
    command(
        [
            "rustc",
            "--edition=2021",
            str(root / "seed_oracle.rs"),
            "--extern",
            "task_contract=" + str(library),
            "-L",
            "dependency=" + str(library.parent),
            "-o",
            str(root / oracle_name),
        ]
    )
    command(["podman", "pull", IMAGE], timeout=300)
    try:
        command(
            [
                "podman",
                "run",
                "--detach",
                "--name",
                container,
                "--label",
                "purpose=kehila-sp002",
                "--shm-size",
                "256m",
                "--publish",
                f"127.0.0.1:{source_port}:5432",
                "--volume",
                container + ":/var/lib/postgresql/data",
                "--env-file",
                str(root / "container.env"),
                IMAGE,
                "postgres",
                "-c",
                "fsync=on",
                "-c",
                "synchronous_commit=on",
                "-c",
                "full_page_writes=on",
                "-c",
                "max_connections=50",
            ]
        )
        if run_phase(root, "setup_database.py", timeout=30, arguments=(str(root),)):
            raise RuntimeError("Database setup failed; inspect setup_database.py.log")

        def phase(name):
            return run_phase(root, name)

        if phase("common.py"):
            raise RuntimeError("Role setup failed")
        if phase("schema_cases.py") != 1:
            raise RuntimeError("Unexpected schema verdict; retain evidence")
        results = json.loads((root / "phase-a-results.json").read_text())
        failures = [row["case"] for row in results if row["status"] == "FAIL"]
        if failures != ["A12.privileged-old-core"]:
            raise RuntimeError("Unexpected schema failure set: " + str(failures))
        if run_phase(
            root, "setup_database.py", timeout=30,
            arguments=(str(root), "compactor-grant"), log_name="setup-grant.log",
        ):
            raise RuntimeError("Compactor grant setup failed; inspect setup-grant.log")
        if phase("protocol_cases.py") != 2:
            raise RuntimeError(
                "Protocol experiment did not finish with its declared product gaps"
            )
        for name in [
            "recovery_cases.py",
            "additional_cases.py",
            "concurrent_workload.py",
            "maximum_key.py",
            "payload_guard_cases.py",
        ]:
            if phase(name):
                raise RuntimeError("Experiment failure: " + name)
        combined = [json.loads((root / "A01.json").read_text())]
        for name in [
            "phase-a-results.json",
            "phase-b-results.json",
            "phase-c-results.json",
            "additional-results.json",
            "concurrent-results.json",
            "max-random-key-results.json",
            "guard-results.json",
        ]:
            combined.extend(json.loads((root / name).read_text()))
        (root / "final-results.json").write_text(json.dumps(combined, indent=2))
        print(
            "Completed bounded experiment. Frozen baseline retains its known failure; corrected guard tested separately. Six product checks remain. Artifact directory:",
            root,
        )
        return 2
    finally:
        errors = cleanup_resources(root, [restore, container])
        if errors:
            print("Cleanup failed: " + "; ".join(errors), file=sys.stderr)
            if sys.exc_info()[0] is None:
                raise RuntimeError(
                    "Experiment completed but cleanup failed; inspect cleanup.json"
                )


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
