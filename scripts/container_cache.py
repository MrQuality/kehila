"""An explicitly prepared, image/manifest-scoped Podman dependency volume.

Only downloaded dependencies persist. Compilation outputs and test results do not.
The cache is an optimization; a marker never substitutes for executing checks.
"""
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path

LABEL = 'org.kehila.dependency-cache'


@dataclass(frozen=True)
class DependencyCache:
    key: str

    @property
    def name(self):
        return 'kehila-dependencies-v1-' + self.key

    @classmethod
    def for_source(cls, root, image):
        root = Path(root)
        paths = {root / 'Cargo.lock', root / 'Cargo.toml', root / 'go.work'}
        paths.update((root / 'src').rglob('Cargo.toml'))
        for name in ('go.mod', 'go.sum'):
            paths.update((root / 'go').rglob(name))
        if (root / 'go.work.sum').exists():
            paths.add(root / 'go.work.sum')
        entries = {path.relative_to(root).as_posix():
                   hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(paths)}
        identity = json.dumps({'version': 1, 'image': image, 'inputs': entries}, sort_keys=True)
        return cls(hashlib.sha256(identity.encode()).hexdigest())

    def admit(self, engine, *, prepare):
        found = engine('volume', 'ls', '--filter', 'name=^' + self.name + '$',
                       '--format', '{{.Name}}', capture=True).stdout.split()
        if not found:
            if not prepare:
                raise ValueError('Dependencies are not prepared; run --prepare-dependencies first')
            # Do not silently adopt a concurrently created or differently owned volume.
            engine('volume', 'create', '--driver=local', '--uid=10001', '--gid=10001',
                   '--label', LABEL + '=' + self.key, self.name)
        elif found != [self.name]:
            raise ValueError('Unexpected dependency-cache volume inventory')
        rows = json.loads(engine('volume', 'inspect', self.name, capture=True).stdout)
        if len(rows) != 1:
            raise ValueError('Expected one dependency-cache volume')
        state = rows[0]
        if (state.get('Name') != self.name or state.get('Driver') != 'local'
                or state.get('Labels', {}).get(LABEL) != self.key
                or state.get('Options') not in (None, {})):
            raise ValueError('Refusing an unowned or redirected dependency cache')
        return state['Mountpoint']
