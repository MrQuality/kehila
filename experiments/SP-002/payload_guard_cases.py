"""Verify the proposed correction separately from the frozen failing schema."""

import json

from common import (
    RESULTS,
    ROOT,
    command,
    conn,
    core,
    expect_error,
    initialize,
    persist,
    rec,
    require,
    test,
    validate,
)


def main():
    initialize()
    with conn() as owner:
        owner.execute("CREATE DATABASE qa_guard")
    with conn(db="qa_guard") as owner:
        owner.execute(
            (ROOT / "sql/baseline.sql").read_text(encoding="utf-8"), prepare=False
        )
        owner.execute(
            (ROOT / "payload-identity-guard.sql").read_text(encoding="utf-8"),
            prepare=False,
        )
        owner.execute("INSERT INTO kehila.actors VALUES ('qa_actor_a',true,true)")
        owner.execute(
            "GRANT USAGE ON SCHEMA kehila TO qa_serving; "
            "GRANT SELECT,INSERT ON ALL TABLES IN SCHEMA kehila TO qa_serving; "
            "GRANT USAGE ON ALL SEQUENCES IN SCHEMA kehila TO qa_serving",
            prepare=False,
        )

    cmd = command()
    with conn("qa_serving", db="qa_guard") as serving:
        with serving.transaction():
            original = persist(serving, cmd)
            validate(serving, cmd)
    rec("G01.corrected-create", classification="corrected schema only")

    def same_identity():
        with conn(db="qa_guard") as owner:
            owner.execute(
                "UPDATE kehila.operation_payloads SET operation_row_id=operation_row_id "
                "WHERE operation_row_id=%s",
                (original,),
            )
            validate(owner, cmd)

    test("G02.same-identity-update", same_identity, "corrected schema only")

    def moved_identity():
        target = command()
        with conn(db="qa_guard") as owner:
            try:
                owner.execute("BEGIN")
                replacement = core(owner, target)
                error = expect_error(
                    lambda: owner.execute(
                        "UPDATE kehila.operation_payloads SET operation_row_id=%s WHERE operation_row_id=%s",
                        (replacement, original),
                    ),
                    "23514",
                    "operation_payload_identity_immutable",
                )
            finally:
                owner.execute("ROLLBACK")
            validate(owner, cmd)
            require(
                owner.execute(
                    "SELECT count(*) FROM kehila.operations WHERE target_project_id=%s",
                    (target["project_id"],),
                ).fetchone()
                == (0,)
            )
            return error

    test("G03.identity-move-rejected", moved_identity, "corrected schema only")

    def serving_denied():
        with conn("qa_serving", db="qa_guard") as serving:
            return expect_error(
                lambda: serving.execute(
                    "UPDATE kehila.operation_payloads SET operation_row_id=operation_row_id"
                ),
                "42501",
            )

    test("G04.serving-update-denied", serving_denied, "corrected schema only")

    def retirement():
        with conn(db="qa_guard") as owner:
            with owner.transaction():
                owner.execute(
                    "UPDATE kehila.operations SET payload_retired=true WHERE operation_row_id=%s",
                    (original,),
                )
                owner.execute(
                    "DELETE FROM kehila.operation_payloads WHERE operation_row_id=%s",
                    (original,),
                )
            require(
                owner.execute(
                    "SELECT payload_retired FROM kehila.operations WHERE operation_row_id=%s",
                    (original,),
                ).fetchone()
                == (True,)
            )
            require(
                owner.execute(
                    "SELECT count(*) FROM kehila.operation_payloads WHERE operation_row_id=%s",
                    (original,),
                ).fetchone()
                == (0,)
            )

    test("G05.retirement-preserved", retirement, "corrected schema only")

    (ROOT / "guard-results.json").write_text(
        json.dumps(RESULTS, indent=2), encoding="utf-8"
    )
    return 1 if any(row["status"] == "FAIL" for row in RESULTS) else 0


if __name__ == "__main__":
    raise SystemExit(main())
