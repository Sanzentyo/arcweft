#!/usr/bin/env python3
import csv, hashlib, io, json, pathlib, sys, zipfile
ZERO='0'*64
ALLOWED={'.md','.csv','.json','.txt','.py'}
FORBIDDEN={'.rs','.toml','.lock','.patch','.diff','.rej','.orig','.exe','.dll','.so','.dylib'}
EXPECTED_COMMIT='a38c736ba577172b1f4c3fe1a0c3e85443e97e6f'

def digest(b): return hashlib.sha256(b).hexdigest()

def fail(msg):
    print('FAIL:',msg)
    raise SystemExit(1)

def main(path):
    p=pathlib.Path(path)
    with zipfile.ZipFile(p,'r') as z:
        bad=z.testzip()
        if bad: fail('CRC '+bad)
        names=z.namelist()
        if names != sorted(names): fail('members not sorted')
        if len(names)!=len(set(names)): fail('duplicate member')
        if any(n.endswith('/') for n in names): fail('directory member')
        for info in z.infolist():
            if info.date_time != (1980,1,1,0,0,0): fail('timestamp '+info.filename)
            if ((info.external_attr>>16)&0o777) != 0o644: fail('mode '+info.filename)
            ext=pathlib.PurePosixPath(info.filename).suffix.lower()
            if ext in FORBIDDEN or ext not in ALLOWED: fail('extension '+info.filename)
        data={n:z.read(n) for n in names}
    if data.get('OPEN_QUESTIONS.md') != b'none\n': fail('OPEN_QUESTIONS bytes')
    if EXPECTED_COMMIT.encode() not in data['FINAL_STATUS.md']: fail('commit')
    if b'OPEN_QUESTIONS=0' not in data['FINAL_STATUS.md']: fail('open status')
    if b'PRODUCTION_OVERLAY_INCLUDED=0' not in data['FINAL_STATUS.md']: fail('overlay status')
    manifest=data['MANIFEST.txt'].decode().splitlines()
    if [line.split('  ',1)[1] for line in manifest] != names: fail('manifest names')
    for line in manifest:
        h,n=line.split('  ',1)
        expected=ZERO if n=='MANIFEST.txt' else digest(data[n])
        if h != expected: fail('manifest hash '+n)
    rows=list(csv.DictReader(io.StringIO(data['TEST_MATRIX.csv'].decode())))
    ids=[r['id'] for r in rows]
    if len(ids)!=len(set(ids)): fail('duplicate test IDs')
    if len(rows)<100: fail('test coverage too small')
    contract=json.loads(data['contract.json'])
    if contract['open_questions'] != 0 or len(contract['decisions']) != 10: fail('decisions')
    tags=contract['tags']; versions=contract['versions']
    if tags != {'admission_exact':0,'admission_producer_wide':1,'awbc_constant_opaque':18,'awbc_runtime_type_opaque':23,'runtime_value_opaque':16}: fail('tags')
    if versions['awbc_codec'] != {'from':10,'to':11}: fail('codec')
    if versions['awbc_abi'] != {'from':1,'to':1}: fail('abi')
    if versions['session_save'] != {'from':2,'to':3}: fail('save')
    print('PASS')
    print('MEMBERS='+str(len(names)))
    print('TEST_ROWS='+str(len(rows)))
    print('ZIP_SHA256='+digest(p.read_bytes()))

if __name__=='__main__':
    if len(sys.argv)!=2: raise SystemExit('usage: validate_package.py archive.zip')
    main(sys.argv[1])
