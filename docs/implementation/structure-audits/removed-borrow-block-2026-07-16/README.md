# Removed borrow-block structural audit

- Jujutsu change: `knxxvmxlvktv`
- Parent revision: `38a5ce0534bc6819ee6b9de1155558e57343c46f`
- Command: `cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/removed-borrow-block-2026-07-16`
- Result: 2,747 files, 1,306 Rust files, 640,768 physical Rust LOC,
  90 package manifests, 0 errors, and 126 pre-existing warnings.

`file_metrics.csv` records exact bytes, physical LOC, classification, generated
status, and embedded-test status for every scanned file, including the largest
workspace Rust files and every changed Rust file. `dependency_edges.csv`
records workspace fan-in/fan-out evidence. This deletion changes no Cargo
manifest, feature, dependency edge, public re-export, or crate boundary.

## Changed warning-level files

| Path | Owner/responsibility | Bytes | Physical LOC | Classification |
|---|---|---:|---:|---|
| `crates/arcweft-compiler/src/persistent.rs` | compiler persistent fingerprint projection | 53,467 | 1,418 | production with embedded tests |
| `crates/arcweft-lang-sema/src/checker.rs` | type-check orchestration | 86,751 | 2,479 | production |
| `crates/arcweft-lang-sema/src/checker/helpers.rs` | local type-check domain helpers | 43,164 | 1,212 | production |
| `crates/arcweft-lang-sema/src/semantic.rs` | semantic fact/control-flow analysis | 76,068 | 2,030 | production |
| `crates/arcweft-lang-sema/src/tests/typecheck.rs` | type-check unit tests | 125,157 | 4,112 | test |
| `crates/arcweft-lsp/src/features/actions.rs` | LSP code-action traversal | 48,397 | 1,486 | production with embedded tests |
| `crates/arcweft-verify/src/lib.rs` | verifier orchestration/facade | 66,480 | 1,917 | production facade |

Every edit in these hotspots removes the obsolete flow variant, traversal arm,
fixture, or now-unused helper. No file grows and no responsibility is added, so
this coherent deletion does not justify an unrelated decomposition. The
existing warning-level ownership debt remains visible in `violations.md` for a
dedicated structural cut.

