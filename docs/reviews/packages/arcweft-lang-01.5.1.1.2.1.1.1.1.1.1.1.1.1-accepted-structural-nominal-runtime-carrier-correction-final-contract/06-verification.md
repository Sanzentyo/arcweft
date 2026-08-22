# Verification record

この文書だけが「実際に検証した」範囲の authority である。design に記載した implementation command は、ここに成功記録がない限り未実行である。

## Inputs read through EOF

| input | lines | bytes | SHA-256 |
|---|---:|---:|---|
| `2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier-correction.md` | 110 | 5752 | `cbe6a1f1f20f2c5c11df678b8098165ce8931820ece459c7bf1cf203be7bc5a4` |
| `Rust Skill.txt` | 57 | 5045 | `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665` |
| `前提(Sanzentyo-arcweft).txt` | 1 | 250 | `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1` |

## Repository authority

- checkout exists: `False`
- remote: `UNAVAILABLE`
- inspected SHA: `UNAVAILABLE`
- branch display: `UNAVAILABLE`

## Commands actually run

| command | exit | bounded output |
|---|---:|---|

## Package checks performed

- every required package file exists and is non-empty;
- request is copied byte-for-byte as `REQUEST.md`;
- traceability IDs are unique and each has POS/NEG rows;
- SHA-256 manifest is generated after content finalization;
- ZIP central directory is opened and every member CRC is tested;
- no `.git`, target directory, production source overlay, executable, or symlink is packaged.

## Explicitly not claimed

- production code was not changed or compiled as part of this design-only artifact;
- full workspace fmt/clippy/test/restart suites are not claimed unless present above with exit 0;
- a wire tag numeric value is not guessed when current owner table has not already reserved it; the implementation sequence requires allocation in the authoritative table and immediate golden-byte lock;
- source behavior after SHA `UNAVAILABLE` is outside this package;
- referenced predecessor ZIP contents are not claimed read when the ZIP is absent from the checkout; presence/hash status is listed in `02-current-source-evidence.md`.

## Design completeness

- extracted normative request rows: `3`
- traceability rows generated: `3`
- requirement-specific POS rows: `3`
- requirement-specific NEG rows: `3`
- OPEN_QUESTIONS: **0**
