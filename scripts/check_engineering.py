#!/usr/bin/env python3
"""Validate the engineering register and its deterministic human-readable view.

This checks record integrity, not the truth of evidence or external compliance.
"""
import argparse
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
REGISTER = Path('docs/engineering/requirements.json')
PREAMBLE = Path('docs/engineering/STANDARD-PREAMBLE.md')
DOCUMENT = Path('docs/engineering/ENGINEERING-STANDARD.md')
FIELDS = {'id', 'area', 'gate', 'obligation', 'statement', 'rationale', 'sources',
          'required_evidence', 'enforcement', 'status', 'evidence', 'gap', 'work'}
STATUSES = {'satisfied', 'partial', 'planned'}
OBLIGATIONS = {'required': {'MUST', 'SHALL'}, 'recommended': {'SHOULD'},
               'optional': {'MAY'}}
GATE_RANK = {f'E{rank}': rank for rank in range(1, 6)}


def reference_error(root, reference):
    if not isinstance(reference, str) or not reference:
        return 'reference must be a nonempty string'
    path, _, anchor = reference.partition('#')
    target = (root / path).resolve()
    if not target.is_relative_to(root.resolve()) or not target.is_file():
        return f'missing or out-of-repository reference: {reference}'
    if anchor:
        occurrences = target.read_text(encoding='utf-8').count(f'<a id="{anchor}"></a>')
        if occurrences == 0:
            return f'missing explicit anchor: {reference}'
        if occurrences != 1:
            return f'duplicate explicit anchor: {reference}'
    return None


def validate(data, root=ROOT):
    errors = []
    if not isinstance(data, dict) or set(data) != {'version', 'sources', 'requirements'}:
        return ['invalid register fields']
    if type(data['version']) is not int or data['version'] != 4:
        errors.append('invalid version')
    sources = data['sources']
    if not isinstance(sources, dict) or not sources:
        return errors + ['sources must be a nonempty object']
    for key, source in sources.items():
        if (not isinstance(source, dict) or set(source) != {'title', 'url'}
                or not isinstance(source['title'], str) or not source['title'].strip()
                or not isinstance(source['url'], str) or not source['url'].startswith('https://')):
            errors.append(f'invalid source: {key}')
    requirements = data['requirements']
    if not isinstance(requirements, list) or not requirements:
        return errors + ['requirements must be a nonempty list']
    seen = set()
    for row in requirements:
        if not isinstance(row, dict) or set(row) != FIELDS:
            errors.append('invalid requirement fields')
            continue
        identity = row['id']
        if not isinstance(identity, str) or not re.fullmatch(r'[A-Z]+-[0-9]{3}', identity) or identity in seen:
            errors.append(f'invalid or duplicate ID: {identity}')
        else:
            seen.add(identity)
        if not isinstance(row['gate'], str) or row['gate'] not in GATE_RANK:
            errors.append(f'{identity}: invalid gate')
        for field in ('area', 'statement', 'rationale', 'required_evidence', 'enforcement', 'gap'):
            if not isinstance(row[field], str) or not row[field].strip():
                errors.append(f'{identity}: empty {field}')
        if isinstance(row['statement'], str) and not re.search(r'\b(MUST|SHALL|SHOULD|MAY)\b', row['statement']):
            errors.append(f'{identity}: missing normative language')
        obligation = row['obligation']
        if not isinstance(obligation, str) or obligation not in OBLIGATIONS:
            errors.append(f'{identity}: invalid obligation')
        elif isinstance(row['statement'], str):
            tokens = set(re.findall(r'\b(MUST|SHALL|SHOULD|MAY)\b', row['statement']))
            if not tokens.intersection(OBLIGATIONS[obligation]):
                errors.append(f'{identity}: obligation does not match normative language')
            if obligation != 'required' and tokens.intersection({'MUST', 'SHALL'}):
                errors.append(f'{identity}: nonmandatory row contains mandatory clauses')
        if not isinstance(row['status'], str) or row['status'] not in STATUSES:
            errors.append(f'{identity}: invalid status')
        for field in ('sources', 'evidence', 'work'):
            if not isinstance(row[field], list) or any(not isinstance(x, str) or not x for x in row[field]):
                errors.append(f'{identity}: invalid {field}')
                continue
            if field != 'evidence' and not row[field]:
                errors.append(f'{identity}: empty {field}')
            for reference in row[field]:
                if field == 'sources':
                    error = None if reference in sources else f'unknown source: {reference}'
                else:
                    error = reference_error(root, reference)
                if error:
                    errors.append(f'{identity}: {error}')
        if row['status'] in ('satisfied', 'partial') and not row['evidence']:
            errors.append(f'{identity}: implemented status requires evidence')
        if isinstance(row['work'], list) and not any(isinstance(x, str) and re.fullmatch(r'docs/product/backlog.md#b-[0-9]{3}', x) for x in row['work']):
            errors.append(f'{identity}: backlog destination required')
    return errors


def render(data, preamble):
    if not preamble.strip():
        raise ValueError('Authored preamble must not be empty')
    lines = [preamble.rstrip(), '', '## Reference standards', '']
    for key, source in data['sources'].items():
        lines.append(f"- **{key}:** [{source['title']}]({source['url']}).")
    lines += ['', '## Requirement register', '',
              'Paths below are relative to the repository root in the register. Status applies to the entire requirement, not just its existing tests.', '']
    def links(refs):
        return ', '.join(f'[{ref}](../../{ref})' for ref in refs) or 'None yet.'
    for row in data['requirements']:
        lines += [f'<a id="{row["id"].lower()}"></a>',
                  f"### {row['id']} — {row['area']}", '', row['statement'], '',
                  f"- **Rationale:** {row['rationale']}",
                  f"- **First applicable gate:** {row['gate']} (cumulative thereafter).",
                  f"- **Obligation:** {row['obligation']}.",
                  f"- **Sources:** {', '.join(row['sources'])}.",
                  f"- **Required evidence:** {row['required_evidence']}",
                  f"- **Enforcement:** {row['enforcement']}",
                  f"- **Current status:** {row['status']}. {row['gap']}",
                  f"- **Current evidence:** {links(row['evidence'])}",
                  f"- **Implementation / backlog / decisions:** {links(row['work'])}", '']
    return '\n'.join(lines)


def gate_blockers(data, gate):
    """Metadata gaps in mandatory controls; recommendations remain review inputs."""
    return [row['id'] for row in data['requirements']
            if GATE_RANK[row['gate']] <= GATE_RANK[gate] and row['obligation'] == 'required'
            and row['status'] != 'satisfied']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true', help='Regenerate the standard after changing the register')
    parser.add_argument('--gate', choices=GATE_RANK,
                        help='Fail on mandatory cumulative gaps; not release approval')
    args = parser.parse_args()
    data = json.loads((ROOT / REGISTER).read_text(encoding='utf-8'))
    errors = validate(data, ROOT)
    if errors:
        raise ValueError('\n'.join(errors))
    expected = render(data, (ROOT / PREAMBLE).read_text(encoding='utf-8'))
    if args.write:
        (ROOT / DOCUMENT).write_text(expected, encoding='utf-8', newline='\n')
    elif (ROOT / DOCUMENT).read_text(encoding='utf-8') != expected:
        raise ValueError('Standard is stale; run python scripts/check_engineering.py --write')
    if args.gate:
        advisory = [row['id'] for row in data['requirements']
                    if GATE_RANK[row['gate']] <= GATE_RANK[args.gate]
                    and row['obligation'] != 'required'
                    and row['status'] != 'satisfied']
        if advisory:
            print('Nonblocking recommendations/options needing assessment: ' + ', '.join(advisory))
        blockers = gate_blockers(data, args.gate)
        if blockers:
            raise ValueError(f'{args.gate} missing evidence: ' + ', '.join(blockers))
    print(f"Engineering register: {len(data['requirements'])} valid requirements; document synchronized")


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, TypeError) as error:
        print(f'ERR_ENGINEERING_STANDARD: {error}', file=sys.stderr)
        sys.exit(1)
