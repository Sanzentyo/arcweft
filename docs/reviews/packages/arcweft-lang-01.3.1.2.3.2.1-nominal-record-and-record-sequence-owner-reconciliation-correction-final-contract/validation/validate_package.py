from __future__ import annotations

import argparse
import csv
import hashlib
import json
from pathlib import Path
from zipfile import ZipFile

BASE = 'arcweft-lang-01.3.1.2.3.2.1-nominal-record-and-record-sequence-owner-reconciliation-correction-final-contract'
REQUIRED = {
    "README.md", "FINAL_CONTRACT.md", "RUST_OWNERS_AND_APIS.md",
    "NOMINAL_LAYOUT_AND_PROJECTION.md", "ERROR_AND_PRECEDENCE.md",
    "VISITOR_AND_CARRIER_CONTRACT.md", "IMPLEMENTATION_ORDER.md",
    "PRODUCER_CONSUMER_DELETION_INVENTORY.md",
    "PRODUCER_CONSUMER_DELETION_INVENTORY.csv", "TEST_MATRIX.csv",
    "TEST_MATRIX.md", "COMPILE_FAIL_MATRIX.md", "TRAIT_CODEC_AND_PERSISTENCE.md",
    "DEPENDENCY_AND_SHARING.md", "SUPERSESSION_DELTA.md", "DECISION_REGISTER.md",
    "SYMBOL_CLOSURE.json", "REQUIREMENTS_TRACEABILITY.md",
    "REPOSITORY_EVIDENCE.md", "APPLICABLE_INSTRUCTIONS.md", "VALIDATION.md",
    "FINAL_STATUS.md", "OPEN_QUESTIONS.md", "PARENT_ARTIFACTS.sha256",
    "SOURCE_INPUTS.sha256", "sources/SOURCE_REQUEST.md",
    "sources/IMPLEMENTATION_EVIDENCE.md", "sources/ROOT_AGENTS_AT_TARGET.md",
    "sources/SUPPLIED_RUST_SKILL.txt", "sources/SUPPLIED_ARCWEFT_PREMISE.txt",
    "sources/GIT_CLONE_ATTEMPT.txt", "contract.json", "MANIFEST.txt",
    "validation/reference_model.py", "validation/test_reference_model.py",
    "validation/validate_package.py", "validation/build_zip.py",
    "validation/reference-model-output.txt", "validation/package-validation-output.txt",
}


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    p=argparse.ArgumentParser()
    p.add_argument("package", type=Path)
    p.add_argument("--zip", dest="archive", type=Path)
    a=p.parse_args()
    root=a.package.resolve()
    assert root.name == BASE, root
    files={x.relative_to(root).as_posix() for x in root.rglob("*") if x.is_file()}
    missing=REQUIRED-files
    assert not missing, sorted(missing)
    assert (root/"OPEN_QUESTIONS.md").read_text() == "none\n"

    status=(root/"FINAL_STATUS.md").read_text()
    for token in [
        "STATUS=READY_FOR_IMPLEMENTATION", "OPEN_QUESTIONS=0", "PRODUCTION_CHANGES=0",
        "NOMINAL_LAYOUT_OWNER=RuntimeNominalRecordLayout",
        "SEQUENCE_ERROR_OWNER=RuntimeSeqError", "LOCAL_EXACT_COMMIT_CHECKOUT=NO",
        "AWBC_ABI_VERSION=1", "AWBC_CODEC_VERSION=8",
    ]:
        assert token in status, token

    normative="\n".join((root/name).read_text() for name in [
        "FINAL_CONTRACT.md", "RUST_OWNERS_AND_APIS.md", "NOMINAL_LAYOUT_AND_PROJECTION.md",
        "ERROR_AND_PRECEDENCE.md", "VISITOR_AND_CARRIER_CONTRACT.md", "IMPLEMENTATION_ORDER.md",
    ])
    for token in [
        "RuntimeNominalRecordLayout", "RuntimeSeqError", "try_from_accepted_layout",
        "validate_against_layout", "RuntimeExpr::NominalRecord", "TypeLayoutHash",
        "RuntimeRecordFieldId", "NominalRecordField", "RecordColumn", "RecordField",
    ]:
        assert token in normative, token
    # Absent names may occur only to state their prohibition/supersession, never as a declaration/signature.
    api=(root/"RUST_OWNERS_AND_APIS.md").read_text()
    assert "pub struct RuntimeNominalRecordSchema" not in api
    assert "type RuntimeNominalRecordSchema" not in api
    assert "pub enum RecordSeqError" not in api
    assert "Result<Self, RecordSeqError>" not in api

    closure=json.loads((root/"SYMBOL_CLOSURE.json").read_text())
    assert closure["open_questions"] == 0
    assert closure["forbidden_absent_symbols"] == ["RuntimeNominalRecordSchema", "RecordSeqError"]
    contract=json.loads((root/"contract.json").read_text())
    assert contract["status"] == "READY_FOR_IMPLEMENTATION"
    assert contract["open_questions"] == 0

    with (root/"TEST_MATRIX.csv").open(newline="") as f:
        rows=list(csv.DictReader(f))
    assert len(rows) >= 70, len(rows)
    kinds={r["kind"] for r in rows}
    assert {"positive","negative","boundary","precedence","tamper","structural","compile_fail","full_gate","golden","parity"} <= kinds
    ids={r["id"] for r in rows}
    assert len(ids)==len(rows)
    for required in ["NREC-015","NREC-036","NREC-053","NREC-059","NREC-069","NREC-078"]:
        assert required in ids

    with (root/"PRODUCER_CONSUMER_DELETION_INVENTORY.csv").open(newline="") as f:
        inv=list(csv.DictReader(f))
    assert len(inv) >= 40, len(inv)
    assert len({r["id"] for r in inv})==len(inv)

    parents=(root/"PARENT_ARTIFACTS.sha256").read_text()
    for h in ['e95de2a9958000034a48f8c5228c8a4ff17f62226195cce4c0ef93e398c816e4', 'a52453fd07fdacf10205cbf621077f923ded714b83e4c64b9b69c52a7350ff7f', 'd053fae201afa104f7db9914aebbc08f2456875d1229f5325f86235d4bc0ea94']: assert h in parents
    inputs=(root/"SOURCE_INPUTS.sha256").read_text()
    assert '37f493cca3a98e191a482c651db16a32ebbf821eafe8ce129e852340f3f5f6a1' in inputs and 'b6cc3d636884365645ba3dc7a47817bf4158c4dcd6ec5178c0214cb08354398f' in inputs
    assert sha(root/"sources/SOURCE_REQUEST.md") == '37f493cca3a98e191a482c651db16a32ebbf821eafe8ce129e852340f3f5f6a1'
    assert sha(root/"sources/IMPLEMENTATION_EVIDENCE.md") == 'b6cc3d636884365645ba3dc7a47817bf4158c4dcd6ec5178c0214cb08354398f'
    assert sha(root/"sources/ROOT_AGENTS_AT_TARGET.md") == '90bae8bface6d390246538c60842da7d71d1ebd576ae3fa403019caa35a91498'
    assert sha(root/"sources/SUPPLIED_RUST_SKILL.txt") == '1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665'
    assert sha(root/"sources/SUPPLIED_ARCWEFT_PREMISE.txt") == 'cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1'
    assert sha(root/"sources/GIT_CLONE_ATTEMPT.txt") == '375e2c2b558282fe2caea22edce8f362c37d8d221532fb9fe3ff460f28731b4a'

    # No production overlay-like members.
    forbidden_suffixes=(".patch", ".diff", ".rej")
    assert not [x for x in files if x.endswith(forbidden_suffixes)]
    assert not [x for x in files if x.startswith("crates/") or x.startswith("production/")]

    manifest=(root/"MANIFEST.txt").read_text().splitlines()
    manifest_map={}
    for line in manifest:
        if not line: continue
        h, rel=line.split("  ",1)
        manifest_map[rel]=h
    assert set(manifest_map)==files
    for rel,h in manifest_map.items():
        if rel=="MANIFEST.txt":
            assert h=="0"*64
        else:
            assert sha(root/rel)==h, rel

    zip_state="NOT_REQUESTED"
    if a.archive:
        with ZipFile(a.archive) as z:
            assert z.testzip() is None
            names=z.namelist()
            assert all(not n.startswith("/") and ".." not in Path(n).parts for n in names)
            file_names={n for n in names if not n.endswith("/")}
            expected={f"{BASE}/{rel}" for rel in files}
            assert file_names==expected, (len(file_names),len(expected))
            for rel in files:
                assert z.read(f"{BASE}/{rel}") == (root/rel).read_bytes(), rel
        zip_state="PASS"

    print(json.dumps({
        "file_count": len(files), "inventory_rows": len(inv), "test_rows": len(rows),
        "decision_rows": 44, "symbol_added": len(closure["added"]),
        "manifest": "PASS", "zip": zip_state,
    }, sort_keys=True))

if __name__ == "__main__": main()
