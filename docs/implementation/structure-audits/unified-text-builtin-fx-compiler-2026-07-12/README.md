# Typed RichText built-in Fx structural audit — 2026-07-12

Audit subject: Jujutsu working change `pzllnvpo`, based on `31972dcf`
(`Align layer capture pixels and object identity`). The canonical audit command
scanned the current checkout after the built-in RichText Fx compiler was split
by responsibility.

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-builtin-fx-compiler-2026-07-12
```

The checkout contains 1,249 Rust files and 615,551 physical Rust LOC. The audit
reports 0 errors and 128 pre-existing warnings.

## Changed-boundary measurements

| Path | Crate | Bytes | Physical LOC | Classification | Responsibility |
| --- | --- | ---: | ---: | --- | --- |
| `src/checker/fx/span.rs` | `arcweft-lang-sema` | 4,273 | 120 | production | atomic authored Fx/RichText span validation |
| `src/tests/fx.rs` | `arcweft-lang-sema` | 5,379 | 255 | unit-test module | Fx type and span behavior |
| `src/fx/evaluator.rs` | `arcweft-presentation` | 32,546 | 957 | production | single deterministic typed sampler evaluator |
| `src/fx/program.rs` | `arcweft-presentation` | 29,722 | 912 | production | validated value-program schema and instruction contract |
| `src/fx/render_resource.rs` | `arcweft-presentation` | 29,789 | 856 | production with embedded focused tests | typed renderer-resource resolution |
| `src/fx.rs` | `arcweft-runtime-plan` | 21,061 | 595 | production with embedded focused tests | authored and generated Fx definition inventory lowering |
| `src/render_text/fx/builtins.rs` | `arcweft-runtime-plan` | 8,645 | 233 | production with embedded focused tests | final target/phase classification and deterministic built-in identity |
| `src/render_text/fx/builtins/inventory.rs` | `arcweft-runtime-plan` | 4,701 | 120 | production | reachable HIR dialogue inventory traversal |
| `src/render_text/fx/builtins/program.rs` | `arcweft-runtime-plan` | 22,538 | 652 | production | built-in graph and sampler composition |
| `src/render_text/fx/builtins/program/attrs.rs` | `arcweft-runtime-plan` | 6,297 | 197 | production | typed source-unit parsing and attribute validation |
| `src/render_text/fx/builtins/program/value_expr.rs` | `arcweft-runtime-plan` | 5,846 | 190 | production | small typed sampler-expression builder |
| `src/render_text/fx/expander.rs` | `arcweft-runtime-plan` | 11,179 | 298 | production | per-line typed Fx span expansion and stable ordinal assignment |
| `src/render_text/tests.rs` | `arcweft-runtime-plan` | 55,533 | 1,768 | unit-test module | RichText lowering behavior; below the 2,500-LOC test warning threshold |

The initially cohesive but 1,002-line built-in program was decomposed before
the cut: graph composition is now 652 LOC, while source attribute validation
and bytecode expression construction are narrow leaf modules. No production
file touched by this slice exceeds the 1,200-LOC warning threshold, and the
ordinary responsibility module remains in the preferred 300–800 LOC range.

Dependency fan-out/fan-in is 7/8 for `arcweft-lang-sema`, 6/17 for
`arcweft-presentation`, 5/16 for `arcweft-render-text`, and 9/7 for
`arcweft-runtime-plan`. No Cargo edge or crate direction changed. The generated
`file_metrics.csv`, `dependency_edges.csv`, `public_type_duplicates.csv`, and
`violations.md` are the exact machine-readable evidence for this checkout.
