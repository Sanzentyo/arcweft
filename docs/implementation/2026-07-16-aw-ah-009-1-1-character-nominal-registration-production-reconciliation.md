# AW-AH-009.1.1 Character nominal registration production reconciliation

## Status

Implemented and validated as one public cut against base commit
`3af0b4be3c62bf06d73c624e85e6a4352f9c4ace` (Jujutsu parent change
`knxxvmxl`). The source of truth is
`arcweft-aw-ah-009.1.1-character-nominal-registration-production-reconciliation-final-contract.zip`.

No compatibility interval, dual reader, deprecated alias, source gate, or
persisted semantic-environment format was added.

## Completed contract

- `arcweft-source` owns checked document IDs, content revisions, immutable
  document identities, source-set revisions, and revision-bound spans. Former
  semantic source identities and unbound source-anchor constructors are gone.
- `arcweft-character` has one runtime JSON decoder and one source-backed
  registration decoder. Structural JSON tokens, duplicate decoded keys,
  fingerprints, catalogs, typed decode diagnostics, and manifest limits retain
  their original source evidence. A runtime manifest cannot enter registration.
- `arcweft-launch` source-backed parsing retains structural TOML key/value and
  array-element tokens. Quoted-string content has its own token-map span, so LSP
  locations exclude delimiters without substring reconstruction.
- `arcweft-lang-hir::symbol::ProjectSymbolTable` replaces the callable-only
  table. Callable and external declarations share one world/revision-bound,
  deterministic linker with opaque external IDs and bounded limits/work.
- `arcweft-lang-sema::registration` owns the sole complete-world registrar,
  external-owner registry, immutable registered environment, inventory
  descriptor/digest/revision, bounded diagnostics, and production limits.
- `CharacterNominalFamily::family()` is the sole character classifier, and the
  inherent exhaustive `TypeKind::first_mismatch()` supplies deterministic typed
  mismatch paths.
- `arcweft-compiler` accepts a `ProjectCompilationContext`, performs
  registration before semantic work, and flushes pending cache stores only
  after the complete cut succeeds. The old environment-taking entry point is a
  compile-fail case.
- CLI, LSP, tooling, verify-LSP, adapter, package/view, presentation, scene,
  bundle, dialogue, runtime-plan, and project-loader callers use the same
  revision-bound facts. LSP broad caches use checked accepted-environment
  generations and preserve the prior generation/cache after failed rebuilds.
- Every production maximum is owned once: source bytes in `arcweft-source`,
  manifest collections in `arcweft-character::manifest::limits`, project-symbol
  limits in HIR, and world registration limits in sema.

## Deletions and negative API evidence

Compile-fail and direct caller migration cover:

- `CharacterManifest::from_json`;
- `CharacterNominalKind` and `kind()`;
- `TypeCheckEnv::with_character_manifest`;
- `CallableSymbolTable` and callable-only link/resolution errors;
- raw/parsed external declaration IDs and public external-input fields;
- `compile_project_with_env` and the old cache signature.

The final production source contains none of those compatibility entry points.
The removed borrow-block grammar likewise has no production CST/AST/HIR node or
dedicated historical diagnostic; historical design notes are not executable
syntax support.

## Structural result

The canonical report is
[`structure-audits/aw-ah-009-1-1-production-reconciliation/`](structure-audits/aw-ah-009-1-1-production-reconciliation/).
It records the complete file inventory and Cargo dependency graph. The cut has
zero error-level structural violations. Existing repository-wide warnings are
retained in the report rather than hidden.

The previously identified hotspots were decomposed into responsibility
facades/modules. `checker.rs` is below both package gates. The project-symbol
table/linker remains one cohesive module because binding insertion, fixed-point
resolution, ambiguity ordering, work charging, and bounded reporting form one
transaction; a module-level rationale records that choice.

| Path | Bytes | Physical LOC | Class | Embedded tests |
| --- | ---: | ---: | --- | --- |
| `crates/arcweft-source/src/document.rs` | 12,334 | 386 | production | yes |
| `crates/arcweft-character/src/manifest.rs` | 667 | 20 | production facade | no |
| `crates/arcweft-character/src/manifest/model.rs` | 26,839 | 873 | production | no |
| `crates/arcweft-character/src/manifest/registration.rs` | 35,411 | 1,038 | production | no |
| `crates/arcweft-launch/src/lib.rs` | 1,049 | 25 | production facade | no |
| `crates/arcweft-lang-hir/src/symbol.rs` | 1,488 | 40 | production facade | no |
| `crates/arcweft-lang-hir/src/symbol/table.rs` | 39,565 | 1,099 | production | no |
| `crates/arcweft-lang-sema/src/env.rs` | 591 | 18 | production facade | no |
| `crates/arcweft-lang-sema/src/diagnostics.rs` | 346 | 10 | production facade | no |
| `crates/arcweft-lang-sema/src/registration.rs` | 908 | 23 | production facade | no |
| `crates/arcweft-lang-sema/src/checker.rs` | 62,736 | 1,752 | production | no |
| `crates/arcweft-lsp/src/profiles.rs` | 446 | 18 | production facade | no |

Normal-dependency fan-in/fan-out at this cut is recorded in the generated
dependency CSV; the relevant crate pairs are source `18/2`, character `10/6`,
launch `3/4`, HIR `10/3`, sema `8/10`, compiler `4/14`, and LSP `0/25`.

## Validation

All commands used `CARGO_INCREMENTAL=0` for Rust compilation.

```bash
cargo test -p arcweft-character -p arcweft-launch -p arcweft-lang-hir -p arcweft-lang-sema -p arcweft-compiler -p arcweft-lsp --all-targets --all-features
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just verify
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/aw-ah-009-1-1-production-reconciliation
```

All commands passed. The first `just verify` attempt exposed one dialogue test
that had not supplied the newly required source anchor. The fixture was bound
to its authored text through a checked generated `SourceDocument`; no fallback
constructor was restored. The focused dialogue test and the complete
`just verify` rerun then passed.

Tier 2 MCP stdio and exact visual golden tests were not run because this cut
does not change those risk areas.

## Non-goals and deviations

- CSS/Takumi, native Style/exported-part, proof grammar/typed-AST identity,
  AWBC/save/bundle formats, rendering, and new LSP definition/rename/signature
  features were not redesigned by this cut.
- No result-changing deviation from the final contract was taken. Internal
  source files may group closely coupled declaration/binding or inventory
  records more coarsely than the design sketch; public ownership, dependency
  direction, hard limits, and transaction boundaries match the contract, and
  the exact current measurements are checked in.
- AW-AH-009.2 and AW-AH-009.3 remain sequenced after this landed substrate.
