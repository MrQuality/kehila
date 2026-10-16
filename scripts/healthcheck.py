#!/usr/bin/env python3
"""Probe all requested TCP ports; additionally verify OpenSearch HTTP readiness."""
import argparse
import json
import socket
import sys
import time
import urllib.request

PORTS = {"PostgreSQL": 5432, "FerretDB": 27017, "NATS": 4222, "OpenSearch": 9200}


def probe(host, services=None):
    selected = PORTS if services is None else {name: PORTS[name] for name in services}
    errors = []
    for name, port in selected.items():
        try:
            with socket.create_connection((host, port), timeout=2):
                pass
        except OSError as error:
            errors.append(f"{name} {host}:{port}: {error}")
    if 'OpenSearch' not in selected:
        return errors
    try:
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        with opener.open(f"http://{host}:9200/_cluster/health", timeout=3) as response:
            status = json.load(response)
        if status.get("status") not in ("yellow", "green"):
            errors.append(f"OpenSearch cluster not ready: {status.get('status')}")
    except (OSError, ValueError) as error:
        errors.append(f"OpenSearch HTTP: {error}")
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--wait", type=float, default=0, help="Readiness deadline in seconds")
    parser.add_argument("--services", nargs='+', choices=PORTS, default=list(PORTS))
    args = parser.parse_args()
    deadline = time.monotonic() + args.wait
    while True:
        errors = probe(args.host, args.services)
        if not errors:
            result = {"ok": True, "tcp_ports": {name: PORTS[name] for name in args.services}}
            if 'OpenSearch' in args.services:
                result['opensearch_http'] = 'ready'
            print(json.dumps(result))
            return 0
        if time.monotonic() >= deadline:
            print(json.dumps({"ok": False, "errors": errors}), file=sys.stderr)
            return 1
        time.sleep(2)


if __name__ == "__main__":
    sys.exit(main())
