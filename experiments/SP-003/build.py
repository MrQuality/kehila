"""Build the isolated oracle against exact Cargo-reported existing dependencies."""
from contextlib import contextmanager
import json
from pathlib import Path
import subprocess
import sys

SOURCE = Path(__file__).resolve().parent
REPO = SOURCE.parents[1]
sys.path.insert(0, str(REPO / "scripts"))
from verification_budget import command_timeout
from temp_work import owned_directory


def command(args, *, input=None, timeout=120):
    result = subprocess.run(args, cwd=REPO, input=input, capture_output=True, timeout=command_timeout(timeout))
    if result.returncode:
        raise RuntimeError((result.stderr or result.stdout).decode(errors="replace"))
    return result.stdout


def oracle_inputs(metadata, messages):
    """Bind exact direct libraries and search paths to this Cargo build graph."""
    manifest = (REPO / 'src/pure/project_create_codec/Cargo.toml').resolve()
    producers = [package for package in metadata['packages']
                 if package['name'] == 'project_create_codec'
                 and Path(package['manifest_path']).resolve() == manifest]
    if len(producers) != 1:
        raise RuntimeError('Cargo must resolve the owned oracle dependency producer')
    nodes = [node for node in metadata['resolve']['nodes']
             if node['id'] == producers[0]['id']]
    if len(nodes) != 1:
        raise RuntimeError('Cargo must resolve one producer dependency node')
    packages = {package['id']: package for package in metadata['packages']}
    libraries = {}
    for name in ('task_contract', 'serde_json', 'sha2'):
        dependencies = [dependency['pkg'] for dependency in nodes[0]['deps']
                        if dependency['name'] == name]
        if (len(dependencies) != 1
                or packages[dependencies[0]]['name'] != name):
            raise RuntimeError('Oracle producer must own direct dependency ' + name)
        paths = {Path(filename) for record in messages
                 if record.get('reason') == 'compiler-artifact'
                 and record.get('package_id') == dependencies[0]
                 and record.get('target', {}).get('name') == name
                 and record.get('target', {}).get('kind') == ['lib']
                 for filename in record['filenames'] if filename.endswith('.rlib')}
        if len(paths) != 1:
            raise RuntimeError('Cargo must report one library for each exact dependency')
        libraries[name] = paths.pop()
    # Cargo's intermediate layout is internal and changed in 1.100. Exact
    # --extern inputs retain direct-edge identity; transitive lookup follows
    # only library artifacts reported for this producer's resolved closure.
    graph = {node['id']: node for node in metadata['resolve']['nodes']}
    closure, pending = set(), [producers[0]['id']]
    while pending:
        identity = pending.pop()
        if identity in closure:
            continue
        if identity not in graph or identity not in packages:
            raise RuntimeError('Cargo must resolve the complete oracle dependency graph')
        closure.add(identity)
        pending.extend(edge['pkg'] for edge in graph[identity]['deps'])
    directories = tuple(sorted({Path(filename).parent for record in messages
                                if record.get('reason') == 'compiler-artifact'
                                and record.get('package_id') in closure
                                and record.get('target', {}).get('kind') == ['lib']
                                for filename in record['filenames']
                                if filename.endswith('.rlib')}, key=str))
    return libraries, directories


@contextmanager
def build():
    """Keep unique compiler outputs alive only while the caller uses the oracle."""
    command(["rustfmt", "--edition", "2021", "--check",
             *(str(SOURCE / name) for name in ("main.rs", "codec.rs", "tests.rs"))])
    hosts = [line.removeprefix('host: ').strip()
             for line in command(['rustc', '-vV']).decode().splitlines()
             if line.startswith('host: ')]
    if len(hosts) != 1 or not hosts[0]:
        raise RuntimeError('Rust compiler must report one native host target')
    host = hosts[0]
    metadata = json.loads(command(['cargo', 'metadata', '--locked', '--offline',
                                   '--format-version=1', '--filter-platform', host]))
    messages = command(["cargo", "build", "--locked", "--offline", "-p", "project_create_codec",
                        "--lib", "--target", host, "--message-format=json"], timeout=450)
    libraries, directories = oracle_inputs(metadata, [json.loads(line) for line in messages.splitlines()])
    with owned_directory('kehila-sp003-', retry_seconds=5) as root:
        suffix = ".exe" if sys.platform == "win32" else ""
        args = ["rustc", "--edition=2021", "-D", "warnings", str(SOURCE / "main.rs")]
        for name, library in libraries.items():
            args += ["--extern", name + "=" + str(library)]
        for directory in directories:
            args += ["-L", "dependency=" + str(directory)]
        tests = root / ("codec_tests" + suffix)
        command([*args, "--test", "-o", str(tests)])
        output = command([str(tests)])
        binary = root / ("codec_oracle" + suffix)
        command([*args, "-o", str(binary)])
        yield binary, output.decode()


if __name__ == "__main__":
    with build() as (_, output):
        print(output, end="")
