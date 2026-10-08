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


def core(c, cmd, actor="qa_actor_a", o=None, **changes):
    o = origin(c) if o is None else o
    v = dict(
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
        replay_origin_ms=o,
        replay_deadline_ms=o + PERIOD,
        payload_retired=False,
    )
    v.update(changes)
    sql = (
        "INSERT INTO kehila.operations ("
        + ",".join(v)
        + ") VALUES ("
        + ",".join(["%s"] * len(v))
        + ") RETURNING operation_row_id"
    )
    return c.execute(sql, list(v.values())).fetchone()[0]


def payload(c, op, cmd, result=None):
    c.execute(
        "INSERT INTO kehila.operation_payloads VALUES (%s,%s,1,%s)",
        (op, request_bytes(cmd), Jsonb(result or {})),
    )


def statements(c, cmd, actor="qa_actor_a", o=None):
    """Yield after each of the 26 physical row writes, allowing fault injection."""
    r = seed(cmd)
    p = r["project"]
    cfg = r["configuration"]
    pid = cmd["project_id"]
    op = core(c, cmd, actor, o)
    yield "operations", op
    payload(c, op, cmd, r)
    yield "operation_payloads", op
    c.execute(
        "INSERT INTO kehila.projects(project_id,name,current_prefix,estimate_unit,configuration_revision,next_sequence,ever_estimated,archived,created_by,creation_operation_row_id,seed_profile) VALUES (%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s)",
        (
            pid,
            p["name"],
            p["prefix"],
            p["estimate_unit"],
            p["configuration_revision"],
            p["next_sequence"],
            p["ever_estimated"],
            p["archived"],
            actor,
            op,
            r["seed_profile"],
        ),
    )
    yield "projects", op
    c.execute(
        "INSERT INTO kehila.configuration_revisions VALUES (%s,1,1,%s,%s,%s,%s,'project_create','project')",
        (pid, Jsonb(cfg), Jsonb(p), actor, op),
    )
    yield "configuration_revisions", op
    c.execute(
        "INSERT INTO kehila.project_grants VALUES (%s,%s,'initial_owner',true,1)",
        (pid, actor),
    )
    yield "project_grants", op
    c.execute(
        "INSERT INTO kehila.project_grant_events(project_id,actor_id,version,access_profile,active,granted_by,granting_operation_row_id,command_family) VALUES (%s,%s,1,'initial_owner',true,%s,%s,'project_create')",
        (pid, actor, actor, op),
    )
    yield "project_grant_events", op
    for i, s in enumerate(cfg["statuses"]):
        c.execute(
            "INSERT INTO kehila.statuses VALUES (%s,%s,%s,%s,%s,%s)",
            (pid, s["id"], s["name"], s["phase"], s["archived"], i),
        )
        yield "statuses", op
    for i, w in enumerate(cfg["workflows"]):
        c.execute(
            "INSERT INTO kehila.workflows VALUES (%s,%s,%s,'new',%s,%s)",
            (pid, w["id"], w["initial_status_id"], w["archived"], i),
        )
        yield "workflows", op
        for j, s in enumerate(w["status_ids"]):
            c.execute(
                "INSERT INTO kehila.workflow_statuses VALUES (%s,%s,%s,%s)",
                (pid, w["id"], s, j),
            )
            yield "workflow_statuses", op
        for j, e in enumerate(w["permitted_phase_changes"]):
            c.execute(
                "INSERT INTO kehila.workflow_phase_changes VALUES (%s,%s,%s,%s,%s)",
                (pid, w["id"], *e, j),
            )
            yield "workflow_phase_changes", op
    for i, t in enumerate(cfg["types"]):
        c.execute(
            "INSERT INTO kehila.work_item_types VALUES (%s,%s,%s,%s,%s,%s)",
            (
                pid,
                t["id"],
                t["default_workflow_id"],
                t["title_field_id"],
                t["archived"],
                i,
            ),
        )
        yield "work_item_types", op
        for j, w in enumerate(t["permitted_workflows"]):
            c.execute(
                "INSERT INTO kehila.type_workflows VALUES (%s,%s,%s,%s)",
                (pid, t["id"], w, j),
            )
            yield "type_workflows", op
    for i, f in enumerate(cfg["fields"]):
        c.execute(
            "INSERT INTO kehila.fields VALUES (%s,%s,%s,%s,%s,%s,%s,%s,%s)",
            (
                pid,
                f["id"],
                f["owner_type"],
                f["name"],
                f["kind"],
                f["origin"],
                f["usage"],
                f["archived"],
                i,
            ),
        )
        yield "fields", op
    return op


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


def decide(c, cmd, actor="qa_actor_a", now=None):
    auth = c.execute("SELECT * FROM kehila.qa_lock_actor(%s)", (actor,)).fetchone()
    if auth != (True, True):
        return "unauthorized", None
    row = c.execute(
        "SELECT operation_row_id FROM kehila.operations WHERE actor_id=%s AND command_family='project_create' AND target_kind='project' AND target_project_id=%s AND token=%s",
        (actor, cmd["project_id"], cmd["operation_id"]),
    ).fetchone()
    if row:
        o = c.execute("SELECT * FROM kehila.qa_lock_core(%s)", (row[0],)).fetchone()
        # Explicit columns rather than relying on table tuple layout.
        retired, at, end, digest = c.execute(
            "SELECT payload_retired,replay_origin_ms,replay_deadline_ms,request_sha256 FROM kehila.operations WHERE operation_row_id=%s",
            (row[0],),
        ).fetchone()
        n = origin(c) if now is None else now
        if retired:
            return (
                "replay_expired"
                if bytes(digest) == hashlib.sha256(request_bytes(cmd)).digest()
                else "operation_id_reused"
            ), None
        p = c.execute(
            "SELECT request_bytes,result FROM kehila.operation_payloads WHERE operation_row_id=%s",
            (row[0],),
        ).fetchone()
        require(p is not None)
        if bytes(p[0]) != request_bytes(cmd):
            return "operation_id_reused", None
        if bytes(digest) != hashlib.sha256(bytes(p[0])).digest():
            return "invalid_operation", None
        if n < at:
            return "invalid_operation", None
        if n >= end:
            return "replay_expired", None
        return "replay", p[1]
    if c.execute(
        "SELECT 1 FROM kehila.projects WHERE project_id=%s", (cmd["project_id"],)
    ).fetchone():
        return "project_already_exists", None
    persist(c, cmd, actor)
    validate(c, cmd, actor)
    return "created", seed(cmd)


def create(cmd, actor="qa_actor_a", now=None):
    with conn("qa_serving") as c:
        with c.transaction():
            return decide(c, cmd, actor, now)


if __name__ == "__main__":
    initialize()
    install_roles()
    print("Roles installed")
