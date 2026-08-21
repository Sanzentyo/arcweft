# Read-only validation guide

## Package-level validation

From an extracted package root:

```sh
python3 tools/validate_package.py . --self-test
```

Against the independently throwable ZIP:

```sh
python3 tools/validate_package.py   arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1-runtime-handle-batch-and-snapshot-isomorphism-correction-final-contract.zip   --self-test
```

The validator reads only. It validates the manifest, status/version policy,
closed inventories, exact carriers, transaction/layering rules, compile cuts,
100-row test matrix, and all twelve mandatory negative mutations.

## Production implementation validation

The design package contains no production patch. At implementation time, run
the repository commands required by the latest `AGENTS.md`, including format,
workspace check/test/clippy, focused crate tests, dependency-direction checks,
and the source-symbol deletion scans listed in `SOURCE_DELETION_AND_CUTS.md`.

`VALIDATION_OUTPUT.txt` records only commands actually run while assembling this
archive. It deliberately does not claim a Rust compile because the execution
environment used for package assembly had no Rust toolchain and no local
repository checkout.
