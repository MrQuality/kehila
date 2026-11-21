"""Qualify declared Rust minima with explicitly provisioned toolchains.

No toolchain installation, dependency update or support-declaration change is
performed. Run inside the verification container with the required compilers
already available. Missing tools and incompatible locked dependencies fail.
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys

from cleanup_errors import run_main

ROOT = Path(__file__).resolve().parents[1]


def normalized(version):
    if not isinstance(version, str) or re.fullmatch(r'\d+\.\d+(?:\.\d+)?', version) is None:
        raise ValueError('Expected an explicit numeric Rust version')
    return version if version.count('.') == 2 else version + '.0'


def minimum_groups(metadata):
    members = set(metadata['workspace_members'])
    groups = {}
    for package in metadata['packages']:
        if package['id'] not in members:
            continue
        version = normalized(package.get('rust_version'))
        groups.setdefault(version, []).append(package['name'])
    if not groups:
        raise ValueError('No declared workspace compiler minima')
    return {version: sorted(packages) for version, packages in sorted(groups.items())}


def compiler_environment(environment, rustc):
    overrides = ('RUSTC', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER',
                 'CARGO_BUILD_RUSTC', 'CARGO_BUILD_RUSTC_WRAPPER',
                 'CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER')
    if any(environment.get(key) for key in overrides):
        raise ValueError('Compiler or wrapper override would invalidate minimum qualification')
    return dict(environment, CARGO_NET_OFFLINE='true', RUSTC=str(rustc))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--list', action='store_true', help='List declared groups without compiling')
    parser.add_argument('--version', help='Explicit installed compiler version to qualify')
    parser.add_argument('--toolchains-home', type=Path,
                        help='Explicit separate installed-toolchain home; metadata uses the original environment')
    args = parser.parse_args()
    if args.list == bool(args.version):
        parser.error('Select exactly one of --list or --version')
    metadata = subprocess.run(['cargo', 'metadata', '--locked', '--no-deps', '--format-version', '1'],
                              cwd=ROOT, capture_output=True, text=True, check=True, timeout=60)
    groups = minimum_groups(json.loads(metadata.stdout))
    if args.list:
        print(json.dumps(groups, sort_keys=True))
        return
    version = normalized(args.version)
    if version not in groups:
        raise ValueError('Compiler does not match a declared workspace minimum')
    environment = dict(os.environ)
    if args.toolchains_home is not None:
        if not args.toolchains_home.is_absolute():
            raise ValueError('Toolchain home must be absolute')
        home = args.toolchains_home.resolve()
        if home == ROOT or ROOT in home.parents:
            raise ValueError('Toolchains must remain outside source')
        environment['RUSTUP_HOME'] = str(home)
    # `rustup run` refuses an absent toolchain instead of provisioning it.
    compiler = subprocess.run(['rustup', 'run', version, 'rustc', '--version'],
                             env=environment, capture_output=True, text=True, check=True, timeout=30)
    if compiler.stdout.split()[:2] != ['rustc', version]:
        raise ValueError('Installed compiler identity differs from requested minimum')
    sysroot = subprocess.run(['rustup', 'run', version, 'rustc', '--print', 'sysroot'],
                            env=environment, capture_output=True, text=True, check=True, timeout=30)
    rustc = Path(sysroot.stdout.strip()) / 'bin' / ('rustc.exe' if os.name == 'nt' else 'rustc')
    if not rustc.is_absolute() or not rustc.is_file():
        raise ValueError('Selected compiler executable unavailable')
    environment = compiler_environment(environment, rustc)
    command = ['rustup', 'run', version, 'cargo', 'check', '--locked', '--offline', '--all-targets']
    for package in groups[version]:
        command += ['-p', package]
    print('MINIMUM RUST:', version, ', '.join(groups[version]), flush=True)
    subprocess.run(command, cwd=ROOT, env=environment, check=True, timeout=600)


if __name__ == '__main__':
    sys.exit(run_main(main, 'ERR_RUST_MINIMUM'))
