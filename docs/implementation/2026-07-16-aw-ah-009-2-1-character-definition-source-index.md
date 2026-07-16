# AW-AH-009.2.1 Character definition source index

## Status and basis

The implementation-ready substrate is implemented in Jujutsu change
`ztnnmyzu` on Git base `45ecf9add7d8`. The source package is
`arcweft-aw-ah-009.2.1-character-nominal-definition-source-index-final-contract.zip`
with SHA-256
`89b0ecbab84b9954626f139e320d2dba3f7a273a92ff0d6cbd0dc922c50770d7`.

This cut does not claim the entire package complete. Three repository-integration
boundaries remain explicitly separated below. No compatibility reader, dual
publication API, source-name inverse parser, source gate, CSS/Takumi path, or
partial character-definition response was added.

## Implemented contract

- `arcweft-character` retains quote-free raw JSON string ranges and owns a total
  typed projection from validated manifest descriptors to exact declaration
  value/selection spans. Escapes, Unicode, CRLF, duplicates, foreign/unknown
  descriptors, missing tokens, and non-string tokens fail deterministically.
- `arcweft-lang-sema::registration::CharacterDefinitionIndex` is immutable and
  bound to the exact world, symbol revision, source-set revision, documents,
  descriptor provenance, co-definitions, and member reverse maps. It is built
  only inside the complete character registration transaction; any error rejects
  the candidate world.
- Sema owns request-scoped typed character references and cursor queries. Owner,
  local member, expected nominal family/part, ambiguity, recovery, stale state,
  resource failure, and integrity failure are structured outcomes. Registered
  character variants participate in the existing expected-short-variant check.
- `arcweft-project-loader` returns registration facts together with exact file
  paths, ownership, and access. It overlays logical project/module/character
  documents, retains the exact resolved character manifest file, validates only
  a known target path, and never reconstructs paths from labels.
- One accepted LSP candidate atomically publishes profile key, registered world,
  source registry, overlay set, generation, and generation-owned typed caches.
  Failed candidate construction/rebuild preserves the previous accepted `Arc`,
  generation, and caches.
- Character definition uses checked client positions, captures one accepted
  generation, keys caches by all semantic/source identities, rechecks target
  availability outside the cache, returns exact `LocationLink` ranges, and is
  all-or-empty for co-definitions. A changed target produces a typed stale error
  and schedules a complete profile rebuild.
- Definition dispatch preserves the latest View exported-part metadata route,
  then handles character tokens exclusively, then falls through to presentation
  style definition. Open/change/save/close rebuild affected profiles, and close
  removes the overlay before rebuilding from disk.

## Structural result

The canonical report is
[`structure-audits/aw-ah-009-2-1-character-definition-source-index/`](structure-audits/aw-ah-009-2-1-character-definition-source-index/).
It scanned 2,923 files, including 1,454 Rust files and 678,475 Rust physical
LOC, and reported zero error-level violations and 129 repository-wide warnings.

| Path | Bytes | Physical LOC | Class | Embedded test LOC |
| --- | ---: | ---: | --- | ---: |
| `crates/arcweft-character/src/manifest/registration/declaration.rs` | 9,135 | 252 | production | 0 |
| `crates/arcweft-character/src/manifest/registration.rs` | 36,031 | 1,054 | production | 0 |
| `crates/arcweft-lang-sema/src/character_definition.rs` | 34,882 | 1,017 | production | 0 |
| `crates/arcweft-lang-sema/src/registration/source_index.rs` | 33,067 | 967 | production | 0 |
| `crates/arcweft-lsp/src/features/character_definition.rs` | 19,365 | 503 | production | 0 |
| `crates/arcweft-lsp/src/profiles/cache.rs` | 37,869 | 1,168 | production | 313 |
| `crates/arcweft-lsp/src/profiles/environment.rs` | 12,351 | 332 | production | 0 |
| `crates/arcweft-lsp/src/session.rs` | 30,536 | 761 | production | 0 |
| `crates/arcweft-lsp/src/session/character_definition_tests.rs` | 16,648 | 450 | integration-style unit test | 0 |
| `crates/arcweft-project-loader/src/environment.rs` | 18,660 | 567 | production | 112 |
| `crates/arcweft-project-loader/src/source_document.rs` | 4,060 | 116 | production | 39 |

The relevant normal-dependency fan-in/fan-out values are character `10/6`,
sema `8/10`, project-loader `2/15`, and LSP `0/26`. No crate boundary or Cargo
dependency was added by this cut. The LSP cache remains below the 1,200-LOC
production warning; its embedded tests cover atomic generation replacement,
overflow, shutdown, and invalidation.

## Validation

Rust commands used `CARGO_INCREMENTAL=0`.

```bash
cargo fmt --all -- --check
cargo check -p arcweft-character -p arcweft-lang-sema -p arcweft-project-loader -p arcweft-lsp --all-targets
cargo clippy -p arcweft-character -p arcweft-lang-sema -p arcweft-project-loader -p arcweft-lsp --all-targets --all-features -- -D warnings
cargo test -p arcweft-character manifest::registration
cargo test -p arcweft-lang-sema registration
cargo test -p arcweft-project-loader
cargo test -p arcweft-lsp definition
cargo test -p arcweft-lsp profiles
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/aw-ah-009-2-1-character-definition-source-index
```

Results were respectively: format/check/Clippy passed; character manifest 18
passed; sema registration 55 unit tests and 2 compile-fail cases passed;
project-loader 87 unit, 3 dependency, and 6 end-to-end tests passed; LSP
definition 12 passed (including 7 new character cases and View composition);
LSP profiles 17 passed; structural audit reported 0 errors.

Workspace-wide Clippy was attempted and reached unrelated crates, but the
checkout does not contain `web/assets/noto-sans-jp-vf.ttf`, which is included by
player/render test code. The focused changed-crate Clippy above passes with
warnings denied. Tier 2 MCP stdio and exact visual goldens were not run because
this cut does not change those risk areas.

## Explicit remaining boundaries

These are not silently counted as complete:

1. [AW-AH-009.2.1.1 launch/profile overlay reconciliation](../reviews/requests/2026-07-16-aw-ah-009.2.1.1-launch-profile-overlay-production-reconciliation.md)
   must define re-resolution when open `arcw.toml` or topology resources change.
   Current character/source overlays are authoritative only after the already
   disk-resolved profile and resource list have been selected.
2. [AW-AH-009.2.1.2 source-adapter diagnostics reconciliation](../reviews/requests/2026-07-16-aw-ah-009.2.1.2-source-adapter-diagnostics-production-reconciliation.md)
   must define generated/non-file explicit URI seeding and bounded
   generation-owned missing/unreadable/unmapped diagnostics. Current file-backed
   targets are exact and all-or-empty; unavailable targets return no link.
3. [AW-AH-009.2.1.3 shared query budget and verification reconciliation](../reviews/requests/2026-07-16-aw-ah-009.2.1.3-shared-query-budget-verification-reconciliation.md)
   must freeze one budget shared across sema and LSP and complete the mandatory
   O/M/C/A/S/K/L/N direct-test trace. Current production collections are bounded,
   but reference collection and target adaptation do not charge one shared
   request counter, and the package's exhaustive matrix is not yet implemented.
   The prescribed `cargo test -p arcweft-lang-sema character_definition`
   command currently selects zero tests; the LSP end-to-end cases do not replace
   that required direct sema suite.

AW-AH-009.2.2 rename must not treat these three items as complete or redesign
the verified source-index substrate to work around them.
