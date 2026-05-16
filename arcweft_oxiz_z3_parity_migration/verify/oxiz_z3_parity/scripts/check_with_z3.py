#!/usr/bin/env python3
from __future__ import annotations
import argparse, json, subprocess, shutil
from pathlib import Path


def main() -> None:
    ap = argparse.ArgumentParser(description="Run z3 against expected SMT2 fixtures and compare check-sat output.")
    ap.add_argument("--root", default=".", help="arcweft repository root")
    ap.add_argument("--z3", default="z3", help="z3 executable")
    args = ap.parse_args()
    if shutil.which(args.z3) is None:
        raise SystemExit(f"z3 executable not found: {args.z3}")
    root = Path(args.root) / "verify" / "oxiz_z3_parity"
    data = json.loads((root / "MANIFEST.json").read_text())
    failures = []
    for e in data["entries"]:
        path = root / "expected_smt2" / e["logic"] / e["benchmark"]
        cp = subprocess.run([args.z3, str(path)], text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
        got = cp.stdout.strip().splitlines()[0] if cp.stdout.strip() else ""
        if got != e["expected"]:
            failures.append((e["id"], e["expected"], got, cp.stderr.strip()))
    if failures:
        for fid, exp, got, err in failures:
            print(f"FAIL {fid}: expected {exp}, got {got}; stderr={err}")
        raise SystemExit(1)
    print(f"OK: {len(data['entries'])} z3 fixture results matched")

if __name__ == "__main__":
    main()
