"""Independent workload/identity accounting before the SQL state witness."""
import base64
import copy
import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import capacity_plan as plan
import capacity_results as results


def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()


def fixture(index=0):
    cell = plan.cell(index)
    assert cell.layout == 'shared' and cell.concurrency == 1 and cell.profile == 'compact'
    fields = ['field-' + str(index) for index in range(4)]
    slots, known = [], {}
    for slot in range(16):
        item = 'item-' + str(slot)
        values = [{'id':field, 'value':format(slot + field_index, '064x')}
                  for field_index, field in enumerate(fields)]
        readable = {'project_id':'project', 'item_id':item, 'prefix':'CP0', 'sequence':str(slot+1)}
        saved = {'codec_version':1, 'kind':'item_create_result', 'value':{
            'project':{'id':'project', 'name':'Capacity project 0', 'prefix':'CP0',
                       'configuration_revision':'1', 'next_sequence':str(slot+2), 'archived':False,
                       'estimate_unit':'hours', 'ever_estimated':False},
            'item':{'id':item, 'project_id':'project', 'type_id':'type', 'workflow_id':'workflow',
                    'status_id':'status', 'version':'1', 'archived':False},
            'data':{'estimate':None, 'values':[{'id':value['id'], 'value':{'kind':'text','value':value['value']}} for value in values]},
            'phase':'new', 'readable_id':readable}}
        seed = {'registration':{'request_id':str(slot+1), 'project_id':'project', 'target_item_id':item,
                                 'token':format(100_000+slot,'032x'), 'intent_handle':format(200_000+slot,'032x')},
                'request':{'project_id':'project','type_id':'type','workflow_id':None,
                           'expected_configuration_revision':'1','estimate':None,'values':values},
                'result':saved, 'readable_id':readable}
        slots.append({'slot':slot,'project_id':'project','item_id':item,'field_ids':fields,
                      'seed':copy.deepcopy(seed),'expected_final_version':1,'final_result':saved,'unresolved':False})
        known[slot] = copy.deepcopy(saved)
    phases = []
    global_ordinal = 0
    for phase, count in [('warmup',10), ('measured',100), ('control',1)]:
        attempts = []
        for ordinal in range(count):
            saved = known[0]
            version = int(saved['value']['item']['version'])
            edit = phase == 'control' or cell.mix == 'edit' or (cell.mix == 'mixed' and ordinal%2==1)
            body = {'project_id':'project','item_id':'item-0'}
            canonical = None
            if edit:
                token = (1000 if phase=='warmup' else 2000 if phase=='measured' else 12000)+ordinal
                body.update(operation_id=format(token,'032x'),expected_item_version=str(version),
                            expected_configuration_revision='1',estimate={'action':'keep'},
                            fields=[{'id':field,'action':'set','value':format(global_ordinal+10,'064x')} for field in fields])
                canonical = {'codec_version':1,'kind':'item_edit_request','value':{
                    **{key:value for key,value in body.items() if key!='fields'},
                    'fields':[{'id':field['id'],'edit':{'action':'set','value':{'kind':'text','value':field['value']}}} for field in body['fields']]}}
                changed = copy.deepcopy(saved)
                changed['kind'] = 'item_edit_result'
                changed['value'].pop('readable_id',None)
                changed['value']['item']['version'] = str(version+1)
                changed['value']['project']['next_sequence'] = '17'
                changed['value']['data']['values'] = [{'id':field['id'],'value':{'kind':'text','value':field['value']}} for field in body['fields']]
                known[0] = changed
                response = {'outcome':'committed','result':changed}
            else:
                response = {'outcome':'read','result':saved,'readable_id':slots[0]['seed']['readable_id']}
            raw = encoded(response)
            attempts.append({'phase':phase,'ordinal':ordinal,'slot':0,'session':ordinal%4,'edit':edit,
                'project':'project','item':'item-0','expected_item_version':version,'request':body,
                'canonical_base64':base64.b64encode(encoded(canonical)).decode() if canonical else None,
                'request_json_bytes':len(encoded(body)),'response_body_bytes':len(raw),
                'response_base64':base64.b64encode(raw).decode(),'status':200,'latency_ns':10_000,
                'outcome':'success','transport_error':None,'integrity_error':None,'uncertain':False})
            global_ordinal += 1
        phases.append({'phase':phase,'configured_concurrency':1,'planned_requests':count,
                       'request_window_ns':count*100_000,'checkpoint_ns':1000,'attempts':attempts,
                       'recoveries':[],'rows':None,'sampling':None,'errors':[]})
    slots[0]['expected_final_version'] = int(known[0]['value']['item']['version'])
    slots[0]['final_result'] = known[0]
    work = {'schema':1,'plan':cell.document(),'observer':'off','valid':True,'phases':phases,'slots':slots,
            'setup':{'elapsed_ns':25_000_000_000,'authentication_ns':24_100_000_000,
                     'authentication_pacing_ns':24_000_000_000,
                     'projects':[{'project_id':'project','configuration_json_bytes':1000}],
                     'text_generation':'synthetic ASCII fixture'},
            'postgres':{'server_version':'16.test','max_connections':'8','ssl':'on',
                        'database_bytes_after':1024,'wal_bytes_after':16384},
            'latency_boundary':results.LATENCY_BOUNDARY,'throughput_boundary':results.THROUGHPUT_BOUNDARY,
            'tls_policy':'new verified TLS connection per request','stage_population':'aggregate'}
    owned = {'schema':1,'runtime_joined':True,'connection_accounting_valid':True,
             'expected_connections':152,'total_ns':30_000_000_000,
             'connections':{'accepted':152,'completed':152,'capacity_rejected':0,'protocol_rejected':0,
                            'timed_out':0,'tls_rejected':0,'http_rejected':0,
                            'tls_shutdown_rejected':0,'peer_close_rejected':0}}
    return cell, work, owned


class CapacityResultTests(unittest.TestCase):
    def test_known_blocker_control_requires_actual_sampled_transaction_edge(self):
        def control(target, xid, dispatch):
            fingerprint = ' FROM kehila_auth.' + ('fence(' if target=='actor' else 'save_item_edit(')
            return {'target':target,'target_key':'owner' if target=='actor' else 'project','target_ctid':'(0,1)',
                    'query_fingerprint':fingerprint,'known_blocker_observed':True,'edge_gone':True,'blocker_idle':True,
                    'blocker_pid':2,'blocker_transaction_id':xid,'blocker_backend_start':'start',
                    'blocker_transaction_start':'transaction','dispatch_ns':dispatch,'release_ns':dispatch+3,
                    'response_ns':dispatch+4,'sample_count':1,'after_release':{'backends':[]}}
        controls = [control('actor','3',0),control('project','4',10)]
        samples = [{'started_ns':c['dispatch_ns']+1,'finished_ns':c['dispatch_ns']+2,'backends':[
            {'pid':1,'state':'active','wait_type':'Lock','wait_event':'transactionid','blocking_pids':[2],
             'query':'SELECT *'+c['query_fingerprint']+'...)','backend_start':'waiter-start',
             'transaction_start':'waiter-transaction','ungranted_locks_json':json.dumps([
                 {'locktype':'transactionid','transaction_id':c['blocker_transaction_id']}])}]} for c in controls]
        phase = {'controls':controls,'attempts':[{'outcome':'success','edit':True}]*2,
                 'recoveries':[],'sampling':{'samples':samples}}
        results.validate_calibration(phase,'project')
        for mutation in ('xid','edge','query','release'):
            changed=copy.deepcopy(phase)
            backend=changed['sampling']['samples'][0]['backends'][0]
            if mutation=='xid': backend['ungranted_locks_json']='[]'
            elif mutation=='edge': backend['blocking_pids']=[]
            elif mutation=='query': backend['query']='unrelated'
            else: changed['controls'][0]['release_ns']=0
            with self.subTest(mutation=mutation),self.assertRaises(ValueError):
                results.validate_calibration(changed,'project')

    def test_stage_distributions_keep_overlapping_wall_and_thread_cpu_separate(self):
        event = lambda stage, wall, cpu: {'stage':stage, 'offset_ns':0, 'duration_ns':wall,
                                         'cpu_time_ns':cpu, 'completed':True}
        phase = {'rows':[{'id':0, 'phase_offset_ns':1, 'events':[
            event('Execution',100,None), event('CpuLane',80,None), event('CpuWork',50,30)]}],
                 'sampling':{'interval_ns':5_000_000,'sample_limit':10,'samples':[
                     {'started_ns':0,'finished_ns':12,'backends':[
                         {'pid':42,'state':'active','wait_type':'Lock','wait_event':'transactionid','blocking_pids':[43]}]}]}}
        summary = results.observation_summary(phase)
        self.assertEqual(summary['stages']['CpuWork']['thread_cpu_ns']['p95'],30)
        self.assertEqual(summary['stages']['Execution']['wall_ns']['p95'],100)
        self.assertEqual(summary['stages']['Execution']['thread_cpu_ns']['count'],0)
        self.assertEqual(summary['sampling']['backend_observations'][0]['count'],1)
        phase['rows'][0]['events'][0]['cpu_time_ns']=20
        with self.assertRaises(ValueError): results.observation_summary(phase)

    def test_complete_read_cell_keeps_control_separate_from_measured_population(self):
        cell, work, owned = fixture()
        validated = results.validate(work,owned,cell,'off')
        self.assertEqual(validated.summary['measured']['attempted'],100)
        self.assertEqual(validated.summary['measured']['successful'],100)
        self.assertEqual(validated.summary['measured']['latency_ns']['p95'],10_000)
        self.assertEqual(sum(row['type']=='seed' for row in validated.records),16)
        writes = [row for row in validated.records if row['type']=='attempt' and row['edit']]
        self.assertEqual(len(writes),1)
        self.assertEqual(writes[0]['phase'],'control')
        self.assertIs(writes[0]['accepted'],True)
        self.assertEqual(base64.b64decode(writes[0]['request_base64']),
                         base64.b64decode(work['phases'][2]['attempts'][0]['canonical_base64']))

    def test_failure_labels_and_lost_edit_reconciliation_are_independent_requirements(self):
        for mutation in ('label', 'lost_edit'):
            cell, work, owned = fixture(15 if mutation == 'lost_edit' else 0)
            attempt = work['phases'][0]['attempts'][-1]
            if mutation == 'label':
                raw = encoded({'error': 'busy'})
                attempt.update(status=429, outcome='unavailable', response_body_bytes=len(raw),
                               response_base64=base64.b64encode(raw).decode())
            else:
                attempt.update(status=None, outcome='transport_failure', uncertain=False,
                               transport_error='connection lost', response_base64=None, response_body_bytes=0)
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                results.validate(work, owned, cell, 'off')

    def test_wrong_plan_population_session_or_final_version_cannot_be_promoted(self):
        for mutation in ('plan','population','session','version'):
            cell, work, owned = fixture()
            if mutation=='plan': work['plan']['concurrency']=True
            elif mutation=='population': work['phases'][1]['attempts'].pop()
            elif mutation=='session': work['phases'][1]['attempts'][1]['session']=0
            else: work['slots'][0]['expected_final_version']=1
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                results.validate(work,owned,cell,'off')

    def test_canonical_identity_and_accepted_values_must_match_original_request(self):
        for mutation in ('identity','value'):
            cell, work, owned = fixture()
            attempt = work['phases'][2]['attempts'][0]
            if mutation=='identity':
                canonical=json.loads(base64.b64decode(attempt['canonical_base64']))
                canonical['value']['operation_id']='f'*32
                attempt['canonical_base64']=base64.b64encode(encoded(canonical)).decode()
            else:
                response=json.loads(base64.b64decode(attempt['response_base64']))
                response['result']['value']['data']['values'][0]['value']['value']='f'*64
                raw=encoded(response)
                attempt['response_base64']=base64.b64encode(raw).decode()
                attempt['response_body_bytes']=len(raw)
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                results.validate(work,owned,cell,'off')

    def test_uncertain_original_is_resolved_by_its_exact_identity_without_becoming_a_success_sample(self):
        cell, work, owned = fixture(15)
        original = work['phases'][0]['attempts'][-1]
        recovery = copy.deepcopy(original)
        recovery['phase']='recovery'
        work['phases'][0]['recoveries'].append(recovery)
        original.update(status=503,outcome='uncertain',uncertain=True)
        raw=encoded({'error':'outcome_unknown'})
        original.update(response_base64=base64.b64encode(raw).decode(),response_body_bytes=len(raw))
        owned['expected_connections']+=1
        owned['connections']['accepted']+=1
        owned['connections']['completed']+=1
        validated=results.validate(work,owned,cell,'off')
        self.assertEqual(validated.summary['warmup']['successful'],9)
        self.assertEqual(validated.summary['warmup']['recoveries'],1)
        writes=[row for row in validated.records if row['type']=='attempt' and row['phase']=='warmup']
        self.assertEqual(sum(row['accepted'] for row in writes),10)
        recovery['session']=(recovery['session']+1)%4
        with self.assertRaises(ValueError): results.validate(work,owned,cell,'off')

    def test_missing_ownership_and_invalid_phase_are_rejected(self):
        for mutation in ('joined','accounted','valid','phase'):
            cell,work,owned=fixture()
            if mutation=='joined': owned['runtime_joined']=False
            elif mutation=='accounted': owned['connection_accounting_valid']=False
            elif mutation=='valid': work['valid']=False
            else: work['phases'][1]['errors']=['trace invalid']
            with self.subTest(mutation=mutation),self.assertRaises(ValueError):
                results.validate(work,owned,cell,'off')


if __name__ == '__main__': unittest.main()
