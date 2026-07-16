# Proof-concurrency v6.1.1 Stage 1 Flow dialogue context

## Scope

This review cut is based on Git `2a8454f48767`. It gives the private lossless
grammar direct ownership of Flow bodies and of bracketed dialogue-call
expressions inside Flow expression positions. It remains crate-private and does
not change the production AST, HIR, runtime, serialized formats, or public
parser API.

The shared document lexer and cursor now emit `FlowBody`, `Block`, statement,
and expression descendants without reparsing source strings. In Flow
expression positions, call-shaped dialogue surfaces such as
`alice.say()[本文です。[p]]` and direct non-ASCII dialogue surfaces such as
`alice[おはよう。[p]]` emit `DialogueCallExpression`. Ordinary index expressions
such as `rows[0]` and `rows[index]` continue to emit `IndexExpression`.

An unclosed call-shaped dialogue surface emits a typed missing close and
`syntax.expression.missing_dialogue_close`, then synchronizes before the next
top-level declaration. This diagnostic describes current grammar recovery; it
is not a removed-syntax recognizer.

## Ownership

- `parser::shadow_flow` owns the private Flow body boundary and delegates its
  body to the shared statement grammar;
- `parser::expression` owns dialogue/index disambiguation over already lexed
  tokens and the `DialogueCallExpression` event shape;
- `parser::statement` selects dialogue-aware expression parsing only for Flow
  positions;
- `parser::document` retains the only full-source lexer, cursor, event stream,
  and lossless build orchestration.

There is no second source parse, string projection, compatibility branch,
source gate, CSS route, or Takumi route in this cut.

## Direct evidence

`flow_dialogue_context_distinguishes_content_from_indexing` checks the two
dialogue surfaces and two ordinary indexes in one Flow body, verifies their
independent typed grammar nodes, and requires byte-for-byte green-tree source
round-trip.

`unclosed_dialogue_content_recovers_before_the_next_item` checks a missing
dialogue close, the typed diagnostic, retained `DialogueCallExpression`, and
successful synchronization to a following proof item.

## Validation

The following commands pass with `CARGO_INCREMENTAL=0`:

```bash
cargo test -p arcweft-lang-syntax parser::dialogue_expression_tests --lib -- --nocapture
cargo test -p arcweft-lang-syntax --lib
cargo clippy -p arcweft-lang-syntax --all-targets -- -D warnings
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The focused tests pass 2/2 and the syntax library passes 175/175. Workspace
check and Clippy complete with all targets and all features and no warning. The
structural audit scans 2,980 files, 1,466 Rust files, 684,123 physical Rust LOC,
and 90 package manifests with zero errors and 128 repository-wide warnings.

The canonical report is under
`structure-audits/proof-concurrency-v6-1-1-stage-1-dialogue-context-2026-07-16/`.

- `parser.rs`: 23,777 bytes / 684 physical LOC, production facade;
- `parser/document.rs`: 17,683 bytes / 556 physical LOC, production
  full-source orchestration;
- `parser/expression.rs`: 18,000 bytes / 554 physical LOC, production Pratt and
  dialogue-context expression grammar;
- `parser/statement.rs`: 25,064 bytes / 744 physical LOC, production statement
  grammar;
- `parser/shadow_flow.rs`: 1,483 bytes / 46 physical LOC, production Flow
  boundary;
- `parser/dialogue_expression_tests.rs`: 2,622 bytes / 88 physical LOC, direct
  unit-test module.

All in-scope production files remain below structural warning thresholds. No
Cargo dependency, feature, crate boundary, or dependency fan-in/fan-out changes
in this cut.

## Remaining work

Stage 1 remains open. Complete descendants for the remaining item families and
the remaining malformed/recovery cross-products still need direct full-source
events and tests. Stages 2 through 8 remain open; this cut does not allocate
production syntax identity, publish the shadow tree, switch AST/HIR, or change
runtime assertion identity. The following
`2026-07-16-proof-concurrency-v6-1-1-stage-1-declaration-diagnostics.md` cut
reconciles the shared predicate/proof recovery codes without changing this
Flow/dialogue ownership boundary.
