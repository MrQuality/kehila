"""Own the bounded socket/thread lifecycle for the response-loss experiment."""

import socket
import struct
import sys
import threading


def recv_exact(connection, size):
    data = b""
    while len(data) < size:
        chunk = connection.recv(size - len(data))
        if not chunk:
            raise EOFError("Peer disconnected before the complete frame")
        data += chunk
    return data


class CommitLossProxy:
    """Observe server COMMIT+idle, suppress its responses, then close the client."""

    IO_TIMEOUT = 5
    JOIN_TIMEOUT = 8

    def __init__(self, host, port):
        self.backend = (host, port)
        self.commit_sent = threading.Event()
        self.committed = threading.Event()
        self.stopping = threading.Event()
        self.lock = threading.Lock()
        self.connections = []
        self.log = []
        self.errors = []
        self.forward_thread = None
        self.listener = socket.socket()
        try:
            self.listener.bind(("127.0.0.1", 0))
            self.listener.listen(1)
            self.listener.settimeout(0.2)
            self.port = self.listener.getsockname()[1]
            self.thread = threading.Thread(target=self.run, name="commit-loss-proxy")
            self.thread.start()
        except BaseException:
            self.listener.close()
            raise

    def __enter__(self):
        return self

    def __exit__(self, error_type, error, traceback):
        self.close()
        if self.errors:
            diagnostic = "Commit-loss proxy: " + "; ".join(self.errors)
            if error is None:
                raise RuntimeError(diagnostic)
            # Python 3.10 has no exception notes; retain the primary exception.
            print(diagnostic, file=sys.stderr)

    @staticmethod
    def shutdown(connection):
        try:
            connection.shutdown(socket.SHUT_RDWR)
        except OSError:
            pass
        connection.close()

    def own(self, connection):
        with self.lock:
            if self.stopping.is_set():
                self.shutdown(connection)
                raise OSError("Proxy cancelled")
            self.connections.append(connection)
        connection.settimeout(self.IO_TIMEOUT)
        return connection

    def forward(self, client, server):
        try:
            while not self.stopping.is_set():
                kind = recv_exact(client, 1)
                header = recv_exact(client, 4)
                size = struct.unpack("!I", header)[0]
                data = recv_exact(client, size - 4)
                if kind == b"Q" and data.rstrip(b"\0").strip().upper() == b"COMMIT":
                    self.commit_sent.set()
                    self.log.append("forwarding client COMMIT")
                server.sendall(kind + header + data)
        except (EOFError, OSError) as error:
            if not self.stopping.is_set() and not self.committed.is_set():
                self.errors.append("Client forwarding failed: " + str(error))
                self.cancel()

    def run(self):
        try:
            while not self.stopping.is_set():
                try:
                    client, _ = self.listener.accept()
                    break
                except socket.timeout:
                    continue
            else:
                return
            client = self.own(client)
            server = self.own(socket.create_connection(self.backend, timeout=self.IO_TIMEOUT))
            header = recv_exact(client, 4)
            size = struct.unpack("!I", header)[0]
            server.sendall(header + recv_exact(client, size - 4))
            self.forward_thread = threading.Thread(
                target=self.forward, args=(client, server), name="commit-loss-forward"
            )
            self.forward_thread.start()
            seen_commit = False
            while not self.stopping.is_set():
                kind = recv_exact(server, 1)
                header = recv_exact(server, 4)
                size = struct.unpack("!I", header)[0]
                data = recv_exact(server, size - 4)
                if self.commit_sent.is_set():
                    if kind == b"C" and data == b"COMMIT\0":
                        seen_commit = True
                        self.log.append("observed server CommandComplete COMMIT; withheld")
                    if kind == b"Z" and data == b"I" and seen_commit:
                        self.log.append("observed server ReadyForQuery idle; withheld")
                        self.committed.set()
                        break
                else:
                    client.sendall(kind + header + data)
        except Exception as error:
            if not self.stopping.is_set():
                self.errors.append("Server forwarding failed: " + str(error))
        finally:
            self.cancel()
            if self.forward_thread is not None:
                self.forward_thread.join(self.JOIN_TIMEOUT)

    def cancel(self):
        with self.lock:
            self.stopping.set()
            self.listener.close()
            for connection in self.connections:
                self.shutdown(connection)
            self.connections.clear()

    def close(self):
        self.cancel()
        self.thread.join(self.JOIN_TIMEOUT)
        if self.thread.is_alive():
            self.errors.append("Proxy thread did not stop within its deadline")
        if self.forward_thread is not None:
            self.forward_thread.join(self.JOIN_TIMEOUT)
            if self.forward_thread.is_alive():
                self.errors.append("Forwarding thread did not stop within its deadline")

    def finish(self):
        """Wait for protocol completion; the context manager owns cancellation."""
        self.thread.join(self.JOIN_TIMEOUT)
        if self.thread.is_alive():
            raise RuntimeError("Proxy did not finish within its deadline")
        if self.errors:
            raise RuntimeError("; ".join(self.errors))
