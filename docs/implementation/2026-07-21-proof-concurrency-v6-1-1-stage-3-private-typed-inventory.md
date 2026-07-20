# Proof concurrency v6.1.1 — Stage 3 private typed inventory

Date: 2026-07-21

## Outcome

This cut prepares, but does not perform, the Proof-concurrency v6.1.1 Stage 3
public syntax switch. The accepted source package is
`arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip`,
with SHA-256
`1b7de5f2c10a5b29d67c72011e4272df9a76af8907fd21fe162de54809fc69ef`.
The implementation cut started from Git
`cea6436e7eb0f7815ebfc9d41ad21d806a0a4647`.

The private attached grammar now has a discriminant-complete typed inventory:

- every `SyntaxKind` explicitly selects `IdentityBearing`,
  `StructuralWrapper`, or `Token`; there is no wildcard identity default;
- every identity-bearing kind explicitly selects an `AstTag` family, while
  every wrapper and token explicitly selects no typed family;
- the test-only `SyntaxKind::ALL` vocabulary proves that the identity and
  typed-family tables remain complete when a kind is added;
- item, statement, expression, pattern, type, and retained-declaration
  classification are inherent `SyntaxKind` behavior rather than budget-local
  projections; and
- `SyntaxRole` and ordinal-free `SyntaxRoleClass` have their own responsibility
  module instead of inflating the kind inventory.

`AstTag` is deliberately only a coarse inventory and navigation tag. Exact
`SyntaxKind + SyntaxRole` remains the attachment and ownership authority.
Marker casts still require the marker's exact `SyntaxKind` and its expected
family tag. No caller may turn two distinct concrete kinds into the same typed
node merely because their `AstTag` matches.

The private snapshot attachment now records:

- the nearest identity-bearing parent, skipping structural wrappers;
- children in exact grammar order;
- every child grouped by exact `SyntaxRole`; and
- repeated instances of the same exact role without claiming singular access.

This last rule is required for valid surfaces such as an assertion with
multiple `Condition` children. `child(role)` succeeds only for a unique exact
role; `children_with_role(role)` returns every occurrence in source order.
Attachment construction is separated into an inventory builder, immutable
record parts, exact lookup errors, typed marker nodes, and immutable snapshot
data. No range search, source-text search, or stringly reconstruction was
introduced.

## Direct behavioral evidence

The added and retained tests prove:

- the entire `SyntaxKind` vocabulary has one explicit identity class;
- an `AstTag` exists if and only if the kind is identity-bearing;
- representative items, statements, expressions, patterns, types, body nodes,
  delimiters, and recovery nodes select their intended family;
- source-file, predicate, and proof marker casts require their exact concrete
  kinds;
- root-to-item-to-body-to-expression navigation follows exact roles through
  non-owning structural wrappers;
- an ordinary call exposes its exact `Callee` child without reparsing text;
- repeated `Condition` children remain ordered and cannot be mistaken for a
  unique child; and
- Rowan and typed handles still round-trip within one immutable snapshot while
  a structurally equal foreign Rowan root is rejected.

The AW-AH-007/008 rich-text contract remains compatible with the selected
direction: ordered and ranged arguments must attach as ordinary expression
children. This cut does not flatten those expressions or introduce a raw-text
reader.

## Stage 3 consumer inventory

A read-only census of the current checkout found:

| Detached/public surface | References | Files | Production subset |
| --- | ---: | ---: | ---: |
| `.typed_tree()` | 308 | 79 | 38 references in 26 files |
| `TypedSyntaxTree` | 43 | 19 | 28 references in 12 consumer files |
| `.into_typed_tree()` | 45 | 24 | test-only |
| `parse_source(...)` | 334 | 99 | conservative lexical upper bound |
| `ParsedFragmentKind` | 30 | 9 | mixed production and tests |
| `parse_fragment(...)` | 9 | 4 | mixed production and tests |

The public switch is therefore not a safe single-file change. The blocking
ownership paths are:

1. `source::ParsedSource` and incremental `ParsedSource` still publish the
   detached typed tree while the accepted grammar is crate-private;
2. HIR accepts `&TypedSyntaxTree`, clones detached item/expression structures,
   and retains text-only provenance;
3. compiler, project-loader, CLI, and LSP independently invoke
   `parse_source(...)` instead of sharing one syntax-database lineage;
4. LSP and tooling have detached exhaustive visitors plus range, ordinal, and
   source scanning;
5. fragment payloads own detached `Expr`, `Stmt`, and `Item` values and flow
   through syntax, tooling, Agent REPL, and CLI;
6. descendant substring and callback parsers remain authoritative at several
   call sites;
7. downstream consumers still perform raw reparses;
8. expression source-range support reconstructs descendant ranges;
9. tests and builders construct detached owned enums directly; and
10. rich-text ordered/ranged arguments must migrate as ordinary attached
    expression children rather than being flattened or reparsed.

The required dependency order for the later atomic migration is:

1. finish private attached accessors;
2. make one bound `ParsedSource` own the accepted grammar;
3. introduce HIR entry through `SyntaxNodeId`, typed arenas, and source-backed
   provenance;
4. move compiler and project-loader to syntax-database ownership;
5. move LSP to the same database and lineage;
6. migrate sema, tooling, LSP, CLI, and other detached visitors;
7. define bound and explicitly unbound fragment ownership;
8. replace detached test builders;
9. delete detached trees, substring/source scans, and raw reparse paths; and
10. run workspace, Tier 2, Agent, MCP, and capture validation before publishing
    the public contract.

No compatibility alias, dual public reader, deprecated wrapper, migration
shim, source gate, or historical syntax diagnostic is part of this plan.

## Completion boundary

This cut is complete as private Stage 3 preparation only. It intentionally
does not:

- expose `AstTag`, `SyntaxNodeId`, `AstNode<K>`, or attachment navigation as a
  public API;
- switch `ParsedSource`, HIR, sema, runtime-plan, verifier, CLI, LSP, Agent, or
  MCP consumers;
- preserve the detached AST through an adapter or dual reader;
- implement HIR arenas, proof runtime identity, assertion persistence, or
  codec changes; or
- claim that the atomic Stage 3 public switch is complete.

The next implementation cut must follow the dependency order above. A partial
public switch would create two syntax authorities and is not an acceptable
intermediate state.

## Verification

All commands ran from the repository root:

- `cargo check -p arcweft-lang-syntax --all-features`: passed;
- `cargo test -p arcweft-lang-syntax --all-features`: passed, including 390
  unit tests, every integration and compile-fail suite, and 3 doc tests;
- `cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings`:
  passed;
- `cargo fmt --all -- --check`: passed;
- `git diff --check`: passed; and
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/proof-concurrency-v6-1-1-stage-3-private-inventory-2026-07-21`:
  scanned 3,429 files, 1,785 Rust files, 820,906 Rust physical lines, and 93
  manifests; it reported 0 errors and 131 existing warnings.

Tier 2 is not required for this private syntax-only cut. It changes no public
contract and reaches no runtime, render, Agent, MCP, or capture path. The
subsequent public syntax/HIR switch is a broad integration cut and must run
`just test-tier2`, including reconciliation of current resource URIs, semantic
identities, and authored View geometry.

## Structural audit

The current `arcweft-lang-syntax` dependency graph has 14 incoming and 8
outgoing normal/development edges. No manifest, dependency, feature, or crate
boundary changes in this cut.

| Changed Rust file | Bytes | Physical LOC | Classification | Embedded test LOC | Responsibility |
| --- | ---: | ---: | --- | ---: | --- |
| `src/attachment.rs` | 17,512 | 502 | production | 166 | attachment orchestration, exact parent/child role inventory, invariant tests |
| `src/attachment/error.rs` | 2,606 | 68 | production | 0 | exact lookup and attachment failures |
| `src/attachment/node.rs` | 3,144 | 120 | production | 0 | exact typed marker and cast boundary |
| `src/attachment/snapshot.rs` | 10,801 | 384 | production | 0 | qualified identities, immutable records, snapshot handles |
| `src/grammar.rs` | 522 | 13 | facade | 0 | private grammar responsibility modules |
| `src/grammar/budget.rs` | 13,479 | 390 | production | 71 | transactional grammar budgets |
| `src/grammar/kinds.rs` | 35,882 | 1,110 | production | 60 | discriminant-complete vocabulary, identity classes, typed-family table |
| `src/grammar/roles.rs` | 5,945 | 195 | production | 17 | exact semantic roles and ordinal-free role classes |

No changed production file crosses the 1,200-line warning threshold.
`grammar/kinds.rs` deliberately keeps the two wildcard-free exhaustive
classification tables together: their completeness, rather than brevity, is
the invariant. The module comment and narrow Clippy exemptions record that
cohesive-table exception.

The five largest non-generated workspace production Rust files at this audit
were:

| File | Bytes | Physical LOC |
| --- | ---: | ---: |
| `crates/arcweft-lang-sema/src/checker/module.rs` | 93,423 | 2,482 |
| `crates/arcweft-core/src/engine/eval/calls.rs` | 89,488 | 2,481 |
| `crates/arcweft-core/src/value.rs` | 83,366 | 2,465 |
| `crates/arcweft-cli/src/toolchain_profile.rs` | 75,712 | 2,463 |
| `crates/arcweft-bundle/src/container.rs` | 78,366 | 2,393 |

None is changed by this cut. The typed node, role vocabulary, immutable
snapshot, and attachment orchestration remain separate responsibility modules.
