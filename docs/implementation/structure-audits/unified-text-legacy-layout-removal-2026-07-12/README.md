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

## Current changed Rust files

| Path | Owner | Bytes | Physical LOC | Classification | Major responsibility |
| --- | --- | ---: | ---: | --- | --- |
| `crates/arcweft-glyphon/src/lib.rs` | arcweft-glyphon | 587 | 15 | production facade | intentional prepared-text/text-engine exports |
| `crates/arcweft-text-layout/src/config.rs` | arcweft-text-layout | 4,904 | 131 | production | canonical shaped-layout request and errors |
| `crates/arcweft-text-layout/src/geometry.rs` | arcweft-text-layout | 2,152 | 88 | production | renderer-independent point/size/rectangle types |
| `crates/arcweft-text-layout/src/lib.rs` | arcweft-text-layout | 1,271 | 36 | production facade | intentional canonical layout exports |
| `crates/arcweft-text-layout/src/model.rs` | arcweft-text-layout | 1,025 | 28 | production | glyph orientation and vertical-form enums |
| `crates/arcweft-text-layout/src/vertical_clusters.rs` | arcweft-text-layout | 8,082 | 225 | production | Unicode grapheme/orientation/text-combine clustering |

Both changed facades are below the 250 LOC target. All ordinary changed
responsibility modules are below 300 LOC because this cut removes behavior
rather than adding a new subsystem. The two changed Cargo manifests only remove
the `arcweft-core` dev dependency used by the deleted compatibility tests.

## Deleted Rust files

Exact pre-deletion measurements from the preceding audit:

| Path | Bytes | Physical LOC | Classification | Former responsibility |
| --- | ---: | ---: | --- | --- |
| `crates/arcweft-text-layout/src/effects.rs` | 4,260 | 139 | production | estimated layout-phase effect reserve |
| `crates/arcweft-text-layout/src/horizontal.rs` | 8,358 | 230 | production | unshaped horizontal placement |
| `crates/arcweft-text-layout/src/layout.rs` | 8,441 | 242 | production | `layout_frame` orchestration |
| `crates/arcweft-text-layout/src/ruby.rs` | 14,606 | 449 | production | unshaped ruby placement |
| `crates/arcweft-text-layout/src/ruby_metrics.rs` | 2,787 | 90 | production | estimated ruby metrics |
| `crates/arcweft-text-layout/src/vertical.rs` | 15,136 | 451 | production | unshaped vertical placement |
| `crates/arcweft-text-layout/src/vertical_breaks.rs` | 15,653 | 449 | production | legacy vertical break scoring |
| `crates/arcweft-text-layout/src/vertical_columns.rs` | 14,303 | 451 | production | legacy vertical column planner |
| `crates/arcweft-text-layout/src/tests.rs` | 58,976 | 1,587 | unit tests | top-level estimated-layout tests |
| `crates/arcweft-text-layout/src/tests/ruby.rs` | 26,841 | 674 | unit tests | legacy ruby matrix |
| `crates/arcweft-text-layout/src/tests/vertical_class_mix.rs` | 74,126 | 1,749 | unit tests | legacy vertical class matrix |
| `crates/arcweft-text-layout/src/tests/vertical_sequences.rs` | 31,461 | 778 | unit tests | legacy sequence matrix |

All deleted paths belonged exclusively to the removed
`LineDisplayFrame -> LaidOutText -> GlyphArea` route. Canonical shaped layout
retains separate `document_layout`, `document_vertical`, `document_ruby`,
`vertical_clusters`, `vertical_orientation`, generated JLREQ data, shaping,
and `TextLayout` modules plus their direct tests.

## Largest current production Rust files

| Path | Bytes | Physical LOC | Embedded tests | Major responsibility |
| --- | ---: | ---: | --- | --- |
| `crates/arcweft-core/src/value.rs` | 84,017 | 2,500 | no | core runtime values |
| `crates/arcweft-core/src/engine/eval/calls.rs` | 89,488 | 2,481 | no | engine call evaluation |
| `crates/arcweft-lang-sema/src/checker/expr.rs` | 94,248 | 2,469 | no | expression type checking |
| `crates/arcweft-cli/src/toolchain_profile.rs` | 75,712 | 2,463 | yes | toolchain profile commands |
| `crates/arcweft-lang-sema/src/checker.rs` | 85,502 | 2,456 | no | semantic checker orchestration |
| `crates/arcweft-core/src/awbc/product_step.rs` | 93,512 | 2,430 | no | Product AWBC execution |
| `crates/arcweft-runtime-driver/src/session.rs` | 93,063 | 2,408 | no | bundle session orchestration |
| `crates/arcweft-bundle/src/container.rs` | 78,267 | 2,389 | yes | AWFB container codec |
| `crates/arcweft-cli/src/app/debug.rs` | 77,792 | 2,376 | yes | debug commands |
| `crates/arcweft-runtime-plan/src/expr.rs` | 83,504 | 2,363 | no | runtime expression lowering |

The largest workspace test files remain existing CLI/compiler integration
matrices and are unchanged by this deletion.

## Dependency fan-in and fan-out

| Crate | Fan-in | Fan-out before | Fan-out after | Change |
| --- | ---: | ---: | ---: | --- |
| `arcweft-text-layout` | 5 | 7 | 6 | removed test-only `arcweft-core` edge |
| `arcweft-glyphon` | 3 | 8 | 7 | removed test-only `arcweft-core` edge |

No production dependency direction changed. The shaped text path remains
`render-text -> text-layout -> glyphon -> render-wgpu/player`, with no renderer
or platform dependency introduced into the Sans I/O layout crate.
