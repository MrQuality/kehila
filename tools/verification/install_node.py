#!/usr/bin/env python3
"""Install pinned Linux page-test tooling into a new explicit external prefix.

Used by the toolchain image and disposable Linux CI. No npm packages, browser,
system trust changes or archive-path extraction are involved.
"""
import argparse
import hashlib
import platform
import shutil
import tarfile
import tempfile
import urllib.request
from pathlib import Path

VERSION = '24.22.2'
SHA256 = '48cc0e21e048790e54c0cf5749c2725005313d652651c40b7a21feb9697aaf75'
ARCHIVE = 'node-v' + VERSION + '-linux-x64'
URL = 'https://nodejs.org/dist/v' + VERSION + '/' + ARCHIVE + '.tar.xz'
DOWNLOAD_LIMIT = 50 * 1024**2


def install(prefix):
    if platform.system() != 'Linux' or platform.machine() not in ('x86_64', 'amd64'):
        raise ValueError('Pinned page-test toolchain requires Linux amd64')
    if not prefix.is_absolute() or prefix == prefix.parent or prefix.exists() or prefix.is_symlink():
        raise ValueError('Expected a new absolute external installation prefix')
    with tempfile.TemporaryDirectory(prefix='kehila-node-') as folder:
        archive = Path(folder) / 'node.tar.xz'
        digest, length = hashlib.sha256(), 0
        with urllib.request.urlopen(URL, timeout=60) as source, archive.open('xb') as target:
            while chunk := source.read(1024 * 1024):
                length += len(chunk)
                if length > DOWNLOAD_LIMIT:
                    raise ValueError('Node download exceeds reviewed bound')
                digest.update(chunk)
                target.write(chunk)
        if digest.hexdigest() != SHA256:
            raise ValueError('Node artifact checksum mismatch')
        prefix.mkdir()
        try:
            with tarfile.open(archive, 'r:xz') as bundle:
                for relative, maximum, mode in [('bin/node', 128 * 1024**2, 0o755),
                                                 ('LICENSE', 4 * 1024**2, 0o644)]:
                    member = bundle.getmember(ARCHIVE + '/' + relative)
                    if not member.isfile() or not 0 < member.size <= maximum:
                        raise ValueError('Unexpected Node artifact member')
                    destination = prefix / relative
                    destination.parent.mkdir(exist_ok=True)
                    with bundle.extractfile(member) as source, destination.open('xb') as target:
                        shutil.copyfileobj(source, target)
                    destination.chmod(mode)
        except BaseException:
            # This invocation exclusively created this exact prefix. Never
            # remove a pre-existing installation or follow archive paths.
            shutil.rmtree(prefix)
            raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prefix', required=True, type=Path)
    install(parser.parse_args().prefix)


if __name__ == '__main__':
    main()
