#!/usr/bin/env python3
"""Report verified finite storage evidence without accepting a representation."""
import argparse
import hashlib
import json
from pathlib import Path

from container_storage_growth import read_verified_sample
from storage_growth_plan import select
from storage_growth_results import require, validate_census, wal_delta
from storage_history_model import encode_chain, encoded_bytes, reconstruct_at

_STAGES = ('baseline', 'create', 'edit-1', 'edit-5', 'edit-10', 'edit-20')
_COMPONENTS = ('heap_main_bytes', 'heap_auxiliary_bytes', 'index_bytes',
               'toast_main_bytes', 'toast_auxiliary_bytes', 'toast_index_bytes', 'total_bytes')
_COHORTS = {'complete': tuple(range(24)), 'baseline': tuple(range(20)), 'near_sha': tuple(range(20, 24))}


def storage_contract(source):
    """Shared inspected contracts, not an automated semantic-equivalence verdict.

    Full inventories remain bound separately. Changes in resource orchestration
    still require source/evidence review; matching this subset cannot grant it.
    """
    required = {'Cargo.toml', 'Cargo.lock', 'src/io/kehila_authority/Cargo.toml',
                'scripts/storage_growth_plan.py', 'scripts/storage_growth_results.py',
                'tests/integration/storage_growth_historical.sql',
                'tests/integration/storage_growth_observer.sql', 'tests/integration/storage_growth_state.sql'}
    require(required <= set(source), 'Storage contract inventory incomplete')
    # Historical inventories predate the shared matrix. Preserve their original
    # fingerprints; include the matrix whenever it belongs to the observed source.
    selected = {name: digest for name, digest in source.items() if name in required
                or name == 'scripts/storage_growth_samples.json' or (
        name.endswith(('.rs', '.sql', '.toml')) and (
            name.startswith(('storage/postgresql/', 'src/pure/')) or
            (name.startswith('src/io/kehila_authority/src/') and '/tests/' not in name
             and not name.endswith('/tests.rs'))))}
    return hashlib.sha256(encoded_bytes(selected)).hexdigest()


def growth_rows(sample, role, observations):
    """Within-role deltas only; allocation is not a live-content byte count."""
    require(role in {'native', 'historical'}, 'Unknown storage report role')
    result = []
    previous = None
    for stage, census in observations:
        validate_census(census)
        relations = census['relations']
        logical = census['logical']
        history = next(row for row in relations if row['name'] == 'kehila.item_revisions')
        row = {'sample': sample.index, 'profile': sample.profile, 'entropy': sample.entropy,
               'edit': sample.edit, 'repetition': sample.repetition, 'role': role, 'stage': stage,
               'revisions': logical['revisions'], 'history_text_bytes': logical['logical_history_text_bytes'],
               'request_bytes': logical['logical_request_bytes'],
               'result_text_bytes': logical['logical_result_text_bytes'],
               'pending_request_bytes': logical['logical_pending_request_bytes'],
               'database_allocated_bytes': census['database_allocated_bytes'],
               'wal_directory_allocated_bytes': census['wal_directory_allocated_bytes'],
               'history_relation_bytes': history['total_bytes']}
        row.update({field: sum(relation[field] for relation in relations) for field in _COMPONENTS})
        row['relation_bytes'] = row.pop('total_bytes')
        for key in ('revisions_added', 'history_text_growth_bytes', 'request_growth_bytes',
                    'result_text_growth_bytes', 'pending_request_growth_bytes', 'relation_growth_bytes',
                    'database_growth_bytes', 'wal_directory_growth_bytes', 'generated_wal_bytes',
                    'relation_bytes_per_added_revision'):
            row[key] = None
        if previous is not None:
            prior, prior_row = previous
            added = row['revisions'] - prior_row['revisions']
            require(added >= 0, 'Storage history count decreased')
            row.update(revisions_added=added,
                       history_text_growth_bytes=row['history_text_bytes']-prior_row['history_text_bytes'],
                       request_growth_bytes=row['request_bytes']-prior_row['request_bytes'],
                       result_text_growth_bytes=row['result_text_bytes']-prior_row['result_text_bytes'],
                       pending_request_growth_bytes=row['pending_request_bytes']-prior_row['pending_request_bytes'],
                       relation_growth_bytes=row['relation_bytes']-prior_row['relation_bytes'],
                       database_growth_bytes=row['database_allocated_bytes']-prior_row['database_allocated_bytes'],
                       wal_directory_growth_bytes=row['wal_directory_allocated_bytes']-prior_row['wal_directory_allocated_bytes'],
                       generated_wal_bytes=wal_delta(prior, census))
            row['relation_bytes_per_added_revision'] = row['relation_growth_bytes']/added if added else None
        result.append(row)
        previous = census, row
    return result


def model_comparison(histories):
    require(isinstance(histories, list) and len(histories) == 21,
            'Comparison requires the complete observed history chain')
    modeled = encode_chain(histories)
    counts = []
    byte_counts = [len(encoded_bytes(history)) for history in histories]
    checked_bytes = []
    for version, history in enumerate(histories, 1):
        reconstructed, reads = reconstruct_at(modeled, version)
        require(reconstructed == history, 'Modeled history reconstruction differs')
        counts.append(reads)
        checked_bytes.append(sum(byte_counts[version-reads:version]))
    return {'histories': 21, 'input_sha256': hashlib.sha256(encoded_bytes(histories)).hexdigest(),
            'full_encoded_bytes': len(encoded_bytes(histories)),
            'modeled_encoded_bytes': len(encoded_bytes(modeled)),
            'maximum_records_read': max(counts), 'mean_records_read': sum(counts)/len(counts),
            'maximum_history_bytes_checksummed': max(checked_bytes),
            'mean_history_bytes_checksummed': sum(checked_bytes)/len(checked_bytes),
            'physical_savings': 'unmeasured',
            'interpretation': 'logical encoding and reconstruction work only; no adopted schema/codec/restore capability'}


def complete_matrix(values):
    """Require all validated pair summaries exactly once under one binding."""
    return complete_selection(values, list(range(24)))


def complete_selection(values, indices):
    """Explicit cohort completeness does not imply full matrix completeness."""
    expected = {sample.index for sample in select(indices)}
    observed = {}
    binding = None
    for value in values:
        document = value['sample']
        sample = select([document['index']])[0]
        require(document == sample.document() and sample.index not in observed,
                'Storage report has another or duplicate workload')
        require(bool(value['binding']), 'Storage report binding is absent')
        if binding is None:
            binding = value['binding']
        require(value['binding'] == binding, 'Storage report source or environment changed')
        observed[sample.index] = value
    require(set(observed) == expected, 'Storage report matrix/explicit selection incomplete')
    return [observed[index] for index in sorted(observed)]


def coverage(reports):
    """Coverage across two declared cohorts; never pool them as one profile."""
    require(isinstance(reports, list) and len(reports) == 2
            and {report['cohort'] for report in reports} == {'baseline', 'near_sha'},
            'Storage coverage requires both declared cohorts exactly once')
    common = None
    profile = None
    seen = set()
    bindings = {}
    for report in reports:
        name = report['cohort']
        binding = report['binding']
        require(report['selection'] == list(_COHORTS[name]) and report['matrix_complete'] is False,
                'Storage cohort selection/completeness differs')
        complete_selection([dict(pair, binding=binding) for pair in report['pairs']], report['selection'])
        require(not seen.intersection(report['selection']), 'Storage cohort coverage overlaps')
        seen.update(report['selection'])
        storage = {key: binding[key] for key in ('storage_contract_sha256', 'postgres', 'column_storage')}
        execution = {key: value for key, value in binding['execution_profile'].items()
                     if key not in {'database_memory_bytes', 'database_tmpfs'}}
        if common is None:
            common, profile = storage, execution
        require(storage == common and execution == profile, 'Storage cohorts changed shared contracts/profile')
        memory = binding['execution_profile']['database_memory_bytes']
        require(memory in (256*1024**2, 512*1024**2)
                and (name != 'baseline' or memory == 256*1024**2), 'Storage cohort memory profile differs')
        tmpfs = binding['execution_profile']['database_tmpfs']
        require(set(tmpfs) == {'/var/lib/postgresql/data'} and
                f'size={memory//1024**2}m' in tmpfs['/var/lib/postgresql/data'].split(','),
                'Storage cohort data profile differs')
        bindings[name] = binding
    require(seen == set(range(24)), 'Storage cohort union incomplete')
    return {'schema': 1, 'completed_pairs': 24, 'bindings': bindings, 'common_contract': common,
            'interpretation': 'complete coverage with separate source/resource cohorts; no homogeneous-profile or causal resource comparison'}


def read_sample(root):
    root = root.resolve(strict=True)
    receipt = json.loads(root.with_name(root.name+'-receipt.json').read_text(encoding='utf-8'))
    sample = select([receipt['storage_sample_index']])[0]
    binding = read_verified_sample(root, receipt, sample)
    inventory = json.loads((root/'source-sha256.json').read_text(encoding='utf-8'))
    binding['storage_contract_sha256'] = storage_contract(inventory)
    selection = json.loads((root/'selection.json').read_text(encoding='utf-8'))
    require(selection['scope'] == 'storage-sample' and selection['plan'] == sample.document()
            and selection['observed_items'] == ['storage-sample'], 'Storage selection evidence differs')
    folder = root/'storage'
    rows = []
    alternatives = {}
    postgres = None
    column_storage = None
    for role, file in (('native', 'storage-sample-observation.txt'),
                       ('historical', 'storage-historical-observation.txt')):
        stages = (*_STAGES, 'retired', 'pruned') if role == 'historical' else _STAGES
        censuses = [(stage, json.loads((folder/f'{role}-{stage}.json').read_text(encoding='utf-8')))
                    for stage in stages]
        rows.extend(growth_rows(sample, role, censuses))
        settings = censuses[0][1]['postgres']
        columns = censuses[0][1]['column_storage']
        require(postgres is None or (postgres == settings and column_storage == columns),
                'Paired storage schema/server settings differ')
        postgres, column_storage = settings, columns
        observed = [json.loads(line) for line in (root/file).read_text(encoding='utf-8').splitlines()
                    if line.strip().startswith('{')]
        histories = [row['history'] for row in observed if row['type'] == 'revision']
        alternatives[role] = model_comparison(histories)
    binding.update(postgres=postgres, column_storage=column_storage)
    return {'sample': sample.document(), 'binding': binding, 'evidence_sha256': receipt['evidence_sha256'],
            'rows': rows, 'alternative': alternatives}


def main():
    parser = argparse.ArgumentParser(description=__doc__, fromfile_prefix_chars='@')
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--observation', type=Path, action='append')
    mode.add_argument('--coverage', type=Path, action='append', help='Combine the two explicitly complete cohort reports')
    parser.add_argument('--cohort', choices=tuple(_COHORTS), default='complete')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.coverage:
        require(args.cohort == 'complete', 'Coverage cannot select another cohort')
        report = coverage([json.loads(path.read_text(encoding='utf-8')) for path in args.coverage])
        report['report_sha256'] = [hashlib.sha256(path.read_bytes()).hexdigest() for path in args.coverage]
        with args.output.open('x', encoding='utf-8') as stream:
            json.dump(report, stream, sort_keys=True, indent=2, allow_nan=False)
            stream.write('\n')
        return
    values = complete_selection([read_sample(path) for path in args.observation], list(_COHORTS[args.cohort]))
    source = Path(__file__)
    model = source.with_name('storage_history_model.py')
    report = {'schema': 1, 'cohort': args.cohort, 'selection': [pair['sample']['index'] for pair in values],
              'matrix_complete': args.cohort == 'complete', 'binding': values[0]['binding'],
              'pairs': [{key: value for key, value in pair.items() if key != 'binding'} for pair in values],
              'analysis_sha256': {'reporter': hashlib.sha256(source.read_bytes()).hexdigest(),
                                  'alternative_model': hashlib.sha256(model.read_bytes()).hexdigest()},
              'limitations': ['finite one-item-per-role samples; allocation/page effects are not normalized',
                              'historical project setup is minimal; do not pool role database/WAL totals',
                              'WAL is cluster-generated, not exact causal per-edit or backup retention cost',
                              'alternative compares logical encodings and bounded reconstruction only',
                              'capacity/history disposition and any production representation qualification remain separate']}
    with args.output.open('x', encoding='utf-8') as stream:
        json.dump(report, stream, sort_keys=True, indent=2, allow_nan=False)
        stream.write('\n')


if __name__ == '__main__':
    main()
