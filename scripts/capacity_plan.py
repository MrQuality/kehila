"""Closed synthetic workload matrix; a cell index fixes every workload dimension."""
from dataclasses import dataclass
from itertools import product

_DIMENSIONS = tuple(product(('compact', 'large'), ('shared', 'independent'),
                            ('read', 'edit', 'mixed'), (1, 2, 4, 8, 16), (1, 2, 3)))


@dataclass(frozen=True)
class Cell:
    index: int
    profile: str
    layout: str
    mix: str
    concurrency: int
    repetition: int

    def __post_init__(self):
        if (type(self.index) is not int or not 0 <= self.index < len(_DIMENSIONS)
                or any(type(value) is not str for value in (self.profile, self.layout, self.mix))
                or type(self.concurrency) is not int or type(self.repetition) is not int
                or (self.profile, self.layout, self.mix, self.concurrency, self.repetition)
                != _DIMENSIONS[self.index]):
            raise ValueError('Capacity cell does not match the finite workload plan')

    def document(self):
        return {'schema': 1, 'index': self.index, 'profile': self.profile,
                'synthetic_configuration': True,
                'fields_per_item': 4 if self.profile == 'compact' else 128,
                'text_bytes_per_field': 64 if self.profile == 'compact' else 1024,
                'layout': self.layout, 'projects': 1 if self.layout == 'shared' else 4,
                'mix': self.mix, 'concurrency': self.concurrency, 'repetition': self.repetition,
                'seeded_items': 16, 'authenticated_sessions': 4,
                'warmup_requests': 10, 'measured_requests': 100}


def cell(index):
    if type(index) is not int or not 0 <= index < len(_DIMENSIONS):
        raise ValueError('Select a capacity cell index from 0 through 179')
    return Cell(index, *_DIMENSIONS[index])


def cells():
    return tuple(cell(index) for index in range(len(_DIMENSIONS)))


def select(indices):
    if not isinstance(indices, (tuple, list)) or not indices:
        raise ValueError('Select an explicit nonempty capacity cell batch')
    selected = tuple(cell(index) for index in indices)
    if len({entry.index for entry in selected}) != len(selected):
        raise ValueError('Capacity cell indices must not repeat within a batch')
    return tuple(sorted(selected, key=lambda entry: entry.index))
