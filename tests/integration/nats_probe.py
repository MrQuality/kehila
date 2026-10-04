#!/usr/bin/env python3
"""Check the NATS handshake and JetStream account API against a running server."""
import json
import os
import socket
import sys


def probe():
    host = os.environ.get("KEHILA_NATS_HOST", "127.0.0.1")
    port = int(os.environ.get("KEHILA_NATS_PORT", "4222"))
    with socket.create_connection((host, port), timeout=3) as conn:
        conn.settimeout(3)
        with conn.makefile("rb") as reader:
            def line():
                value = reader.readline(65537)
                if len(value) > 65536 or not value.endswith(b"\r\n"):
                    raise ValueError("Invalid or oversized NATS frame")
                return value

            greeting = line()
            if not greeting.startswith(b"INFO "):
                raise ValueError("NATS INFO greeting missing")
            info = json.loads(greeting[5:])
            if info.get("jetstream") is not True:
                raise ValueError("JetStream is not enabled")
            conn.sendall(b'CONNECT {"verbose":false,"pedantic":true,"lang":"python","version":"0.1.0"}\r\nPING\r\n')
            for _ in range(32):
                frame = line()
                if frame == b"PONG\r\n":
                    break
                if frame == b"PING\r\n":
                    conn.sendall(b"PONG\r\n")
                elif not (frame.startswith(b"INFO ") or frame == b"+OK\r\n"):
                    raise ValueError(f"Unexpected handshake: {frame!r}")
            else:
                raise ValueError("No PONG received")
            inbox = ("_INBOX.kehila." + os.urandom(12).hex()).encode("ascii")
            conn.sendall(b"SUB " + inbox + b" 1\r\nUNSUB 1 1\r\nPUB $JS.API.INFO " + inbox + b" 0\r\n\r\n")
            for _ in range(32):
                frame = line()
                if frame.startswith(b"MSG "):
                    parts = frame.split()
                    if len(parts) != 4 or parts[1] != inbox or parts[2] != b"1":
                        raise ValueError("Unexpected JetStream response subject")
                    size = int(parts[-1])
                    if not 0 < size <= 65536:
                        raise ValueError("Invalid JetStream payload length")
                    payload = reader.read(size)
                    if len(payload) != size or reader.read(2) != b"\r\n":
                        raise ValueError("Truncated JetStream payload")
                    account = json.loads(payload)
                    if account.get("error") or account.get("type") != "io.nats.jetstream.api.v1.account_info_response":
                        raise ValueError(f"JetStream account check failed: {account}")
                    return {"ok": True, "server_id": info["server_id"],
                            "version": info["version"], "jetstream": True,
                            "response_type": account["type"]}
                if frame == b"PING\r\n":
                    conn.sendall(b"PONG\r\n")
                elif not (frame.startswith(b"INFO ") or frame == b"+OK\r\n"):
                    raise ValueError(f"Unexpected NATS response: {frame!r}")
            raise ValueError("No JetStream account response received")


if __name__ == "__main__":
    try:
        print(json.dumps(probe()))
    except (OSError, ValueError, KeyError) as error:
        print(f"NATS_CHECK_FAILED: {error}", file=sys.stderr)
        sys.exit(2)
