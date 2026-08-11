# Final contract

## 1. Normative status and scope

This document is normative for AW-AH-009.3.1. `MUST`, `MUST NOT`, `SHALL`, and `SHALL NOT` are binding implementation requirements.

The selected model makes authored syntax explicit without changing semantic call meaning:

1. Parenthesized calls and postfix callback-block applications remain one semantic `Expr::Call` variant.
2. The variant wraps a private-field `CallExpr` whose exhaustive `CallSurfaceSyntax` distinguishes the two authored surfaces.
3. Only the parenthesized surface owns `ArgumentListSyntax`.
4. Callback blocks own exact callback syntax and never synthesize `(`, `)`, or comma ranges.
5. Source-AST calls are parser-created only.
6. Generated applications use a source-independent semantic/runtime expression type.
7. All AW-AH-009.3 query, resolver, cache, identity, limit, precedence, and accepted-world policies remain unchanged except that the resolver first projects a parenthesized carrier.

No optional argument-list field is placed on `CallExpr`. The enum itself proves which syntax exists.

## 2. Final public syntax shape

The following shapes are owned by `arcweft-lang-syntax::expr`. Fields shown without `pub` are private. All types derive `Clone`, `Debug`, `Eq`, and `PartialEq`. Existing unrelated `Expr` variants are unchanged.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expr {
    // Existing variants remain in their current order and shape.
    Call(CallExpr),
    // Existing variants continue.
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallExpr {
    callee: Box<Expr>,
    args: Vec<CallArg>,
    syntax: CallSurfaceSyntax,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallSurfaceSyntax {
    Parenthesized(ParenthesizedCallSyntax),
    CallbackBlock(CallbackBlockCallSyntax),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParenthesizedCallSyntax {
    callee: TextRange,
    arguments: ArgumentListSyntax,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallbackBlockCallSyntax {
    callee: TextRange,
    callback: CallbackBlockSyntax,
}
```

`TextRange` remains the existing half-open UTF-8 byte range. The range coordinate space is the exact source accepted by the owning parse operation: byte zero for standalone expression fragments and document-absolute bytes for full-source parsing. Mapped dialogue source must project token ranges into document coordinates before these types are constructed.

### 2.1 Read-only call accessors

```rust
impl CallExpr {
    pub fn callee(&self) -> &Expr;
    pub fn args(&self) -> &[CallArg];
    pub const fn syntax(&self) -> &CallSurfaceSyntax;
    pub const fn parenthesized_syntax(&self) -> Option<&ParenthesizedCallSyntax>;
    pub const fn callback_block_syntax(&self) -> Option<&CallbackBlockCallSyntax>;
    pub const fn callee_range(&self) -> TextRange;
    pub const fn range(&self) -> TextRange;
}

impl CallSurfaceSyntax {
    pub const fn callee_range(&self) -> TextRange;
    pub const fn range(&self) -> TextRange;
    pub const fn argument_list(&self) -> Option<&ArgumentListSyntax>;
}

impl ParenthesizedCallSyntax {
    pub const fn callee_range(&self) -> TextRange;
    pub const fn argument_list(&self) -> &ArgumentListSyntax;
    pub const fn range(&self) -> TextRange;
}

impl CallbackBlockCallSyntax {
    pub const fn callee_range(&self) -> TextRange;
    pub const fn callback(&self) -> &CallbackBlockSyntax;
    pub const fn range(&self) -> TextRange;
}
```

`ParenthesizedCallSyntax::range()` is derived as `callee.start()..arguments.end_offset()`. `CallbackBlockCallSyntax::range()` is derived as `callee.start()..callback.close_brace().end()`. No duplicated stored full range can drift from its delimiters.

No public mutable accessor is added. No public constructor creates any of these call values.

## 3. Exact parenthesized argument-list carrier

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArgumentListSyntax {
    open_paren: TextRange,
    arguments: Vec<CallArgumentSyntax>,
    separators: Vec<TextRange>,
    trailing_comma: Option<TextRange>,
    terminator: ArgumentListTerminatorSyntax,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArgumentListTerminatorSyntax {
    Closed {
        close_paren: TextRange,
    },
    RecoveredMissing {
        insertion: usize,
        boundary: CallRecoveryBoundarySyntax,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallRecoveryBoundarySyntax {
    EndOfExpression,
    Token {
        kind: CallRecoveryTokenKind,
        range: TextRange,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallRecoveryTokenKind {
    Comma,
    Semicolon,
    Colon,
    FatArrow,
    CloseParen,
    CloseBracket,
    CloseBrace,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallArgumentSyntax {
    range: TextRange,
    value: TextRange,
    form: CallArgumentFormSyntax,
    recovery: CallArgumentRecoverySyntax,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallArgumentFormSyntax {
    Positional,
    Named {
        name: TextRange,
        equals: TextRange,
    },
    Spread {
        ellipsis: TextRange,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallArgumentRecoverySyntax {
    Parsed,
    Recovered {
        diagnostic: TextRange,
    },
}
```

The spread form records Arcweft's current postfix ellipsis. For `value...`, `value` ends at or before `ellipsis.start()` and the full argument ends at `ellipsis.end()`.

### 3.1 Argument-list accessors

```rust
impl ArgumentListSyntax {
    pub const fn open_paren(&self) -> TextRange;
    pub fn arguments(&self) -> &[CallArgumentSyntax];
    pub fn separators(&self) -> &[TextRange];
    pub const fn trailing_comma(&self) -> Option<TextRange>;
    pub const fn terminator(&self) -> &ArgumentListTerminatorSyntax;
    pub const fn close_paren(&self) -> Option<TextRange>;
    pub const fn recovery_boundary(&self) -> Option<CallRecoveryBoundarySyntax>;
    pub const fn end_offset(&self) -> usize;
    pub const fn range(&self) -> TextRange;
    pub const fn content_range(&self) -> TextRange;
    pub const fn contains_signature_cursor(&self, cursor: usize) -> bool;
    pub fn active_argument_slot(&self, cursor: usize) -> Option<usize>;
}

impl ArgumentListTerminatorSyntax {
    pub const fn close_paren(&self) -> Option<TextRange>;
    pub const fn end_offset(&self) -> usize;
}

impl CallArgumentSyntax {
    pub const fn range(&self) -> TextRange;
    pub const fn value_range(&self) -> TextRange;
    pub const fn form(&self) -> &CallArgumentFormSyntax;
    pub const fn recovery(&self) -> CallArgumentRecoverySyntax;
}
```

`content_range()` starts at `open_paren.end()`. For a closed list it ends at `close_paren.start()`. For a recovered missing close it ends at `insertion`.

`contains_signature_cursor(cursor)` is true when:

- `open_paren.end() <= cursor <= close_paren.start()` for a closed list; or
- `open_paren.end() <= cursor <= insertion` for a recovered missing close.

It is false at `open_paren.start()`, after the authored `)`, and after the owning recovery boundary.

`active_argument_slot(cursor)` first requires `contains_signature_cursor(cursor)`. It returns the number of separators whose `end() <= cursor`, including the empty next slot after a trailing comma. The AW-AH-009.3 resolver retains its existing named, reordered, duplicate, spread, partial-call, overload, and parameter-clamping policy; this method supplies the exact syntactic slot only.

### 3.2 Argument-list invariants

A value is constructible only when all rules below hold:

1. `open_paren` denotes the exact one-byte ASCII `(` token.
2. `Closed.close_paren` denotes the exact one-byte ASCII `)` token.
3. Each separator and trailing comma denotes an exact one-byte ASCII `,` token.
4. Every range is a valid UTF-8 byte boundary in the owning source.
5. `open_paren` precedes every argument, comma, and terminator.
6. `arguments` are non-overlapping and in authored order.
7. `separators.len() == arguments.len().saturating_sub(1)`.
8. Separator `i` lies strictly between argument `i` and argument `i + 1`.
9. `trailing_comma` is present only when at least one argument exists and lies after the last argument and before the terminator.
10. A named argument has a nonempty identifier range, an exact `=` token range, and a value range after `=`. Its full range begins at the name and ends at the value.
11. A spread argument has an exact `...` token after its value. Its full range ends at the ellipsis.
12. A positional argument's full and value ranges are equal.
13. `CallArgumentRecoverySyntax::Recovered.diagnostic` is nonempty, lies within the argument's value range, and corresponds to one retained parser diagnostic.
14. A closed list ends at `close_paren.end()`.
15. A recovered list stores no close-paren range. Its `insertion` is the exact byte position at which `)` is missing.
16. For `EndOfExpression`, `insertion` equals the exclusive end supplied by the owning parser.
17. For a token boundary, `insertion == range.start()`, the boundary token is authored and not consumed by the call parser, and the token kind matches the lexer token.
18. The semantic `CallArg` vector has exactly one element per `CallArgumentSyntax` in the same order and with the same positional/named/spread form.

Whitespace and comments are not stored as punctuation. They remain recoverable from the owning `SourceDocument` between exact token ranges.

## 4. Exact callback-block carrier

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallbackBlockSyntax {
    open_brace: TextRange,
    parameters: CallbackParameterHeaderSyntax,
    body: TextRange,
    close_brace: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallbackParameterHeaderSyntax {
    ImplicitZero,
    Explicit {
        parameters: Vec<CallbackParameterSyntax>,
        separators: Vec<TextRange>,
        fat_arrow: TextRange,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallbackParameterSyntax {
    range: TextRange,
    pattern: TextRange,
    type_ascription: Option<CallbackParameterTypeSyntax>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallbackParameterTypeSyntax {
    colon: TextRange,
    ty: TextRange,
}
```

### 4.1 Callback accessors

```rust
impl CallbackBlockSyntax {
    pub const fn open_brace(&self) -> TextRange;
    pub const fn parameters(&self) -> &CallbackParameterHeaderSyntax;
    pub const fn body_range(&self) -> TextRange;
    pub const fn close_brace(&self) -> TextRange;
    pub const fn closure_range(&self) -> TextRange;
}

impl CallbackParameterHeaderSyntax {
    pub fn parameters(&self) -> &[CallbackParameterSyntax];
    pub fn separators(&self) -> &[TextRange];
    pub const fn fat_arrow(&self) -> Option<TextRange>;
    pub const fn is_implicit_zero(&self) -> bool;
}

impl CallbackParameterSyntax {
    pub const fn range(&self) -> TextRange;
    pub const fn pattern_range(&self) -> TextRange;
    pub const fn type_ascription(&self) -> Option<CallbackParameterTypeSyntax>;
}

impl CallbackParameterTypeSyntax {
    pub const fn colon(&self) -> TextRange;
    pub const fn ty_range(&self) -> TextRange;
}
```

`closure_range()` is derived from `open_brace.start()..close_brace.end()`.

### 4.2 Callback invariants

1. `open_brace` and `close_brace` denote exact authored one-byte ASCII brace tokens.
2. A typed callback surface always has a closing brace. An unclosed callback remains an incomplete/invalid callback parse; no close range is invented.
3. `body` is nonempty, lies inside the braces, and spans the first through last nontrivia body token. Multi-statement bodies use one exact enclosing range.
4. `ImplicitZero` corresponds to a closure with zero parameters and no `=>` token.
5. `Explicit` contains at least one parameter; the current empty-header spelling `{ => body }` remains rejected.
6. Explicit separators are exact comma tokens and `separators.len() == parameters.len() - 1`.
7. `fat_arrow` is the exact authored `=>` token after the final parameter and before the body.
8. Each parameter is in authored order. Its full range starts at its pattern and ends at its type when typed.
9. A type ascription contains an exact `:` token and a nonempty type range after it.
10. The syntax parameter count, order, pattern spelling, and optional type-ascription presence match the semantic `Expr::Closure` parameter vector exactly.
11. A callback call contains exactly one semantic argument: `CallArg::Positional(Expr::Closure { .. })`.
12. No callback syntax type exposes `ArgumentListSyntax`, parenthesis ranges, comma-separated call arguments, or an active-parameter index for the outer callback application.

## 5. Parser-only construction and validation

The public `Expr::call` and `Expr::selected_call` constructors are deleted. No replacement public constructor, deprecated alias, extension trait, compatibility module, or unchecked builder is introduced.

Construction is crate-private and validation is performed before a value enters the source AST:

```rust
impl ArgumentListSyntax {
    pub(crate) fn try_from_parser(
        source: &str,
        source_base: usize,
        init: ArgumentListSyntaxInit,
    ) -> Result<Self, CallSyntaxInvariantError>;
}

impl CallbackBlockSyntax {
    pub(crate) fn try_from_parser(
        source: &str,
        source_base: usize,
        init: CallbackBlockSyntaxInit,
    ) -> Result<Self, CallSyntaxInvariantError>;
}

impl ParenthesizedCallSyntax {
    pub(crate) fn try_from_parser(
        callee: TextRange,
        arguments: ArgumentListSyntax,
    ) -> Result<Self, CallSyntaxInvariantError>;
}

impl CallbackBlockCallSyntax {
    pub(crate) fn try_from_parser(
        callee: TextRange,
        callback: CallbackBlockSyntax,
    ) -> Result<Self, CallSyntaxInvariantError>;
}

impl CallExpr {
    pub(crate) fn try_parenthesized(
        callee: Expr,
        args: Vec<CallArg>,
        syntax: ParenthesizedCallSyntax,
    ) -> Result<Self, CallSyntaxInvariantError>;

    pub(crate) fn try_callback_block(
        callee: Expr,
        closure: Expr,
        syntax: CallbackBlockCallSyntax,
    ) -> Result<Self, CallSyntaxInvariantError>;
}
```

The crate-private init records contain the fields of the corresponding public type and are not re-exported. `try_from_parser` checks known spans against the exact source bytes at construction time; it does not search for delimiters. Lexer token identity is authoritative. Checked arithmetic is mandatory for every base/range projection.

`CallSyntaxInvariantError` is crate-private and exhaustive:

```rust
pub(crate) enum CallSyntaxInvariantError {
    InvalidUtf8Boundary,
    InvalidTokenRange,
    RangeOrder,
    ArgumentCountMismatch,
    ArgumentFormMismatch,
    SeparatorCountMismatch,
    InvalidTrailingComma,
    InvalidRecoveryBoundary,
    InvalidCalleeRange,
    InvalidCallbackArgument,
    InvalidCallbackParameterHeader,
    InvalidCallbackBody,
    OffsetOverflow,
}
```

An invariant error is an internal parser failure, not a user-recoverable grammar diagnostic. It aborts the parse transaction or current top-level parse result according to the existing parser failure boundary. It is never converted to a fake range.

## 6. Recovery contract

### 6.1 Recovering expression result

The strict expression parser and the full-source parser share one parser implementation. The internal result is:

```rust
pub(crate) struct ParsedExpr {
    expr: Expr,
    range: TextRange,
    diagnostics: Vec<ExprParseError>,
    stats: ExprParseStats,
}

pub(crate) struct ExprParseScope {
    source_range: TextRange,
    end_boundary: CallRecoveryBoundarySyntax,
}

pub(crate) fn parse_expr_recovering_at(
    source: &str,
    scope: ExprParseScope,
) -> Result<ParsedExpr, ExprParseError>;
```

`scope.source_range` identifies the exact bytes offered by the owner. For `EndOfExpression`, its end is the recovery insertion point. For a token boundary, the token is present in the owning token stream and remains unconsumed.

The existing strict public fragment API calls this function and returns an error when `diagnostics` is nonempty. Full document, View, dialogue, speaker, and line-plan parsers append the diagnostics to their existing error sink and retain the recovered expression. Thus ordinary strict parsing remains strict while source/HIR for editor features can retain a typed call.

### 6.2 Missing `)`

After an authored `(`, the parser returns a parenthesized `CallExpr` with `ArgumentListTerminatorSyntax::RecoveredMissing` when it reaches the owner-supplied end or a typed owner boundary before `)`.

The call parser SHALL:

1. finish any already parsed argument;
2. stop before the boundary token;
3. record `insertion` at the exact boundary start or owner end;
4. emit one missing-close diagnostic whose primary range is the zero-width insertion position and whose related range is `open_paren`;
5. leave the boundary token available to the owning construct;
6. derive the recovered call range through the insertion point;
7. never synthesize a close-paren `TextRange`.

A nested owner such as an outer argument list, bracket sequence, index, record field, closure header, dialogue option head, or speaker colon therefore retains its own delimiter and recovery authority.

### 6.3 Isolated malformed argument

When one argument expression fails, the parser synchronizes only to the next top-level comma, authored `)`, or owner boundary. Nested delimiters are respected. A nonempty synchronized segment becomes the corresponding current `CallArg` form with `Expr::Raw(exact_segment_source)` as its value, and its `CallArgumentSyntax` is marked `Recovered { diagnostic }`.

The parser retains a named head and spread suffix when those tokens were parsed unambiguously before value failure. No new invalid `CallArg` variant is added.

Empty arguments, consecutive commas, a missing comma between two otherwise parseable expressions, malformed named heads, and impossible spread placement remain ordinary grammar errors. They do not create empty ranges or phantom argument slots.

### 6.4 Offset preservation

Call-producing parse paths SHALL operate on original bytes plus an exact base or existing source map. The current `normalize_dot_continuations` path, raw `find`-based call-head recovery, and `parse_static_generic_call` source scan are removed from call construction.

Static generic/turbofish callees are parsed by the Pratt/path token grammar before the source-scanning helper is deleted. Dialogue mapped source projects each known token span through `DialogueContentSourceMap`; it never searches the document to relocate a token.

## 7. Dialogue and speaker ownership

The shared `ArgumentListSyntax` is also the only parenthesized carrier for dialogue/content and colon-style speaker special surfaces. These are not converted into fake ordinary `Expr::Call` nodes.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeakerLineSurface {
    source_line: TextRange,
    head: TextRange,
    argument_list: Option<ArgumentListSyntax>,
    inline_content: Option<TextRange>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentCallSurface {
    head: TextRange,
    callee: TextRange,
    argument_list: Option<ArgumentListSyntax>,
    content: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentCall {
    callee: String,
    options: LineOptions,
    content: DialogueContent,
    plan: Option<LinePlan>,
    surface: ContentCallSurface,
    range: TextRange,
}
```

Accessors are:

```rust
impl SpeakerLineSurface {
    pub const fn source_line_range(&self) -> TextRange;
    pub const fn head_range(&self) -> TextRange;
    pub const fn argument_list(&self) -> Option<&ArgumentListSyntax>;
    pub const fn inline_content_range(&self) -> Option<TextRange>;
}

impl ContentCallSurface {
    pub const fn head_range(&self) -> TextRange;
    pub const fn callee_range(&self) -> TextRange;
    pub const fn argument_list(&self) -> Option<&ArgumentListSyntax>;
    pub const fn content_range(&self) -> TextRange;
}

impl ContentCall {
    pub const fn surface(&self) -> &ContentCallSurface;
}
```

`SpeakerLineSurface` ceases to be `Copy`; `SpeakerLine::surface()` returns `&SpeakerLineSurface`.

`alice(args): text` and `alice.say(args)[text]` own an exact list. `alice: text` and `alice[text]` have `None` because no parentheses were authored. `None` here denotes absence of an authored optional surface on a special-form AST, not a compatibility escape for `Expr::Call`.

The speaker/content parser uses the same token-level parenthesized-list parser as ordinary calls. `LineOptions` is built by zipping semantic `CallArg` values with the parallel `CallArgumentSyntax` entries. Raw values and value ranges come from those exact entries; the parser does not split the argument text again.

## 8. Signature-help applicability and cursor contract

The one AW-AH-009.3 sema resolver consumes only a parenthesized `ArgumentListSyntax` projection.

For an ordinary call:

```rust
let Some(parenthesized) = call.parenthesized_syntax() else {
    return Ok(SignatureQueryOutcome::NotApplicable);
};
let argument_list = parenthesized.argument_list();
```

For a speaker or content special form, the resolver reads `surface.argument_list()`. A special form without authored parentheses is `NotApplicable`.

The outer `CallSurfaceSyntax::CallbackBlock` is always `NotApplicable` at every cursor position in its callback braces, parameter header, arrow, body, and brace boundaries. The resolver and its cache are not invoked for that outer surface. A nested parenthesized call inside the callback body is selected normally by the existing innermost-call rule.

For `Button("Send").on_click { action.invoke(value = label) }`:

- a cursor in `Button("Send")` uses that parenthesized call;
- a cursor on `.on_click` or in the callback's outer brace/header/body space is not applicable to the outer callback call;
- a cursor in `action.invoke(...)` uses the nested parenthesized call.

The remaining AW-AH-009.3 active-parameter rules are unchanged: authored named/reordered arguments map by semantic binding; duplicate and spread handling remains typed; nested calls choose the innermost applicable list; recovered lists remain eligible through their insertion boundary; positions after an authored close are inapplicable.

## 9. HIR and sema preservation

Current Arcweft HIR retains `arcweft_lang_syntax::expr::Expr` directly. The new syntax types derive `Clone`, so lowering and module cloning preserve the exact `CallSurfaceSyntax` without a parallel HIR call enum or a second source map.

All semantic consumers that do not care about authored punctuation migrate from:

```rust
Expr::Call { callee, args } => check(callee, args)
```

to:

```rust
Expr::Call(call) => check(call.callee(), call.args())
```

They SHALL NOT branch on the surface. Type checking, effect checking, runtime lowering, verifier lowering, project facts, labels, and ordinary traversal continue to consume semantic callee/arguments.

The single signature resolver is the only semantic consumer required to project `ParenthesizedCallSyntax`. It does not clone or reparse source and does not create a second resolver for callbacks.

No new serialization format is introduced. The call-surface types are immutable in-memory syntax/HIR data bound by the existing `SourceDocumentIdentity` path selected by AW-AH-009.3.

## 10. Non-authored generated applications

`arcweft_lang_syntax::Expr` represents authored or recovered source syntax. It has no `Generated` call surface and no source-less call constructor.

Current genuinely generated executable applications use the existing source-independent `arcweft_core::value::RuntimeExpr::Call` and the existing runtime-plan builders/lowering contexts. That model owns semantic callee/arguments and intentionally owns no `TextRange`, `ArgumentListSyntax`, or callback delimiter.

Rules:

1. A test that needs source-AST behavior parses an authored source string or attached fragment and receives real ranges.
2. A runtime/compiler generator that needs an executable call constructs `RuntimeExpr::Call` through its existing semantic owner.
3. No current caller may create a source `Expr::Call` solely to feed runtime lowering.
4. A future need for generated, type-checkable semantic expressions requires a separately reviewed semantic-IR owner; it does not reopen source-AST construction in this cut.

This is the complete selected non-authored representation. No fake source document, zero range, optional syntax payload, or generated syntax variant is permitted.

## 11. Limits, charging, and failures

This reconciliation introduces no client-configurable limit and does not change AW-AH-009.3 limits.

- Every parsed or recovered argument counts as one existing argument/expression unit.
- Every callback parameter counts against the existing closure-parameter limit.
- Each retained parser diagnostic counts against the existing diagnostic limit.
- Validation is linear in the number of arguments/parameters and charges existing parse work.
- Every offset addition, subtraction, range projection, and list count uses checked arithmetic.
- Limit or arithmetic failure preserves the existing parser transaction/failure boundary and never publishes a partially validated call surface.

Signature cache keys and invalidation remain those of AW-AH-009.3. Callback `NotApplicable` results are not inserted as successful resolver entries because the resolver is not called.

## 12. Direct replacement and forbidden compatibility

The final repository contains only `Expr::Call(CallExpr)`. It contains none of the following:

- the old struct-like `Expr::Call { callee, args }`;
- a legacy `Expr::Call` spelling in another enum for source AST compatibility;
- public `Expr::call` or `Expr::selected_call`;
- a public unchecked `CallExpr` constructor;
- `Option<ArgumentListSyntax>` on ordinary `CallExpr`;
- zero-length fake delimiter ranges;
- callback braces stored as an argument list;
- a callback-specific signature resolver;
- a source scan that reconstructs call delimiters or argument ownership;
- an extension trait, compatibility alias/module, deprecated accessor, dual AST, or removed-syntax recognizer.

Compiler breakage from the direct variant replacement is the inventory mechanism for exhaustive migration. The implementation is merged only after the workspace is restored to a single compiling model.
