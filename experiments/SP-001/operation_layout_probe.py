"""Bounded FerretDB check for immutable task-operation storage."""
import json
import uuid

from pymongo import ASCENDING, DESCENDING, MongoClient
from pymongo.errors import DuplicateKeyError


def main():
    client = MongoClient('mongodb://ferretdb:27017', serverSelectionTimeoutMS=3000)
    name = 'operation_probe_' + uuid.uuid4().hex[:8]
    records = client.sp001[name]
    outcome = {'collection': name}
    try:
        records.create_index([('task_id', ASCENDING), ('version', ASCENDING)], unique=True)
        records.create_index([('task_id', ASCENDING), ('operation_id', ASCENDING)], unique=True)
        records.insert_one({'_id': 'a', 'task_id': 'task-1', 'version': 1,
                            'operation_id': 'op-1', 'title': 'First'})
        for label, record in [
                ('duplicate_version', {'_id': 'b', 'task_id': 'task-1', 'version': 1,
                                       'operation_id': 'op-2', 'title': 'Racer'}),
                ('duplicate_operation', {'_id': 'c', 'task_id': 'task-1', 'version': 2,
                                         'operation_id': 'op-1', 'title': 'Changed'})]:
            try:
                records.insert_one(record)
            except DuplicateKeyError:
                outcome[label] = 'rejected'
            else:
                outcome[label] = 'accepted_incorrectly'
        latest = records.find_one({'task_id': 'task-1'}, sort=[('version', DESCENDING)])
        outcome['latest_version'] = latest['version']
        outcome['indexes'] = sorted(records.index_information())
        assert outcome['duplicate_version'] == 'rejected', outcome
        assert outcome['duplicate_operation'] == 'rejected', outcome
        assert outcome['latest_version'] == 1, outcome
        print(json.dumps(outcome), flush=True)
    finally:
        records.drop()


if __name__ == '__main__':
    main()
