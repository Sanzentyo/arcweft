#!/usr/bin/env python3
from __future__ import annotations
import argparse
from pathlib import Path
import zipfile

FIXED = (2026, 8, 10, 0, 0, 0)

def main() -> None:
    p = argparse.ArgumentParser(); p.add_argument("--root", type=Path, required=True); p.add_argument("--output", type=Path, required=True); a=p.parse_args()
    root=a.root.resolve(); files=sorted((x for x in root.rglob("*") if x.is_file()), key=lambda x:x.relative_to(root).as_posix())
    with zipfile.ZipFile(a.output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for f in files:
            rel=(Path(root.name)/f.relative_to(root)).as_posix(); info=zipfile.ZipInfo(rel, FIXED); info.compress_type=zipfile.ZIP_DEFLATED; info.external_attr=0o100644<<16
            z.writestr(info, f.read_bytes(), compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)
if __name__ == "__main__": main()
