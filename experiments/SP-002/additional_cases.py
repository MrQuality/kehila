from common import (
    CONF,
    RESULTS,
    ROOT,
    TABLES,
    command,
    conn,
    core,
    create,
    expect_error,
    initialize,
    payload,
    require,
    seed,
    test,
)
import hashlib
import json
import psycopg
import subprocess
import uuid


def case_keys():
    c = conn()
    p = "case_key_" + uuid.uuid4().hex
    try:
        with c.transaction():
            for token in ["Token", "token"]:
                cmd = command(p, token)
                op = core(c, cmd)
                payload(c, op, cmd)
        require(
            c.execute(
                "SELECT count(*) FROM kehila.operations WHERE target_project_id=%s",
                (p,),
            ).fetchone()
            == (2,)
        )
        return dict(case_distinct=True)
    finally:
        c.close()


def max_index():
    c = conn(db="qa_mapping")
    randomid = lambda: uuid.uuid4().hex * 4
    try:
        actor = randomid()
        c.execute("INSERT INTO kehila.actors VALUES (%s,true,true)", (actor,))
        cmd = command(randomid(), randomid())
        with c.transaction():
            op = core(
                c,
                cmd,
                actor,
                target_kind="relationship",
                command_family="relationship_create",
                target_relationship_id=randomid(),
            )
            payload(c, op, cmd)
        return dict(
            identity_bytes=128,
            actor_target_project_relationship_token_strings=4,
            block_size=8192,
            inserted=True,
        )
    finally:
        c.close()


def digest():
    cmd = command()
    c = conn()
    with c.transaction():
        op = core(c, cmd, request_sha256=b"x" * 32)
        payload(c, op, cmd, seed(cmd))
    c.close()
    require(create(cmd)[0] == "invalid_operation")
    return dict(
        incoherent_digest_rejected=True,
        scope="QA JSON v1 only; final codec blocked",
    )


def restore_compare():
    # Compare the exact restored dump to a second clean database, avoiding later source mutations.
    runtime = json.loads((ROOT / "runtime.json").read_text())
    container = runtime["container"]
    restored = runtime["restore_container"]
    with conn() as c:
        c.execute("CREATE DATABASE qa_dump_reference TEMPLATE template0")
    subprocess.run(
        [
            "podman",
            "cp",
            str(ROOT / "database.dump"),
            container + ":/tmp/qa-reference.dump",
        ],
        check=True,
        capture_output=True,
    )
    p = subprocess.run(
        [
            "podman",
            "exec",
            container,
            "pg_restore",
            "-U",
            "postgres",
            "--exit-on-error",
            "-d",
            "qa_dump_reference",
            "/tmp/qa-reference.dump",
        ],
        capture_output=True,
    )
    require(p.returncode == 0, lambda: p.stderr.decode())
    source = conn(db="qa_dump_reference")
    cfg = dict(CONF, port=runtime["restore_port"])
    dest = psycopg.connect(**cfg, autocommit=True)
    try:
        before = source.execute(
            "SELECT max(operation_row_id) FROM kehila.operations"
        ).fetchone()[0]
        extra = [
            x[0]
            for x in dest.execute(
                "SELECT target_project_id FROM kehila.operations WHERE operation_row_id>%s",
                (before,),
            )
        ]
        # One explicit post-restore creation is excluded from the content comparison.
        require(len(extra) == 1, lambda: extra)
        inventory = {}
        for table in [*TABLES, "actors"]:
            if table == "actors":
                where = ""
                args = ()
            elif table == "operations":
                where = " WHERE operation_row_id<=%s"
                args = (before,)
            elif table == "operation_payloads":
                where = " WHERE operation_row_id<=%s"
                args = (before,)
            else:
                where = " WHERE project_id <> ALL(%s)"
                args = (extra,)
            q = (
                "SELECT to_jsonb(t)::text FROM kehila."
                + table
                + " t"
                + where
                + " ORDER BY to_jsonb(t)::text"
            )
            rows1 = [x[0] for x in source.execute(q, args)]
            rows2 = [x[0] for x in dest.execute(q, args)]
            require(rows1 == rows2, lambda: table)
            inventory[table] = dict(
                rows=len(rows1),
                sha256=hashlib.sha256("\n".join(rows1).encode()).hexdigest(),
            )
        # Compare role attributes and object ACLs, not only successful admin reads.
        roles = "SELECT rolname,rolsuper,rolinherit,rolcreaterole,rolcreatedb,rolcanlogin FROM pg_roles WHERE rolname LIKE 'qa_%' ORDER BY rolname"
        require(source.execute(roles).fetchall() == dest.execute(roles).fetchall())
        acls = "SELECT c.relname,c.relacl::text,pg_get_userbyid(c.relowner) FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='kehila' ORDER BY c.relname"
        require(source.execute(acls).fetchall() == dest.execute(acls).fetchall())
        funcs = "SELECT p.proname,p.proacl::text,pg_get_userbyid(p.proowner),p.proconfig,p.prosecdef FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace WHERE n.nspname='kehila' ORDER BY p.proname"
        require(source.execute(funcs).fetchall() == dest.execute(funcs).fetchall())
        # After restoration, the fresh-retired insert guard is active again.
        orig = CONF["port"]
        CONF["port"] = runtime["restore_port"]
        try:
            cmd = command()
            with conn() as c:

                def run():
                    with c.transaction():
                        core(c, cmd, payload_retired=True)

                d = expect_error(run, "23514", "operation_core_fresh")
        finally:
            CONF["port"] = orig
        (ROOT / "restore-content-hashes.json").write_text(
            json.dumps(inventory, indent=2)
        )
        return dict(
            all_tables_exact=True,
            roles_acls_functions_exact=True,
            fresh_guard=d,
            tables=inventory,
        )
    finally:
        source.close()
        dest.close()


def main():
    initialize()

    test("A09.case-distinct-key", case_keys)

    test("A07.maximum-index-key", max_index, "modified-schema compatibility experiment")

    test("B14.qa-digest-coherence", digest, "experimental QA codec")

    test(
        "C03.exact-restored-content-and-acls",
        restore_compare,
        "owner-controlled restore experiment",
    )

    (ROOT / "additional-results.json").write_text(
        json.dumps(RESULTS, indent=2, default=str)
    )

    raise SystemExit(1 if any(x["status"] == "FAIL" for x in RESULTS) else 0)


if __name__ == "__main__":
    main()
