# AW-AH-015 validation matrix

## Required commands after applying to a full checkout

```bash
cargo fmt --all -- --check
cargo test -p arcweft-text-layout --lib
cargo test -p arcweft-text-layout --test vertical_break_quality
cargo test -p arcweft-glyphon --test shared_text_layout
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-rich-text
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Run the repository's exact Native/Web/headless vertical fixtures required by
`docs/implementation/test-execution-policy.md`; renderers must consume the
common `TextLayout` and must not introduce a second score path.

## Full-checkout evidence

The production overlay was compiled in the Arcweft checkout on 2026-07-14.
The focused evidence below reflects the final reviewed corpus, including its
`owner_approved` status:

| Command | Result |
| --- | --- |
| `cargo check -p arcweft-text-layout --all-targets --all-features` | pass |
| `cargo test -p arcweft-text-layout --lib` | pass, 24 tests |
| `cargo test -p arcweft-text-layout --test document_layout` | pass, 12 tests |
| `cargo test -p arcweft-text-layout --test vertical_break_quality` | pass, 3 tests |
| `cargo test -p arcweft-glyphon --test shared_text_layout` | pass, 6 tests |
| `cargo clippy -p arcweft-text-layout --all-targets --all-features -- -D warnings` | pass |
| `cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/aw-ah-015-vertical-break-quality` | pass, 0 errors / 127 warnings |
| `cargo fmt --all -- --check` | pass at the final integration cut |
| `cargo check --workspace --all-targets --all-features` | pass at the final integration cut |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pass at the final integration cut |
| `just test-workspace` | pass at the final integration cut |
| `just test-rich-text` | pass at the final integration cut |
| `just test-doc` | pass at the final integration cut |

The final dry-run structural audit, after all parallel package slices were
integrated, scanned 2,731 files, 1,299 Rust files, and 635,093 Rust physical
LOC with zero errors and 127 existing warning-level hotspots.

## Behavioral matrix

| Requirement | Direct evidence |
| --- | --- |
| Hard UAX/JLREQ separation | policy unit tests and corpus punctuation cases |
| Scale invariance | every corpus case at four uniform scales |
| Strictness enters objective | loose/strict closing-opening cases |
| Hanging ordering | hanging and non-hanging competition unit tests |
| Terminal escape | narrow and unbreakable corpus cases |
| Total tie-break | direct equal-score path test |
| Typed invalid inputs | NaN, resource-limit, zero, tiny, huge, and cursor-boundary unit tests |
| Closed policy codec | serde round-trip plus unknown/object rejection |
| Hash identity | `TextLayoutHash` v2 includes stable policy ID |
| Direction parity | paired RL/LR corpus group |
| Backend parity | existing common-layout integration tests after full checkout application |

## Package-environment boundary

The delivery environment did not contain `rustc`, `cargo`, a network-reachable
GitHub checkout, or the repository dependency graph. Therefore this package was
validated structurally and by an independent deterministic corpus evaluator,
but the Rust commands above were not executed here. See the package-level
`VERIFICATION_BOUNDARIES.md` and `verification/` logs. Do not report those
commands as passed until they run in a full checkout.
