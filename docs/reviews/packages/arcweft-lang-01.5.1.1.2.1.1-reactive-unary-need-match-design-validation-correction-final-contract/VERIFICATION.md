# Verification report

## Result

Package construction validation is designed to finish with **PASS** in both the
staging directory and a clean extraction of the final ZIP.

Authority:

- repository: `Sanzentyo/arcweft`
- exact current main: `cec30b57fa734efb059d7b846b397ac7d2b0701a`
- inspected production baseline: `0fa8a3b845b2dc966f181f450a1ca1f36e49d966`
- current main is one documentation-only commit ahead; no Rust file changed
- failed return SHA-256 supplied by the correction:
  `C5857AFCFCDDC88D2F642C4B4ACB0E61A68BBC4AC0BE42755BA9C2593B20E732`

## Actually verified

1. The uploaded correction request and the complete Rust skill/premise inputs
   were read before design work.
2. Root and relevant nested `AGENTS.md` files were read from the exact current
   main through the GitHub connector.
3. Current maintained Need/View documentation, final-HIR parent contract,
   semantic/HIR/core/AWBC/View/bundle/runtime-driver code, save/replacement
   owners, and old-Await consumers were inspected at the exact Git SHA.
4. The baseline-to-current comparison was checked: one documentation-only
   commit, zero Rust changes.
5. This archive is generated from scratch and contains no production overlay,
   patch, branch, fixture, manifest edit, or PR.
6. `tools/validate_package.py` checks required artifacts, exact
   `OPEN_QUESTIONS.md` bytes, selected decisions, version-1 markers, source line
   evidence, consumer/deletion/traceability/test minima, machine-model
   consistency, forbidden paths, and SHA-256 manifest completeness.
7. The validator is run on staging and again after clean extraction.
8. ZIP CRC and all manifest hashes are checked after extraction.

## Evidence counts

| Evidence family | Rows | Minimum |
|---|---:|---:|
| source evidence | 90 | 80 |
| Rust line evidence | 77 | 65 |
| producer/consumer | 30 | 25 |
| deletion | 40 | 35 |
| requirements traceability | 72 | 60 |
| test matrix | 445 | 350 |
| bounded-work limits | 22 | 20 |

## Deliberately not claimed

No Arcweft production source was locally cloned or changed, and no production
Cargo/check/test/Clippy/fmt/doc/generated/native/Web/headless/Agent/Tier-2 gate
was executed. Those commands are implementation admission requirements listed in
`VALIDATION_GATES.md`; this design-only ZIP does not report them as passed.

The package validator proves package completeness and internal consistency. It
does not prove that production code already implements this contract.
