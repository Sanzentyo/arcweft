# Proof concurrency v6.1.1 — Stage 3 private attached accessors

Date: 2026-07-21

## Outcome

This cut completes the private attached-accessor predecessor required before
the Proof-concurrency v6.1.1 Stage 3 public syntax switch. The accepted source
package is
`arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip`,
with SHA-256
`1b7de5f2c10a5b29d67c72011e4272df9a76af8907fd21fe162de54809fc69ef`.
The cut started from Git
`c59649ad20f4af1f95875e85bf1a22fe74584799`.

The private accepted grammar now has syntax-owned, snapshot-bound access for:

- the whole source file and its ordered top-level items;
- retained declarations whose prefixes and names belong to an exact
  `DeclarationHeader`, and predicate/proof declarations whose equivalent
  children belong directly to the item;
- authored and missing declaration bodies, expression bodies, predicate/proof
  blocks, delimiters, ordered statements, and authored or omitted block tails;
- assertion conditions, `let` pattern/type/initializer children, proof calls,
  binary operands, ordinary call callees, and ordered named/positional call
  arguments;
- parameter patterns and types, whole-binding and record patterns, generic
  type arguments, and function parameter/result types;
- documentation, attributes, visibility, names, paths, declaration parts,
  bodies, delimiters, and recovery/missing families; and
- exact structured failures for missing, ambiguous, wrong-kind, wrong-family,
  non-ordinal, and non-contiguous child access.

Every identity-bearing `SyntaxKind` now has one private zero-sized `AstKind`
marker contract. A discriminant-complete test compares that marker inventory
with the exhaustive identity and `AstTag` tables, rejects duplicate markers,
and rejects markers for wrappers or tokens.

Family nodes are conveniences over explicit concrete-kind predicates. They
cannot be constructed from `AstTag` alone. Exact marker casts still require
the concrete `SyntaxKind` and its expected `AstTag`, and every child accessor
uses the attached `SyntaxRole`. Ordered access checks authored ordinals in
grammar order; it never sorts by range, searches source text, reparses a
substring, or wraps a detached AST value.

## Direct behavioral evidence

The new tests prove:

- `SourceFile -> Element(n) -> Item` navigation preserves authored order and
  exact ranges;
- ordinary predicate/proof prefixes and retained declaration headers expose
  documentation, attributes, visibility, names, parameters, and bodies from
  their actual grammar owner rather than a fabricated uniform wrapper;
- expression bodies, binary children, ordinary call arguments, proof-block
  statements, repeated assertion conditions, pattern children, and type
  children retain exact roles and source order;
- singular access to two valid `Condition` children returns an ambiguity
  error rather than treating repetition as absence;
- missing names, missing bodies, missing closing delimiters, and recovery
  members retain exact concrete kinds and zero-width ranges where applicable;
- wrong exact-kind and wrong exact-family casts return structured errors;
- a typed handle cannot cross an immutable snapshot lineage; and
- typed and Rowan handles continue to round-trip only within their owning
  snapshot.

## Rich-text attachment boundary

The public rich-text parser already retains ordered and ranged tag arguments,
but the private accepted grammar does not yet own identity-bearing rich-text
tag or tag-argument nodes. `DialogueCallExpression` currently retains its
bracket payload losslessly as tokens.

This cut deliberately does not:

- reparse the bracket payload from its source range;
- manufacture shadow `CallArgument` descendants;
- wrap the detached `DialogueTagArg` representation; or
- publish a dual reader.

A direct guard test verifies that rich-text payload spelling produces no
attached `Argument(n)` children in this private predecessor. Rich-text tags and
their ordered scalar/expression payloads must join the accepted grammar in the
same coherent cut that binds the shared parser to `ParsedSource`; only then may
typed tag-argument accessors be enabled. Ordinary call expressions already use
the final exact `CallArgument + Argument(n) + Name/Operand` ownership model.

## Completion boundary

This is still private Stage 3 preparation. It does not:

- expose `TypedSyntaxTree`, `AstNode<K>`, family nodes, snapshot identities, or
  child-access errors as public API;
- switch `ParsedSource`, HIR, sema, runtime-plan, verifier, CLI, LSP, Agent,
  MCP, or capture consumers;
- preserve the detached AST with an adapter, alias, compatibility wrapper, or
  second public reader;
- implement the bound rich-text grammar described above; or
- claim that the package's atomic typed-AST/HIR/runtime identity migration is
  complete.

The next dependency-ordered cut is the bound `ParsedSource`/shared-parser
ownership switch. If that switch cannot remain atomic with its HIR consumer
boundary, implementation must stop before publishing either reader.

## Verification

All commands ran from the repository root:

- `cargo check -p arcweft-lang-syntax --all-features`: passed;
- `cargo test -p arcweft-lang-syntax --all-features`: passed, including 401
  unit tests, every integration and compile-fail suite, and 3 doc tests;
- `cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings`:
  passed;
- `cargo fmt --all -- --check`: passed;
- `git diff --check`: passed; and
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/proof-concurrency-v6-1-1-stage-3-private-accessors-2026-07-21`:
  scanned 3,433 files, 1,788 Rust files, 822,891 Rust physical lines,
  and 93 manifests; it reported 0 errors and 131 existing warnings.

Tier 2 is not required for this private syntax-only cut. It changes no public
contract and reaches no runtime, renderer, Agent, MCP, or capture path. The
later public syntax/HIR switch is a broad integration cut and must run
`just test-tier2`.

## Structural audit

No manifest, dependency, feature, or crate boundary changed. Responsibilities
remain split between attachment orchestration, exact access, family
classification, marker nodes, failures, immutable snapshot data, and role
vocabulary.

| Changed Rust file | Bytes | Physical LOC | Classification | Embedded test module LOC | Responsibility |
| --- | ---: | ---: | --- | ---: | --- |
| `src/attachment.rs` | 35,084 | 982 | production with embedded unit tests | 644 | attachment construction and direct invariant fixtures |
| `src/attachment/access.rs` | 21,968 | 652 | production | 22 | exact-role accessors and typed body/tail unions |
| `src/attachment/error.rs` | 4,310 | 119 | production | 0 | attachment, lookup, and child-access failures |
| `src/attachment/family.rs` | 10,328 | 325 | production | 49 | explicit concrete-kind family nodes |
| `src/attachment/node.rs` | 15,554 | 439 | production | 51 | complete exact marker inventory and typed handles |
| `src/grammar/roles.rs` | 8,219 | 259 | production | 20 | semantic roles, role classes, and owned ordinal projection |

No changed file crosses the applicable structural warning threshold.
`attachment.rs` remains below the 1,200-line production warning threshold;
its next public-consumer cut should move the existing embedded fixtures to a
dedicated unit-test responsibility module before adding materially more tests.

The `arcweft-lang-syntax` graph remains at 14 incoming and 8 outgoing
normal/development workspace edges. The five largest non-generated production
Rust files in this checkout were unchanged by this cut:

| File | Bytes | Physical LOC |
| --- | ---: | ---: |
| `crates/arcweft-lang-sema/src/checker/module.rs` | 93,423 | 2,482 |
| `crates/arcweft-core/src/engine/eval/calls.rs` | 89,488 | 2,481 |
| `crates/arcweft-core/src/value.rs` | 83,366 | 2,465 |
| `crates/arcweft-cli/src/toolchain_profile.rs` | 75,712 | 2,463 |
| `crates/arcweft-bundle/src/container.rs` | 78,366 | 2,393 |
