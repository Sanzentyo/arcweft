# AW-AH-007/008 — private attached RichText grammar

Date: 2026-07-21

## Outcome

This cut implements the private syntax prerequisite for the later Proof public
authority switch. The accepted source package is
`arcweft-aw-ah-007-008-typed-rich-text-attribute-validation-final-contract.zip`,
with SHA-256
`dbf72681e97377fc6a5b592579bf29f1e5640105acf1d4446d13d0209fcfd209`.
Implementation started from Git
`118a987065f7a2985086794a604e3bc4ee8623b0`.

The private lossless document grammar now owns `RichText` tags and their
argument descendants directly inside `DialogueCallExpression`. It uses the
document parser's existing lexer cursor, event stream, attachment transaction,
and identity allocation. It does not call the public dialogue parser, reparse a
substring through `parse_expr`, or attach a detached AST value.

The existing public D1/D2/D3.1 dialogue syntax remains the production
authority. A syntax-neutral argument scan in `text::rich_text_tag` supplies the
same lexical classification, decoded scalar value, diagnostics, and absolute
ranges to both the public representation and the private grammar. This is an
internal shared scanner, not a second public reader.

## Private node ownership

Each authored tag receives `RichTextTag(n)` in source order. The private tree
owns exact nodes for:

- opening tags, end tags, and tag names;
- ordinary argument payloads and ordered `Argument(n)` descendants;
- positional, named, invalid, and missing-value arguments;
- named keys, the first unescaped unquoted `=`, and value nodes;
- authored value tokens, content, opening quotes, and closing quotes;
- exact invalid-argument issue ranges; and
- dedicated `fx`, dialogue-call, and condition payloads.

Dedicated expression payloads invoke the existing private Pratt emitter over
the same document token slice. They do not retain a string payload for a later
parse. Ordinary argument values partition the already-lexed token at their
exact ranges when a lexical token spans two RichText descendants, including
`pattern==value` and quoted content.

## Recovery and transaction invariants

The shared argument scan supplies the same missing and invalid ranges to both
consumers. The private grammar retains zero-width missing values and distinct
identity paths for invalid argument and issue nodes even when those nodes have
the same source range.

An unterminated double-quoted value no longer hides the recovering `]` from the
private grammar. A normally closed string remains one token, including `]` in
its content, and an escaped `\]` does not become a recovery boundary. The
unterminated quote diagnostic retains its exact authored range, and subsequent
RichText tags remain attached in source order. The recovery token has a
dedicated unterminated-string kind, so the same lexer change cannot turn a
malformed ordinary expression string into a valid literal.

Escapes, interpolation, natural and ASCII ruby, raw spans, inline raw spans,
inline styles, and bracket ruby are classified as bounded opaque dialogue
surfaces before tag emission. Brackets inside those surfaces therefore do not
receive invented RichText identities. The classifier retains the public
content-limit charges without constructing or reading a public dialogue AST.

The private grammar also applies the public 4,096-tag, 32,768-content-argument,
and 16,384-byte tag-body limits before allocating descendants. Excess and
overlong markup remains lossless and opaque, including nested bracket bytes.
Each exhausted content budget emits its diagnostic once, latches for the rest
of the dialogue, suppresses every further over-budget descendant allocation,
and coalesces adjacent recovery text instead of allocating one recovery node
per excess surface.

Attachment failure is still atomic. The direct failure-injection test proves
that a RichText attachment failure commits neither lineage nor node slots; a
retry receives the same slots as an uncontended control database.

## Public boundary and non-goals

This cut deliberately does not:

- publish the private typed tree or make it the `ParsedSource` authority;
- change the public AST, HIR, sema, runtime-plan, formatter, LSP, Agent, MCP,
  renderer, or capture path;
- invoke `parse_dialogue_text` from production private grammar code;
- preserve a dual reader, public adapter, compatibility wrapper, or removed
  syntax recognizer; or
- claim completion of the atomic Proof syntax/HIR/runtime identity switch.

The public switch must still migrate all compiler and tooling consumers in one
coherent authority change. This cut only removes the RichText inventory gap
that previously blocked that migration.

## Direct behavioral evidence

The focused and crate-wide tests prove:

- authored tag and argument order, roles, ranges, and lossless Rowan text;
- agreement between public and private tag/argument/key/equals/value/quote
  ranges across CRLF, Unicode whitespace, and Unicode content;
- exact partitioning when one lexer token contains both `=` and value bytes;
- distinct present-empty and zero-width missing value identities;
- invalid escape and unterminated quote issue ranges;
- dedicated typed expression children for `fx`, `call`, and `if` payloads;
- recovery before a following tag after an unterminated quote;
- absence of invented tag identities inside escape, interpolation, ruby, raw,
  inline-raw, and inline-style surfaces;
- exact content-tag, content-argument, and tag-body limits, including a single
  latched limit diagnostic, allocation suppression after exhaustion, and
  opaque recovery for an overlong body containing an inner tag spelling;
- rejection of a non-dialogue unterminated string as a literal expression;
- distinct identities for equal-range recovery descendants; and
- lineage and node-slot rollback after injected attachment failure.

## Verification

All commands ran from the repository root:

- `cargo check -p arcweft-lang-syntax --all-targets --all-features`: passed;
- `cargo test -p arcweft-lang-syntax --no-fail-fast`: passed, including 428
  unit tests, every integration and compile-fail suite, and 3 doc tests;
- `cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings`:
  passed;
- `cargo fmt --all -- --check`: passed;
- `git diff --check`: passed; and
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/aw-ah-007-008-private-rich-text-grammar-2026-07-21`:
  scanned 3,465 files, 1,806 Rust files, 831,695 Rust physical lines, and 94
  manifests; it reported 0 errors and 133 existing warnings.

Tier 2 is not required for this private syntax-only cut. It changes no public
contract and reaches no runtime, renderer, Agent, MCP, or capture path. The
later public authority switch remains a Tier 2 integration cut.

## Structural boundary

No manifest, dependency, feature, public API, or crate boundary changed. The
new `parser/rich_text_grammar.rs` responsibility module owns only private event
emission over the shared scan and document cursor. The small
`text/dialogue_opaque.rs` module owns bounded classification of non-tag
dialogue surfaces. Neutral tag and argument facts remain with the RichText
scanner, while attachment access, exact-kind markers, family membership, and
semantic roles remain in their existing responsibility modules.

The final audit ran after rebasing onto `main` at
`888a0c094c19628d3175e0b6654875684b264e84`. Its detailed reports are in
[the checked-in audit directory](structure-audits/aw-ah-007-008-private-rich-text-grammar-2026-07-21/).
The `arcweft-lang-syntax` graph remains 14 incoming and 8 outgoing normal or
development workspace dependency edges; no edge changed in this cut.

| Changed Rust file | Bytes | Physical LOC | Classification | Embedded test LOC | Responsibility |
| --- | ---: | ---: | --- | ---: | --- |
| `src/incremental/database_tests.rs` | 68,114 | 1,918 | unit-test module | 1,918 | database rollback and identity reconciliation fixtures |
| `src/attachment.rs` | 47,471 | 1,353 | production with embedded tests | 1,023 | attachment transaction plus direct private invariant fixtures |
| `src/grammar/kinds.rs` | 38,328 | 1,174 | production with embedded tests | 60 | exhaustive node/token and identity vocabulary |
| `src/text.rs` | 37,189 | 1,061 | production with embedded tests | 260 | public dialogue scan orchestration and latched text limits |
| `src/parser/document.rs` | 32,317 | 999 | production | 0 | one-pass document cursor and exact token partition access |
| `src/text/rich_text_tag.rs` | 31,455 | 936 | production | 0 | shared neutral RichText tag/argument lexical scan |
| `src/parser.rs` | 25,809 | 775 | parser facade | 0 | private parser responsibility modules and public entry points |
| `src/attachment/access.rs` | 25,197 | 737 | production with embedded tests | 22 | role-driven attached RichText accessors |
| `src/parser/rich_text_grammar.rs` | 24,118 | 691 | production | 0 | private RichText event emission, token partitioning, and latched limits |
| `src/parser/expression.rs` | 18,404 | 565 | production | 0 | dialogue-call integration with private RichText emission |
| `src/parser/lexer.rs` | 15,807 | 531 | production with embedded tests | 39 | shared lexer and unterminated-string recovery boundary |
| `src/attachment/node.rs` | 16,683 | 461 | production with embedded tests | 52 | exact attached marker inventory |
| `src/attachment/family.rs` | 11,314 | 348 | production with embedded tests | 50 | explicit RichText family membership |
| `src/grammar/roles.rs` | 9,334 | 299 | production with embedded tests | 20 | RichText roles, role classes, and ordinals |
| `src/parser/dialogue_expression_tests.rs` | 8,980 | 282 | unit-test module | 282 | dialogue recovery, limit, and lossless private grammar fixtures |
| `src/text/dialogue_opaque.rs` | 7,900 | 198 | production | 0 | bounded non-tag dialogue-surface classification |
| `src/parser/document_tests.rs` | 4,963 | 161 | unit-test module | 161 | one-pass lexer and ordinary-expression recovery fixtures |
| `tests/rich_text_tag_arguments.rs` | 17,853 | 529 | integration test | 529 | public lossless scan parity, recovery, and latched content-limit fixtures |

`attachment.rs` crosses the 1,200-line warning only because its 330-line
production transaction is followed by 1,023 lines of direct private attachment
fixtures. The audit reports no error-level ownership violation. The tests stay
co-located in this private predecessor because they exercise constructors and
failure injection that are not public API; the public-consumer switch should
move the accumulated fixtures to a dedicated test responsibility module before
adding another material attachment test slice.

For completeness, the largest unchanged non-generated production files in the
same checkout were:

| File | Bytes | Physical LOC |
| --- | ---: | ---: |
| `crates/arcweft-lang-sema/src/checker/module.rs` | 93,423 | 2,482 |
| `crates/arcweft-core/src/engine/eval/calls.rs` | 89,488 | 2,481 |
| `crates/arcweft-core/src/value.rs` | 83,366 | 2,465 |
| `crates/arcweft-cli/src/toolchain_profile.rs` | 75,712 | 2,463 |
| `crates/arcweft-bundle/src/container.rs` | 78,366 | 2,393 |
