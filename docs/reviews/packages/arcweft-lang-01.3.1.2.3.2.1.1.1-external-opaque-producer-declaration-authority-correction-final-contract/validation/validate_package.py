#!/usr/bin/env python3
from __future__ import annotations
import argparse, csv, hashlib, io, json, pathlib, re, sys, zipfile

FORBIDDEN_SUFFIXES = {'.rs','.patch','.diff','.rej','.orig','.exe','.dll','.so','.dylib'}
FIXED_TIME = (2026, 8, 12, 0, 0, 0)
FIXED_MODE = 0o100644

def fail(msg):
    raise SystemExit('FAIL: ' + msg)

def load_source(path):
    p=pathlib.Path(path)
    if p.is_dir():
        return {str(x.relative_to(p)).replace('\\','/'):x.read_bytes() for x in sorted(p.rglob('*')) if x.is_file()}
    with zipfile.ZipFile(p) as z:
        if z.testzip() is not None: fail('CRC failure')
        names=z.namelist()
        if names != sorted(names): fail('members not sorted')
        if len(names)!=len(set(names)): fail('duplicate member')
        lowered=[n.casefold() for n in names]
        if len(lowered)!=len(set(lowered)): fail('case-colliding member')
        for i in z.infolist():
            n=i.filename
            pp=pathlib.PurePosixPath(n)
            if n.startswith('/') or '..' in pp.parts or '\\' in n or re.match(r'^[A-Za-z]:',n): fail('unsafe path '+n)
            if n.endswith('/'): fail('directory entry '+n)
            if i.date_time != FIXED_TIME: fail('timestamp '+n)
            if (i.external_attr >> 16) != FIXED_MODE: fail('mode '+n)
            if pathlib.PurePosixPath(n).suffix.lower() in FORBIDDEN_SUFFIXES: fail('forbidden extension '+n)
        return {n:z.read(n) for n in names}

def main():
    ap=argparse.ArgumentParser(); ap.add_argument('path'); ns=ap.parse_args()
    files=load_source(ns.path)
    required={
      'README.md','FINAL_CONTRACT.md','RUST_OWNERS_AND_APIS.md','SCHEMA_2_CODEC_AND_DERIVE.md',
      'ACCEPTED_CATALOG_AND_PUBLICATION.md','ERROR_AND_PRECEDENCE.md','DIGEST_AND_GENERATED_SOURCE.md',
      'PRODUCER_CONSUMER_DELETION_INVENTORY.csv','IMPLEMENTATION_ORDER.md','TEST_MATRIX.csv',
      'REQUIREMENTS_TRACEABILITY.md','REPOSITORY_EVIDENCE.md','VERIFICATION_REPORT.md',
      'FINAL_STATUS.md','PACKAGE_STATUS.txt','OPEN_QUESTIONS.md','SOURCE_REQUEST.md','contract.json','MANIFEST.txt'
    }
    missing=required-files.keys()
    if missing: fail('missing '+','.join(sorted(missing)))
    if files['OPEN_QUESTIONS.md'] != b'none\n': fail('OPEN_QUESTIONS exact bytes')
    status=files['PACKAGE_STATUS.txt'].decode()
    for token in ['STATUS=READY_FOR_IMPLEMENTATION','OPEN_QUESTIONS=0','PRODUCTION_CODE_CHANGED=NO','PRODUCTION_OVERLAY_INCLUDED=NO','PACKAGE_VALIDATION=PASS']:
        if token not in status: fail('status token '+token)
    for name,data in files.items():
        if b'\r' in data: fail('CR byte '+name)
        if pathlib.PurePosixPath(name).suffix.lower() in {'.md','.txt','.csv','.json','.toml','.py'}:
            try: data.decode('utf-8')
            except UnicodeDecodeError: fail('non-utf8 '+name)
    manifest=files['MANIFEST.txt'].decode().splitlines()
    if manifest != sorted(manifest, key=lambda x:x.split('  ',1)[1]): fail('manifest not path-sorted')
    got={}
    for line in manifest:
        m=re.fullmatch(r'([0-9a-f]{64})  ([^\\]+)',line)
        if not m: fail('bad manifest row '+line)
        h,n=m.groups(); got[n]=h
    if set(got)!=set(files): fail('manifest/file set mismatch')
    for n,b in files.items():
        expect='0'*64 if n=='MANIFEST.txt' else hashlib.sha256(b).hexdigest()
        if got[n]!=expect: fail('hash mismatch '+n)
    json.loads(files['contract.json'])
    for n,b in files.items():
        if n.endswith('.json'): json.loads(b)
        if n.endswith('.csv'):
            rows=list(csv.reader(io.StringIO(b.decode())))
            if not rows: fail('empty csv '+n)
    tests=list(csv.DictReader(io.StringIO(files['TEST_MATRIX.csv'].decode())))
    ids=[r['id'] for r in tests]
    if len(ids)<100 or len(ids)!=len(set(ids)): fail('test matrix count/IDs')
    inv=list(csv.DictReader(io.StringIO(files['PRODUCER_CONSUMER_DELETION_INVENTORY.csv'].decode())))
    if len(inv)<50: fail('inventory row count')
    req=files['SOURCE_REQUEST.md'].decode()
    for heading in ['Required exact decisions','Error precedence','Required producer and consumer inventory','Required tests','Implementation order required from the return','Expected output']:
        if heading not in req: fail('source request incomplete: '+heading)
    contract=files['contract.json'].decode()
    for token in ['AdapterOpaqueTypeProducerId','ArcweftRustOpaqueTypeProducerId','arcweft.environment-manifest.v2','arcweft.accepted-nominal-catalog.v2']:
        if token not in contract: fail('contract token '+token)
    all_text=b'\n'.join(files.values()).decode('utf-8','ignore')
    for token in ['OPEN_QUESTIONS=0','schema_version','opaque_producer','std.','ExactIdentity','ProducerWide','G3a','G3b']:
        if token not in all_text: fail('closure token '+token)
    print(f'PASS files={len(files)} tests={len(tests)} inventory={len(inv)}')

if __name__=='__main__': main()
