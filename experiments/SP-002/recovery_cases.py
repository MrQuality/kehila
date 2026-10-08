from common import (
    CONF,
    PERIOD,
    RESULTS,
    ROOT,
    assert_absent,
    command,
    conn,
    counts,
    create,
    decide,
    expect_error,
    initialize,
    persist,
    require,
    test,
    validate,
)
import hashlib
import json
import psycopg
import subprocess
import threading
import time
import socket, struct, statistics


def podman(*args, input=None):
    p = subprocess.run(["podman", *args], input=input, capture_output=True, timeout=45)
    if p.returncode:
        raise RuntimeError(
            "Podman " + str(args) + ": " + p.stderr.decode(errors="replace")
        )
    return p.stdout + p.stderr if args[0] == "logs" else p.stdout


def recv_exact(s, n):
    b = b""
    while len(b) < n:
        x = s.recv(n - len(b))
        if not x:
            raise EOFError()
        b += x
    return b


class CommitLossProxy:
    """Forward COMMIT; observe server COMMIT+idle, suppress responses, close client."""

    def __init__(self):
        self.listener = socket.socket()
        self.listener.bind(("127.0.0.1", 0))
        self.listener.listen(1)
        self.port = self.listener.getsockname()[1]
        self.commit_sent = threading.Event()
        self.committed = threading.Event()
        self.log = []
        self.errors = []
        self.thread = threading.Thread(target=self.run)
        self.thread.start()

    def run(self):
        client = None
        server = None
        try:
            client, _ = self.listener.accept()
            server = socket.create_connection((CONF["host"], CONF["port"]), timeout=15)
            client.settimeout(15)
            header = recv_exact(client, 4)
            size = struct.unpack("!I", header)[0]
            server.sendall(header + recv_exact(client, size - 4))

            def forward():
                try:
                    while True:
                        kind = recv_exact(client, 1)
                        header = recv_exact(client, 4)
                        n = struct.unpack("!I", header)[0]
                        data = recv_exact(client, n - 4)
                        if (
                            kind == b"Q"
                            and data.rstrip(b"\0").strip().upper() == b"COMMIT"
                        ):
                            self.commit_sent.set()
                            self.log.append("forwarding client COMMIT")
                        server.sendall(kind + header + data)
                except (EOFError, OSError):
                    pass

            tx = threading.Thread(target=forward)
            tx.start()
            seen = False
            while True:
                kind = recv_exact(server, 1)
                header = recv_exact(server, 4)
                n = struct.unpack("!I", header)[0]
                data = recv_exact(server, n - 4)
                if self.commit_sent.is_set():
                    if kind == b"C" and data == b"COMMIT\0":
                        seen = True
                        self.log.append(
                            "observed server CommandComplete COMMIT; withheld"
                        )
                    if kind == b"Z" and data == b"I" and seen:
                        self.log.append("observed server ReadyForQuery idle; withheld")
                        self.committed.set()
                        break
                else:
                    client.sendall(kind + header + data)
            client.shutdown(socket.SHUT_RDWR)
            client.close()
            server.close()
            tx.join(2)
        except Exception as e:
            self.errors.append(str(e))
        finally:
            for s in [client, server, self.listener]:
                if s:
                    try:
                        s.close()
                    except OSError:
                        pass

    def finish(self):
        self.thread.join(20)
        require(not self.thread.is_alive())
        require(not self.errors, lambda: self.errors)


def disconnect_before():
    cmd = command()
    c = conn("qa_serving")
    c.execute("BEGIN")
    require(decide(c, cmd)[0] == "created")
    c.close()
    with conn() as obs:
        end = time.monotonic() + 3
        while counts(obs, cmd["project_id"])["projects"] and time.monotonic() < end:
            time.sleep(0.01)
    assert_absent(cmd["project_id"])
    require(create(cmd)[0] == "created")
    return dict(before_commit_disconnect="no committed rows", retry="created once")


def response_loss():
    cmd = command()
    proxy = CommitLossProxy()
    cfg = dict(
        CONF,
        user="qa_serving",
        port=proxy.port,
        sslmode="disable",
        connect_timeout=5,
    )
    c = psycopg.connect(**cfg, autocommit=True)
    try:
        c.execute("SET statement_timeout='15s'")
        c.execute("BEGIN")
        require(decide(c, cmd)[0] == "created")
        try:
            c.execute("COMMIT")
        except psycopg.OperationalError as e:
            error = type(e).__name__
        else:
            raise AssertionError("Commit response unexpectedly reached client")
        require(proxy.committed.wait(2))
        proxy.finish()
        with conn() as obs:
            validate(obs, cmd)
        require(create(cmd)[0] == "replay")
        return dict(
            client_error=error,
            proxy_trace=proxy.log,
            independent_complete_state=True,
            retry="original result once",
        )
    finally:
        c.close()


def crash():
    good = command()
    require(create(good)[0] == "created")
    bad = command()
    c = conn("qa_serving")
    c.execute("BEGIN")
    require(decide(c, bad)[0] == "created")
    require(
        json.loads(podman("inspect", CONTAINER))[0]["Config"]["Labels"]["purpose"]
        == "kehila-sp002"
    )
    podman("kill", "--signal", "KILL", CONTAINER)
    c.close()
    podman("start", CONTAINER)
    end = time.monotonic() + 25
    last = None
    while time.monotonic() < end:
        try:
            with conn() as obs:
                validate(obs, good)
            break
        except psycopg.Error as e:
            last = e
            time.sleep(0.1)
    else:
        raise AssertionError("Restart failed " + str(last))
    assert_absent(bad["project_id"])
    require(create(good)[0] == "replay")
    logs = podman("logs", CONTAINER).decode(errors="replace")
    (ROOT / "postgres-crash-recovery.log").write_text(logs)
    require("automatic recovery in progress" in logs or "redo starts" in logs)
    return dict(
        fault="SIGKILL isolated PostgreSQL container",
        acknowledged_survived=True,
        uncommitted_absent=True,
        replayed=True,
        settings=json.loads((ROOT / "server-settings.json").read_text()),
    )


def restore():
    dump = podman(
        "exec", CONTAINER, "pg_dump", "-U", "postgres", "-d", CONF["dbname"], "-Fc"
    )
    (ROOT / "database.dump").write_bytes(dump)
    schema = podman(
        "exec",
        CONTAINER,
        "pg_dump",
        "-U",
        "postgres",
        "-d",
        CONF["dbname"],
        "--schema-only",
    )
    (ROOT / "schema.sql").write_bytes(schema)
    roles = podman(
        "exec",
        CONTAINER,
        "pg_dumpall",
        "-U",
        "postgres",
        "--roles-only",
        "--no-role-passwords",
    ).decode()
    selected = (
        "\n".join(
            line
            for line in roles.splitlines()
            if line.startswith(("CREATE ROLE qa_", "ALTER ROLE qa_"))
        )
        + "\n"
    )
    (ROOT / "restore-roles.sql").write_text(selected)
    image = (ROOT / "image-digest.txt").read_text().strip()
    podman(
        "run",
        "--detach",
        "--name",
        RESTORE,
        "--label",
        "purpose=kehila-sp002-restore",
        "--shm-size",
        "256m",
        "--publish",
        "127.0.0.1:" + str(RUNTIME["restore_port"]) + ":5432",
        "--volume",
        RESTORE + ":/var/lib/postgresql/data",
        "--env-file",
        str(ROOT / "container.env"),
        image,
        "postgres",
        "-c",
        "fsync=on",
        "-c",
        "synchronous_commit=on",
        "-c",
        "full_page_writes=on",
    )
    cfg = dict(CONF, port=RUNTIME["restore_port"])
    end = time.monotonic() + 25
    while True:
        try:
            rc = psycopg.connect(**cfg, autocommit=True)
            break
        except psycopg.Error:
            if time.monotonic() > end:
                raise
            time.sleep(0.1)
    rc.execute(selected, prepare=False)
    # Roles restored without password material; provision synthetic login secrets anew.
    from psycopg import sql

    for role in ["qa_serving", "qa_compactor", "qa_unrelated"]:
        rc.execute(
            sql.SQL("ALTER ROLE {} PASSWORD {}").format(
                sql.Identifier(role), sql.Literal(CONF["password"])
            )
        )
    rc.close()
    podman("cp", str(ROOT / "database.dump"), RESTORE + ":/tmp/database.dump")
    out = podman(
        "exec",
        RESTORE,
        "pg_restore",
        "-U",
        "postgres",
        "--exit-on-error",
        "-d",
        CONF["dbname"],
        "/tmp/database.dump",
    )
    (ROOT / "restore.log").write_bytes(out)
    old = CONF["port"]
    CONF["port"] = RUNTIME["restore_port"]
    try:
        with conn() as c:
            incoherent = c.execute(
                "SELECT count(*) FROM kehila.operations o WHERE o.payload_retired = EXISTS(SELECT 1 FROM kehila.operation_payloads p WHERE p.operation_row_id=o.operation_row_id)"
            ).fetchone()[0]
            require(incoherent == 0)
            full = c.execute(
                "SELECT p.request_bytes FROM kehila.operations o JOIN kehila.operation_payloads p USING(operation_row_id) WHERE NOT payload_retired AND o.replay_origin_ms>100"
            ).fetchall()
            require(full)
            tomb = c.execute(
                "SELECT actor_id,target_project_id,token,payload_retired,replay_origin_ms,replay_deadline_ms FROM kehila.operations WHERE payload_retired"
            ).fetchall()
            require(tomb)
            before = c.execute(
                "SELECT max(operation_row_id) FROM kehila.operations"
            ).fetchone()[0]
            require(
                c.execute(
                    "SELECT count(*) FROM pg_trigger t JOIN pg_class r ON t.tgrelid=r.oid JOIN pg_namespace n ON r.relnamespace=n.oid WHERE n.nspname='kehila' AND NOT t.tgisinternal AND t.tgenabled='O'"
                ).fetchone()[0]
                == 5
            )
            acls = c.execute(
                "SELECT tablename,tableowner FROM pg_tables WHERE schemaname='kehila'"
            ).fetchall()
            require(all((owner == "qa_migration" for (_, owner) in acls)))
        # Full record created by QA protocol; mutated current Project need not match saved result.
        cmd = json.loads(bytes(full[0][0]))
        require(create(cmd)[0] == "replay")
        actor, p, token, _, _, _ = tomb[0]
        tcmd = command(p, token)
        require(create(tcmd, actor, now=0)[0] == "replay_expired")
        require(
            create(dict(tcmd, name="changed"), actor, now=0)[0] == "operation_id_reused"
        )
        with conn("qa_serving") as c:
            mutation = expect_error(
                lambda: c.execute("UPDATE kehila.operations SET token=token"),
                "42501",
            )
        with conn() as c:
            immutable = expect_error(
                lambda: c.execute("UPDATE kehila.operations SET token=token"),
                "23514",
                "operation_core_immutable",
            )
            trunc = expect_error(
                lambda: c.execute("TRUNCATE kehila.operations CASCADE"),
                "23514",
                "operation_storage_no_truncate",
            )
        new = command()
        require(create(new)[0] == "created")
        with conn() as c:
            require(
                c.execute(
                    "SELECT operation_row_id FROM kehila.operations WHERE target_project_id=%s",
                    (new["project_id"],),
                ).fetchone()[0]
                > before
            )
        return dict(
            fresh_cluster=True,
            full_rows=len(full),
            tombstones=len(tomb),
            coherent_core_payload=True,
            restored_roles_ownership_acl_functions=True,
            sequence_advanced=True,
            mutation_error=mutation,
            owner_error=immutable,
            truncate_error=trunc,
            restore_order="pg_restore standard pre-data, data, post-data; triggers installed after tombstone load",
            dump_sha256=hashlib.sha256(dump).hexdigest(),
        )
    finally:
        CONF["port"] = old
    test_dummy = None


def measurements():
    timings = []
    for i in range(30):
        cmd = command()
        start = time.perf_counter()
        require(create(cmd)[0] == "created")
        timings.append((time.perf_counter() - start) * 1000)
    c = conn()
    try:
        plans = c.execute(
            "EXPLAIN (ANALYZE,BUFFERS,FORMAT JSON) SELECT operation_row_id FROM kehila.operations WHERE NOT payload_retired AND replay_deadline_ms<=7776000100 ORDER BY replay_deadline_ms LIMIT 20"
        ).fetchone()[0]
        (ROOT / "expiry-plan.json").write_text(json.dumps(plans, indent=2, default=str))
        indexes = c.execute(
            "SELECT indexrelname,pg_relation_size(indexrelid) FROM pg_stat_user_indexes WHERE schemaname='kehila' ORDER BY indexrelname"
        ).fetchall()
        (ROOT / "index-sizes.json").write_text(json.dumps(indexes, indent=2))
        # Add an old bounded batch. Measure updates to the partial-index predicate separately.
        for i in range(30):
            with c.transaction():
                persist(c, command(), o=100)
        ids = [
            x[0]
            for x in c.execute(
                "SELECT operation_row_id FROM kehila.operations WHERE NOT payload_retired AND replay_origin_ms=100 ORDER BY operation_row_id LIMIT 30"
            )
        ]
        wal = c.execute("SELECT pg_current_wal_insert_lsn()").fetchone()[0]
        start = time.perf_counter()
        with conn("qa_compactor") as compact:
            with compact.transaction():
                for op in ids:
                    require(
                        compact.execute(
                            "SELECT kehila.qa_compact(%s,%s)", (op, 100 + PERIOD)
                        ).fetchone()
                        == (True,)
                    )
        elapsed = (time.perf_counter() - start) * 1000
        bytes_wal = c.execute(
            "SELECT pg_wal_lsn_diff(pg_current_wal_insert_lsn(),%s)", (wal,)
        ).fetchone()[0]
        c.execute("VACUUM (ANALYZE) kehila.operations")
        c.execute("VACUUM (ANALYZE) kehila.operation_payloads")
        stats = c.execute(
            "SELECT relname,n_tup_upd,n_tup_hot_upd,n_dead_tup,last_vacuum FROM pg_stat_user_tables WHERE schemaname='kehila' AND relname IN ('operations','operation_payloads')"
        ).fetchall()
        data = dict(
            samples=30,
            conditions="Sequential warm local loopback; includes Rust fixture process, QA validation and connection setup; not adapter-only latency",
            median_ms=statistics.median(timings),
            p95_ms=sorted(timings)[28],
            min_ms=min(timings),
            max_ms=max(timings),
            batch_rows=len(ids),
            batch_ms=elapsed,
            wal_bytes=str(bytes_wal),
            table_stats=stats,
            no_product_threshold=True,
            throughput_claim=False,
        )
        (ROOT / "measurements.json").write_text(json.dumps(data, indent=2, default=str))
        return data
    finally:
        c.close()


def main():
    global RUNTIME, CONTAINER, RESTORE
    initialize()

    RUNTIME = json.loads((ROOT / "runtime.json").read_text())

    CONTAINER = RUNTIME["container"]

    RESTORE = RUNTIME["restore_container"]

    test("C01.before-commit", disconnect_before, "native database fault experiment")

    test("C01.commit-response-loss", response_loss, "native database fault experiment")

    test("C02.durable-process-crash", crash, "native database durability experiment")

    test("C03.clean-cluster-restore", restore, "owner-controlled restore experiment")

    test(
        "C04.bounded-measurements",
        measurements,
        "descriptive experiment, no performance pass threshold",
    )

    (ROOT / "phase-c-results.json").write_text(
        json.dumps(RESULTS, indent=2, default=str)
    )

    raise SystemExit(1 if any(x["status"] == "FAIL" for x in RESULTS) else 0)


if __name__ == "__main__":
    main()
