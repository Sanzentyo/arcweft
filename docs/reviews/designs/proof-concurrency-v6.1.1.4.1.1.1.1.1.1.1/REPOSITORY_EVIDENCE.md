# Repository evidence

## Baseline and policy

- audited `main`: `004ff3d69f241954eb808985878c348b165a815c` (`Adjudicate corrected Proof Select return`);
- `AGENTS.md` blob: `e91f99213dde67953beda6aa078c370a8dc4541d`;
- current request blob: `f8964cabe5da7fcd546edfb78c84ebae7ef73523`;
- second rejection intake blob: `dcef5d910bdc282b4b4aaf954c61ca15b4212476`.

The applicable repository rules require typed layer ownership, direct edits to
Arcweft-owned boundary types, deletion-first migration, and prohibit aliases,
wrappers, extension traits, dual readers, source reparsing, source gates, and
compatibility preservation for unreleased compiler/parser contracts.

## Parser and attachment evidence

| Path | Blob | Evidence used |
|---|---|---|
| `crates/arcweft-lang-syntax/src/parser/lexer.rs` | `605329d3f3135058edfb20a3b518e176672e17a9` | current longest-match `?.` and `..`; final switch deletes combined `?.`, retains compact `..` range token |
| `.../src/parser/expression.rs` | `12ebaa6b9b5d5054c0fa2455652b16c87f00ef2e` | leading dot is ShortVariant; postfix dot creates NameReference or zero-width MissingName after trivia; compact range routes to RangeExpression |
| `.../src/parser/path.rs` | `7eb85cb8a14e4e7c005f3366f5c7bf4c3c513e29` | current shared path walk explains why the replacement uses parenthesized poisoned-target fixtures instead of claiming compact/spaced dotted paths without proof |
| `.../src/parser/expression/composite.rs` | `70969b626f43b268e99040d3550e6fd5640d8681` | parenthesized expression is a lossless DelimitedGroup whose operand remains the semantic target; delimiters preserve authored geometry without a HIR wrapper ID |
| `.../src/expr/lexer.rs` | `ef8eb3cff64b7cb53ce55412c4cdb83478829ba2` | semantic lexer exposes postfix `?` separately from dot |
| `.../src/expr/pratt.rs` | `c3e7469c7c944c11b29595674c37c9b27921c6ac` | postfix order constructs Try before following Select |
| `.../src/expr/prefix.rs` | `8acb710e140939d5d7ffa2e32184fe9981a6a8fe` | value paths consume `::`, leaving ordinary dot to Select in the semantic parser |
| `.../src/attachment.rs` | `5bcaa6a539364ad278b78abae99e1e6c017f9746` | one immutable attachment snapshot, identity map, and no public second reader |
| `.../src/attachment/access.rs` | `e0c045978b96b0e3fa24b040a62c0f48711c6741` | central typed child access and exact role validation |
| `.../src/attachment/node.rs` | `02dcd76d9554381e973dbeb327aca79ec713f138` | exact expression/name markers, including SelectExpression and MissingName |
| `.../src/attachment/family.rs` | `c80fb11d78472241b2a9e5060a11e909f2094f36` | expression/name/recovery family boundaries |
| `.../src/grammar/kinds.rs` | `f6e03b3f69295a48706a44b8ddbe5452cc5b269e` | identity-bearing vocabulary and structural PathSegment/DelimitedGroup behavior |
| `.../src/grammar/roles.rs` | `4453cc551a9eb713e70fbfcc2950e4df3b9ced7c` | existing Target/Field roles; final central projection adds SelectedMember on the original expression-component enum |

The protected `ExpressionProjection`/`PendingExpressionProjection`/
`AttachedExpressionNode` integration is intentionally not yet public in this
revision. Its complete E13-facing shape is fixed by the request and is extended
in place; this package does not infer a parallel owner from missing WIP files.

## Limits and final owner evidence

- `SyntaxLimit` blob `08708ed362da8f903b0653dd9b74067a199c0207`:
  Expressions 262,144; IdentityBearingNodes 1,048,576; Diagnostics 1,024.
- grammar budget charges only actual `SyntaxEvent::Diagnostic`; MissingName by
  itself emits none.
- source document blob `e1b1a545d28f62704a7e7b517620b85b6ffe73b6`:
  registration source bytes 8,388,608 inclusive.
- private final expression owner blob
  `afef8645afe78c205d5f7255223bdd19714752ea` currently contains the single
  private `HirName`; the public expression switch is deliberately pending.
- current sema still imports detached `SelectExpr` in
  `crates/arcweft-lang-sema/src/checker/expr/member.rs`; this is deletion
  inventory, not authority.

## Current-main reconciliation

The latest commit adds the second rejection intake and corrected request. It
changes no production Select implementation. Therefore the package baseline is
exactly the repository state whose parser, attachment, and frozen consumer
inventory the request requires.
