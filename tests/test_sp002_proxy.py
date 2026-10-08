"""Real loopback socket lifecycle checks; PostgreSQL semantics are tested separately."""

import importlib.util
from pathlib import Path
import socket
import threading
import unittest

SOURCE = Path(__file__).resolve().parents[1] / "experiments/SP-002"
SPEC = importlib.util.spec_from_file_location("commit_loss_proxy", SOURCE / "commit_loss_proxy.py")
PROXY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROXY)


class CommitLossProxyTests(unittest.TestCase):
    def assert_stopped(self, proxy):
        self.assertFalse(proxy.thread.is_alive())
        if proxy.forward_thread is not None:
            self.assertFalse(proxy.forward_thread.is_alive())

    def test_no_client_is_cancelled(self):
        with PROXY.CommitLossProxy("127.0.0.1", 1) as proxy:
            pass
        self.assert_stopped(proxy)

    def test_client_disconnect_during_startup_is_joined(self):
        with socket.socket() as backend:
            backend.bind(("127.0.0.1", 0))
            backend.listen(1)
            with self.assertRaises(RuntimeError):
                with PROXY.CommitLossProxy(*backend.getsockname()) as proxy:
                    client = socket.create_connection(("127.0.0.1", proxy.port), timeout=2)
                    with client, backend.accept()[0]:
                        client.shutdown(socket.SHUT_RDWR)
                        proxy.finish()
            self.assert_stopped(proxy)

    def test_backend_connection_failure_is_joined(self):
        with socket.socket() as reserved:
            reserved.bind(("127.0.0.1", 0))
            with self.assertRaises(RuntimeError):
                with PROXY.CommitLossProxy(*reserved.getsockname()) as proxy:
                    with socket.create_connection(("127.0.0.1", proxy.port), timeout=2):
                        proxy.finish()
            self.assert_stopped(proxy)

    def test_primary_error_survives_cleanup(self):
        failure = ValueError("original failure")
        with self.assertRaises(ValueError) as raised:
            with PROXY.CommitLossProxy("127.0.0.1", 1) as proxy:
                raise failure
        self.assertIs(raised.exception, failure)
        self.assert_stopped(proxy)

    def test_missing_commit_observation_cancels_both_threads(self):
        with socket.socket() as backend:
            backend.bind(("127.0.0.1", 0))
            backend.listen(1)
            with self.assertRaisesRegex(ValueError, "commit not observed"):
                with PROXY.CommitLossProxy(*backend.getsockname()) as proxy:
                    with socket.create_connection(("127.0.0.1", proxy.port), timeout=2) as client:
                        with backend.accept()[0] as server:
                            client.sendall(b"\x00\x00\x00\x08\x00\x03\x00\x00")
                            server.settimeout(2)
                            PROXY.recv_exact(server, 8)
                            raise ValueError("commit not observed")
            self.assert_stopped(proxy)


if __name__ == "__main__":
    unittest.main()
