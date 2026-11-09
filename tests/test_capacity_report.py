"""Reporting must preserve failures and repetition boundaries."""
from pathlib import Path
import json
import hashlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import capacity_plan as plan
import capacity_report as report
import native_fixture_assets as assets


def measurement(index, observer='sampled', successful=100, p95=10_000_000):
    cell=plan.cell(index)
    return {'schema':1,'plan':cell.document(),'observer':observer,'calibration':False,
            'latency_boundary':report.LATENCY_BOUNDARY,'throughput_boundary':report.THROUGHPUT_BOUNDARY,
            'normal_four_request_target_met':successful==100 if cell.concurrency==4 else None,
            'normal_four_request_target_observed':True if cell.concurrency==4 else None,
            'measured':{'attempted':100,'successful':successful,'outcomes':{'success':successful,'busy':100-successful},
                        'latency_ns':{'p95':p95},'successful_latency_ns':{'p95':p95},
                        'attempts_per_second':100.0,'successful_per_second':float(successful),
                        'stages':{},'sampling':None,'recoveries':0,'peak_client_inflight':cell.concurrency,
                        'mean_client_inflight':1.0,'active_items':cell.concurrency,
                        'active_projects':min(cell.concurrency,cell.document()['projects']),'active_sessions':4},
            'warmup':{'outcomes':{'success':10},'recoveries':0},
            'control':{'outcomes':{'success':1},'recoveries':0},'connections':{},'setup_ns':24_000_000_000,
            'postgres':{'server_version':'16.test','max_connections':'8','ssl':'on',
                        'database_bytes_after':1000,'wal_bytes_after':16384}}


class CapacityReportTests(unittest.TestCase):
    def test_unobserved_four_request_overlap_remains_unqualified(self):
        values=[measurement(index) for index in (6,7,8)]
        values[0]['measured']['peak_client_inflight']=3
        values[0]['normal_four_request_target_observed']=False
        values[0]['normal_four_request_target_met']=None
        group=report.group_matrix(values,complete=False)[0]
        self.assertIsNone(group['normal_four_request_target_met_all_repetitions'])
        self.assertFalse(group['normal_four_request_target_observed_all_repetitions'])

    def test_analysis_reads_only_manifest_bound_inputs_and_sanitizes_profile(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()/'cell';root.mkdir()
            owner='a'*32;assets.write_owner(root,owner)
            profile={name:0 for name in ('runner_nano_cpus','runner_cpu_period','runner_cpu_quota',
                     'database_memory_bytes','database_pids','database_nano_cpus','database_cpu_period','database_cpu_quota')}
            profile.update(schema=1,owner=owner,runner_image='sha256:'+'b'*64,runner_memory_bytes=2048*1024**2,
                           runner_pids=512,database_image='synthetic-image',database_tmpfs={},
                           unrelated_private_metadata='must not appear')
            documents={'capacity-summary.json':measurement(0),'execution-profile.json':profile,
                       'source-sha256.json':{'public-source':'c'*64},
                       'selection.json':{'plan':plan.cell(0).document(),'observer':'sampled',
                                         'calibration':False,'observed_items':['capacity-cell']}}
            controls=root/'runner-evidence/controls/capacity-cell';controls.mkdir(parents=True)
            for name,body in (('matrix-work.json',b'work'),('matrix-owned.json',b'owned'),('matrix-expectations.jsonl',b'expected')):
                (controls/name).write_bytes(body)
            (root/'capacity-expectations.jsonl').write_bytes(b'expected')
            (root/'runner-evidence'/assets.MANIFEST_FILE).write_bytes(b'assets')
            documents['capacity-summary.json'].update(work_sha256=hashlib.sha256(b'work').hexdigest(),
                ownership_sha256=hashlib.sha256(b'owned').hexdigest(),expectations_sha256=hashlib.sha256(b'expected').hexdigest())
            for name,value in documents.items():(root/name).write_text(json.dumps(value))
            (root/'capacity-cell-observation.txt').write_text('NATIVE_CAPACITY_CELL_STATE_PASS')
            receipt=root.with_name('cell-receipt.json')
            receipt_value={'schema':1,'reset_completed':True,'independent_state_verified':True,
                'calibration':False,'cell_index':0,'observer':'sampled','evidence_sha256':assets.write_manifest(root,owner),
                'asset_sha256':hashlib.sha256(b'assets').hexdigest()}
            receipt.write_text(json.dumps(receipt_value))
            value=report.read_measurement(root,receipt)
            self.assertNotIn('unrelated_private_metadata',value['environment'])
            self.assertNotIn('owner',value['environment'])
            documents['capacity-summary.json']['work_sha256']='d'*64
            (root/'capacity-summary.json').write_text(json.dumps(documents['capacity-summary.json']))
            (root/assets.MANIFEST_FILE).unlink()
            receipt_value['evidence_sha256']=assets.write_manifest(root,owner)
            receipt.write_text(json.dumps(receipt_value))
            with self.assertRaises(ValueError):report.read_measurement(root,receipt)
            (root/'capacity-summary.json').write_text('{}')
            with self.assertRaises(ValueError):report.read_measurement(root,receipt)

    def test_grouped_p95_is_reported_as_per_repetition_distribution_and_failure_is_kept(self):
        values=[measurement(6,p95=10_000_000),measurement(7,p95=20_000_000),measurement(8,successful=99,p95=30_000_000)]
        grouped=report.group_matrix(values, complete=False)
        self.assertEqual(len(grouped),1)
        group=grouped[0]
        self.assertEqual(group['measured_successes'],[100,100,99])
        self.assertEqual(group['per_repetition_p95_ms'],[10.0,20.0,30.0])
        self.assertEqual(group['median_repetition_p95_ms'],20.0)
        self.assertFalse(group['normal_four_request_target_met_all_repetitions'])
        self.assertNotIn('pooled_p95_ms',group)

    def test_complete_matrix_rejects_missing_duplicate_or_calibration_cells(self):
        full=[measurement(index) for index in range(180)]
        self.assertEqual(len(report.group_matrix(full)),60)
        for mutation in ('missing','duplicate','calibration','mode'):
            values=[dict(value) for value in full]
            if mutation=='missing':values.pop()
            elif mutation=='duplicate':values[-1]=values[0]
            elif mutation=='calibration':values[0]['calibration']=True
            else:values[0]['observer']='off'
            with self.subTest(mutation=mutation),self.assertRaises(ValueError):report.group_matrix(values)

    def test_observer_comparisons_keep_success_populations_and_do_not_invent_causal_overhead(self):
        values=[measurement(index,mode,successful=90 if mode=='sampled' else 100,
                            p95=20_000_000 if mode=='sampled' else 10_000_000)
                for index in (6,7,8) for mode in ('off','stages','sampled')]
        comparison=report.compare_observers(values,[6,7,8])[0]
        self.assertEqual(comparison['sampled_to_off_p95_ratios'],[2.0,2.0,2.0])
        self.assertFalse(comparison['all_measured_originals_succeeded_in_all_modes'])
        self.assertEqual(comparison['modes']['sampled']['measured_successes'],[90,90,90])
        values.pop()
        with self.assertRaises(ValueError):report.compare_observers(values,[6,7,8])
        complete_group=[measurement(index,mode) for index in (6,7,8) for mode in ('off','stages','sampled')]
        with self.assertRaises(ValueError):report.compare_observers(complete_group,[6,7,8,96,97,98])


if __name__ == '__main__':unittest.main()
