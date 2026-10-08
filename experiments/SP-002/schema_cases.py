from common import *


def negative(sql, args=(), state="23514", constraint=None, cmd=None):
    c = conn()
    cmd = cmd or command()
    try:

        def run():
            with c.transaction():
                persist(c, cmd)
                c.execute(sql, args or (cmd["project_id"],))

        d = expect_error(run, state, constraint)
        assert_absent(cmd["project_id"])
        return d
    finally:
        c.close()


def a02():
    cmd = command("qa_project_a", "qa_token_a")
    with conn("qa_serving") as c:
        with c.transaction():
            persist(c, cmd)
            c.execute("SET CONSTRAINTS ALL IMMEDIATE")
            validate(c, cmd)
        validate(c, cmd)
    (ROOT / "fixture-command.json").write_text(json.dumps(cmd, indent=2))
    return dict(
        counts=TABLES,
        relational_snapshot_result_equality=True,
        role="qa_serving",
        codec="QA-only JSON v1; not approved codec",
    )


test("A02.complete", a02)


def rollback_stage(n):
    cmd = command()
    c = conn("qa_serving")
    stages = []
    try:
        c.execute("BEGIN")
        for i, (stage, _) in enumerate(statements(c, cmd), 1):
            stages.append(stage)
            if i == n:
                d = expect_error(lambda: c.execute("SELECT 1/0"), "22012")
                break
        c.execute("ROLLBACK")
        assert_absent(cmd["project_id"])
        return dict(write=n, stages=stages, error=d)
    finally:
        c.close()


for n in range(1, 27):
    test(f"A03.write-{n:02}", lambda n=n: rollback_stage(n))
for mode in ["immediate", "commit"]:

    def deferred(mode=mode):
        cmd = command()
        c = conn("qa_serving")
        try:

            def run():
                with c.transaction():
                    for stage, _ in statements(c, cmd):
                        if stage == "work_item_types":
                            break
                    if mode == "immediate":
                        c.execute("SET CONSTRAINTS ALL IMMEDIATE")

            d = expect_error(run, "23503")
            assert_absent(cmd["project_id"])
            return dict(boundary=mode, error=d)
        finally:
            c.close()

    test("A03.deferred-" + mode, deferred)


def visibility(commit):
    cmd = command()
    a = conn("qa_serving")
    b = conn()
    try:
        a.execute("BEGIN")
        persist(a, cmd)
        assert all(v == 0 for v in counts(b, cmd["project_id"]).values())
        a.execute("COMMIT" if commit else "ROLLBACK")
        assert counts(b, cmd["project_id"]) == (
            TABLES if commit else {k: 0 for k in TABLES}
        )
        return dict(before="absent", after="complete" if commit else "absent")
    finally:
        a.close()
        b.close()


for commit in [True, False]:
    test(
        "A04." + ("commit" if commit else "abort"),
        lambda commit=commit: visibility(commit),
    )
violations = [
    (
        "initial-membership",
        "DELETE FROM kehila.workflow_statuses WHERE project_id=%s AND status_id='new'",
        "23503",
        "workflow_initial_membership",
    ),
    (
        "initial-phase",
        "UPDATE kehila.statuses SET phase='active' WHERE project_id=%s AND status_id='new'",
        "23503",
        None,
    ),
    (
        "default-membership",
        "DELETE FROM kehila.type_workflows WHERE project_id=%s AND type_id='task'",
        "23503",
        "type_default_membership",
    ),
    (
        "title-owner",
        "UPDATE kehila.work_item_types SET title_field_id='milestone-title' WHERE project_id=%s AND type_id='task'",
        "23503",
        "type_title_field",
    ),
    (
        "position",
        "UPDATE kehila.statuses SET position=0 WHERE project_id=%s AND status_id='active'",
        "23505",
        None,
    ),
    (
        "attribution-actor",
        "UPDATE kehila.projects SET created_by='qa_actor_b' WHERE project_id=%s",
        "23503",
        None,
    ),
    (
        "attribution-project",
        "UPDATE kehila.projects SET project_id='wrong-project' WHERE project_id=%s",
        "23503",
        None,
    ),
    (
        "attribution-family",
        "UPDATE kehila.projects SET creation_command_family='item_edit' WHERE project_id=%s",
        "23514",
        None,
    ),
    (
        "attribution-kind",
        "UPDATE kehila.projects SET creation_target_kind='user' WHERE project_id=%s",
        "23514",
        None,
    ),
    (
        "phase-edge",
        "UPDATE kehila.workflow_phase_changes SET from_phase='done',to_phase='new' WHERE project_id=%s AND position=0",
        "23514",
        None,
    ),
    (
        "field-kind",
        "UPDATE kehila.fields SET value_kind='number' WHERE project_id=%s AND field_id='task-title'",
        "23514",
        "slice1_field_kind",
    ),
    (
        "field-origin",
        "UPDATE kehila.fields SET origin='project' WHERE project_id=%s AND field_id='task-title'",
        "23514",
        "slice1_field_origin",
    ),
    (
        "field-usage",
        "UPDATE kehila.fields SET usage='required' WHERE project_id=%s AND field_id='task-title'",
        "23514",
        "slice1_field_usage",
    ),
    (
        "field-archival",
        "UPDATE kehila.fields SET archived=true WHERE project_id=%s AND field_id='task-title'",
        "23514",
        "slice1_field_archival",
    ),
]
for name, sql, state, constraint in violations:
    test(
        "A05." + name,
        lambda sql=sql, state=state, constraint=constraint: negative(
            sql, state=state, constraint=constraint
        ),
    )


def bad_core(changes, state="23514", constraint=None):
    cmd = command()
    with conn() as c:

        def run():
            with c.transaction():
                core(c, cmd, **changes)

        d = expect_error(run, state, constraint)
    assert_absent(cmd["project_id"])
    return d


for name, change in [
    ("missing", dict(target_project_id=None)),
    ("extra", dict(target_item_id="extra")),
    ("kind", dict(target_kind="unknown")),
    ("family", dict(command_family="unknown")),
    ("pair", dict(command_family="item_edit")),
    ("slice", dict(command_family="project_metadata")),
]:
    test("A06." + name, lambda change=change: bad_core(change))


def duplicate():
    cmd = command()
    with conn() as c:

        def run():
            with c.transaction():
                op = core(c, cmd)
                payload(c, op, cmd)
                core(c, cmd)

        d = expect_error(run, "23505", "operations_scoped_key")
    assert_absent(cmd["project_id"])
    return d


test("A06.null-components-duplicate", duplicate)
# Separate database: the only schema change is removing the temporary family guard.
with conn() as c:
    c.execute("CREATE DATABASE qa_mapping TEMPLATE template0 ENCODING 'UTF8'")
with conn(db="qa_mapping") as c:
    c.execute((ROOT / "sql" / "baseline.sql").read_text(), prepare=False)
    c.execute(
        "ALTER TABLE kehila.operations DROP CONSTRAINT slice1_operation_family; INSERT INTO kehila.actors VALUES ('qa_actor_a',true,true)",
        prepare=False,
    )
families = [
    "project_create",
    "project_metadata",
    "project_archive",
    "configuration_change",
    "choice_option_admin",
    "item_create",
    "item_edit",
    "item_archive",
    "item_transition",
    "item_conversion",
    "selection",
    "relationship_create",
    "knowledge_create",
    "knowledge_edit",
    "follow_up_create",
]
kinds = ["project", "work_item", "user", "relationship"]
for i, family in enumerate(families):
    for j, kind in enumerate(kinds):

        def pair(i=i, j=j, family=family, kind=kind):
            fields = dict(
                command_family=family,
                target_kind=kind,
                target_project_id=None if kind == "user" else "p",
                target_item_id="i" if kind == "work_item" else None,
                target_user_id="u" if kind == "user" else None,
                target_relationship_id="r" if kind == "relationship" else None,
            )
            expected = 0 if i < 5 else 2 if i == 10 else 3 if i == 11 else 1
            c = conn(db="qa_mapping")
            cmd = command(token=family + "-" + kind)
            try:

                def run():
                    with c.transaction():
                        op = core(c, cmd, **fields)
                        payload(c, op, cmd)

                if j == expected:
                    run()
                    return dict(accepted=True)
                return expect_error(run, "23514", "operations_family_target")
            finally:
                c.close()

        test(
            "A07." + family + "/" + kind,
            pair,
            "modified-schema compatibility experiment",
        )


def scalar(typ, value, valid):
    with conn() as c:
        if valid:
            result = c.execute("SELECT %s::text::kehila." + typ, (value,)).fetchone()[0]
            if typ == "u64":
                assert result == decimal.Decimal(value), (value, result)
            else:
                assert result == value
            return dict(value=value, result=str(result))
        return expect_error(
            lambda: c.execute("SELECT %s::text::kehila." + typ, (value,)), "23514"
        )


for v in ["0", "1.0", str(2**63), str(2**64 - 1)]:
    test("A08.valid-" + v, lambda v=v: scalar("u64", v, True))
for v in ["-1", "0.1", str(2**64), "NaN", "Infinity", "-Infinity"]:
    test("A08.invalid-" + v, lambda v=v: scalar("u64", v, False))
test(
    "A08.deadline-overflow",
    lambda: bad_core(
        dict(replay_origin_ms=2**64 - 1, replay_deadline_ms=2**64 - 1 + PERIOD)
    ),
)
test(
    "A08.window-mismatch",
    lambda: bad_core(
        dict(replay_origin_ms=0, replay_deadline_ms=PERIOD + 1),
        constraint="operations_replay_window",
    ),
)
for label, v, valid in [
    ("empty", "", False),
    ("128", "x" * 128, True),
    ("129", "x" * 129, False),
    ("utf8-128", "é" * 64, True),
    ("utf8-130", "é" * 65, False),
    ("case-A", "Case", True),
    ("case-a", "case", True),
]:
    test("A09.ident-" + label, lambda v=v, valid=valid: scalar("ident", v, valid))


def nul():
    with conn() as c:
        return expect_error(
            lambda: c.execute(
                "SELECT convert_from(decode('610062','hex'),'UTF8')::kehila.ident"
            ),
            "22021",
        )


test("A09.nul-server", nul)
for label, v, valid in [
    ("empty", "", False),
    ("128", "x" * 128, True),
    ("129", "x" * 129, False),
    ("space", "a b", False),
    ("tab", "a\tb", False),
    ("newline", "a\nb", False),
    ("nonascii", "é", False),
    ("punctuation", "!~", True),
]:
    test(
        "A09.token-" + label,
        lambda v=v, valid=valid: scalar("operation_token", v, valid),
    )
for v, valid in [
    ("QA", True),
    ("A", False),
    ("A" + "1" * 11, True),
    ("A" + "1" * 12, False),
    ("qa", False),
    ("1A", False),
]:
    test("A09.prefix-" + v, lambda v=v, valid=valid: scalar("project_prefix", v, valid))


def lone_core():
    cmd = command()
    with conn() as c:

        def run():
            with c.transaction():
                core(c, cmd)

        return expect_error(run, "23514", "operation_payload_shape")


test("A10.core-without-payload", lone_core)
test(
    "A10.fresh-retired",
    lambda: bad_core(dict(payload_retired=True), constraint="operation_core_fresh"),
)


def lone_payload():
    with conn() as c:
        return expect_error(lambda: payload(c, 999999999, command()), "23503")


test("A10.payload-without-core", lone_payload)
for name, sql in [
    (
        "mutate",
        "UPDATE kehila.operations SET token='changed' WHERE target_project_id=%s",
    ),
    ("delete", "DELETE FROM kehila.operations WHERE target_project_id=%s"),
    (
        "noop",
        "UPDATE kehila.operations SET payload_retired=false WHERE target_project_id=%s",
    ),
]:
    test(
        "A10." + name,
        lambda sql=sql: negative(sql, constraint="operation_core_immutable"),
    )
for action in [
    "UPDATE kehila.operations SET payload_retired=false",
    "UPDATE kehila.operations SET token=token",
]:

    def retired_change(action=action):
        cmd = command()
        c = conn()
        try:

            def run():
                with c.transaction():
                    op = persist(c, cmd)
                    c.execute(
                        "UPDATE kehila.operations SET payload_retired=true WHERE operation_row_id=%s",
                        (op,),
                    )
                    c.execute(
                        "DELETE FROM kehila.operation_payloads WHERE operation_row_id=%s",
                        (op,),
                    )
                    c.execute(action + " WHERE operation_row_id=%s", (op,))

            d = expect_error(run, "23514", "operation_core_immutable")
            assert_absent(cmd["project_id"])
            return d
        finally:
            c.close()

    test("A10.retired-" + ("reverse" if "false" in action else "noop"), retired_change)
for role in ["qa_serving", "postgres"]:
    for sql in [
        "TRUNCATE kehila.operation_payloads",
        "TRUNCATE kehila.operations CASCADE",
        "TRUNCATE kehila.operations,kehila.operation_payloads CASCADE",
    ]:

        def truncate(role=role, sql=sql):
            with conn(role) as c:
                return expect_error(
                    lambda: c.execute(sql),
                    "42501" if role == "qa_serving" else "23514",
                    None if role == "qa_serving" else "operation_storage_no_truncate",
                )

        test("A11." + role + "." + sql, truncate)


def serving_move():
    with conn("qa_serving") as c:
        return expect_error(
            lambda: c.execute(
                "UPDATE kehila.operation_payloads SET operation_row_id=operation_row_id"
            ),
            "42501",
        )


test("A12.serving-denied", serving_move)


def privileged_move():
    c = conn()
    cmd = command()
    try:
        c.execute("BEGIN")
        newid = core(c, cmd)
        oldid = c.execute(
            "SELECT operation_row_id FROM kehila.operations WHERE target_project_id='qa_project_a'"
        ).fetchone()[0]
        c.execute(
            "UPDATE kehila.operation_payloads SET operation_row_id=%s WHERE operation_row_id=%s",
            (newid, oldid),
        )
        c.execute("SET CONSTRAINTS ALL IMMEDIATE")
        orphan = c.execute(
            "SELECT count(*) FROM kehila.operations o LEFT JOIN kehila.operation_payloads p USING(operation_row_id) WHERE NOT payload_retired AND p.operation_row_id IS NULL"
        ).fetchone()[0]
        assert orphan == 1
        c.execute("ROLLBACK")
        rec(
            "A12.privileged-old-core",
            "FAIL",
            known_boundary=True,
            role="postgres",
            forced_deferred_checks="passed despite orphan",
            orphan_count=orphan,
            experiment_rolled_back=True,
        )
    finally:
        c.close()


privileged_move()
(ROOT / "phase-a-results.json").write_text(json.dumps(RESULTS, indent=2, default=str))
raise SystemExit(1 if any(x["status"] == "FAIL" for x in RESULTS) else 0)
