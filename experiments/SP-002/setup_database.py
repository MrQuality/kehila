"""Database setup worker, run under an overall parent-process deadline."""

import json
from pathlib import Path
import sys
import time

import psycopg


def connect_ready(config):
    deadline = time.monotonic() + 25
    while True:
        try:
            return psycopg.connect(
                **config,
                autocommit=True,
                connect_timeout=2,
                options="-c statement_timeout=15000 -c lock_timeout=8000",
            )
        except psycopg.OperationalError:
            if time.monotonic() >= deadline:
                raise
            time.sleep(0.1)


def main():
    root = Path(sys.argv[1])
    config = json.loads((root / "connection.json").read_text(encoding="utf-8"))
    with connect_ready(config) as connection:
        if len(sys.argv) == 3 and sys.argv[2] == "compactor-grant":
            connection.execute((root / "compactor-read-grant.sql").read_text(encoding="utf-8"))
            return
        if connection.info.server_version // 10000 != 16:
            raise RuntimeError("Experiment requires PostgreSQL 16")
        settings = {
            name: connection.execute("SHOW " + name).fetchone()[0]
            for name in [
                "server_version", "server_encoding", "fsync", "synchronous_commit",
                "full_page_writes", "block_size", "default_transaction_isolation",
            ]
        }
        if settings["server_encoding"] != "UTF8":
            raise RuntimeError("Experiment requires UTF8 server encoding")
        if not all(settings[name] == "on" for name in ["fsync", "synchronous_commit", "full_page_writes"]):
            raise RuntimeError("Experiment requires enabled durability settings")
        (root / "server-settings.json").write_text(json.dumps(settings, indent=2), encoding="utf-8")
        connection.execute((root / "sql/baseline.sql").read_text(encoding="utf-8"), prepare=False)
    (root / "A01.json").write_text(json.dumps(dict(case="A01.original-schema", status="PASS")), encoding="utf-8")


if __name__ == "__main__":
    main()
