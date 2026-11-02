"""Cache readiness witness, used only inside an owned toolchain container."""
import json
import os
from pathlib import Path
import re
import sys
import uuid


def readiness(action, key, directory=Path('/work/cargo')):
    if not re.fullmatch(r'[0-9a-f]{64}', key):
        raise ValueError('Invalid dependency-cache identity')
    marker = directory / 'kehila-dependencies.json'
    expected = {'version': 1, 'key': key}
    if action == 'clear':
        marker.unlink(missing_ok=True)
    elif action == 'mark':
        temporary = directory / ('.ready-' + uuid.uuid4().hex)
        try:
            with temporary.open('x', encoding='utf-8') as output:
                json.dump(expected, output)
                output.flush()
                os.fsync(output.fileno())
            temporary.replace(marker)
        finally:
            temporary.unlink(missing_ok=True)
    elif action == 'check':
        if marker.is_symlink() or json.loads(marker.read_text(encoding='utf-8')) != expected:
            raise ValueError('Dependency cache is not prepared for this source/toolchain')
    else:
        raise ValueError('Unknown dependency-cache action')


if __name__ == '__main__':
    readiness(*sys.argv[1:])
