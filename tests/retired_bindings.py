"""Deliberate rejection data shared by retirement and branding controls."""
import json
from pathlib import Path

RETIRED = json.loads((Path(__file__).parent / 'fixtures/retired-bindings.json').read_text(
    encoding='utf-8'))
