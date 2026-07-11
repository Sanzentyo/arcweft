# Fx function presentation graph structural audit

- Audited Jujutsu change: `zulokxuymzpqonpuovnxmynrurxoruxz`
- Snapshot commit at audit time: `db9f6201fee4db268a0883f93718070c90a51b39`
- Files scanned: 2,542
- Rust files: 1,190
- Rust physical LOC: 603,951
- Result: 0 errors, 149 warnings

The initial audit reported `checker/expr.rs` at 2,603 physical LOC, above the
2,500 error threshold. Fx constructor checking was moved into the cohesive
`checker/expr/fx.rs` module. The final sizes are 2,469 LOC for `expr.rs` and 146
LOC for the Fx child module.

Relevant new or materially changed responsibility modules:

| Path | Bytes | Physical LOC | Responsibility |
| --- | ---: | ---: | --- |
| `crates/arcweft-lang-sema/src/checker/fx.rs` | 26,813 | 779 | Fx declaration/call inventory, graph validation, budgets |
| `crates/arcweft-lang-sema/src/checker/expr/fx.rs` | 5,643 | 146 | Fx constructor expression checking |
| `crates/arcweft-presentation/src/fx.rs` | 21,837 | 701 | Fx identities, graph/value model, canonical hashes |
| `crates/arcweft-runtime-plan/src/fx.rs` | 17,496 | 511 | HIR Fx graph compilation |
| `crates/arcweft-runtime-plan/src/render_text/fx.rs` | 17,053 | 522 | RichText Fx binding and static expansion |
| `crates/arcweft-view/src/fx.rs` | 10,665 | 367 | retained View Fx instances |

`resource_codec/view/codec.rs` remains a 1,402 LOC warning-level hotspot. This
cut adds bounded/canonical Fx argument handling but does not invent an isolated
Fx codec before complete Fx definition sections exist; the executable graph
request owns that future codec boundary. Exact metrics, dependency edges,
duplicate public type names, and every warning are retained in the sibling CSV
and Markdown reports.
