"""Contained synthetic loopback target; lifetime belongs to container PID 1."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import os
from pathlib import Path
import re
import subprocess
import threading

nonce = os.environ['PROBE_NONCE']
if not re.fullmatch(r'[0-9a-f]{64}', nonce):
    raise ValueError('invalid synthetic challenge')
Path('/run/sshd').mkdir(mode=0o755, exist_ok=True)


class Target(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path != '/' + nonce:
            self.send_error(404)
            return
        body = nonce.encode('ascii')
        self.send_response(200)
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)
        print('synthetic_response_completed', flush=True)

    def log_message(self, *_args):
        pass


target = ThreadingHTTPServer(('127.0.0.1', 31337), Target)
target.daemon_threads = True
threading.Thread(target=target.serve_forever, daemon=True).start()
raise SystemExit(subprocess.call(['/usr/sbin/sshd', '-D', '-e', '-f', '/etc/ssh/probe_config']))
