from __future__ import annotations

import hashlib
import sys
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo

BASE='arcweft-lang-01.3.1.2.3.2.1-nominal-record-and-record-sequence-owner-reconciliation-correction-final-contract'
FIXED=(2026,8,11,0,0,0)

def build(root: Path, output: Path) -> None:
    assert root.name==BASE
    with ZipFile(output,"w",compression=ZIP_DEFLATED,compresslevel=9) as z:
        for path in sorted((p for p in root.rglob("*") if p.is_file()), key=lambda p:p.relative_to(root).as_posix()):
            rel=path.relative_to(root).as_posix()
            info=ZipInfo(f"{BASE}/{rel}", FIXED)
            info.compress_type=ZIP_DEFLATED
            info.external_attr=(0o100644 & 0xFFFF) << 16
            info.create_system=3
            z.writestr(info,path.read_bytes(),compress_type=ZIP_DEFLATED,compresslevel=9)
    print(hashlib.sha256(output.read_bytes()).hexdigest(), output)

if __name__=="__main__": build(Path(sys.argv[1]).resolve(),Path(sys.argv[2]).resolve())
