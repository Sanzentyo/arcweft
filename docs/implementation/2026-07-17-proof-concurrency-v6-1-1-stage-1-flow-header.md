# Proof-concurrency v6.1.1 Stage 1 Flow header grammar

## Scope

This cut completes the common retained `FlowDecl` header shape in the private,
lossless shadow grammar. It is based on main `95a3631483b9` and Jujutsu change
`mnyopyqw`. It does not switch the public parser, syntax identity, HIR,
project, runtime-plan, or runtime contracts.

The existing private Flow body parser remains the sole body authority. This
cut connects its header and body through the document parser's existing lexer,
cursor, and event stream; it introduces no substring reparse, compatibility
alias, removed-syntax recognizer, source gate, CSS route, or Takumi route.

## Typed ownership

- documentation, outer attributes, and visibility attach to the Flow item;
- authored ordinary names become `NameDefinition` nodes;
- entity-reference-only identities remain lossless declaration tokens, while
  the generated `EntityRef Ident` spelling additionally owns its authored
  `NameDefinition`;
- generic and lifetime parameters reuse the shared declaration grammar;
- a Flow owns zero or one typed fixed-parameter group, typed parameter
  patterns and types, an optional return type, and an optional `where` clause;
- `requires` and `ensures` reuse the shared typed contract-clause grammar; and
- the existing `FlowBody` owns the shared block, statement, and expression
  descendants.

Current auxiliary contract families (`invariant`, `assume`, `reads`,
`effects`, `modifies`, and `decreases`) remain byte-preserved under the Flow
owner until their final grammar-node vocabulary is designed. They are not
mistaken for a Flow body and do not prevent subsequent typed `requires` or
`ensures` clauses from attaching. In particular, a brace-delimited
`effects { ... }` list is distinguished from the declaration's actual body
brace by the shared document grouping rule. This cut does not invent
provisional public node kinds for those clauses.

## Recovery

- a second fixed-parameter group is retained in an `ErrorNode` with the shared
  invalid-header diagnostic rather than becoming a curried Flow signature;
- a missing identity owns `MissingName`;
- a missing body owns `MissingBody`; and
- malformed headers synchronize before the following unindented declaration,
  which remains a sibling item.

Every case remains byte-lossless. The private shadow output allocates no
production `SyntaxNodeId` and is not executable.

## Direct evidence

`parser::shadow_flow_tests` covers an attributed public generated identity,
generic and lifetime parameters, a typed state parameter, result type,
`where`, an auxiliary effect list, typed pre/postconditions, and typed Flow
statements in one declaration. Separate cases cover all three declaration
identity spellings, the zero-parameter form, curried-group recovery, and a
missing identity/body followed by a clean proof.

The focused suite passes 4/4. The complete `arcweft-lang-syntax` all-targets
suite passes 197 unit tests together with every integration and UI test.

## Validation

The following commands pass with `CARGO_INCREMENTAL=0` where Cargo is used:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-lang-syntax shadow_flow_tests --lib -- --nocapture
cargo test -p arcweft-lang-syntax --all-targets
cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/proof-concurrency-v6-1-1-stage-1-flow-header-2026-07-17
```

The production fixture first exposed that document grouping treated the brace
of `effects { ... }` as the declaration body and stopped before later clauses.
The shared continuation and brace rules now distinguish contract-list braces
from function/Flow bodies. A later Clippy run rejected only a redundant test
closure; it was replaced by the owning method reference and the exact syntax
and workspace Clippy commands then passed.

## Structure

The canonical report is stored under
`structure-audits/proof-concurrency-v6-1-1-stage-1-flow-header-2026-07-17/`.
It scanned 3,091 files, 1,546 Rust files, 708,476 physical Rust LOC, and 90
manifests with zero errors and 128 pre-existing repository-wide warnings. No
warning names an in-scope file.

- `parser.rs`: 24,106 bytes / 699 physical LOC, production parser facade;
- `parser/document.rs`: 20,952 bytes / 658 physical LOC, private
  document/event orchestration and declaration grouping;
- `parser/shadow_flow.rs`: 3,706 bytes / 123 physical LOC, production Flow
  header/body boundary;
- `parser/shadow_recovery.rs`: 5,681 bytes / 213 physical LOC, shared header
  boundaries; and
- `parser/shadow_flow_tests.rs`: 4,584 bytes / 137 physical LOC, direct test
  module.

All changed production files remain below repository warning thresholds. This
cut changes no Cargo dependency, feature, public API, serialization contract,
or crate boundary, so dependency fan-in and fan-out are unchanged.

## Remaining Stage 1 work

Stage 1 remains open. Auxiliary contract families need their final typed node
decision, and the remaining retained declaration families plus malformed and
recovery cross-products still require direct typed descendants. Lang-01 owns
the migration-gated top-level reductions. Proof-concurrency Stages 2 through 8
remain open.
