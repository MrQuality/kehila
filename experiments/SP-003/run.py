"""Bounded Rust/Python/PostgreSQL codec experiment. Podman only; no production I/O."""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import time
import uuid

from build import build, command, SOURCE, REPO

# Reuse the established host/resource checks and immutable PostgreSQL image pin.
sys.path.insert(0, str(REPO / "experiments/SP-002"))
from manage import available_memory, create_artifact_directory, IMAGE

HEADER = b"kehila\0project_create\0\x00\x00\x00\x01"
REQUEST = dict(operation_id="op", project_id="project", name="Project", prefix="QA", estimate_unit="hours")


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def request_bytes(request):
    fields = [request[k].encode("utf-8") for k in ("operation_id", "project_id", "name", "prefix")]
    return HEADER + b"".join(struct.pack(">I", len(f)) + f for f in fields) + bytes([{"hours": 0, "points": 1}[request["estimate_unit"]]])


class Cases:
    def __init__(self, binary):
        self.binary = binary
        self.records = []

    def oracle(self, action, value, *, valid=True):
        result = subprocess.run([str(self.binary), action], input=json.dumps(value, ensure_ascii=False).encode(),
                                capture_output=True, timeout=15)
        if valid:
            require(result.returncode == 0, result.stderr.decode(errors="replace"))
            return json.loads(result.stdout)
        require(result.returncode == 2 and not result.stdout and result.stderr,
                "Invalid wire input must fail explicitly without a success value")

    def run(self, name, function):
        try:
            function()
            self.records.append(dict(case=name, status="PASS"))
        except Exception as error:
            self.records.append(dict(case=name, status="FAIL", error=str(error)))
        print(name, self.records[-1]["status"], flush=True)


def pure_cases(cases):
    def request_goldens():
        golden = json.loads((SOURCE / "fixtures/request-v1.json").read_text(encoding="utf-8"))
        for row in golden:
            value = cases.oracle("request", row["request"])
            expected = request_bytes(row["request"])
            require(value["hex"] == row["hex"] == expected.hex(), "Frozen request bytes differ")
            require(value["sha256"] == row["sha256"] == hashlib.sha256(expected).hexdigest(), "Digest differs")
            require(value["request"] == row["request"], "Typed request changed")
        variants = [dict(REQUEST, name=s) for s in ("é", "e\u0301", '字"\\', "ab", "a")]
        for variant in variants:
            require(cases.oracle("request", variant)["hex"] == request_bytes(variant).hex(), "UTF8 mismatch")
        require(request_bytes(variants[0]) != request_bytes(variants[1]), "Unicode normalized")
        require(request_bytes(dict(REQUEST, operation_id="a", project_id="bc")) !=
                request_bytes(dict(REQUEST, operation_id="ab", project_id="c")), "Ambiguous concatenation")
    cases.run("R01-R02-request-goldens", request_goldens)

    def malformed_requests():
        encoded = request_bytes(REQUEST)
        for length in range(len(encoded)):
            cases.oracle("decode-request", encoded[:length].hex(), valid=False)
        for invalid in (encoded + b"\0", encoded[:-1] + b"\x02", encoded.replace(b"op", b"\xffp"),
                        encoded.replace(HEADER, HEADER[:-1] + b"\x02")):
            cases.oracle("decode-request", invalid.hex(), valid=False)
    cases.run("R03-request-rejections", malformed_requests)

    def frozen_results():
        fixtures = json.loads((SOURCE / "fixtures/result-v1.json").read_text(encoding="utf-8"))
        actual = cases.oracle("fixture", REQUEST)
        require(actual == fixtures, "Frozen version-one result/snapshot differs from actual pure seed")
        for action in ("result", "snapshot"):
            require(cases.oracle(action, fixtures[action]) == fixtures[action], "Frozen decoder failed")
    cases.run("J01-J05-frozen-result-snapshot", frozen_results)

    def malformed_results():
        base = cases.oracle("fixture", REQUEST)["result"]
        for value in (0, 1.5, -1, None, "", "00", "01", "+1", "-1", "1.0", "1e0", "18446744073709551616"):
            variant = copy.deepcopy(base)
            variant["value"]["project"]["next_sequence"] = value
            cases.oracle("result", variant, valid=False)
        for version in (0, 2, "1", 1.0, None):
            variant = copy.deepcopy(base); variant["codec_version"] = version
            cases.oracle("result", variant, valid=False)
        for key in base["value"]["project"]:
            variant = copy.deepcopy(base); del variant["value"]["project"][key]
            cases.oracle("result", variant, valid=False)
        variant = copy.deepcopy(base); variant["value"]["configuration"]["future"] = True
        cases.oracle("result", variant, valid=False)
    cases.run("J03-J05-wire-rejections", malformed_results)

    def identifiers():
        for identifier, proposed, pure in (("", False, False), ("a"*128, True, True), ("a"*129, False, True),
                                           ("é"*64, True, True), ("é"*65, False, True), ("a\0b", False, True)):
            require(cases.oracle("id", identifier) == dict(proposed=proposed, pure_create=pure), "ID acceptance differs")
    cases.run("I01-pure-proposal-mismatch", identifiers)


def sql(container, statement, *, valid=True):
    result = subprocess.run(["podman", "exec", "-i", container, "psql", "-X", "-qAt", "-v", "ON_ERROR_STOP=1", "-U", "postgres"],
                            input=("SET statement_timeout='10s';\n" + statement).encode(), capture_output=True, timeout=20)
    if valid:
        require(result.returncode == 0, "PostgreSQL statement failed: " + result.stderr.decode(errors="replace"))
        return result.stdout.decode().strip()
    require(result.returncode != 0, "Invalid database value unexpectedly accepted")
    return result.stderr.decode()


def literal(value):
    return "'" + value.replace("'", "''") + "'"


def cleanup(container):
    """Remove only this run's labelled container and its anonymous volumes."""
    try:
        inspection = subprocess.run(["podman", "inspect", container], capture_output=True, timeout=15)
        if inspection.returncode:
            exists = subprocess.run(["podman", "container", "exists", container], capture_output=True, timeout=15)
            return [] if exists.returncode == 1 else ["Unable to prove owned container absence"]
        details = json.loads(inspection.stdout)[0]
        if details["Config"]["Labels"].get("purpose") != "kehila-sp003":
            return ["Unexpected container ownership label; cleanup refused"]
        removal = subprocess.run(["podman", "rm", "--force", "--volumes", container], capture_output=True, timeout=30)
        return [] if removal.returncode == 0 else ["Owned container removal failed"]
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        return ["Cleanup failed: " + type(error).__name__]


def source_fingerprints():
    inputs = [*SOURCE.glob("*.rs"), *SOURCE.glob("*.py"), *SOURCE.glob("fixtures/*.json")]
    return {p.relative_to(SOURCE).as_posix(): hashlib.sha256(p.read_bytes().replace(b"\r\n", b"\n")).hexdigest()
            for p in inputs}


def database_cases(cases, container):
    sql(container, 'CREATE TABLE codec_fixtures (id bigserial PRIMARY KEY, request bytea, document jsonb);')

    def bytes_round_trip():
        for unit in ("hours", "points"):
            request = dict(REQUEST, name="字é'\\", estimate_unit=unit)
            value = cases.oracle("request", request)
            row = int(sql(container, "INSERT INTO codec_fixtures(request) VALUES (decode(" + literal(value["hex"]) + ",'hex')) RETURNING id;"))
            retrieved = sql(container, f"SELECT encode(request,'hex') FROM codec_fixtures WHERE id={row};")
            require(cases.oracle("decode-request", retrieved) == request, "BYTEA typed request differs")
            require(hashlib.sha256(bytes.fromhex(retrieved)).hexdigest() == value["sha256"], "Retrieved digest differs")
    cases.run("D01-BYTEA-request-round-trip", bytes_round_trip)

    def jsonb_round_trip():
        base = cases.oracle("fixture", dict(REQUEST, name="字é'\\"))
        for number in (0, 1, 2**53-1, 2**53, 2**53+1, 2**63, 2**64-1):
            for action in ("result", "snapshot"):
                value = copy.deepcopy(base[action])
                if action == "result":
                    value["value"]["project"]["configuration_revision"] = str(number)
                    value["value"]["project"]["next_sequence"] = str(number)
                    value["value"]["configuration"]["revision"] = str(number)
                else:
                    value["value"]["revision"] = str(number)
                row = int(sql(container, "INSERT INTO codec_fixtures(document) VALUES (" + literal(json.dumps(value, ensure_ascii=False)) + "::jsonb) RETURNING id;"))
                retrieved = json.loads(sql(container, f"SELECT document FROM codec_fixtures WHERE id={row};"))
                require(cases.oracle(action, retrieved) == value, "JSONB/Rust scalar or string changed")
        value = copy.deepcopy(base["result"])
        value["value"]["configuration"]["statuses"].reverse()
        row = int(sql(container, "INSERT INTO codec_fixtures(document) VALUES (" + literal(json.dumps(value)) + "::jsonb) RETURNING id;"))
        retrieved = json.loads(sql(container, f"SELECT document FROM codec_fixtures WHERE id={row};"))
        require(cases.oracle("result", retrieved) == value and value != base["result"], "Array order lost")
    cases.run("D01-J02-J04-JSONB-round-trip", jsonb_round_trip)

    def numeric_comparison():
        n = 2**64-1
        require(sql(container, f"SELECT {n}::numeric;") == str(n), "PostgreSQL numeric loses u64")
        require(int(float(2**53+1)) != 2**53+1, "Expected float precision counterexample missing")
        require(sql(container, "SELECT 1.5::numeric;") == "1.5", "Numeric must not pre-round fractions")
    cases.run("D01-numeric-versus-float", numeric_comparison)

    def nul_rejection():
        errors = [sql(container, "SELECT chr(0);", valid=False),
                  sql(container, "SELECT '{\"id\":\"a\\u0000b\"}'::jsonb;", valid=False)]
        require(all("null" in error.lower() or "\\u0000" in error for error in errors), "NUL failures must be specific")
    cases.run("I01-PostgreSQL-NUL-rejection", nul_rejection)

    def identifier_domain():
        sql(container, 'CREATE DOMAIN codec_id AS text COLLATE "C" CHECK (octet_length(VALUE) BETWEEN 1 AND 128);')
        for identifier, valid in (("", False), ("a"*128, True), ("a"*129, False), ("é"*64, True), ("é"*65, False)):
            result = sql(container, "SELECT " + literal(identifier) + "::codec_id;", valid=valid)
            if valid:
                require(result == identifier, "Text domain changed accepted identifier")
        require(sql(container, "SELECT 'é'::codec_id = 'é'::codec_id;") == "f", "C-collated identity normalized")
    cases.run("I02-PostgreSQL-ID-domain", identifier_domain)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pure", action="store_true")
    parser.add_argument("--podman-connection", help="Explicit existing Podman connection; does not change the default")
    parser.add_argument("--output", type=Path, required=True, help="New ignored artifact directory")
    args = parser.parse_args()
    if args.podman_connection:
        # Environment is local to this runner and inherited only by its children.
        os.environ["CONTAINER_CONNECTION"] = args.podman_connection
    if sys.flags.optimize or sys.platform not in {"win32", "linux"}:
        raise RuntimeError("Use unoptimized Python on Windows/Linux")
    root = args.output.resolve()
    artifact_base = (REPO / ".kehila").resolve()
    if artifact_base not in root.parents:
        raise RuntimeError("Artifacts must be under the ignored .kehila directory")
    create_artifact_directory(root)
    fingerprints = source_fingerprints()
    binary, rust_output = build()
    (root / "rust-tests.txt").write_text(rust_output, encoding="utf-8")
    cases = Cases(binary)
    pure_cases(cases)
    runtime = dict(host=sys.platform, python=sys.version.split()[0], database_executed=False)
    container = "kehila-sp003-" + uuid.uuid4().hex[:12]
    started = False
    cleanup_errors = []
    try:
        require(all(r["status"] == "PASS" for r in cases.records), "Pure cases failed; database execution refused")
        if not args.pure:
            memory = available_memory()
            runtime["memory_available_bytes"] = memory
            require(memory >= 3*1024**3, "At least 3 GiB available host memory required")
            disk_free = shutil.disk_usage(REPO).free
            runtime["artifact_disk_free_bytes"] = disk_free
            require(disk_free >= 2*1024**3, "At least 2 GiB free artifact disk required")
            info = json.loads(command(["podman", "info", "--format", "json"], timeout=20))
            runtime["rootless"] = info["host"]["security"]["rootless"]
            runtime["cgroup_controllers"] = info["host"]["cgroupControllers"]
            command(["podman", "image", "inspect", IMAGE], timeout=20)
            storage_free = int(command(["podman", "run", "--rm", "--network=none", "--memory=128m", IMAGE,
                                        "sh", "-c", "df -Pk / | tail -1 | awk '{print $4}'"]).strip())*1024
            require(storage_free >= 2*1024**3, "At least 2 GiB free container storage required")
            runtime.update(storage_free_bytes=storage_free, cpu_count=os.cpu_count(),
                           podman=command(["podman", "--version"]).decode().strip(),
                           rust=command(["rustc", "--version"]).decode().strip(), image=IMAGE)
            # Mark ownership before creation so a partially successful run is cleaned up.
            started = True
            command(["podman", "run", "-d", "--name", container, "--label", "purpose=kehila-sp003",
                     "--network=none", "--memory=256m", "--cpus=1", "--pids-limit=128", "--tmpfs",
                     "/var/lib/postgresql/data:rw,size=256m", "-e", "POSTGRES_HOST_AUTH_METHOD=trust", IMAGE,
                     "postgres", "-c", "listen_addresses="])
            ready = False
            deadline = time.monotonic() + 30
            while time.monotonic() < deadline:
                probe = subprocess.run(["podman", "exec", container, "pg_isready", "-U", "postgres"], capture_output=True, timeout=5)
                if probe.returncode == 0:
                    ready = True
                    break
                time.sleep(1)
            require(ready, "Database readiness exceeded bounded startup budget")
            memory_max = command(["podman", "exec", container, "cat", "/sys/fs/cgroup/memory.max"]).decode().strip()
            cpu_max = command(["podman", "exec", container, "cat", "/sys/fs/cgroup/cpu.max"]).decode().strip()
            pids_max = command(["podman", "exec", container, "cat", "/sys/fs/cgroup/pids.max"]).decode().strip()
            require(memory_max == str(256*1024**2), "Container memory ceiling is not enforced")
            quota, period = cpu_max.split()
            require(quota != "max" and int(quota) == int(period), "Container one-CPU ceiling is not enforced")
            require(pids_max == "128", "Container process ceiling is not enforced")
            runtime.update(memory_max=memory_max, cpu_max=cpu_max, pids_max=pids_max)
            runtime.update(database_executed=True, postgres=sql(container, "SHOW server_version;"),
                           server_encoding=sql(container, "SHOW server_encoding;"))
            require(sql(container, "SHOW listen_addresses;") == "", "Database TCP listening must be disabled")
            database_cases(cases, container)
    except Exception as error:
        runtime["execution_error"] = str(error)
        cases.records.append(dict(case="database-execution", status="BLOCKED" if not started else "FAIL"))
        raise
    finally:
        if started:
            cleanup_errors.extend(cleanup(container))
        if source_fingerprints() != fingerprints:
            cases.records.append(dict(case="source-stability", status="FAIL"))
        summary = dict(runtime=runtime, cases=cases.records, cleanup_errors=cleanup_errors,
                       source_sha256=fingerprints)
        (root / "results.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    require(not cleanup_errors and all(r["status"] == "PASS" for r in cases.records), "Experiment failed; inspect retained summary")


if __name__ == "__main__":
    main()
