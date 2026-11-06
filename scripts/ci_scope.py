"""Select development or final qualification from a validated event and diff."""
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
from typing import NamedTuple

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('commit_policy', ROOT / '.githooks/pre_commit.py')
POLICY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(POLICY)


class EventContext(NamedTuple):
    base: str | None
    qualification: bool
    force_full: bool


def event_context(event, event_name):
    if not isinstance(event, dict):
        raise ValueError('CI event must be an object')
    if event_name == 'pull_request':
        pr, action = event.get('pull_request'), event.get('action')
        if (not isinstance(pr, dict) or type(pr.get('draft')) is not bool or not isinstance(action, str)
                or action not in {'opened', 'synchronize', 'reopened', 'ready_for_review',
                                  'converted_to_draft', 'labeled'}):
            raise ValueError('Unsupported or malformed pull request event')
        draft = pr['draft']
        if (action == 'ready_for_review' and draft) or (action == 'converted_to_draft' and not draft):
            raise ValueError('Pull request action and draft state disagree')
        label = event.get('label')
        if action == 'labeled' and (not isinstance(label, dict) or not isinstance(label.get('name'), str)
                                    or not label['name']):
            raise ValueError('Labeled event requires its exact event label')
        force_full = action == 'ready_for_review' or (action == 'labeled' and label['name'] == 'qualification')
        pr_base = pr.get('base')
        base = pr_base.get('sha') if isinstance(pr_base, dict) else None
        qualification = not draft or force_full
    elif event_name == 'push' and event.get('ref') == 'refs/heads/main':
        base, qualification, force_full = event.get('before'), True, False
    else:
        raise ValueError('Unsupported CI event or branch')
    if not isinstance(base, str) or not re.fullmatch(r'[0-9a-fA-F]{40}|[0-9a-fA-F]{64}', base):
        raise ValueError('CI event requires a valid comparison base')
    if set(base) == {'0'}:
        if event_name != 'push':
            raise ValueError('Pull request comparison base must not be zero')
        return EventContext(None, True, True)
    return EventContext(base, qualification, force_full)


def choose_scope(event, root, *, event_name=None):
    context = event_context(event, event_name or os.environ.get('GITHUB_EVENT_NAME'))
    try:
        if context.force_full:
            if context.base is not None:
                subprocess.check_output(['git', 'cat-file', '-e', context.base + '^{commit}'],
                                        cwd=root, stderr=subprocess.PIPE, timeout=30)
            return 'full'
        changed = subprocess.check_output(
            ['git', 'diff', '--name-only', '--no-renames', '-z', context.base, 'HEAD', '--'],
            cwd=root, stderr=subprocess.PIPE, timeout=30).decode('utf-8').split('\0')
    except (OSError, UnicodeError, subprocess.SubprocessError) as error:
        raise ValueError('Cannot classify the CI source diff') from error
    paths = [path for path in changed if path]
    if POLICY.verification_scope(paths) == 'docs':
        return 'docs'
    return 'full' if context.qualification else 'development'


def read_event():
    try:
        return json.loads(Path(os.environ['GITHUB_EVENT_PATH']).read_text(encoding='utf-8'))
    except (KeyError, OSError, ValueError) as error:
        raise ValueError('Cannot read the CI event') from error


def main():
    event = read_event()
    scope = choose_scope(event, ROOT)
    with Path(os.environ['GITHUB_OUTPUT']).open('a', encoding='utf-8') as output:
        output.write(f'scope={scope}\n')
    print(f'CI verification scope: {scope}')


if __name__ == '__main__':
    try:
        main()
    except ValueError as error:
        raise SystemExit('CI_SELECTION_FAILED: ' + str(error)) from None
