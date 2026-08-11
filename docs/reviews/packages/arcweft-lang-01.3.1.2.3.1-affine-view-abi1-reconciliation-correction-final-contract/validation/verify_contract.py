#!/usr/bin/env python3
from __future__ import annotations
import argparse, csv, hashlib, json, subprocess, sys
from pathlib import Path
sys.dont_write_bytecode=True
REQ={
"SOURCE_REQUEST.md","PACKAGE_CONTENTS.md","README.md","FINAL_STATUS.md","OPEN_QUESTIONS.md","INPUT_IDENTITIES.md","APPLICABLE_INSTRUCTIONS.md","AUDIT_FINDINGS_AND_DECISIONS.md","FINAL_CONTRACT.md","ABI1_OWNERSHIP_WIRE.md","SNAPSHOT_ACTIVATION_AND_ALLOCATOR.md","DROP_AND_SNAPSHOT_SCHEMA_CORRECTION.md","VIEW_AFFINE_EXECUTION.md","STATIC_REQUIREMENT_AND_FRAGMENT_DISPATCH.md","RUST_SCHEMAS.md","PRODUCT_WIRE_AND_SAVE_DELTA.md","DIAGNOSTICS_AND_PRECEDENCE.md","SUPERSESSION_MATRIX.md","CONSUMER_AND_DELETION_INVENTORY.md","IMPLEMENTATION_ORDER.md","WORK_ACCOUNTING.md","NON_GOALS.md","REQUIREMENTS_TRACEABILITY.md","PRODUCER_CONSUMER_MATRIX.csv","TEST_MATRIX.csv","TEST_MATRIX.md","PARENT_TEST_MATRIX_INDEX.json","contract.json","reference_model/model.py","reference_model/test_model.py","validation/build_zip.py","validation/verify_contract.py","validation/VALIDATION_REPORT.md","validation/reference-model-test-output.txt","MANIFEST.txt"}
ZERO="0"*64

def files(root): return sorted([p for p in root.rglob("*") if p.is_file()], key=lambda p:p.relative_to(root).as_posix())
def digest(b): return hashlib.sha256(b).hexdigest()
def rows(root):
    return [(ZERO if p.name=="MANIFEST.txt" else digest(p.read_bytes()),p.relative_to(root).as_posix()) for p in files(root)]
def write_manifest(root):
    m=root/"MANIFEST.txt"; m.touch(); m.write_text("".join(f"{d}  {p}\n" for d,p in rows(root)),encoding="utf-8",newline="\n")
def main():
    ap=argparse.ArgumentParser(); ap.add_argument("--root",type=Path,required=True); ap.add_argument("--write-manifest",action="store_true"); a=ap.parse_args(); root=a.root.resolve()
    if a.write_manifest: write_manifest(root)
    found={p.relative_to(root).as_posix() for p in files(root)}; missing=REQ-found
    assert not missing, missing
    assert (root/"OPEN_QUESTIONS.md").read_bytes()==b"none\n"
    c=json.loads((root/"contract.json").read_text()); assert c["awbc"]["abi"]==1; assert c["awbc"]["codec"]==8; assert c["awbc"]["abi2_surface"] is False
    assert c["activation"]["one_active_holder_per_execution"] is True
    assert c["allocator"]["serialized_cursor"] is True
    assert c["drop"]["commit_accepts_independent_value"] is False
    assert c["view"]["retained_values"]=="unrestricted_only"
    assert c["static"]["requirement_serialized"] is True and c["static"]["fragment_dispatch"]=="outermost_wins"
    with (root/"TEST_MATRIX.csv").open(encoding="utf-8",newline="") as f: matrix=list(csv.DictReader(f))
    ids=[r["id"] for r in matrix]; assert len(ids)==len(set(ids))==c["tests"]["rows"]
    assert {"ABI","ACT","ALC","DRP","SNP","VOW","REQ","FRG","ATM","DEL","INT"} <= {r["group"] for r in matrix}
    for p in files(root):
        data=p.read_bytes(); assert b"\x00" not in data; text=data.decode("utf-8"); assert "\r" not in text; assert not data or data.endswith(b"\n")
        rel=p.relative_to(root).as_posix(); assert "__pycache__" not in rel and "/target/" not in f"/{rel}/"
    parsed=[]
    for line in (root/"MANIFEST.txt").read_text().splitlines(): parsed.append((line[:64],line[66:]))
    assert parsed==rows(root)
    out=subprocess.run([sys.executable,"-B","-m","unittest","discover","-s",str(root/"reference_model"),"-p","test_*.py","-v"],cwd=root,text=True,capture_output=True)
    assert out.returncode==0, out.stdout+out.stderr
    print(f"PASS files={len(found)} tests={len(matrix)} reference_tests={out.stderr.count(' ... ok')+out.stdout.count(' ... ok')}")
if __name__=="__main__": main()
