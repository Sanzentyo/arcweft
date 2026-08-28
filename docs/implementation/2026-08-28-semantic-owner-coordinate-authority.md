# Semantic owner coordinate authority

Date: 2026-08-28

Status: implemented

## Established implementation

- `HirSemanticPathOwnerId` is the closed project-path owner family for
  expressions, statements, patterns, and locals.
- `HirProjectEvaluationTopology::semantic_path` is the sole project-wide
  lookup. It selects the owner's module, preserves retained entry order, and
  returns a borrowed `HirSemanticPathLocation` only when the owner has exactly
  one rooted path.
- Item and declaration path indexes remain the one stored authority. The
  project lookup does not copy them into a second project-wide map.
- Path-index sealing rejects wrong-module owners, empty or invalid rooted
  paths, missing/extra/mismatched expression hops, and structural-path aliases
  across owner families.
- Duplicate and cycle errors retain the exact typed owner. Capture,
  expression-use, and local-origin duplicates have their own typed errors
  instead of sharing an unqualified `DuplicatePath` result.
- `AcceptedSemanticRootCatalog` delegates through the same HIR owner lookup.
  `SemanticCoordinateIndex` has one owner-based checked-path join and typed
  expression, pattern, statement, and binding projections.
- The project-rooted pattern coordinate remains a different type from the
  Match-arm-relative pattern coordinate.

Bodies are intentionally not represented by a fabricated `BodyId`. C3 body
transcripts must use a separate HIR-owned view over accepted structural body
prefixes and ordered `HirBodyChildEdge` rows.

## Validation

- `cargo test -p arcweft-lang-hir --lib`: 886 passed, 8 ignored.
- `cargo test -p arcweft-lang-sema --all-targets --all-features`: 535
  unit, 12 compile-API, and 4 integration tests passed.
- `cargo check --workspace --all-targets --all-features`: passed.
- `cargo fmt --all -- --check`: passed.
- focused duplicate-location, structural-alias, wrong-module, pattern,
  statement, binding, and checked-expression-hop coordinate tests: passed.
- strict Clippy was attempted for HIR and sema. It remains blocked by two
  pre-existing `match_same_arms` findings in HIR runtime reachability and 95
  pre-existing findings in `arcweft-core` before sema itself is linted. No
  finding named a file changed by this cut.
