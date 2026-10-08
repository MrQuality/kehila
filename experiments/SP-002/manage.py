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


def available_memory():
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
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output", type=Path, help="New artifact directory; never overwritten"
    )
    args = parser.parse_args()
    run_id = "sp002-" + uuid.uuid4().hex[:12]
    root = (args.output or REPO / ".kehila" / "spikes" / "SP-002" / run_id).resolve()
    root.mkdir(parents=True, exist_ok=False)
    import psycopg

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
    libraries = sorted(
        (REPO / "target" / "debug" / "deps").glob("libtask_contract-*.rlib"),
        key=lambda p: p.stat().st_mtime,
    )
    command(
        [
            "rustc",
            "--edition=2021",
            str(root / "seed_oracle.rs"),
            "--extern",
            "task_contract=" + str(libraries[-1]),
            "-o",
            str(root / "seed_oracle.exe"),
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
        deadline = time.monotonic() + 30
        while True:
            try:
                with psycopg.connect(**config, autocommit=True) as conn:
                    assert conn.info.server_version // 10000 == 16
                    settings = {
                        name: conn.execute("SHOW " + name).fetchone()[0]
                        for name in [
                            "server_version",
                            "server_encoding",
                            "fsync",
                            "synchronous_commit",
                            "full_page_writes",
                            "block_size",
                            "default_transaction_isolation",
                        ]
                    }
                    assert settings["server_encoding"] == "UTF8"
                    assert all(
                        settings[name] == "on"
                        for name in ["fsync", "synchronous_commit", "full_page_writes"]
                    )
                    (root / "server-settings.json").write_text(
                        json.dumps(settings, indent=2)
                    )
                    conn.execute(schema.decode(), prepare=False)
                break
            except psycopg.OperationalError:
                if time.monotonic() >= deadline:
                    raise
                time.sleep(0.1)
        (root / "A01.json").write_text(
            json.dumps(dict(case="A01.original-schema", status="PASS"))
        )

        def phase(name):
            result = subprocess.run(
                [sys.executable, str(root / name)],
                cwd=root,
                capture_output=True,
                timeout=300,
            )
            (root / (name + ".log")).write_bytes(result.stdout + result.stderr)
            return result.returncode

        if phase("common.py"):
            raise RuntimeError("Role setup failed")
        if phase("schema_cases.py") != 1:
            raise RuntimeError("Unexpected schema verdict; retain evidence")
        results = json.loads((root / "phase-a-results.json").read_text())
        failures = [row["case"] for row in results if row["status"] == "FAIL"]
        if failures != ["A12.privileged-old-core"]:
            raise RuntimeError("Unexpected schema failure set: " + str(failures))
        with psycopg.connect(**config, autocommit=True) as conn:
            conn.execute((root / "compactor-read-grant.sql").read_text())
        if phase("protocol_cases.py") != 2:
            raise RuntimeError(
                "Protocol experiment did not finish with its declared product gaps"
            )
        for name in [
            "recovery_cases.py",
            "additional_cases.py",
            "concurrent_workload.py",
            "maximum_key.py",
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
        ]:
            combined.extend(json.loads((root / name).read_text()))
        (root / "final-results.json").write_text(json.dumps(combined, indent=2))
        print(
            "Completed bounded experiment. Known privileged move failure and six product checks remain. Artifact directory:",
            root,
        )
        return 2
    finally:
        for name in [restore, container]:
            inspection = subprocess.run(
                ["podman", "inspect", name], capture_output=True
            )
            if inspection.returncode == 0:
                details = json.loads(inspection.stdout)[0]
                if details["Config"]["Labels"].get("purpose") in {
                    "kehila-sp002",
                    "kehila-sp002-restore",
                }:
                    subprocess.run(
                        ["podman", "stop", "--time", "15", name],
                        capture_output=True,
                        timeout=30,
                    )


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
