#!/usr/bin/env python3
"""Report retained finite observations without pooling percentiles or hiding failures."""
import argparse
from collections import defaultdict
import hashlib
import json
from pathlib import Path
from statistics import median

from capacity_plan import cell as planned_cell, select
from capacity_results import canonical, parse, require, LATENCY_BOUNDARY, THROUGHPUT_BOUNDARY
from native_fixture_assets import MANIFEST_FILE, _directory, _hash


def read_measurement(root, receipt_path):
    """Validate the manifest and the exact small evidence closure used in analysis.

    Full raw inventories were verified at export. This does not claim a fresh
    revalidation of every retained raw request/body/key file.
    """
    root = _directory(root)
    receipt = parse(receipt_path.read_bytes())
    require(receipt.get('schema') == 1 and receipt.get('reset_completed') is True
            and receipt.get('independent_state_verified') is True
            and receipt.get('calibration') is False, 'Expected a complete ordinary observation receipt')
    manifest_path = root / MANIFEST_FILE
    require(_hash(manifest_path) == receipt['evidence_sha256'], 'Evidence manifest differs from its receipt')
    manifest_raw = manifest_path.read_bytes()
    require(hashlib.sha256(manifest_raw).hexdigest() == receipt['evidence_sha256'], 'Parsed evidence manifest changed')
    manifest = parse(manifest_raw)
    entries = {entry['path']:entry for entry in manifest['entries']}
    require(len(entries) == len(manifest['entries']), 'Duplicate evidence manifest entry')

    def read(name, *, document=True):
        entry = entries[name]
        path = root / name
        require(entry['kind'] == 'file' and path.stat().st_size == entry['bytes']
                and _hash(path) == entry['sha256'], 'Analysis input differs from retained evidence')
        raw = path.read_bytes()
        require(hashlib.sha256(raw).hexdigest() == entry['sha256'], 'Parsed analysis input changed')
        return parse(raw) if document else raw.decode('utf-8')

    summary = read('capacity-summary.json')
    profile = read('execution-profile.json')
    selection = read('selection.json')
    source = read('source-sha256.json')
    sql = read('capacity-cell-observation.txt', document=False)
    controls = 'runner-evidence/controls/capacity-cell/'
    for field, names in {
        'work_sha256':(controls+'matrix-work.json',),
        'ownership_sha256':(controls+'matrix-owned.json',),
        'expectations_sha256':('capacity-expectations.jsonl',controls+'matrix-expectations.jsonl'),
    }.items():
        require(all(entries[name]['kind']=='file' and entries[name]['sha256']==summary[field] for name in names),
                'Summary raw-input cross-reference differs from retained manifest')
    require(entries['runner-evidence/'+MANIFEST_FILE]['sha256']==receipt['asset_sha256'],
            'Generated asset manifest differs from receipt')
    cell = planned_cell(receipt['cell_index'])
    require(summary['calibration'] is False and canonical(summary['plan']) == canonical(cell.document())
            and summary['latency_boundary'] == LATENCY_BOUNDARY and summary['throughput_boundary'] == THROUGHPUT_BOUNDARY
            and canonical(selection['plan']) == canonical(cell.document())
            and summary['observer'] == receipt['observer'] == selection['observer']
            and selection['calibration'] is False and selection['observed_items'] == ['capacity-cell']
            and 'NATIVE_CAPACITY_CELL_STATE_PASS' in sql, 'Observation selection or independent witness differs')
    require(manifest['schema'] == 1 and manifest['owner'] == profile['owner'], 'Observation owner differs')
    environment = {name:profile[name] for name in ('schema','runner_image','runner_memory_bytes','runner_pids',
        'runner_nano_cpus','runner_cpu_period','runner_cpu_quota','database_image','database_memory_bytes',
        'database_pids','database_nano_cpus','database_cpu_period','database_cpu_quota','database_tmpfs')}
    require(isinstance(environment.get('runner_image'),str) and environment['runner_image']
            and environment.get('runner_memory_bytes') and environment.get('runner_pids'),
            'Observed runner profile is incomplete')
    environment['postgres'] = {name:summary['postgres'][name] for name in ('server_version','max_connections','ssl')}
    return {'summary':summary, 'source_sha256':hashlib.sha256(canonical(source)).hexdigest(),
            'environment':environment}


def _admit(summary):
    require(summary.get('schema') == 1 and summary.get('calibration') is False,
            'Calibration or invalid evidence cannot enter the ordinary matrix')
    cell = planned_cell(summary['plan']['index'])
    require(canonical(summary['plan']) == canonical(cell.document())
            and summary['observer'] in {'off','stages','sampled'}, 'Observation plan differs')
    measured = summary['measured']
    require(type(measured['attempted']) is int and measured['attempted'] == 100
            and type(measured['successful']) is int and 0 <= measured['successful'] <= 100
            and sum(measured['outcomes'].values()) == 100, 'Original measured population differs')
    return cell


def _group(values):
    values = sorted(values, key=lambda value:value['plan']['repetition'])
    p95 = [value['measured']['latency_ns']['p95'] / 1e6 for value in values]
    first = values[0]['plan']
    return {'profile':first['profile'],'layout':first['layout'],'mix':first['mix'],
            'concurrency':first['concurrency'],'repetitions':[value['plan']['repetition'] for value in values],
            'measured_successes':[value['measured']['successful'] for value in values],
            'outcomes':[value['measured']['outcomes'] for value in values],
            'per_repetition_p95_ms':p95,'median_repetition_p95_ms':median(p95),
            'per_repetition_success_p95_ms':[None if value['measured']['successful_latency_ns']['p95'] is None
                else value['measured']['successful_latency_ns']['p95']/1e6 for value in values],
            'successful_per_second':[value['measured']['successful_per_second'] for value in values],
            'observed_peak_inflight':[value['measured']['peak_client_inflight'] for value in values],
            'observed_mean_inflight':[value['measured']['mean_client_inflight'] for value in values],
            'active_items':[value['measured']['active_items'] for value in values],
            'active_projects':[value['measured']['active_projects'] for value in values],
            'active_sessions':[value['measured']['active_sessions'] for value in values],
            'stages_by_repetition':[value['measured']['stages'] for value in values],
            'sampling_by_repetition':[value['measured']['sampling'] for value in values],
            'warmup_outcomes':[value['warmup']['outcomes'] for value in values],
            'serial_control_outcomes':[value['control']['outcomes'] for value in values],
            'reconciliation_counts_by_repetition':[{phase:value[phase]['recoveries']
                for phase in ('warmup','measured','control')} for value in values],
            'transport_outcomes_whole_cell':[value['connections'] for value in values],
            'setup_seconds':[value['setup_ns']/1e9 for value in values],
            'database_bytes_after':[value['postgres']['database_bytes_after'] for value in values],
            'wal_bytes_after':[value['postgres']['wal_bytes_after'] for value in values],
            'normal_four_request_target_met_all_repetitions':
                (None if any(value['normal_four_request_target_met'] is None for value in values)
                 else all(value['normal_four_request_target_met'] is True for value in values)) if first['concurrency']==4 else None,
            'normal_four_request_target_observed_all_repetitions':
                all(value['normal_four_request_target_observed'] is True for value in values) if first['concurrency']==4 else None}


def group_matrix(values, *, complete=True):
    observed, grouped = set(),defaultdict(list)
    for value in values:
        cell = _admit(value)
        require(value['observer'] == 'sampled' and cell.index not in observed, 'Duplicate or non-sampled matrix cell')
        observed.add(cell.index)
        grouped[(cell.profile,cell.layout,cell.mix,cell.concurrency)].append(value)
    require(not complete or observed == set(range(180)), 'Full matrix has missing cells')
    for group in grouped.values():
        require(not complete or {value['plan']['repetition'] for value in group} == {1,2,3}, 'Missing repetition')
    return [_group(grouped[key]) for key in sorted(grouped)]


def compare_observers(values, expected_indices):
    expected = {(entry.index, mode) for entry in select(expected_indices) for mode in ('off','stages','sampled')}
    observed, grouped = set(),defaultdict(lambda:defaultdict(list))
    for value in values:
        cell = _admit(value)
        identity = (cell.index,value['observer'])
        require(identity not in observed, 'Duplicate observer comparison cell')
        observed.add(identity)
        grouped[(cell.profile,cell.layout,cell.mix,cell.concurrency)][value['observer']].append(value)
    require(observed == expected, 'Observer comparison differs from its explicit finite cell closure')
    result = []
    for key, modes in sorted(grouped.items()):
        require(set(modes)=={'off','stages','sampled'} and all(
            {value['plan']['repetition'] for value in group}=={1,2,3} for group in modes.values()),
            'Observer comparison lacks a mode or repetition')
        distributions = {mode:_group(group) for mode,group in modes.items()}
        baseline = distributions['off']['per_repetition_p95_ms']
        require(all(value>0 for value in baseline), 'Invalid observer latency baseline')
        result.append({'profile':key[0],'layout':key[1],'mix':key[2],'concurrency':key[3],
            'modes':distributions,'sampled_to_off_p95_ratios':[
                sampled/off for sampled,off in zip(distributions['sampled']['per_repetition_p95_ms'],baseline)],
            'stages_to_off_p95_ratios':[
                stages/off for stages,off in zip(distributions['stages']['per_repetition_p95_ms'],baseline)],
            'all_measured_originals_succeeded_in_all_modes':all(value['measured']['successful']==100
                for group in modes.values() for value in group),
            'interpretation':'observed repetition ratios; different accepted populations and scheduler noise prevent a pure causal cost claim'})
    return result


def main():
    parser=argparse.ArgumentParser(description=__doc__,fromfile_prefix_chars='@')
    parser.add_argument('--observation',type=Path,action='append',required=True,
                        help='Complete export directory with sibling <name>-receipt.json; repeat explicitly')
    parser.add_argument('--comparisons',action='store_true')
    parser.add_argument('--expected-cell',type=int,action='append',
                        help='Required finite cell set for comparisons; repeat for every selected repetition')
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    measurements=[read_measurement(path,path.with_name(path.name+'-receipt.json')) for path in args.observation]
    require(len({value['source_sha256'] for value in measurements})==1
            and len({canonical(value['environment']) for value in measurements})==1,
            'Measurement source or observed environment differs')
    summaries=[value['summary'] for value in measurements]
    require(args.comparisons == (args.expected_cell is not None), 'Comparison reports require explicit expected cells')
    groups=compare_observers(summaries,args.expected_cell) if args.comparisons else group_matrix(summaries)
    report={'schema':1,'kind':'observer_comparison' if args.comparisons else 'ordinary_matrix',
            'source_sha256':measurements[0]['source_sha256'],'environment':measurements[0]['environment'],
            'latency_boundary':LATENCY_BOUNDARY,'throughput_boundary':THROUGHPUT_BOUNDARY,
            'target_population':'100 original measured requests with observed overlap4; p95 <=1 second and all100 succeed; whole-cell transport failures also reject target. Unobserved overlap leaves target unqualified. Warmup/control/recovery reported separately.',
            'percentile_method':'nearest rank within each100 original requests; report repetition p95 values and median, not pooled p95',
            'groups':groups}
    with args.output.open('x',encoding='utf-8') as stream:json.dump(report,stream,sort_keys=True,indent=2)
    print('CAPACITY REPORT: retained selections, source and original populations validated')


if __name__=='__main__':main()
