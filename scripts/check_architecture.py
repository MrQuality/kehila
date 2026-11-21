"""Bounded pure dependency/API guard; semantic purity still requires review.

Use Cargo and Go's own metadata instead of parsing their manifest languages.
Clippy resolves Rust calls and type aliases, so import renaming cannot hide a
configured external-observation API. New dependency names require review here.
"""
import json
import os
from pathlib import Path
import subprocess
import sys

from cleanup_errors import run_main
from go_inventory import GO_MODULES
from temp_work import owned_directory

ROOT = Path(__file__).resolve().parents[1]
RUST_DEPENDENCIES = {'task_contract', 'project_create_codec', 'serde', 'serde_json', 'sha2'}
GO_IMPORTS = {'bytes', 'cmp', 'errors', 'math', 'math/bits', 'slices',
              'sort', 'strconv', 'strings', 'unicode', 'unicode/utf8'}
LINTS = ('clippy::disallowed_methods', 'clippy::disallowed_types', 'clippy::disallowed_macros')


def lint_arguments():
    return [argument for lint in LINTS for argument in ('--forbid', lint)]


def hash_collection_controls():
    """Exercise constructor paths and the intentionally stricter type policy."""
    for collection, types, values in (
            ('HashSet', 'u8', '[1u8]'), ('HashMap', 'u8,u8', '[(1u8,2u8)]')):
        path = 'std::collections::' + collection
        programs = {
            'new': f'pub fn value() -> usize {{ let values: {path}<{types}> = {path}::new(); values.len() }}',
            'from': f'pub fn value() -> usize {{ let values = {path}::from({values}); values.len() }}',
            'default': f'pub fn value() -> usize {{ let values: {path}<{types}> = Default::default(); values.len() }}',
            'collect': f'pub fn value() -> usize {{ {values}.into_iter().collect::<{path}<{types}>>().len() }}',
            'explicit-deterministic': f'pub fn value() -> usize {{ type Values = {path}<{types}, '
                'std::hash::BuildHasherDefault<std::collections::hash_map::DefaultHasher>>; '
                'let values = Values::default(); values.len() }',
        }
        for name, program in programs.items():
            yield collection + '-' + name, program, LINTS[0] if name == 'new' else LINTS[1]
    for collection, types, values in (
            ('BTreeSet', 'u8', '[1u8]'), ('BTreeMap', 'u8,u8', '[(1u8,2u8)]')):
        path = 'std::collections::' + collection
        yield collection, (f'pub fn value() -> usize {{ let a: {path}<{types}> = Default::default(); '
            f'let b = {path}::from({values}); let c = {values}.into_iter().collect::<{path}<{types}>>(); '
            'a.len()+b.len()+c.len() }'), None


def compiler_controls(environment):
    """Prove alias rejection and deterministic allowances with the actual compiler."""
    cases = (
        ('memory', 'pub fn value() -> Vec<u8> { use std::io::Write; let mut v = Vec::new(); '
         'v.write_all(b"ok").unwrap(); v }', None),
        ('environment-alias', 'use std::env as observation; pub fn value() { '
         'let _ = observation::var("PURITY_CONTROL"); }', LINTS[0]),
        ('filesystem-function-alias', 'use std::fs::read as observation; '
         'pub fn value() { let _ = observation("unused"); }', LINTS[0]),
        ('path-observation-alias', 'use std::path::Path as Location; '
         'pub fn value() { let _ = Location::new("unused").exists(); }', LINTS[0]),
        ('clock-alias', 'use std::time::SystemTime as Clock; '
         'pub fn value() { let _ = Clock::now(); }', LINTS[0]),
        ('process-type-alias', 'use std::process::Command as Work; '
         'pub fn value(_: Option<Work>) {}', LINTS[1]),
        ('process-constructor-alias', 'use std::process::Command as Work; '
         'pub fn value() { let _ = Work::new("never executed"); }', LINTS[0]),
        ('output-macro', 'pub fn value() { println!("unreachable: only compiled"); }', LINTS[2]),
        ('directory-builder', 'pub fn value() { let _ = std::fs::DirBuilder::new(); }', LINTS[0]),
        ('unix-socket', '#[cfg(unix)] pub fn value() { '
         'let _ = std::os::unix::net::UnixStream::pair(); }', LINTS[0]) if os.name == 'posix'
         else ('directory-create', 'pub fn value() { let _ = std::fs::create_dir("unused"); }', LINTS[0]),
        *hash_collection_controls(),
        ('memory-again', 'pub fn value() -> String { format!("{}", 42) }', None),
    )
    with owned_directory('kehila-purity-controls-') as directory:
        manifest = directory / 'Cargo.toml'
        manifest.write_text('[package]\nname="purity_control"\nversion="0.0.0"\n'
                            'edition="2021"\n[workspace]\n', encoding='utf-8')
        source = directory / 'src'
        source.mkdir()
        isolated = dict(environment, CARGO_TARGET_DIR=str(directory / 'target'))
        for label, program, expected in cases:
            (source / 'lib.rs').write_text(program, encoding='utf-8')
            result = subprocess.run(['cargo', 'clippy', '--offline', '--manifest-path', str(manifest),
                '--lib', '--message-format=json', '--', *lint_arguments()], cwd=directory,
                env=isolated, capture_output=True, text=True, timeout=60)
            codes = {entry['message'].get('code', {}).get('code')
                     for entry in documents(result.stdout)
                     if entry.get('reason') == 'compiler-message' and entry['message'].get('code')}
            if expected is None:
                if result.returncode != 0:
                    raise ValueError(label + ': deterministic API control failed\n' + result.stderr)
            elif result.returncode != 101 or expected not in codes:
                raise ValueError(label + ': expected compiler purity rejection absent\n' +
                                 result.stdout + result.stderr)


def rust_packages(metadata, root):
    pure = (root / 'src/pure').resolve()
    expected = {path.resolve() for path in pure.rglob('Cargo.toml')}
    packages = [package for package in metadata['packages']
                if Path(package['manifest_path']).resolve() in expected]
    if not packages or {Path(p['manifest_path']).resolve() for p in packages} != expected:
        raise ValueError('Pure crate inventory differs from Cargo metadata')
    members = set(metadata['workspace_members'])
    for package in packages:
        if package['id'] not in members:
            raise ValueError('Pure crate is outside the verified workspace')
        for target in package.get('targets', []):
            if {'custom-build', 'bin', 'proc-macro'} & set(target['kind']):
                raise ValueError(package['name'] + ': unsupported pure target ' + target['kind'][0])
        for dependency in package['dependencies']:
            name = dependency['name']  # Cargo supplies the actual name, not its rename.
            if name not in RUST_DEPENDENCIES:
                raise ValueError(package['name'] + ': unreviewed pure dependency ' + name)
            path = dependency.get('path')
            if path is not None and not Path(path).resolve().is_relative_to(pure):
                raise ValueError(package['name'] + ': dependency leaves the pure layer')
    return sorted(package['name'] for package in packages)


def documents(text):
    decoder = json.JSONDecoder()
    while text.strip():
        value, end = decoder.raw_decode(text.lstrip())
        yield value
        text = text.lstrip()[end:]


def check_go(packages, root):
    pure = (root / 'go/pure').resolve()
    seen = set()
    for package in packages:
        directory = Path(package['Dir']).resolve()
        if not directory.is_relative_to(pure):
            raise ValueError('Go pure inventory contains an adapter')
        seen.add(directory)
        for field in ('Imports', 'TestImports', 'XTestImports'):
            allowed = GO_IMPORTS | ({'testing'} if field != 'Imports' else set())
            for name in package.get(field, []):
                if name not in allowed:
                    raise ValueError(package['ImportPath'] + ': unreviewed pure import ' + name)
        if package.get('CgoFiles'):
            raise ValueError('Pure Go packages cannot contain cgo')
    expected = {path.parent.resolve() for path in pure.rglob('*.go')}
    if not seen or seen != expected:
        raise ValueError('Go pure source inventory differs from go list')


def main():
    def output(command):
        result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, timeout=60)
        if result.returncode != 0:
            raise ValueError('Metadata command failed: ' + ' '.join(command) + '\n' + result.stderr)
        return result.stdout

    names = rust_packages(json.loads(output(
        ['cargo', 'metadata', '--locked', '--no-deps', '--format-version', '1'])), ROOT)
    patterns = ['./' + module + '/...' for module in GO_MODULES if module.startswith('go/pure/')]
    if not patterns:
        raise ValueError('No pure Go modules selected')
    check_go(documents(output(['go', 'list', '-json', *patterns])), ROOT)
    command = ['cargo', 'clippy', '--locked', '--lib']
    for name in names:
        command += ['-p', name]
    command += ['--', *lint_arguments()]
    environment = dict(os.environ, CLIPPY_CONF_DIR=str(ROOT / 'tools/verification/pure'))
    compiler_controls(environment)
    subprocess.run(command, cwd=ROOT, env=environment, check=True, timeout=300)


if __name__ == '__main__':
    sys.exit(run_main(main, 'ERR_ARCHITECTURE'))
