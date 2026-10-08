"""Validate artifacts required by the ordered additional-case phase."""

import json


def require_restore_artifacts(root):
    missing = [name for name in ["connection.json", "runtime.json", "database.dump", "phase-c-results.json"]
               if not (root / name).is_file()]
    if missing:
        raise RuntimeError(
            "Additional cases require earlier runner phases; missing: " + ", ".join(missing)
            + ". Run manage.py to prepare the schema and restore fixtures."
        )
    recovery = json.loads((root / "phase-c-results.json").read_text(encoding="utf-8"))
    if not any(row.get("case") == "C03.clean-cluster-restore" and row.get("status") == "PASS"
               for row in recovery):
        raise RuntimeError("Additional cases require a successful recovery restore phase; run manage.py")
