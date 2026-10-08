from common import (
    PERIOD,
    RESULTS,
    ROOT,
    TABLES,
    assert_absent,
    command,
    conn,
    core,
    counts,
    create,
    decide,
    expect_error,
    initialize,
    install_helpers,
    origin,
    payload,
    persist,
    rec,
    require,
    test,
    validate,
)
import json
import psycopg
import threading
import time


def db_error(role, sql, args=()):
    with conn(role) as c:
        return expect_error(lambda: c.execute(sql, args), "42501")


def lock_helpers():
    with conn("qa_serving") as c:
        with c.transaction():
            require(
                c.execute("SELECT * FROM kehila.qa_lock_actor('qa_actor_a')").fetchone()
                == (True, True)
            )
            require(
                c.execute(
                    "SELECT operation_row_id FROM kehila.qa_lock_core(1)"
                ).fetchone()
                == (1,)
            )
    return dict(role="qa_serving", actor_lock="NO KEY UPDATE", core_lock="SHARE")


def search_path():
    with conn("qa_serving") as c:
        with c.transaction():
            c.execute(
                "CREATE TEMP TABLE actors(actor_id text,active boolean,can_create_project boolean)"
            )
            c.execute(
                "INSERT INTO actors VALUES ('qa_denied',true,true); SET search_path=pg_temp,public",
                prepare=False,
            )
            require(
                c.execute("SELECT * FROM kehila.qa_lock_actor('qa_denied')").fetchone()
                == (False, False)
            )
    with conn() as c:
        acl = c.execute(
            "SELECT p.proname,pg_get_userbyid(p.proowner),p.prosecdef,p.proconfig,p.proacl::text FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace WHERE n.nspname='kehila' ORDER BY proname"
        ).fetchall()
        (ROOT / "function-acls.json").write_text(json.dumps(acl, indent=2, default=str))
        require(
            all(
                (
                    not any(
                        (
                            entry.startswith("=")
                            for entry in (row[4] or "").strip("{}").split(",")
                        )
                    )
                    for row in acl
                )
            )
        )
    return dict(
        temp_shadowing="did not substitute actor", acl_snapshot="function-acls.json"
    )


def wait_blocked(pid, blocker, observer, timeout=4):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        row = observer.execute(
            "SELECT pg_blocking_pids(%s),wait_event_type,wait_event FROM pg_stat_activity WHERE pid=%s",
            (pid, pid),
        ).fetchone()
        if row and blocker in row[0]:
            return dict(
                pid=pid, blocker=blocker, wait_event_type=row[1], wait_event=row[2]
            )
        time.sleep(0.01)
    raise AssertionError("Expected lock schedule not reached: " + str((pid, blocker)))


def worker(c, fn):
    box = {}
    started = threading.Event()

    def run():
        try:
            started.set()
            with c.transaction():
                box["result"] = fn(c)
        except Exception as e:
            box["error"] = e

    t = threading.Thread(target=run)
    t.start()
    require(started.wait(1))
    return t, box


def join(t, box):
    t.join(12)
    require(not t.is_alive(), "worker timeout")
    if "error" in box:
        raise box["error"]
    return box["result"]


def same_key(commit, n):
    cmd = command(token="same-key-" + str(n))
    a = conn("qa_serving")
    b = conn("qa_serving")
    obs = conn()
    try:
        ap = a.info.backend_pid
        bp = b.info.backend_pid
        a.execute("BEGIN")
        first = decide(a, cmd)
        require(first[0] == "created")
        t, box = worker(b, lambda c: decide(c, cmd))
        trace = wait_blocked(bp, ap, obs)
        a.execute("COMMIT" if commit else "ROLLBACK")
        second = join(t, box)
        require(second[0] == ("replay" if commit else "created"), lambda: second)
        validate(obs, cmd)
        return dict(
            iteration=n,
            first_commit=commit,
            trace=trace,
            second=second[0],
            counts=counts(obs, cmd["project_id"]),
        )
    finally:
        a.close()
        b.close()
        obs.close()


def independence(same_actor, n):
    ca = command()
    cb = command()
    a = conn("qa_serving")
    b = conn("qa_serving")
    obs = conn()
    try:
        a.execute("BEGIN")
        require(decide(a, ca)[0] == "created")
        t, box = worker(
            b, lambda c: decide(c, cb, "qa_actor_a" if same_actor else "qa_actor_b")
        )
        if same_actor:
            trace = wait_blocked(b.info.backend_pid, a.info.backend_pid, obs)
            a.execute("COMMIT")
            out = join(t, box)
        else:
            out = join(t, box)
            trace = dict(second_committed_while_first_open=True)
            require(counts(obs, ca["project_id"])["projects"] == 0)
            a.execute("COMMIT")
        require(out[0] == "created")
        validate(obs, ca)
        validate(obs, cb, "qa_actor_a" if same_actor else "qa_actor_b")
        return trace
    finally:
        a.close()
        b.close()
        obs.close()


def collisions():
    cmd = command()
    require(create(cmd)[0] == "created")
    require(create(cmd, "qa_actor_b")[0] == "project_already_exists")
    other = dict(cmd, operation_id="different-token")
    require(create(other)[0] == "project_already_exists")
    with conn() as c:
        require(counts(c, cmd["project_id"]) == TABLES)
    return dict(no_adoption=True, no_new_rows=True)


def exact_replay():
    cmd = command()
    out = create(cmd)
    require(out[0] == "created")
    with conn() as c:
        before = c.execute(
            "SELECT replay_origin_ms,replay_deadline_ms FROM kehila.operations WHERE target_project_id=%s",
            (cmd["project_id"],),
        ).fetchone()
        c.execute(
            "UPDATE kehila.projects SET name='Current changed name' WHERE project_id=%s",
            (cmd["project_id"],),
        )
    for _ in range(3):
        require(create(cmd) == ("replay", out[1]))
    for change in [
        dict(name="Other"),
        dict(name="QA Project "),
        dict(prefix="QB"),
        dict(estimate_unit="points"),
    ]:
        require(create(dict(cmd, **change))[0] == "operation_id_reused")
    with conn() as c:
        require(counts(c, cmd["project_id"]) == TABLES)
        require(
            c.execute(
                "SELECT replay_origin_ms,replay_deadline_ms FROM kehila.operations WHERE target_project_id=%s",
                (cmd["project_id"],),
            ).fetchone()
            == before
        )
    return dict(replayed_original_result=True, origin_unchanged=True)


def revocation_first(replay, n):
    cmd = command()
    if replay:
        require(create(cmd)[0] == "created")
    a = conn()
    b = conn("qa_serving")
    obs = conn()
    try:
        a.execute("BEGIN")
        a.execute(
            "UPDATE kehila.actors SET can_create_project=false WHERE actor_id='qa_actor_a'"
        )
        t, box = worker(b, lambda c: decide(c, cmd))
        trace = wait_blocked(b.info.backend_pid, a.info.backend_pid, obs)
        a.execute("COMMIT")
        require(join(t, box)[0] == "unauthorized")
        if not replay:
            assert_absent(cmd["project_id"])
        return trace
    finally:
        a.execute(
            "UPDATE kehila.actors SET active=true,can_create_project=true WHERE actor_id='qa_actor_a'"
        )
        a.close()
        b.close()
        obs.close()


def command_first(replay, n):
    cmd = command()
    if replay:
        require(create(cmd)[0] == "created")
    a = conn("qa_serving")
    b = conn()
    obs = conn()
    try:
        a.execute("BEGIN")
        require(decide(a, cmd)[0] == ("replay" if replay else "created"))

        def revoke(c):
            c.execute(
                "UPDATE kehila.actors SET active=false WHERE actor_id='qa_actor_a'"
            )
            return "revoked"

        t, box = worker(b, revoke)
        trace = wait_blocked(b.info.backend_pid, a.info.backend_pid, obs)
        a.execute("COMMIT")
        require(join(t, box) == "revoked")
        require(create(cmd)[0] == "unauthorized")
        return trace
    finally:
        b.execute(
            "UPDATE kehila.actors SET active=true,can_create_project=true WHERE actor_id='qa_actor_a'"
        )
        a.close()
        b.close()
        obs.close()


def mismatch():
    cmd = command()
    c = conn()
    try:
        c.execute("BEGIN")
        persist(c, cmd)
        c.execute(
            "UPDATE kehila.project_grants SET active=false WHERE project_id=%s",
            (cmd["project_id"],),
        )
        c.execute("SET CONSTRAINTS ALL IMMEDIATE")
        try:
            validate(c, cmd)
        except AssertionError:
            pass
        else:
            raise AssertionError("Harness accepted current/history mismatch")
        c.execute("ROLLBACK")
        assert_absent(cmd["project_id"])
        return dict(
            sql_allows_mismatch=True,
            harness_validation_rejects=True,
            rolled_back=True,
        )
    finally:
        c.close()


def project_lock():
    cmd = command()
    require(create(cmd)[0] == "created")
    a = conn()
    b = conn("qa_serving")
    try:
        a.execute("BEGIN")
        a.execute(
            "SELECT * FROM kehila.projects WHERE project_id=%s FOR UPDATE",
            (cmd["project_id"],),
        )
        t, box = worker(b, lambda c: decide(c, cmd))
        require(join(t, box)[0] == "replay")
        a.execute("ROLLBACK")
        return dict(replay_completed_while_project_locked=True)
    finally:
        a.close()
        b.close()


def clock_sample():
    cmd = command()
    c = conn("qa_serving")
    try:
        c.execute("BEGIN")
        start = c.execute(
            "SELECT floor(extract(epoch from transaction_timestamp())*1000)::numeric"
        ).fetchone()[0]
        c.execute("SELECT * FROM kehila.qa_lock_actor('qa_actor_a')")
        c.execute("SELECT pg_sleep(0.2)")
        o = origin(c)
        require(o - start >= 180)
        op = persist(c, cmd, o=o)
        c.execute("SELECT pg_sleep(0.1)")
        beforecommit = origin(c)
        c.execute("COMMIT")
        v = c.execute(
            "SELECT replay_origin_ms,replay_deadline_ms FROM kehila.operations WHERE operation_row_id=%s",
            (op,),
        ).fetchone()
        require(v == (o, o + PERIOD))
        require(beforecommit - o >= 90)
        return dict(
            transaction_start_ms=str(start),
            sample_ms=o,
            precommit_ms=beforecommit,
            stored_window=[str(x) for x in v],
            no_commit_adjustment=True,
        )
    finally:
        c.close()


def boundaries():
    cmd = command()
    c = conn()
    with c.transaction():
        persist(c, cmd, o=100)
    c.close()
    for n, expected in [
        (99, "invalid_operation"),
        (100, "replay"),
        (100 + PERIOD - 1, "replay"),
        (100 + PERIOD, "replay_expired"),
        (101 + PERIOD, "replay_expired"),
    ]:
        require(create(cmd, now=n)[0] == expected)
        require(create(dict(cmd, name="changed"), now=n)[0] == "operation_id_reused")
        with conn() as c:
            c.execute(
                "UPDATE kehila.actors SET can_create_project=false WHERE actor_id='qa_actor_a'"
            )
        try:
            require(create(cmd, now=n)[0] == "unauthorized")
            require(create(dict(cmd, name="changed"), now=n)[0] == "unauthorized")
        finally:
            with conn() as c:
                c.execute(
                    "UPDATE kehila.actors SET can_create_project=true WHERE actor_id='qa_actor_a'"
                )
    with conn() as c:
        require(counts(c, cmd["project_id"]) == TABLES)
    return dict(
        origin=100,
        deadline=100 + PERIOD,
        payload_still_present=True,
        all_boundaries_and_authority_precedence=True,
    )


def compaction():
    cmd = command()
    c = conn()
    with c.transaction():
        op = persist(c, cmd, o=100)
    c.close()
    with conn("qa_compactor") as c:
        early = expect_error(
            lambda: c.execute(
                "SELECT kehila.qa_compact(%s,%s)", (op, 100 + PERIOD - 1)
            ),
            "22023",
        )
    with conn() as c:
        c.execute("BEGIN")
        c.execute(
            "UPDATE kehila.operations SET payload_retired=true WHERE operation_row_id=%s",
            (op,),
        )
        failure = expect_error(lambda: c.execute("SELECT 1/0"), "22012")
        c.execute("ROLLBACK")
        require(counts(c, cmd["project_id"]) == TABLES)
    with conn("qa_compactor") as c:
        require(
            c.execute("SELECT kehila.qa_compact(%s,%s)", (op, 100 + PERIOD)).fetchone()
            == (True,)
        )
        require(
            c.execute("SELECT kehila.qa_compact(%s,%s)", (op, 0)).fetchone() == (False,)
        )
    with conn() as c:
        require(
            c.execute(
                "SELECT payload_retired FROM kehila.operations WHERE operation_row_id=%s",
                (op,),
            ).fetchone()
            == (True,)
        )
        require(counts(c, cmd["project_id"]) == dict(TABLES, operation_payloads=0))
    for n in [0, 99, 100, 100 + PERIOD - 1, 100 + PERIOD, 2**64 - 1]:
        require(create(cmd, now=n)[0] == "replay_expired")
        require(create(dict(cmd, name="changed"), now=n)[0] == "operation_id_reused")
    return dict(
        early_rejected=early,
        mid_failure=failure,
        permanent_core_retained=True,
        tombstone_no_revival=True,
    )


def race(replay_first, n):
    cmd = command()
    owner = conn()
    with owner.transaction():
        op = persist(owner, cmd, o=100)
    owner.close()
    a = conn("qa_serving" if replay_first else "qa_compactor")
    b = conn("qa_compactor" if replay_first else "qa_serving")
    obs = conn()
    try:
        a.execute("BEGIN")
        if replay_first:
            require(decide(a, cmd, now=100)[0] == "replay")
            t, box = worker(
                b,
                lambda c: c.execute(
                    "SELECT kehila.qa_compact(%s,%s)", (op, 100 + PERIOD)
                ).fetchone()[0],
            )
        else:
            require(
                a.execute(
                    "SELECT kehila.qa_compact(%s,%s)", (op, 100 + PERIOD)
                ).fetchone()
                == (True,)
            )
            t, box = worker(b, lambda c: decide(c, cmd, now=100))
        trace = wait_blocked(b.info.backend_pid, a.info.backend_pid, obs)
        a.execute("COMMIT")
        out = join(t, box)
        require(out is True if replay_first else out[0] == "replay_expired")
        require(create(cmd, now=99)[0] == "replay_expired")
        require(counts(obs, cmd["project_id"]) == dict(TABLES, operation_payloads=0))
        return dict(
            trace=trace,
            ordering="replay-first" if replay_first else "compact-first",
            result=str(out),
        )
    finally:
        a.close()
        b.close()
        obs.close()


def full_rollback():
    cmd = command()
    c = conn()
    with c.transaction():
        persist(c, cmd, o=100)
    c.close()
    require(create(cmd, now=100 + PERIOD)[0] == "replay_expired")
    require(create(cmd, now=100 + PERIOD - 1)[0] == "replay")
    return dict(
        full_record_can_replay_after_clock_rollback=True, no_highwater_claim=True
    )


def json_numbers():
    values = [2**53 + 1, 2**63, 2**64 - 1]
    data = dict(values=values, ordered=["b", "a"], unicode=["é", "e\u0301"])
    cmd = command()
    c = conn()
    try:
        with c.transaction():
            op = core(c, cmd)
            payload(c, op, cmd, data)
        got = c.execute(
            "SELECT result FROM kehila.operation_payloads WHERE operation_row_id=%s",
            (op,),
        ).fetchone()[0]
        require(got == data)
        require(got["unicode"][0] != got["unicode"][1])
        return dict(values=values, preserved=True, scope="JSONB + Python driver only")
    finally:
        c.close()


def timeout_cancel(cancel=False):
    cmd = command()
    a = conn()
    b = conn("qa_serving")
    obs = conn()
    try:
        a.execute("BEGIN")
        a.execute(
            "SELECT * FROM kehila.actors WHERE actor_id='qa_actor_a' FOR NO KEY UPDATE"
        )
        b.execute("SET lock_timeout='250ms'")
        if cancel:
            b.execute("SET lock_timeout='8s'")
            t, box = worker(b, lambda c: decide(c, cmd))
            trace = wait_blocked(b.info.backend_pid, a.info.backend_pid, obs)
            b.cancel()
            t.join(5)
            require(isinstance(box.get("error"), psycopg.Error))
            require(box["error"].sqlstate == "57014")
            d = dict(sqlstate="57014", trace=trace)
        else:

            def run():
                with b.transaction():
                    decide(b, cmd)

            d = expect_error(run, "55P03")
        require(b.execute("SELECT 1").fetchone() == (1,))
        assert_absent(cmd["project_id"])
        a.execute("ROLLBACK")
        require(create(cmd)[0] == "created")
        return dict(
            error=d,
            retry_same_token=True,
            budget="two attempts, bounded 15s statement timeout",
        )
    finally:
        a.close()
        b.close()
        obs.close()


def deadlock():
    a = conn()
    b = conn()
    obs = conn()
    try:
        a.execute("BEGIN")
        b.execute("BEGIN")
        a.execute(
            "SELECT * FROM kehila.actors WHERE actor_id='qa_actor_a' FOR NO KEY UPDATE"
        )
        b.execute(
            "SELECT * FROM kehila.actors WHERE actor_id='qa_actor_b' FOR NO KEY UPDATE"
        )

        def second(c):
            c.execute(
                "SELECT * FROM kehila.actors WHERE actor_id='qa_actor_b' FOR NO KEY UPDATE"
            )
            return "locked"

        # a already owns its transaction; its worker must not commit a nested savepoint.
        box = {}

        def run():
            try:
                box["result"] = second(a)
            except Exception as e:
                box["error"] = e

        t = threading.Thread(target=run)
        t.start()
        trace = wait_blocked(a.info.backend_pid, b.info.backend_pid, obs)
        errors = []
        try:
            b.execute(
                "SELECT * FROM kehila.actors WHERE actor_id='qa_actor_a' FOR NO KEY UPDATE"
            )
        except psycopg.Error as e:
            errors.append(e.sqlstate)
            b.execute("ROLLBACK")
        t.join(5)
        require(not t.is_alive())
        if "error" in box:
            errors.append(box["error"].sqlstate)
        a.execute("ROLLBACK")
        b.execute("ROLLBACK")
        require("40P01" in errors, lambda: errors)
        cmd = command()
        require(create(cmd)[0] == "created")
        return dict(errors=errors, trace=trace, complete_transaction_retry=True)
    finally:
        a.close()
        b.close()
        obs.close()


def main():
    initialize()

    install_helpers()

    for role, sql, args in [
        (
            "qa_serving",
            "SELECT * FROM kehila.actors WHERE actor_id='qa_actor_a' FOR NO KEY UPDATE",
            (),
        ),
        ("qa_serving", "SELECT * FROM kehila.operations FOR SHARE", ()),
        (
            "qa_serving",
            "UPDATE kehila.actors SET active=false WHERE actor_id='qa_actor_a'",
            (),
        ),
        (
            "qa_serving",
            "UPDATE kehila.actors SET can_create_project=false WHERE actor_id='qa_actor_a'",
            (),
        ),
        ("qa_serving", "UPDATE kehila.operations SET payload_retired=true", ()),
        ("qa_serving", "DELETE FROM kehila.operations", ()),
        ("qa_serving", "DELETE FROM kehila.project_grants", ()),
        ("qa_serving", "ALTER TABLE kehila.actors ADD COLUMN forbidden int", ()),
        ("qa_serving", "ALTER TABLE kehila.operations DISABLE TRIGGER ALL", ()),
        ("qa_serving", "SET ROLE qa_migration", ()),
        ("qa_serving", "SET ROLE qa_function_owner", ()),
        ("qa_serving", "SELECT kehila.qa_compact(1,0)", ()),
        ("qa_unrelated", "SELECT * FROM kehila.qa_lock_actor('qa_actor_a')", ()),
        ("qa_unrelated", "SELECT * FROM kehila.qa_lock_core(1)", ()),
        ("qa_unrelated", "SELECT kehila.qa_compact(1,0)", ()),
        ("qa_serving", "CREATE TABLE kehila.forbidden(n int)", ()),
    ]:
        test(
            ("B01." if "FOR " in sql or "UPDATE kehila.actors" in sql else "B02.")
            + role
            + "."
            + sql,
            lambda role=role, sql=sql, args=args: db_error(role, sql, args),
            "experimental ACLs",
        )

    test("B01.helpers", lock_helpers, "experimental helpers")

    test("B02.search-path", search_path, "experimental helpers")

    for commit in [True, False]:
        for n in range(20):
            test(
                "B03." + ("commit" if commit else "abort") + f".{n:02}",
                lambda commit=commit, n=n: same_key(commit, n),
                "experimental protocol",
            )

    for same in [True, False]:
        for n in range(20):
            test(
                "B04." + ("same-actor" if same else "independent-actors") + f".{n:02}",
                lambda same=same, n=n: independence(same, n),
                "experimental protocol",
            )

    test("B05.collisions", collisions, "experimental protocol")

    test("B06.original-result-and-equality", exact_replay, "experimental protocol")

    for replay in [False, True]:
        for n in range(20):
            test(
                "B07.revoke-first." + str(replay) + f".{n:02}",
                lambda replay=replay, n=n: revocation_first(replay, n),
                "experimental authority protocol",
            )
            test(
                "B07.command-first." + str(replay) + f".{n:02}",
                lambda replay=replay, n=n: command_first(replay, n),
                "experimental authority protocol",
            )

    test("B08.current-history-mismatch", mismatch, "experimental validation")

    rec(
        "B08.grant-management",
        "BLOCKED",
        reason="No grant-management route, final owner-right mapping or command family; baseline slice guard only permits version-one active creation event.",
    )

    test("B09.no-project-lock", project_lock, "experimental protocol")

    test("B10.recorded-clock", clock_sample, "experimental clock mapping")

    test("B11.boundaries", boundaries, "experimental injected trusted time")

    test(
        "B12.compaction",
        compaction,
        "experimental compactor with injected trusted time",
    )

    for first in [True, False]:
        for n in range(20):
            test(
                "B13." + ("replay-first" if first else "compact-first") + f".{n:02}",
                lambda first=first, n=n: race(first, n),
                "experimental race protocol",
            )

    test("B13.full-clock-rollback", full_rollback, "experimental injected time")

    test("B14.jsonb-exactness", json_numbers, "storage/driver experiment")

    rec(
        "B14.final-codec",
        "BLOCKED",
        reason="Final canonical request/result/snapshot codec and old-version decoders are not implemented. QA JSON bytes are not a substitute.",
    )

    test("B15.lock-timeout", timeout_cancel, "experimental bounded retry")

    test("B15.cancellation", lambda: timeout_cancel(True), "experimental bounded retry")

    test("B15.deadlock", deadlock, "experimental bounded retry")

    for case, reason in [
        (
            "B01.production-session-binding",
            "Trusted actor supplied by harness, not real authenticated session",
        ),
        (
            "B05.target-allocation",
            "Targets generated by harness; production allocation absent",
        ),
        ("B10.production-clock", "Harness mapping verified; native adapter absent"),
        (
            "B12.production-compactor",
            "QA helper accepts injected time; not production-safe API",
        ),
    ]:
        rec(case, "BLOCKED", reason=reason)

    (ROOT / "phase-b-results.json").write_text(
        json.dumps(RESULTS, indent=2, default=str)
    )

    raise SystemExit(1 if any(x["status"] == "FAIL" for x in RESULTS) else 2)


if __name__ == "__main__":
    main()
