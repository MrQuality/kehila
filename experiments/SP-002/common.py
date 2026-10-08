"""Experimental protocol; not a product adapter or approved storage codec."""

import json, pathlib, subprocess, hashlib, uuid, traceback, os, sys
import psycopg
from psycopg.types.json import Jsonb

ROOT = pathlib.Path(__file__).resolve().parent
CONF = {}
PERIOD = 7776000000
RESULTS = []
TABLES = {
    "operations": 1,
    "operation_payloads": 1,
    "projects": 1,
    "configuration_revisions": 1,
    "project_grants": 1,
    "project_grant_events": 1,
    "statuses": 3,
    "workflows": 1,
    "workflow_statuses": 3,
    "workflow_phase_changes": 5,
    "work_item_types": 2,
    "type_workflows": 2,
    "fields": 4,
}


def initialize():
    """Load runtime fixtures explicitly; importing cases never starts an experiment."""
    if sys.platform not in {"win32", "linux"}:
        raise RuntimeError("SP-002 supports Windows and Linux hosts only")
    if sys.flags.optimize:
        raise RuntimeError(
            "SP-002 refuses optimized Python; omit -O and unset PYTHONOPTIMIZE"
        )
    CONF.clear()
    CONF.update(json.loads((ROOT / "connection.json").read_text(encoding="utf-8")))
    RESULTS.clear()


def require(condition, message="Experiment invariant failed"):
    """Evaluate invariants explicitly; callable diagnostics are evaluated only on failure."""
    if not condition:
        raise AssertionError(message() if callable(message) else message)


def conn(role="postgres", db=None):
    if not CONF:
        raise RuntimeError("Call initialize() before opening an experiment connection")
    cfg = dict(CONF, user=role)
    if db:
        cfg["dbname"] = db
    c = psycopg.connect(**cfg, autocommit=True, connect_timeout=5)
    c.execute("SET statement_timeout='15s'; SET lock_timeout='8s'", prepare=False)
    return c


def rec(case, status="PASS", **data):
    RESULTS.append(dict(case=case, status=status, **data))
    (ROOT / "results.json").write_text(json.dumps(RESULTS, indent=2, default=str))
    print(case, status, flush=True)


def test(case, fn, classification="schema"):
    try:
        detail = fn()
        rec(case, classification=classification, detail=detail)
    except Exception as e:
        rec(
            case,
            "FAIL",
            classification=classification,
            error=str(e),
            trace=traceback.format_exc(),
        )


def expect_error(fn, state, constraint=None):
    try:
        fn()
    except psycopg.Error as e:
        d = dict(
            sqlstate=e.sqlstate,
            constraint=e.diag.constraint_name,
            table=e.diag.table_name,
            message=e.diag.message_primary,
        )
        require(e.sqlstate == state, lambda: d)
        if constraint:
            require(e.diag.constraint_name == constraint, lambda: d)
        return d
    raise AssertionError("Expected error " + state + " but operation succeeded")


def command(p=None, token="qa-token", name="QA Project", prefix="QA", unit="hours"):
    return dict(
        project_id=p or "qa_" + uuid.uuid4().hex,
        operation_id=token,
        name=name,
        prefix=prefix,
        estimate_unit=unit,
    )


def seed(cmd):
    return json.loads(
        subprocess.check_output(
            [
                str(ROOT / ("seed_oracle" + (".exe" if os.name == "nt" else ""))),
                cmd["project_id"],
                cmd["operation_id"],
                cmd["name"],
                cmd["prefix"],
                cmd["estimate_unit"],
            ],
            text=True,
            encoding="utf-8",
            timeout=15,
        )
    )


def request_bytes(cmd):
    return json.dumps(
        cmd, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode()


def origin(c):
    return int(
        c.execute(
            "SELECT floor(extract(epoch from clock_timestamp())*1000)::numeric"
        ).fetchone()[0]
    )


def core(connection, cmd, actor="qa_actor_a", o=None, **changes):
    replay_origin_ms = origin(connection) if o is None else o
    values = dict(
        actor_id=actor,
        command_family="project_create",
        target_kind="project",
        target_project_id=cmd["project_id"],
        target_item_id=None,
        target_user_id=None,
        target_relationship_id=None,
        token=cmd["operation_id"],
        request_codec_version=1,
        request_sha256=hashlib.sha256(request_bytes(cmd)).digest(),
        required_grant="project_create",
        grant_policy_version=1,
        replay_origin_ms=replay_origin_ms,
        replay_deadline_ms=replay_origin_ms + PERIOD,
        payload_retired=False,
    )
    values.update(changes)
    sql = (
        "INSERT INTO kehila.operations ("
        + ",".join(values)
        + ") VALUES ("
        + ",".join(["%s"] * len(values))
        + ") RETURNING operation_row_id"
    )
    return connection.execute(sql, list(values.values())).fetchone()[0]


def payload(c, op, cmd, result=None):
    c.execute(
        "INSERT INTO kehila.operation_payloads VALUES (%s,%s,1,%s)",
        (op, request_bytes(cmd), Jsonb(result or {})),
    )


def statements(connection, cmd, actor="qa_actor_a", o=None):
    """Yield after each of the 26 physical row writes, allowing fault injection."""
    seed_result = seed(cmd)
    project = seed_result["project"]
    configuration = seed_result["configuration"]
    project_id = cmd["project_id"]
    operation_row_id = core(connection, cmd, actor, o)
    yield "operations", operation_row_id
    payload(connection, operation_row_id, cmd, seed_result)
    yield "operation_payloads", operation_row_id
    connection.execute(
        "INSERT INTO kehila.projects(project_id,name,current_prefix,estimate_unit,configuration_revision,next_sequence,ever_estimated,archived,created_by,creation_operation_row_id,seed_profile) VALUES (%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s)",
        (
            project_id,
            project["name"],
            project["prefix"],
            project["estimate_unit"],
            project["configuration_revision"],
            project["next_sequence"],
            project["ever_estimated"],
            project["archived"],
            actor,
            operation_row_id,
            seed_result["seed_profile"],
        ),
    )
    yield "projects", operation_row_id
    connection.execute(
        "INSERT INTO kehila.configuration_revisions VALUES (%s,1,1,%s,%s,%s,%s,'project_create','project')",
        (project_id, Jsonb(configuration), Jsonb(project), actor, operation_row_id),
    )
    yield "configuration_revisions", operation_row_id
    connection.execute(
        "INSERT INTO kehila.project_grants VALUES (%s,%s,'initial_owner',true,1)",
        (project_id, actor),
    )
    yield "project_grants", operation_row_id
    connection.execute(
        "INSERT INTO kehila.project_grant_events(project_id,actor_id,version,access_profile,active,granted_by,granting_operation_row_id,command_family) VALUES (%s,%s,1,'initial_owner',true,%s,%s,'project_create')",
        (project_id, actor, actor, operation_row_id),
    )
    yield "project_grant_events", operation_row_id
    for position, status in enumerate(configuration["statuses"]):
        connection.execute(
            "INSERT INTO kehila.statuses VALUES (%s,%s,%s,%s,%s,%s)",
            (project_id, status["id"], status["name"], status["phase"], status["archived"], position),
        )
        yield "statuses", operation_row_id
    for position, workflow in enumerate(configuration["workflows"]):
        connection.execute(
            "INSERT INTO kehila.workflows VALUES (%s,%s,%s,'new',%s,%s)",
            (project_id, workflow["id"], workflow["initial_status_id"], workflow["archived"], position),
        )
        yield "workflows", operation_row_id
        for member_position, status_id in enumerate(workflow["status_ids"]):
            connection.execute(
                "INSERT INTO kehila.workflow_statuses VALUES (%s,%s,%s,%s)",
                (project_id, workflow["id"], status_id, member_position),
            )
            yield "workflow_statuses", operation_row_id
        for member_position, phase_change in enumerate(workflow["permitted_phase_changes"]):
            connection.execute(
                "INSERT INTO kehila.workflow_phase_changes VALUES (%s,%s,%s,%s,%s)",
                (project_id, workflow["id"], *phase_change, member_position),
            )
            yield "workflow_phase_changes", operation_row_id
    for position, item_type in enumerate(configuration["types"]):
        connection.execute(
            "INSERT INTO kehila.work_item_types VALUES (%s,%s,%s,%s,%s,%s)",
            (
                project_id,
                item_type["id"],
                item_type["default_workflow_id"],
                item_type["title_field_id"],
                item_type["archived"],
                position,
            ),
        )
        yield "work_item_types", operation_row_id
        for member_position, workflow_id in enumerate(item_type["permitted_workflows"]):
            connection.execute(
                "INSERT INTO kehila.type_workflows VALUES (%s,%s,%s,%s)",
                (project_id, item_type["id"], workflow_id, member_position),
            )
            yield "type_workflows", operation_row_id
    for position, field in enumerate(configuration["fields"]):
        connection.execute(
            "INSERT INTO kehila.fields VALUES (%s,%s,%s,%s,%s,%s,%s,%s,%s)",
            (
                project_id,
                field["id"],
                field["owner_type"],
                field["name"],
                field["kind"],
                field["origin"],
                field["usage"],
                field["archived"],
                position,
            ),
        )
        yield "fields", operation_row_id
    return operation_row_id


def persist(c, cmd, actor="qa_actor_a", o=None):
    last = None
    for _, last in statements(c, cmd, actor, o):
        pass
    return last


def counts(c, p):
    out = {}
    for t in TABLES:
        if t == "operations":
            sql = "SELECT count(*) FROM kehila.operations WHERE target_project_id=%s"
        elif t == "operation_payloads":
            sql = "SELECT count(*) FROM kehila.operation_payloads p JOIN kehila.operations o USING(operation_row_id) WHERE o.target_project_id=%s"
        else:
            sql = "SELECT count(*) FROM kehila." + t + " WHERE project_id=%s"
        out[t] = c.execute(sql, (p,)).fetchone()[0]
    return out


def assert_absent(p):
    with conn() as c:
        require(all((n == 0 for n in counts(c, p).values())), lambda: counts(c, p))


def load_result(c, p):
    return c.execute(
        "SELECT result FROM kehila.operation_payloads p JOIN kehila.operations o USING(operation_row_id) WHERE target_project_id=%s",
        (p,),
    ).fetchone()[0]


def reconstruct(c, p):
    x = c.execute(
        "SELECT name,current_prefix,estimate_unit,configuration_revision,next_sequence,ever_estimated,archived,seed_profile FROM kehila.projects WHERE project_id=%s",
        (p,),
    ).fetchone()
    project = dict(
        id=p,
        name=x[0],
        prefix=x[1],
        estimate_unit=x[2],
        configuration_revision=int(x[3]),
        next_sequence=int(x[4]),
        ever_estimated=x[5],
        archived=x[6],
    )
    statuses = [
        dict(id=s, name=n, phase=ph, archived=a, group_id=None)
        for s, n, ph, a in c.execute(
            "SELECT status_id,name,phase,archived FROM kehila.statuses WHERE project_id=%s ORDER BY position",
            (p,),
        )
    ]
    workflows = []
    for w, initial, a in c.execute(
        "SELECT workflow_id,initial_status_id,archived FROM kehila.workflows WHERE project_id=%s ORDER BY position",
        (p,),
    ):
        workflows.append(
            dict(
                id=w,
                initial_status_id=initial,
                archived=a,
                status_ids=[
                    x[0]
                    for x in c.execute(
                        "SELECT status_id FROM kehila.workflow_statuses WHERE project_id=%s AND workflow_id=%s ORDER BY position",
                        (p, w),
                    )
                ],
                permitted_phase_changes=[
                    list(x)
                    for x in c.execute(
                        "SELECT from_phase,to_phase FROM kehila.workflow_phase_changes WHERE project_id=%s AND workflow_id=%s ORDER BY position",
                        (p, w),
                    )
                ],
            )
        )
    types = []
    for t, w, f, a in c.execute(
        "SELECT type_id,default_workflow_id,title_field_id,archived FROM kehila.work_item_types WHERE project_id=%s ORDER BY position",
        (p,),
    ):
        types.append(
            dict(
                id=t,
                default_workflow_id=w,
                title_field_id=f,
                archived=a,
                permitted_workflows=[
                    x[0]
                    for x in c.execute(
                        "SELECT workflow_id FROM kehila.type_workflows WHERE project_id=%s AND type_id=%s ORDER BY position",
                        (p, t),
                    )
                ],
            )
        )
    fields = [
        dict(
            id=f,
            owner_type=t,
            name=n,
            kind=k,
            origin=o,
            usage=u,
            archived=a,
            options=[],
        )
        for f, t, n, k, o, u, a in c.execute(
            "SELECT field_id,owner_type_id,name,value_kind,origin,usage,archived FROM kehila.fields WHERE project_id=%s ORDER BY position",
            (p,),
        )
    ]
    return dict(
        project=project,
        configuration=dict(
            project_id=p,
            revision=int(x[3]),
            project_archived=x[6],
            statuses=statuses,
            status_groups=[],
            workflows=workflows,
            types=types,
            fields=fields,
            relationship_types=[],
        ),
        seed_profile=x[7],
    )


def validate(c, cmd, actor="qa_actor_a"):
    p = cmd["project_id"]
    expected = seed(cmd)
    require(counts(c, p) == TABLES, lambda: counts(c, p))
    require(reconstruct(c, p) == expected)
    require(load_result(c, p) == expected)
    snap = c.execute(
        "SELECT configuration_snapshot,project_snapshot FROM kehila.configuration_revisions WHERE project_id=%s",
        (p,),
    ).fetchone()
    require(snap == (expected["configuration"], expected["project"]))
    grant = c.execute(
        "SELECT g.actor_id,g.access_profile,g.active,g.version,e.actor_id,e.access_profile,e.active,e.version,e.granted_by FROM kehila.project_grants g JOIN kehila.project_grant_events e USING(project_id,actor_id) WHERE g.project_id=%s",
        (p,),
    ).fetchone()
    require(
        grant
        == (actor, "initial_owner", True, 1, actor, "initial_owner", True, 1, actor),
        lambda: grant,
    )
    return expected


def install_roles():
    c = conn()
    password = CONF["password"]
    for role, login in [
        ("qa_migration", False),
        ("qa_function_owner", False),
        ("qa_serving", True),
        ("qa_compactor", True),
        ("qa_unrelated", True),
    ]:
        from psycopg import sql

        c.execute(
            sql.SQL("CREATE ROLE {} {} NOINHERIT").format(
                sql.Identifier(role), sql.SQL("LOGIN" if login else "NOLOGIN")
            )
        )
        if login:
            c.execute(
                sql.SQL("ALTER ROLE {} PASSWORD {}").format(
                    sql.Identifier(role), sql.Literal(password)
                )
            )
    c.execute("ALTER SCHEMA kehila OWNER TO qa_migration")
    for row in c.execute(
        "SELECT tablename FROM pg_tables WHERE schemaname='kehila'"
    ).fetchall():
        c.execute("ALTER TABLE kehila." + row[0] + " OWNER TO qa_migration")
    for name in ["ident", "u64", "operation_token", "project_prefix", "phase"]:
        c.execute("ALTER DOMAIN kehila." + name + " OWNER TO qa_migration")
    for name in [
        "check_operation_payload",
        "protect_operation_core",
        "reject_operation_truncate",
    ]:
        c.execute("ALTER FUNCTION kehila." + name + "() OWNER TO qa_migration")
    c.execute(
        "REVOKE ALL ON SCHEMA kehila FROM PUBLIC; REVOKE ALL ON ALL FUNCTIONS IN SCHEMA kehila FROM PUBLIC; GRANT USAGE ON SCHEMA kehila TO qa_serving,qa_compactor,qa_function_owner; GRANT SELECT ON ALL TABLES IN SCHEMA kehila TO qa_serving; GRANT INSERT ON ALL TABLES IN SCHEMA kehila TO qa_serving; REVOKE INSERT ON kehila.actors FROM qa_serving; GRANT USAGE ON ALL SEQUENCES IN SCHEMA kehila TO qa_serving; GRANT SELECT,UPDATE ON kehila.actors TO qa_function_owner; GRANT SELECT,UPDATE ON kehila.operations TO qa_function_owner; GRANT SELECT,DELETE ON kehila.operation_payloads TO qa_function_owner",
        prepare=False,
    )
    c.execute(
        "INSERT INTO kehila.actors VALUES ('qa_actor_a',true,true),('qa_actor_b',true,true),('qa_denied',false,false)"
    )
    c.close()


def helper_sql():
    return """-- QA feasibility helpers only; supplied actor/time are trusted harness inputs.
CREATE OR REPLACE FUNCTION kehila.qa_lock_actor(p text) RETURNS TABLE(active boolean,can_create_project boolean)
LANGUAGE sql SECURITY DEFINER SET search_path=pg_catalog,kehila,pg_temp AS $$ SELECT active,can_create_project FROM kehila.actors WHERE actor_id=p FOR NO KEY UPDATE $$;
CREATE OR REPLACE FUNCTION kehila.qa_lock_core(p bigint) RETURNS SETOF kehila.operations
LANGUAGE sql SECURITY DEFINER SET search_path=pg_catalog,kehila,pg_temp AS $$ SELECT * FROM kehila.operations WHERE operation_row_id=p FOR SHARE $$;
CREATE OR REPLACE FUNCTION kehila.qa_compact(p bigint,n numeric) RETURNS boolean
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,kehila,pg_temp AS $$
DECLARE o kehila.operations;
BEGIN SELECT * INTO o FROM kehila.operations WHERE operation_row_id=p FOR NO KEY UPDATE;
IF NOT FOUND THEN RAISE EXCEPTION 'unknown operation'; END IF;
IF o.payload_retired THEN RETURN false; END IF;
IF n<o.replay_deadline_ms THEN RAISE EXCEPTION 'not expired' USING ERRCODE='22023'; END IF;
UPDATE kehila.operations SET payload_retired=true WHERE operation_row_id=p;
DELETE FROM kehila.operation_payloads WHERE operation_row_id=p;
RETURN true; END; $$;
ALTER FUNCTION kehila.qa_lock_actor(text) OWNER TO qa_function_owner;
ALTER FUNCTION kehila.qa_lock_core(bigint) OWNER TO qa_function_owner;
ALTER FUNCTION kehila.qa_compact(bigint,numeric) OWNER TO qa_function_owner;
REVOKE ALL ON FUNCTION kehila.qa_lock_actor(text),kehila.qa_lock_core(bigint),kehila.qa_compact(bigint,numeric) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kehila.qa_lock_actor(text),kehila.qa_lock_core(bigint) TO qa_serving;
GRANT EXECUTE ON FUNCTION kehila.qa_compact(bigint,numeric) TO qa_compactor;
"""


def install_helpers():
    s = helper_sql()
    (ROOT / "experimental-helpers.sql").write_text(s)
    with conn() as c:
        c.execute(s, prepare=False)


def decide(connection, cmd, actor="qa_actor_a", now=None):
    auth = connection.execute("SELECT * FROM kehila.qa_lock_actor(%s)", (actor,)).fetchone()
    if auth != (True, True):
        return "unauthorized", None
    row = connection.execute(
        "SELECT operation_row_id FROM kehila.operations WHERE actor_id=%s AND command_family='project_create' AND target_kind='project' AND target_project_id=%s AND token=%s",
        (actor, cmd["project_id"], cmd["operation_id"]),
    ).fetchone()
    if row:
        connection.execute("SELECT * FROM kehila.qa_lock_core(%s)", (row[0],)).fetchone()
        # Explicit columns rather than relying on table tuple layout.
        retired, replay_origin_ms, replay_deadline_ms, digest = connection.execute(
            "SELECT payload_retired,replay_origin_ms,replay_deadline_ms,request_sha256 FROM kehila.operations WHERE operation_row_id=%s",
            (row[0],),
        ).fetchone()
        current_ms = origin(connection) if now is None else now
        if retired:
            return (
                "replay_expired"
                if bytes(digest) == hashlib.sha256(request_bytes(cmd)).digest()
                else "operation_id_reused"
            ), None
        saved_payload = connection.execute(
            "SELECT request_bytes,result FROM kehila.operation_payloads WHERE operation_row_id=%s",
            (row[0],),
        ).fetchone()
        require(saved_payload is not None)
        if bytes(saved_payload[0]) != request_bytes(cmd):
            return "operation_id_reused", None
        if bytes(digest) != hashlib.sha256(bytes(saved_payload[0])).digest():
            return "invalid_operation", None
        if current_ms < replay_origin_ms:
            return "invalid_operation", None
        if current_ms >= replay_deadline_ms:
            return "replay_expired", None
        return "replay", saved_payload[1]
    if connection.execute(
        "SELECT 1 FROM kehila.projects WHERE project_id=%s", (cmd["project_id"],)
    ).fetchone():
        return "project_already_exists", None
    persist(connection, cmd, actor)
    validate(connection, cmd, actor)
    return "created", seed(cmd)


def create(cmd, actor="qa_actor_a", now=None):
    with conn("qa_serving") as c:
        with c.transaction():
            return decide(c, cmd, actor, now)


if __name__ == "__main__":
    initialize()
    install_roles()
    print("Roles installed")
