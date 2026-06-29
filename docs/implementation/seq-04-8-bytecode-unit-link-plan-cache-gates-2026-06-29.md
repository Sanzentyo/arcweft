# Seq04.8 bytecode-unit/link-plan cache gate implementation

Date: 2026-06-29

Implemented as conservative bytecode and link-plan gates.

- Added validated `BytecodeUnitObject` and `LinkPlanObject` `.awbo` gate payloads.
- Added `BytecodeUnitFactsObject`, `LinkPlanFactsObject`, and conservative reuse
  policy enums.
- Enabled typed read/write-through for `QueryKind::BytecodeUnit` and
  `QueryKind::LinkPlan`.
- Gate hits are valid cache evidence but force bytecode rebuild/relink through
  `CacheRecordStatus::HitThenRebuilt`.
- Actual bytecode/link reuse remains a follow-up because current `main` does not
  contain an applied seq04.7 runtime-plan-unit identity implementation.
- Product players remain free of compiler/persistent-cache dependencies.

## Validation

Validated on 2026-06-30 against local `main` after manual application.

Commands run:

```bash
cargo fmt --all
cargo check -p arcweft-project -p arcweft-project-loader -p arcweft-compiler -p arcweft-cli --all-targets --all-features
cargo test -p arcweft-project persistent_object --all-features
cargo test -p arcweft-project-loader persistent_query --all-features
cargo test -p arcweft-compiler persistent_query --all-features
cargo test -p arcweft-cli cache --all-features
cargo test -p arcweft-bundle --all-features
cargo fmt --all -- --check
git diff --check
cargo clippy -p arcweft-project -p arcweft-project-loader -p arcweft-compiler -p arcweft-bundle -p arcweft-cli --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Results:

- Focused project, loader, compiler, CLI cache, and bundle tests passed.
- Clippy passed with `-D warnings` for the touched crate set.
- Structural audit scanned 2052 files, 1028 Rust files, and 484387 Rust physical LOC with 0 errors and 121 warnings.
- `git diff --check` passed.

Design deviation from the package:

- The package applicator script did not match current `main` because the local
  enum context had drifted. The same acceptance criteria were applied manually.

## Follow-up boundary

Actual bytecode/link reuse is covered by:

`docs/reviews/requests/2026-06-30-seq-04.8.1-bytecode-link-actual-reuse-boundary.md`
