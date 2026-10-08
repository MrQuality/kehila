from common import RESULTS, ROOT, command, conn, core, initialize, payload, rec, require
import json
import secrets


def main():
    initialize()

    c = conn(db="qa_mapping")

    actor = secrets.token_hex(64)

    project = secrets.token_hex(64)

    relationship = secrets.token_hex(64)

    token = secrets.token_hex(64)

    cmd = command(project, token)

    c.execute("INSERT INTO kehila.actors VALUES (%s,true,true)", (actor,))

    with c.transaction():
        op = core(
            c,
            cmd,
            actor,
            target_kind="relationship",
            command_family="relationship_create",
            target_relationship_id=relationship,
        )
        payload(c, op, cmd)

    size = c.execute(
        "SELECT pg_column_size(ROW(actor_id,command_family,target_kind,target_project_id,target_item_id,target_user_id,target_relationship_id,token)) FROM kehila.operations WHERE operation_row_id=%s",
        (op,),
    ).fetchone()[0]

    require(len({actor, project, relationship, token}) == 4)

    require(
        all((len(x.encode()) == 128 for x in [actor, project, relationship, token]))
    )

    rec(
        "A07.maximum-random-key",
        classification="modified-schema compatibility experiment",
        detail=dict(
            strings=4,
            bytes_each=128,
            nonrepeating_random_hex=True,
            tuple_bytes=size,
            inserted=True,
        ),
    )

    (ROOT / "max-random-key-results.json").write_text(json.dumps(RESULTS, indent=2))

    c.close()


if __name__ == "__main__":
    main()
