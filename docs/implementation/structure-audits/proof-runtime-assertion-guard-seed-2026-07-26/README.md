# Proof runtime assertion guard seed structural audit

- Date: 2026-07-26
- Base revision: `main@81074bb33f21` (`Delete duplicate source hash authority`)
- Working change: `mtxouyrt`
- Scope: deterministic typed runtime assertion guard seed derivation
- Result: 3,697 files, 1,941 Rust files, 906,932 Rust physical LOC,
  95 package manifests, 0 errors, and 146 repository-wide warnings

## Changed Rust files

| Path | Owner | Kind | Bytes | Physical LOC | Responsibility |
|---|---|---:|---:|---:|---|
| `crates/arcweft-runtime-plan/src/assertion_identity.rs` | `arcweft-runtime-plan` | production | 5,508 | 152 | public typed runtime assertion identity vocabulary and narrow guard-derivation entrypoint |
| `crates/arcweft-runtime-plan/src/assertion_lower.rs` | `arcweft-runtime-plan` | production | 3,902 | 117 | private canonical seed encoding and BLAKE3 derivation |
| `crates/arcweft-runtime-plan/src/lib.rs` | `arcweft-runtime-plan` | facade | 374 | 20 | responsibility-module declarations; the lowering implementation remains private |
| `crates/arcweft-runtime-plan/tests/assertion_identity.rs` | `arcweft-runtime-plan` | integration test | 5,779 | 191 | deterministic golden and typed seed-field separation matrix |

No changed file reaches a size-review threshold. The existing embedded unit
tests in `assertion_identity.rs` occupy 45 physical lines and test the adjacent
condition-index and mode vocabulary; the file is far below the 1,200-LOC
embedded-test trigger.

## Dependency boundary

The runtime-plan crate has 13 recorded outgoing dependency rows and 8 incoming
rows in the all-feature metadata graph. This cut adds no dependency: `blake3`,
`arcweft-core`, and `arcweft-lang-hir` were already normal dependencies. The
new private module uses only those existing lower-layer owners. It adds no
syntax/HIR dependency to `arcweft-core` and no runtime-host production edge.

The 146 warnings are the existing repository-wide audit backlog. None names a
changed file, and this cut adds no error or threshold exception.
