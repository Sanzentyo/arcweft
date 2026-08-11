#!/usr/bin/env python3
from __future__ import annotations

import csv
import hashlib
import json
import sys
from pathlib import Path

root = Path(__file__).resolve().parent
manifest_path = root / 'MANIFEST.json'
errors: list[str] = []

try:
    manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
except Exception as exc:
    raise SystemExit(f'FAIL: cannot parse MANIFEST.json: {exc}')

for entry in manifest['files']:
    path = root / entry['path']
    if not path.is_file():
        errors.append(f'missing: {entry["path"]}')
        continue
    data = path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if digest != entry['sha256']:
        errors.append(f'hash mismatch: {entry["path"]}')
    if len(data) != entry['bytes']:
        errors.append(f'size mismatch: {entry["path"]}')

contract = json.loads((root / 'contract.json').read_text(encoding='utf-8'))
checks = {
    'status final': contract.get('status') == 'final',
    'no production code change': contract.get('production_code_changed') is False,
    'no fallback success': contract.get('fallback_success_allowed') is False,
    'canonical syntax': contract.get('canonical_syntax') == 'Ref<Entity>',
    'single owner enum': contract.get('owner_model', {}).get('enum') == 'BuiltinTypeConstructor',
    'owner correction model': contract.get('owner_model', {}).get('kind') == 'corrected_closed_builtin_constructor_table',
    'no accepted semantic change': contract.get('owner_model', {}).get('accepted_nominal_semantics_changed') is False,
    'no second resolver': contract.get('owner_model', {}).get('second_resolver_allowed') is False,
    'no Named persistence encoding': contract.get('persistence', {}).get('named_encoding_allowed') is False,
}
for label, ok in checks.items():
    if not ok:
        errors.append(f'contract invariant failed: {label}')

with (root / 'TEST_MATRIX.csv').open(encoding='utf-8', newline='') as f:
    tests = list(csv.DictReader(f))
test_ids = [row['id'] for row in tests]
if len(test_ids) != len(set(test_ids)):
    errors.append('duplicate TEST_MATRIX id')
if len(tests) != contract.get('test_matrix_rows'):
    errors.append('test matrix row count differs from contract.json')
required_prefixes = {'MODEL','RES','DET','COL','SRC','POI','CON','IDX','LSP','BC','SAVE','VAL'}
actual_prefixes = {test_id.split('-', 1)[0] for test_id in test_ids}
missing_prefixes = required_prefixes - actual_prefixes
if missing_prefixes:
    errors.append(f'missing test categories: {sorted(missing_prefixes)}')

with (root / 'TRACEABILITY.csv').open(encoding='utf-8', newline='') as f:
    trace = list(csv.DictReader(f))
requirement_ids = [row['requirement_id'] for row in trace]
if len(requirement_ids) != len(set(requirement_ids)):
    errors.append('duplicate TRACEABILITY requirement')
if set(requirement_ids) != set(contract.get('requirements', [])):
    errors.append('traceability requirements differ from contract.json')

required_files = {
    'README.md','FINAL_CONTRACT.md','API_SHAPES.md','OWNERSHIP_COLLISIONS.md',
    'DIAGNOSTICS_POISON_SOURCE.md','CONSUMERS_TOOLING_PERSISTENCE.md',
    'CHANGE_SURFACE.md','IMPLEMENTATION_ORDER.md','TEST_MATRIX.csv',
    'TRACEABILITY.csv','REPOSITORY_EVIDENCE.csv','VALIDATION_EVIDENCE.md',
    'NON_GOALS.md','contract.json','REQUEST.md','VALIDATION_RESULTS.txt',
}
for filename in sorted(required_files):
    if not (root / filename).is_file():
        errors.append(f'required file missing: {filename}')

if errors:
    for error in errors:
        print(f'FAIL: {error}')
    sys.exit(1)
print(f'PASS: {len(manifest["files"])} manifest members, {len(tests)} tests, {len(trace)} requirements')
