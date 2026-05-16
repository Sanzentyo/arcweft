#!/usr/bin/env python3
from __future__ import annotations
import argparse, json, hashlib, urllib.request
from pathlib import Path

START = "emit_smt <<SMT\n"
END = "\nSMT\n}"

def update_block(text: str, smt: str) -> str:
    a = text.index(START) + len(START)
    b = text.index(END, a)
    return text[:a] + smt.rstrip() + text[b:]

def main() -> None:
    ap = argparse.ArgumentParser(description="Vendor exact OxiZ .smt2 sources into Arcweft fixtures.")
    ap.add_argument("--root", default=".", help="arcweft repository root")
    ap.add_argument("--update-awft", action="store_true", help="also rewrite emit_smt blocks in .awft fixtures")
    args = ap.parse_args()
    root = Path(args.root) / "verify" / "oxiz_z3_parity"
    manifest_path = root / "MANIFEST.json"
    data = json.loads(manifest_path.read_text())
    for entry in data["entries"]:
        raw = f"https://raw.githubusercontent.com/{entry['source_repo']}/{entry['source_commit']}/{entry['source_path']}"
        print(f"fetch {entry['id']} <- {raw}")
        with urllib.request.urlopen(raw, timeout=30) as r:
            smt = r.read().decode("utf-8")
        out = root / "expected_smt2" / entry["logic"] / entry["benchmark"]
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(smt, encoding="utf-8")
        entry["expected_smt_sha256"] = hashlib.sha256(smt.encode()).hexdigest()
        if args.update_awft:
            awft = root / "awft" / entry["logic"] / (entry["benchmark"][:-5] + ".awft")
            awft.write_text(update_block(awft.read_text(), smt), encoding="utf-8")
    manifest_path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    with (root / "MANIFEST.tsv").open("w", encoding="utf-8") as f:
        f.write("ordinal\tid\tlogic\tbenchmark\texpected\toxiz_recorded\tsource_path\tawft_path\texpected_smt_path\texpected_smt_sha256\n")
        for e in data["entries"]:
            stem = e["benchmark"][:-5]
            f.write(f"{e['ordinal']}\t{e['id']}\t{e['logic']}\t{e['benchmark']}\t{e['expected']}\t{e['oxiz_recorded']}\t{e['source_path']}\tawft/{e['logic']}/{stem}.awft\texpected_smt2/{e['logic']}/{e['benchmark']}\t{e['expected_smt_sha256']}\n")

if __name__ == "__main__":
    main()
