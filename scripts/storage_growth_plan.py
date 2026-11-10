"""Finite synthetic storage samples, separate from a projected product workload.

Every paired sample has two isolated roles: one native item and one separately
constructed historical item, each with twenty serial edits. Physical results
remain separate by role; their counts share the finite campaign budget.
Near-boundary text length must be calibrated against actual admitted envelopes;
no fixed field-byte count is assumed to be a valid near-boundary record.
"""
from dataclasses import dataclass
from itertools import product
import json
from pathlib import Path

# Also compiled into the Rust fixture. Row positions are retained evidence IDs;
# changing this matrix requires reviewing workload and resource admission again.
_DIMENSIONS = tuple(tuple(row) for row in json.loads(
    Path(__file__).with_name('storage_growth_samples.json').read_text(encoding='utf-8')))
if len(_DIMENSIONS) != 24 or set(_DIMENSIONS) != set(product(
        ('compact', 'large', 'near_boundary'), ('repeated', 'sha256_hex'),
        ('one_field', 'all_fields'), (1, 2))):
    raise ValueError('Storage matrix differs from the finite workload dimensions')


@dataclass(frozen=True)
class Sample:
    index: int
    profile: str
    entropy: str
    edit: str
    repetition: int
    items: int = 2
    edits_per_item: int = 20

    def __post_init__(self):
        if (type(self.index) is not int or not 0 <= self.index < len(_DIMENSIONS)
                or any(type(value) is not str for value in (self.profile, self.entropy, self.edit))
                or type(self.repetition) is not int
                or (self.profile, self.entropy, self.edit, self.repetition) != _DIMENSIONS[self.index]
                or type(self.items) is not int or self.items != 2
                or type(self.edits_per_item) is not int or self.edits_per_item != 20):
            raise ValueError('Storage sample differs from the finite workload plan')

    @property
    def revisions(self):
        return self.items * (1 + self.edits_per_item)

    @property
    def fields(self):
        return 4 if self.profile == 'compact' else 128

    @property
    def text_bytes(self):
        return {'compact': 64, 'large': 1024, 'near_boundary': None}[self.profile]

    def document(self):
        return {'schema': 1, 'index': self.index, 'profile': self.profile,
                'entropy': self.entropy, 'edit': self.edit, 'repetition': self.repetition,
                'items': self.items, 'edits_per_item': self.edits_per_item,
                'roles': ['native', 'historical'], 'items_per_role': 1,
                'revisions_per_role': 21,
                'maximum_item_revisions': self.revisions, 'fields': self.fields,
                'text_bytes': self.text_bytes,
                'minimum_history_bytes': 943_719 if self.profile == 'near_boundary' else None,
                'maximum_envelope_bytes': 1_048_576,
                'checkpoints_edits_per_item': [0, 1, 5, 10, 20],
                'synthetic': True}


def samples():
    return tuple(Sample(index, *dimensions) for index, dimensions in enumerate(_DIMENSIONS))


def select(indices):
    if (not isinstance(indices, (list, tuple)) or not indices
            or any(type(index) is not int or not 0 <= index < len(_DIMENSIONS)
                   for index in indices)
            or len(set(indices)) != len(indices)):
        raise ValueError('Select explicit unique storage sample indices from 0 through 23')
    return tuple(Sample(index, *_DIMENSIONS[index]) for index in sorted(indices))
