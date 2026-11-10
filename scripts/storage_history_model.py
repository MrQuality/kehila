"""Finite comparison model for full checkpoints plus field deltas.

This is not an installed schema, supported codec, migration or retention change.
Inputs must already have passed the real history/identity observers. The model
compares logical encoded bytes and reconstruction work, not physical PostgreSQL
savings or production decode/restore readiness. Core request hashes, successful
identities, replay tombstones and field-use tables remain outside this model.
"""
from copy import deepcopy
import hashlib
import json


def _require(condition, message):
    if not condition:
        raise ValueError(message)


def encoded_bytes(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(',', ':'),
                      allow_nan=False).encode('utf-8')


def _digest(history):
    return hashlib.sha256(encoded_bytes(history)).hexdigest()


def _parts(history):
    metadata = deepcopy(history)
    entries = metadata['value']['result']['value']['data'].pop('values')
    _require(isinstance(entries, list) and all(isinstance(entry, dict)
             and set(entry) == {'id', 'value'} and isinstance(entry['id'], str)
             and entry['id'] for entry in entries), 'Invalid model field entries')
    order = [entry['id'] for entry in entries]
    _require(len(set(order)) == len(order), 'Duplicate model field identity')
    return metadata, order, {entry['id']: entry for entry in entries}


def encode_chain(histories):
    _require(isinstance(histories, list) and 0 < len(histories) <= 21,
             'Model requires one finite observed history chain')
    encoded = []
    previous = {}
    for version, history in enumerate(histories, 1):
        metadata, order, fields = _parts(history)
        record = {'model': 1, 'version': version, 'sha256': _digest(history)}
        if (version-1) % 10 == 0:
            record.update(kind='full', history=deepcopy(history))
        else:
            record.update(kind='delta', base_version=version-1, metadata=metadata, field_order=order,
                          updates=[fields[key] for key in order if previous.get(key) != fields[key]])
        encoded.append(record)
        previous = fields
    return encoded


def reconstruct_at(records, version):
    _require(type(version) is int and 1 <= version <= 21 and isinstance(records, list)
             and 0 < len(records) <= 21, 'Invalid model reconstruction request')
    indexed = {}
    for record in records:
        _require(isinstance(record, dict) and type(record.get('version')) is int
                 and 1 <= record['version'] <= 21 and record['version'] not in indexed,
                 'Invalid model record identity')
        indexed[record['version']] = record
    checkpoints = [number for number, record in indexed.items()
                   if number <= version and record.get('kind') == 'full']
    _require(bool(checkpoints), 'Model full checkpoint missing')
    start = max(checkpoints)
    _require(version-start < 10, 'Model reconstruction chain exceeds bound')
    history = None
    fields = {}
    for number in range(start, version+1):
        _require(number in indexed, 'Model predecessor missing')
        record = indexed[number]
        _require(type(record.get('model')) is int and record['model'] == 1,
                 'Unknown model format version')
        if number == start:
            history = deepcopy(record['history'])
        else:
            _require(record.get('kind') == 'delta' and record.get('base_version') == number-1,
                     'Model predecessor identity mismatch')
            order = record['field_order']
            updates = record['updates']
            _require(isinstance(order, list) and len(set(order)) == len(order)
                     and isinstance(updates, list) and len({entry['id'] for entry in updates}) == len(updates)
                     and all(entry['id'] in order for entry in updates), 'Invalid model field delta')
            fields.update({entry['id']: deepcopy(entry) for entry in updates})
            _require(all(key in fields for key in order), 'Model field predecessor missing')
            history = deepcopy(record['metadata'])
            data = history['value']['result']['value']['data']
            _require('values' not in data, 'Model metadata unexpectedly contains field values')
            data['values'] = [fields[key] for key in order]
        _require(_digest(history) == record.get('sha256'), 'Model reconstructed history checksum mismatch')
        _, _, fields = _parts(history)
    return history, version-start+1
