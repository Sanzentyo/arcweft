# Structural audit size error refactor

Date: 2026-07-04

## Status

This cut resolves the current structural audit `SIZE001` error-level Rust file
size findings. No public API or runtime behavior is intentionally changed; the
work moves cohesive code blocks into child modules while keeping the existing
crate boundaries and item visibility narrow.

The refactor was applied on Jujutsu change
`mtrzvkousyotxvnrsppmwxulnrsopwxy`.

## Refactor Summary

| Original file | Before LOC | After LOC | Extracted module | Extracted LOC | Boundary |
| --- | ---: | ---: | --- | ---: | --- |
| `crates/arcweft-cli/src/app/bundle.rs` | 2,845 | 2,130 | `crates/arcweft-cli/src/app/bundle/tests.rs` | 714 | Unit tests for bundle sidecars, product AWBC attachment, image assets, and patch run behavior. |
| `crates/arcweft-core/src/value.rs` | 2,512 | 2,399 | `crates/arcweft-core/src/value/sequence_constructors.rs` | 125 | Public runtime sequence constructor functions and repeat helpers. |
| `crates/arcweft-lang-sema/src/checker/expr.rs` | 2,643 | 2,286 | `crates/arcweft-lang-sema/src/checker/expr/support.rs` | 376 | Expression checker helper types and functions for choices, agent helpers, field typing, and branch joining. |
| `crates/arcweft-runtime-plan/src/flow.rs` | 2,679 | 2,297 | `crates/arcweft-runtime-plan/src/flow/tests.rs` | 378 | Unit tests for flow optimizer and dialogue-default lowering behavior. |

All four production files now sit below the 2,500 LOC error threshold. The
`bundle.rs` and `flow.rs` embedded test modules were moved into child test
modules, which also removes their `TEST001` warnings.

## Structural Audit

Final command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/structure-audit-refactor-final
```

Result:

- files scanned: 2,331
- Rust files: 1,121
- Rust physical LOC: 523,154
- package manifests: 91
- violations: `0 error(s), 129 warning(s)`

Largest remaining Rust hotspots observed by the final audit:

| Path | Bytes | Physical LOC | Kind |
| --- | ---: | ---: | --- |
| `crates/arcweft-text-layout/src/vertical_orientation.rs` | 357,456 | 12,399 | generated |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 255,414 | 7,944 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | 243,051 | 6,758 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | 222,475 | 6,161 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | 209,852 | 5,651 | integration test |

## Validation

Passed:

```bash
cargo check -p arcweft-core -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-cli
cargo test -p arcweft-core value --quiet
cargo test -p arcweft-lang-sema --lib --quiet -- --skip tests::parser_basics::rejects_unparenthesized_presentation_call
cargo test -p arcweft-runtime-plan flow::tests --quiet
cargo test -p arcweft-cli app::bundle::tests --quiet
cargo fmt --all -- --check
cargo clippy -p arcweft-core -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-cli --all-targets -- -D warnings
cargo clippy --workspace --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/structure-audit-refactor-final
```

Observed unrelated failure:

```bash
cargo test -p arcweft-lang-sema --lib --quiet
```

This ran 310 tests and failed one parser-basics test:
`tests::parser_basics::rejects_unparenthesized_presentation_call`. The failure
reports that the fixture no longer produces parse errors. This refactor does not
touch parser code or that fixture; it is left as an existing semantic/parser
test expectation issue outside this size-error cleanup.

## Remaining TODOs

- The structural audit warning backlog remains at 129 warnings. The current cut
  intentionally targets error-level size findings only.
- Follow-up warning cleanup should be split by ownership area rather than folded
  into this mechanical size refactor.

## Design Deviations

None.
