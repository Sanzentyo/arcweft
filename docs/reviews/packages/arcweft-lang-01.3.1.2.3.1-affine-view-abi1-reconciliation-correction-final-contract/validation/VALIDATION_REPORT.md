# Validation report

## Results

| Class | Result | Exact scope |
|---|---|---|
| Parent ZIP transport | PASS | both supplied parent archives passed CRC/path extraction checks |
| Input identity | PASS | SHA-256 recorded for both parent ZIPs, audit report, Rust Skill, and project premise |
| Rust policy | PASS | supplied 56-line Rust Skill read completely through final line |
| Correction decision closure | PASS | ABI 1, activation scope, allocator cursor, drop typestate, snapshot traits, View ownership, static requirement, and fragment dispatch all selected |
| Open questions | PASS | `OPEN_QUESTIONS.md` exactly `none\n` |
| Machine-readable contract | PASS | ABI 1/codec 8, no ABI-2 surface, domain activation, cursor, exact drop, unrestricted View, wire requirement, outermost dispatch |
| Correction test matrix | PASS | 234 unique rows across 11 required groups |
| Reference model | PASS | 16/16 standard-library `unittest` cases |
| Text/package structure | PASS | safe relative UTF-8 LF files, final LF, no NUL/symlink/cache/VCS/build output |
| Member manifest | PASS | SHA-256 verified for every member; manifest self-row uses 64 zeroes |
| Deterministic ZIP | PASS | sorted fixed-timestamp builds are byte-identical |
| ZIP transport | PASS | final archive passes `unzip -t` |
| Production Rust/Cargo validation | NOT RUN | design-only package; no production checkout or patch |

## Commands actually executed

```text
unzip -t <affine-parent.zip>
unzip -t <view-parent.zip>
sha256sum <inputs>
python3 -B -m unittest discover -s reference_model -p 'test_*.py' -v
python3 -B validation/verify_contract.py --root . --write-manifest
python3 -B validation/build_zip.py --root . --output <archive-a>.zip
python3 -B validation/build_zip.py --root . --output <archive-b>.zip
cmp <archive-a>.zip <archive-b>.zip
unzip -t <archive-a>.zip
```

## Mandatory production evidence still required

Implementation must record pass/fail/blocked/not-run for focused tests, `cargo fmt`, workspace check, strict Clippy, workspace tests, relevant Tier 2, Cargo metadata, deterministic products, and canonical structure audit/gate on the current exact Git SHA.
