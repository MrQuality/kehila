"""One explicit Go module scope for workspace, formatting, vet and tests."""
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
GO_MODULES = ('go/pure/sync_contract', 'go/io/task_api')
GO_PACKAGES = ['./' + module + '/...' for module in GO_MODULES]


def validate_layout(root, workspace):
    """Fail on omitted modules/sources instead of silently reducing check scope."""
    root = root.resolve()
    expected = {(root / module).resolve() for module in GO_MODULES}
    entries = workspace.get('Use') if isinstance(workspace, dict) else None
    if not isinstance(entries, list) or any(
            not isinstance(entry, dict) or not isinstance(entry.get('DiskPath'), str)
            or not entry['DiskPath'] for entry in entries):
        raise RuntimeError('Invalid Go workspace module list')
    used = [(root / entry['DiskPath']).resolve() for entry in entries]
    if len(used) != len(set(used)) or set(used) != expected:
        raise RuntimeError('Go workspace differs from GO_MODULES in scripts/go_inventory.py')
    discovered = {path.parent.resolve() for path in (root / 'go').rglob('go.mod')}
    if discovered != expected:
        raise RuntimeError('Go module inventory differs from source tree; update inventory and go.work')
    files = sorted((root / 'go').rglob('*.go'))
    if not files:
        raise RuntimeError('No Go files found; verification scope cannot be empty')
    uncovered = [path.relative_to(root).as_posix() for path in files
                 if not any(path.resolve().is_relative_to(module) for module in expected)]
    if uncovered:
        raise RuntimeError('Go sources outside inventoried modules: ' + ', '.join(uncovered))
    return [path.relative_to(root).as_posix() for path in files]


def go_files():
    # -json is a read-only view supplied by Go, not a second go.work parser.
    result = subprocess.run(['go', 'work', 'edit', '-json', str(ROOT / 'go.work')],
                            cwd=ROOT, check=True, capture_output=True, text=True, timeout=30)
    try:
        workspace = json.loads(result.stdout)
    except ValueError as error:
        raise RuntimeError('Go returned invalid workspace JSON') from error
    return validate_layout(ROOT, workspace)
