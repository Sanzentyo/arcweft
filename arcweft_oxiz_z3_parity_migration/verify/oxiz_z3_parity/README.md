# OxiZ Z3 parity fixtures migrated to Arcweft verify

This package is laid out as a drop-in patch tree for `Sanzentyo/arcweft`.

It contains `168` one-to-one entries from the OxiZ `bench/z3_parity/results.json` manifest at commit `9f6bb93df338fd8e965511e9e1abc97ed3ca395f`. Each entry has:

- an Arcweft script fixture under `verify/oxiz_z3_parity/awft/<logic>/<case>.awft`;
- an expected SMT-LIB2 file under `verify/oxiz_z3_parity/expected_smt2/<logic>/<case>.smt2`;
- manifest metadata in `MANIFEST.json` and `MANIFEST.tsv`;
- a checklist in `CHECKLIST.md`;
- a Rust integration test under `crates/arcweft-cli/tests/verify_oxiz_z3_parity.rs`.

## How to apply

Copy the directories in this package into the root of `Sanzentyo/arcweft`:

```bash
rsync -a verify crates /path/to/arcweft/
cd /path/to/arcweft
cargo test -p arcweft-cli --test verify_oxiz_z3_parity
```

## What the test checks

The integration test reads every manifest row, opens the matching `.awft` file, extracts the `emit_smt <<SMT ... SMT` block, and compares it byte-for-byte with the checked-in expected SMT-LIB2 file after trimming only final trailing newlines. It also checks that every case appears in `CHECKLIST.md`.

## Optional exact source vendoring

The shipped expected SMT2 fixtures are normalized Arcweft emission fixtures keyed one-to-one to OxiZ names, logics, and expected Z3 results. To replace them with exact OxiZ source `.smt2` bodies, run:

```bash
python3 verify/oxiz_z3_parity/scripts/vendor_oxiz_sources.py --root . --update-awft
```

That script downloads the pinned OxiZ files from GitHub, updates `expected_smt2`, rewrites each `.awft` `emit_smt` block, and refreshes hashes in `MANIFEST.json` and `MANIFEST.tsv`.
