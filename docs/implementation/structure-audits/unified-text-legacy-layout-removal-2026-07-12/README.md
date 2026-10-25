# Unified text legacy-layout removal structural audit — 2026-07-12

Revision measured: Jujutsu change `lwzommlm`.

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-legacy-layout-removal-2026-07-12
```

The audit scanned 2,633 files, including 1,244 Rust files and 613,600 physical
Rust LOC. It reports 0 errors and 128 warnings. Relative to the preceding
visual-parity audit, this removes 12 Rust files, 8,683 physical Rust LOC, and
three warnings.

## Deleted-file measurements

Exact pre-deletion measurements from the preceding audit:

| Path | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `crates/arcweft-text-layout/src/effects.rs` | 4,260 | 139 | production |
| `crates/arcweft-text-layout/src/horizontal.rs` | 8,358 | 230 | production |
| `crates/arcweft-text-layout/src/layout.rs` | 8,441 | 242 | production |
| `crates/arcweft-text-layout/src/ruby.rs` | 14,606 | 449 | production |
| `crates/arcweft-text-layout/src/ruby_metrics.rs` | 2,787 | 90 | production |
| `crates/arcweft-text-layout/src/vertical.rs` | 15,136 | 451 | production |
| `crates/arcweft-text-layout/src/vertical_breaks.rs` | 15,653 | 449 | production |
| `crates/arcweft-text-layout/src/vertical_columns.rs` | 14,303 | 451 | production |
| `crates/arcweft-text-layout/src/tests.rs` | 58,976 | 1,587 | unit tests |
| `crates/arcweft-text-layout/src/tests/ruby.rs` | 26,841 | 674 | unit tests |
| `crates/arcweft-text-layout/src/tests/vertical_class_mix.rs` | 74,126 | 1,749 | unit tests |
| `crates/arcweft-text-layout/src/tests/vertical_sequences.rs` | 31,461 | 778 | unit tests |

## Structured audit evidence

- [File measurements](file_metrics.csv)
- [Dependency edges](dependency_edges.csv)
- [Public type name screening](public_type_duplicates.csv)
- [Structural findings](violations.md)
