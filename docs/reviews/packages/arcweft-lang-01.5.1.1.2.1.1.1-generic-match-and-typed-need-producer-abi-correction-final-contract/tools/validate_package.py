#!/usr/bin/env python3
from __future__ import annotations

import csv
import hashlib
import json
import sys
from collections import Counter
from pathlib import Path

REQUIRED = {
    'README.md','INPUT_REQUEST.md','OPEN_QUESTIONS.md','FINAL_CONTRACT.md','DECISION_REGISTER.md',
    'RUST_SCHEMAS.md','GENERIC_CHECKED_MATCH.md','SELECTOR_RESULT_ABI.md','GUARD_EXECUTION.md',
    'TYPED_NEED_PRODUCER_ABI.md','BUNDLE_CROSS_SECTION.md','WIRE_TYPE_DIGEST_ALLOCATION.md',
    'PERSISTENCE_REPLAY_REPLACEMENT.md','RESOURCE_REGISTRY_INPUT.md','OWNERS_AND_APIS.md',
    'DEPENDENCY_GRAPH.md','FAILURE_PRECEDENCE_AND_ATOMICITY.md','COMPILE_CLEAN_SEQUENCE.md',
    'CURRENT_SOURCE_EVIDENCE.md','SOURCE_EVIDENCE.csv','REQUIREMENT_TRACEABILITY.md',
    'REQUIREMENT_TRACEABILITY.csv','PRODUCER_CONSUMER_MATRIX.md','PRODUCER_CONSUMER_MATRIX.csv',
    'DELETION_MATRIX.md','DELETION_MATRIX.csv','TEST_MATRIX.md','TEST_MATRIX.csv',
    'STRUCTURAL_ABSENCE.md','VERIFICATION_SCOPE.md','VALIDATION.md','VALIDATION.json',
    'tools/validate_package.py','MANIFEST.json','MANIFEST.sha256','SHA256SUMS'
}
EXPECTED_SHA = '4bda1cdcea63fdf7aac32691d756c1c0e1fc693e'


def fail(message: str) -> None:
    raise SystemExit(f'VALIDATION FAILED: {message}')


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open('rb') as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()


def rows(root: Path, name: str) -> list[dict[str, str]]:
    with (root / name).open(encoding='utf-8', newline='') as f:
        return list(csv.DictReader(f))


def main() -> None:
    root = Path(sys.argv[1] if len(sys.argv) > 1 else '.').resolve()
    actual = {p.relative_to(root).as_posix() for p in root.rglob('*') if p.is_file()}
    missing = REQUIRED - actual
    if missing:
        fail(f'missing required files: {sorted(missing)}')
    if (root / 'OPEN_QUESTIONS.md').read_bytes() != b'none\n':
        fail('OPEN_QUESTIONS.md must be exactly "none\\n"')

    meta = json.loads((root / 'VALIDATION.json').read_text(encoding='utf-8'))
    if meta.get('schema_version') != 1 or meta.get('status') != 'pass':
        fail('VALIDATION.json schema/status')
    if meta.get('current_git_sha') != EXPECTED_SHA:
        fail('current Git SHA mismatch')
    if meta.get('readiness', {}).get('exact_decisions_closed') != 17:
        fail('exact decision closure count')
    if meta.get('readiness', {}).get('open_questions') != 'none':
        fail('open questions metadata')
    if meta.get('version_markers', {}).get('arcweft_owned') != [1]:
        fail('Arcweft version marker set must be [1]')

    source = rows(root, 'SOURCE_EVIDENCE.csv')
    trace = rows(root, 'REQUIREMENT_TRACEABILITY.csv')
    pc = rows(root, 'PRODUCER_CONSUMER_MATRIX.csv')
    deletion = rows(root, 'DELETION_MATRIX.csv')
    tests = rows(root, 'TEST_MATRIX.csv')
    if len(source) < 40: fail('source evidence matrix undersized')
    if len(trace) != 17: fail('traceability must contain exactly D01-D17')
    if [r['id'] for r in trace] != [f'D{i:02d}' for i in range(1,18)]: fail('traceability IDs/order')
    if len(pc) < 35: fail('producer/consumer matrix undersized')
    if len(deletion) < 25: fail('deletion matrix undersized')
    if len(tests) < 120: fail('test matrix undersized')
    classes = Counter(r['test_class'] for r in tests)
    for required in ['positive','negative','tamper','exact-limit','one-over','rollback','differential','structural','tier-2']:
        if classes[required] == 0:
            fail(f'missing test class {required}')

    schemas = (root / 'RUST_SCHEMAS.md').read_text(encoding='utf-8')
    for forbidden in ['RuntimeTypeRef', 'RuntimeBindingOwnership', 'arm_expression', 'NeedInvocationIdentity', 'scrutinee_type: TypeKind', 'result_type: TypeKind', 'CheckedMatchArmCoverage', 'TBD', 'FIXME', 'TO BE DECIDED']:
        if forbidden in schemas:
            fail(f'unresolved/forbidden normative schema token {forbidden}')
    required_schema = [
        'CheckedExpressionResolution', 'Match(Box<CheckedMatch>)', 'ViewMatchSiteId',
        'NeedHandle(RuntimeNeedHandle)', 'NeedHandle { payload: AwbcTypeId }',
        'MakeNeedHandle', 'ViewReactiveBindingSectionV1', 'DecodedViewMatchSelection',
        'LocalInstallTransaction', 'VerifiedNeedHandle', 'FinalSemanticCatalogs',
        'CheckedMatchRef', 'RuntimeViewMatchSelectorSeed', 'RuntimeCheckedType',
        'Need(Box<RuntimeCheckedType>)', 'ViewReactiveSourceMapEntryV1',
        'input_state_type_digest',
    ]
    for token in required_schema:
        if token not in schemas:
            fail(f'missing exact schema token {token}')

    combined = '\n'.join((root / name).read_text(encoding='utf-8') for name in [
        'FINAL_CONTRACT.md','SELECTOR_RESULT_ABI.md','GUARD_EXECUTION.md',
        'TYPED_NEED_PRODUCER_ABI.md','RESOURCE_REGISTRY_INPUT.md',
        'COMPILE_CLEAN_SEQUENCE.md','STRUCTURAL_ABSENCE.md'
    ])
    for token in [
        'synthetic nominal', 'Variant', 'Tuple([])', 'frame', 'Branch',
        'RuntimeValue::NeedHandle', 'String', '0x1e', 'bit 4',
        '(active GenerationId, NeedId)', 'ResourceTypeRegistry', 'strict version-1',
        'atomic', 'no second endpoint table'
    ]:
        if token not in combined:
            fail(f'missing normative decision phrase {token}')

    manifest = json.loads((root / 'MANIFEST.json').read_text(encoding='utf-8'))
    if manifest.get('schema_version') != 1 or manifest.get('algorithm') != 'sha256':
        fail('manifest schema/algorithm')
    entries = manifest.get('files')
    if not isinstance(entries, list) or not entries:
        fail('manifest entries')
    listed = {entry['path'] for entry in entries}
    envelope = {'MANIFEST.json','MANIFEST.sha256','SHA256SUMS'}
    if listed | envelope != actual:
        fail(f'manifest exact-set mismatch: missing={sorted(actual-(listed|envelope))}, extra={sorted((listed|envelope)-actual)}')
    if listed & envelope:
        fail('manifest must not list its envelope files')
    for entry in entries:
        p = root / entry['path']
        if p.stat().st_size != entry['bytes']:
            fail(f"size mismatch {entry['path']}")
        if sha256(p) != entry['sha256']:
            fail(f"hash mismatch {entry['path']}")
    manifest_digest = sha256(root / 'MANIFEST.json')
    if (root / 'MANIFEST.sha256').read_text(encoding='utf-8').strip() != manifest_digest:
        fail('MANIFEST.sha256 mismatch')
    sums = {}
    for line in (root / 'SHA256SUMS').read_text(encoding='utf-8').splitlines():
        digest, path = line.split('  ', 1)
        sums[path] = digest
    if sums != {e['path']: e['sha256'] for e in entries}:
        fail('SHA256SUMS mismatch')

    print(f'VALIDATION PASSED: {root.name}; files={len(actual)}; tests={len(tests)}; evidence={len(source)}')

if __name__ == '__main__':
    main()
