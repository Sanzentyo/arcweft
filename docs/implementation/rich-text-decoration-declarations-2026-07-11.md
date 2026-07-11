# Reusable Rich-Text Decoration Declarations — 2026-07-11

> Historical implementation record: the provisional `decoration` surface
> described below has been removed and replaced without compatibility aliases
> by typed `#[fx] fn ... -> Fx` entries. See
> [Fx Function Presentation Graph](fx-function-presentation-graph-2026-07-11.md)
> for the current implementation and verification evidence.

## Result

Arcweft now has module-level, reusable visual-decoration declarations for
dialogue text. A declaration can combine existing span builders, declare
required and defaulted named parameters, accept an explicit custom-argument
bag, and compose another declaration:

```arcw
decoration emphasis(accent = "#ffd060") {
    strong()
    color(value=accent)
}

decoration notice(
    accent,
    amplitude = 2px,
    seed = "notice",
    ...effect_args,
) {
    decorate(.emphasis, accent=accent)
    effect(.wave, amp=amplitude, seed=seed, effect_args...)
}

alice: [decorate .notice accent="#ff6b8a" speed=2]warning[/decorate][p]
```

The explicit `[decorate .name ...]` spelling is intentional. Bare `[name]`
tags remain unknown controls. A dot tag without attributes remains a marker,
while a dot tag with attributes remains an inferred custom effect. Adding a
declaration therefore cannot silently reinterpret existing dialogue content.

## Implemented contract

- `decoration` is a typed, module-local top-level syntax/HIR item and preserves
  attributes, documentation, source ranges, default expressions, and builder
  layers. `pub decoration` is rejected rather than promising an export path the
  module-local selector grammar cannot represent.
- Parameters are named. `name` is required, `name = value` has a default, and
  one final `...rest` parameter captures otherwise unknown named invocation
  arguments. Positional invocation arguments are rejected.
- Defaults, overrides, and builder values must be deterministic closed
  authoring values. HIR owns the shared `DecorationConst` classifier used by
  sema and runtime-plan; signed durations are accepted, while character
  literals and runtime expressions are rejected consistently.
- Quoted effect/custom parameter values preserve their string type through
  default, override, and rest binding and lower as `RichTextParam::Text`;
  unquoted booleans, integers, milli values, selectors, and renderer-owned raw
  tokens retain the ordinary rich-text parameter inference contract.
- A bare identifier in a builder is a declared parameter reference. Raw
  registry words must be quoted or written as selectors, making misspelled
  parameter names diagnosable.
- Supported layers are `em`, `strong`, `color`, `font`, `size`, `style`,
  `layout`, `transform`, `effect`, and nested `decorate`. Composition cycles,
  missing required values, duplicate values, invalid spreads, and unknown
  declarations/builders are rejected semantically.
- Empty bodies and malformed builder shapes are rejected. Style, layout, and
  transform use shared closed canonical selector inventories; effect selectors
  remain registry-extensible, and all selector positions require literal
  `.Ident` syntax rather than a parameter.
- Declarations cannot hide dialogue controls, reveal speed, object identity,
  calls, signals, conditionals, or `phase=host_event` effects. The abstraction
  is visual and deterministic rather than a second control-flow surface.
- Runtime-plan lowering expands each invocation to the existing ordered
  `StyleStart`/`StyleEnd` nodes and closes layers in reverse order as one
  authored span. Nested inferred style closes remain valid, while crossing,
  reset, missing, or unmatched decoration closes are rejected.
- Expanded layers also produce `InlineSpan` cascade contributions from the
  same typed expansion result. Direct and decoration tags retain authored
  order, declaration defaults point back to the invocation selector, explicit
  overrides point to their value ranges, and LSP hover reports the style that
  is actually rendered.
- The renderer, bundle codec, and session-save schemas are unchanged because
  no decoration declaration survives as retained runtime state.
- Dialogue-tag tokenization is quote-aware and exposes typed argument/end-tag
  ranges. Dialogue content owns a provenance map from its indentation-stripped,
  LF-normalized bytes back to the original document, and runtime-plan, tooling,
  and LSP consumers project through that map instead of treating token ranges
  as document-absolute. This permits values containing whitespace or `]`
  without string splitting or losing diagnostic locations across LF/CRLF input.
- Expression-form `fn(args)[content]` dialogue calls now own a boxed typed
  `DialogueContent`, including text diagnostics and LF/CRLF provenance. Sema
  checks their decorations, spans, marks, and interpolations before lowering;
  runtime-plan consumes the typed content without lossy reparsing.
- Direct `color`, `font`, and `size` tags now resolve short, quoted, and
  canonical `value=...` spellings through one scalar boundary. Canonicalizing
  `[color #a8b5ff:text]` therefore preserves the same typed RGB value.
- Rich-text end-tag aliases are owned once by `arcweft-dialogue`; syntax and
  retained-text rendering no longer maintain drifting copies of the family
  inventory.
- Expansion budgets have success-at-the-limit and overflow coverage for depth,
  declaration visits, and concrete layers.
- `just test-rich-text` now covers syntax, HIR, sema, runtime-plan, both rich
  text render layers, the reusable-decoration sample, and the native visual
  smoke route.

## Verification

All final validation passed:

```bash
cargo fmt --all
git diff --check
just --unstable --fmt --check
cargo check -p arcweft-dialogue -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-lang-sema -p arcweft-render-text -p arcweft-runtime-plan -p arcweft-tooling -p arcweft-lsp --all-targets --all-features
cargo test -p arcweft-dialogue -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-lang-sema -p arcweft-render-text -p arcweft-runtime-plan -p arcweft-tooling -p arcweft-lsp --lib --tests --no-fail-fast
cargo test -p arcweft-runtime-plan render_text::decoration
cargo test -p arcweft-render-text quoted_hex_color_uses_the_same_typed_rgb_value_as_unquoted_color
cargo test -p arcweft-tooling canonical_rich_text_preserves_explicit_decoration_spans
cargo run -p arcweft-cli --quiet -- check samples/rich-text-decorations.arcw
cargo run -p arcweft-cli --quiet -- check samples/rich-text-full-grammar.arcw
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-rich-text
just test-workspace
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/rich-text-decoration-declarations-2026-07-11
```

An earlier `just test-workspace` attempt ended before tests ran for one crate
because rustc reported an LLVM output-stream I/O failure while another local
workspace was compiling concurrently. After that process finished, the same
recipe completed successfully without a code change. The final post-review
run also passed in 655.9 seconds. `just test-rich-text` completed the native
visual and exact-capture smoke tests in 383.5 seconds.

## Structural audit

The audit was captured for Jujutsu change `lnyouptm` over repository revision
`ff77940d`. It scanned 2,537 files and 1,189 Rust files (605,266 physical Rust
LOC), with **0 errors and 150 warnings**. The complete reports are under
`docs/implementation/structure-audits/rich-text-decoration-declarations-2026-07-11/`.

Two intentional normal dependency edges were added so one low-level dialogue
vocabulary owns rich-text family aliases: `arcweft-lang-syntax ->
arcweft-dialogue` and `arcweft-render-text -> arcweft-dialogue`.
`arcweft-dialogue` now has fan-out 4 and fan-in 3; `arcweft-lang-syntax` has
fan-out 5 and fan-in 13; `arcweft-render-text` has fan-out 4 and fan-in 16.
No dependency points from a lower runtime/data layer into compiler semantics or
tooling.

Task-relevant measurements from the current checkout are:

| Owning crate | File | Bytes | Physical LOC | Kind | Embedded test LOC | Main responsibility |
| --- | --- | ---: | ---: | --- | ---: | --- |
| `arcweft-dialogue` | `src/rich_text.rs` | 1,446 | 45 | production | 17 | shared rich-text family and alias vocabulary |
| `arcweft-lang-hir` | `src/decoration.rs` | 8,206 | 229 | production | 31 | shared builder, constant, and resource-limit policy |
| `arcweft-lang-syntax` | `src/text.rs` | 39,209 | 1,121 | production | 191 | quote-aware dialogue/ruby/span tokenization and typed relative ranges |
| `arcweft-lang-syntax` | `src/parser/decoration.rs` | 19,052 | 535 | production | 141 | declaration grammar, recovery, and parser tests |
| `arcweft-lang-syntax` | `src/parser/dialogue.rs` | 31,939 | 809 | production | 0 | dialogue forms and expression-content provenance |
| `arcweft-lang-syntax` | `src/ast/dialogue.rs` | 27,295 | 1,014 | production | 0 | typed content, tokens, diagnostics, and source-map boundary |
| `arcweft-lang-syntax` | `src/ast/dialogue/source_map.rs` | 8,775 | 242 | production | 0 | normalized-content to authored-document provenance |
| `arcweft-lang-syntax` | `src/expr.rs` | 68,851 | 2,243 | production | 53 | expression AST, including boxed typed dialogue content |
| `arcweft-lang-syntax` | `src/expr/source_ranges.rs` | 51,734 | 1,488 | production | 215 | structural expression and dialogue-content source projection |
| `arcweft-lang-sema` | `src/checker/decoration.rs` | 29,080 | 820 | production | 0 | catalog, parameter/body validation, and cycle detection |
| `arcweft-lang-sema` | `src/checker/decoration/expansion.rs` | 12,349 | 354 | production | 35 | binding-sensitive expansion validation and budgets |
| `arcweft-lang-sema` | `src/checker/decoration/span.rs` | 6,721 | 186 | production | 0 | atomic span and typed host-event phase validation |
| `arcweft-runtime-plan` | `src/render_text/decoration.rs` | 32,876 | 925 | production | 2 | binding, custom-argument forwarding, and typed layer expansion |
| `arcweft-runtime-plan` | `src/render_text/decoration/contributions.rs` | 7,783 | 234 | production | 0 | expanded-layer cascade assignments and provenance |
| `arcweft-runtime-plan` | `src/render_text/decoration/expander.rs` | 9,106 | 232 | production | 0 | authored-span state, contribution order, and reverse close expansion |
| `arcweft-runtime-plan` | `src/render_text/decoration/tests.rs` | 23,868 | 741 | test | 0 | expansion, nesting, constants, limits, contributions, and malformed spans |
| `arcweft-tooling` | `src/dialogue_content.rs` | 17,329 | 475 | production | 0 | shared typed dialogue-content traversal for tooling consumers |
| `arcweft-render-text` | `src/lib.rs` | 58,188 | 1,745 | production/facade | 500 | resolved rich-text model and canonical scalar conversion |

`text.rs` triggered the audit because it grew by more than 300 LOC, but remains
below the 1,200-LOC warning threshold and its added code belongs to the existing
text/token boundary. Source provenance is isolated in
`ast/dialogue/source_map.rs`; declaration expansion, span state, resource
budgets, cascade contributions, and tests also have separate responsibility
modules.

Three task-touched files retain pre-existing warning-level hotspots.
`expr.rs` (SIZE001/TEST001) changed only its dialogue-call field, which is boxed
so the typed content does not inflate downstream enums. `expr/source_ranges.rs`
(SIZE001/TEST001) owns grammar-sensitive structural projection and received the
matching typed-content traversal; it remains below the 2,500-LOC error level.
`arcweft-render-text/src/lib.rs` (SIZE001/SIZE002/TEST001) received the owning
scalar conversion used by color/font/size. Decomposing either broad expression
grammar or the existing render facade is an independent repository refactor,
not missing reusable-decoration behavior, and no error-level exception was
introduced.

## Remaining TODOs and deviations

There are no missing items for the accepted reusable-decoration contract and
no design deviations. No follow-up design request is required. The existing
expression/source-range and render-facade warning-level decompositions remain
repository-wide structural cleanup rather than feature gaps.
