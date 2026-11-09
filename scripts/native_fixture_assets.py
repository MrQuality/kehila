"""Transfer generated native-fixture evidence before resetting its fixed workspace."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import sys

WORK = Path('/work/native-authority')
SOURCE = Path(__file__).resolve().parents[1]
OWNER_FILE = '.fixture-owner.json'
MANIFEST_FILE = '.asset-manifest.json'


def _owner(value):
    if not isinstance(value, str) or re.fullmatch(r'[0-9a-f]{32}', value) is None:
        raise ValueError('Expected the native fixture session owner')


def _directory(root):
    root = Path(root)
    if (not root.is_absolute() or root.resolve(strict=True) != root.absolute()
            or not stat.S_ISDIR(root.lstat().st_mode)):
        raise ValueError('Expected an absolute regular fixture directory without links')
    return root


def _regular(path):
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
        raise ValueError('Fixture assets must be regular files with one link or directories')
    return info


def _hash(path):
    before = _regular(path)
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    after = _regular(path)
    if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (
            after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns):
        raise ValueError('Fixture asset changed during evidence hashing')
    return digest.hexdigest()


def write_owner(root, owner):
    _owner(owner)
    root = _directory(root)
    if any(root.iterdir()):
        raise ValueError('Cannot claim existing material as generated fixture assets')
    with (root / OWNER_FILE).open('x', encoding='utf-8') as stream:
        json.dump({'schema': 1, 'owner': owner}, stream, sort_keys=True)


def _inventory(root, owner):
    _owner(owner)
    root = _directory(root)
    _regular(root / OWNER_FILE)
    ownership = json.loads((root / OWNER_FILE).read_text(encoding='utf-8'))
    if (not isinstance(ownership, dict) or type(ownership.get('schema')) is not int
            or ownership != {'schema': 1, 'owner': owner}):
        raise ValueError('Generated fixture ownership does not match this session')
    device = root.lstat().st_dev
    result = []
    for path in sorted(root.rglob('*'), key=lambda entry: entry.relative_to(root).as_posix()):
        info = path.lstat()
        if info.st_dev != device:
            raise ValueError('Fixture assets cross a filesystem boundary')
        relative = path.relative_to(root).as_posix()
        if stat.S_ISDIR(info.st_mode):
            result.append({'path': relative, 'kind': 'directory'})
        else:
            _regular(path)
            if relative != MANIFEST_FILE:
                result.append({'path': relative, 'kind': 'file',
                               'bytes': info.st_size, 'sha256': _hash(path)})
    return result


def write_manifest(root, owner):
    root = Path(root)
    document = {'schema': 1, 'owner': owner, 'entries': _inventory(root, owner)}
    path = root / MANIFEST_FILE
    with path.open('x', encoding='utf-8') as stream:
        json.dump(document, stream, sort_keys=True, separators=(',', ':'))
    return _hash(path)


def verify_export(root, owner, digest):
    root = _directory(root)
    if not isinstance(digest, str) or re.fullmatch(r'[0-9a-f]{64}', digest) is None:
        raise ValueError('Expected an exact fixture evidence manifest digest')
    path = root / MANIFEST_FILE
    if _hash(path) != digest:
        raise ValueError('Fixture evidence manifest digest differs')
    document = json.loads(path.read_text(encoding='utf-8'))
    if (not isinstance(document, dict) or type(document.get('schema')) is not int or document != {
            'schema': 1, 'owner': owner, 'entries': _inventory(root, owner)}):
        raise ValueError('Fixture evidence inventory differs from the exported manifest')


def _separate_paths(root, source, target):
    root = _directory(root)
    source = _directory(Path(source).resolve(strict=True))
    target = _directory(target)  # Reject lexical aliases into generated assets.
    if (root == source or source in root.parents or root in source.parents
            or root == target or root in target.parents or target in root.parents
            or any((parent / '.git').exists() for parent in (root, *root.parents))):
        raise ValueError('Refusing fixture cleanup near source or build output')
    return root


def reset(owner, digest):
    # This operation has no configurable path. The caller has already observed
    # owned database removal and verified the complete external evidence copy.
    if sys.platform != 'linux' or WORK != Path('/work/native-authority'):
        raise ValueError('Fixture reset requires the fixed Linux generated workspace')
    if os.environ.get('CARGO_TARGET_DIR') != '/work/target':
        raise ValueError('Fixture reset requires the known selected build-output directory')
    root = _separate_paths(WORK, SOURCE, Path('/work/target'))
    verify_export(root, owner, digest)
    for path in (root, *root.rglob('*')):
        if path.lstat().st_uid != os.geteuid():
            raise ValueError('Fixture assets have a different filesystem owner')
    shutil.rmtree(root)
