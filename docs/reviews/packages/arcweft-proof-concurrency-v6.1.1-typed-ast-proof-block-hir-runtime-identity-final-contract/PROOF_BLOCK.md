# Typed predicate/proof body and `ProofBlock` contract

## 1. Ownership and visibility

The final types live in `arcweft-lang-syntax::ast::predicate_proof`. Every field is private. Constructors are `pub(crate)` and are called only by `attachment.rs` after the grammar tree and syntax identities are complete. Public callers receive immutable, cloneable snapshot-owned handles.

No type in this module stores an authoritative source string. Source text is obtained from its attached `SourceDocument` and node range.

## 2. Exact surface types

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PredicateItem {
    node: AstNode<PredicateItemKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofItem {
    node: AstNode<ProofItemKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PredicateBody {
    node: AstNode<PredicateBodyKind>,
    form: PredicateBodyForm,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PredicateBodyForm {
    Expression(PredicateExpressionBody),
    Block(PredicateBlock),
    Missing(MissingBody),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofBody {
    node: AstNode<ProofBodyKind>,
    form: ProofBodyForm,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofBodyForm {
    Expression(ProofExpressionBody),
    Block(ProofBlock),
    Missing(MissingBody),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PredicateExpressionBody {
    node: AstNode<ExpressionBodyKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofExpressionBody {
    node: AstNode<ExpressionBodyKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PredicateBlock {
    node: AstNode<PredicateBlockKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofBlock {
    node: AstNode<ProofBlockKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PredicateStmt {
    Let(PureLetStmt),
    /// Recovery-only structured assertion; semantic validation always rejects it.
    Assertion(AssertionStmt),
    Error(ErrorStmt),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofStmt {
    Let(PureLetStmt),
    ProofCall(ProofCallStmt),
    Assertion(AssertionStmt),
    Error(ErrorStmt),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PureLetStmt {
    node: AstNode<LetStatementKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofCallStmt {
    node: AstNode<ProofCallStatementKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorStmt {
    node: AstNode<ErrorStatementKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BlockTail {
    Authored(ExprNode),
    Omitted(OmittedBlockTail),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OmittedBlockTail {
    node: AstNode<OmittedBlockTailKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissingBody {
    node: AstNode<MissingBodyKind>,
}
```

`ExprNode`, `PatternNode`, `TypeNode`, `AssertionStmt`, attributes, visibility, generic parameters, fixed parameters, `where` predicates, and contract clauses are the shared typed authorities from their owning modules. There are no proof-local clones of expression, pattern, type, or assertion enums.

`PredicateBody` and `ProofBody` are structs, not payload-only enums, because the grammar-level body wrapper is independently identity-bearing. Their `form` fields carry the expression/block/missing choice without hiding the wrapper `SyntaxNodeId`.

## 3. Public accessors

```rust
impl PredicateItem {
    pub fn id(&self) -> SyntaxNodeId;
    pub fn attributes(&self) -> impl ExactSizeIterator<Item = OuterAttribute> + '_;
    pub fn docs(&self) -> Option<DocBlock>;
    pub fn visibility(&self) -> Option<VisibilityNode>;
    pub fn name(&self) -> NameNode;
    pub fn generic_parameters(&self) -> Option<GenericParameterGroup>;
    pub fn parameters(&self) -> FixedParameterGroup;
    pub fn where_clause(&self) -> Option<WhereClauseNode>;
    pub fn requires(&self) -> impl ExactSizeIterator<Item = RequiresClause> + '_;
    pub fn ensures(&self) -> impl ExactSizeIterator<Item = EnsuresClause> + '_;
    pub fn body(&self) -> PredicateBody;
    pub fn range(&self) -> SourceRange;
    pub fn source_span(&self) -> SourceSpan;
    pub fn is_recovered(&self) -> bool;
}

impl ProofItem {
    pub fn id(&self) -> SyntaxNodeId;
    pub fn attributes(&self) -> impl ExactSizeIterator<Item = OuterAttribute> + '_;
    pub fn docs(&self) -> Option<DocBlock>;
    pub fn visibility(&self) -> Option<VisibilityNode>;
    pub fn name(&self) -> NameNode;
    pub fn generic_parameters(&self) -> Option<GenericParameterGroup>;
    pub fn parameters(&self) -> FixedParameterGroup;
    pub fn return_type(&self) -> Option<ReturnTypeNode>;
    pub fn where_clause(&self) -> Option<WhereClauseNode>;
    pub fn requires(&self) -> impl ExactSizeIterator<Item = RequiresClause> + '_;
    pub fn ensures(&self) -> impl ExactSizeIterator<Item = EnsuresClause> + '_;
    pub fn body(&self) -> ProofBody;
    pub fn range(&self) -> SourceRange;
    pub fn source_span(&self) -> SourceSpan;
    pub fn is_recovered(&self) -> bool;
}

impl PredicateBody {
    pub fn id(&self) -> SyntaxNodeId;
    pub fn form(&self) -> &PredicateBodyForm;
    pub fn range(&self) -> SourceRange;
    pub fn source_span(&self) -> SourceSpan;
}

impl ProofBody {
    pub fn id(&self) -> SyntaxNodeId;
    pub fn form(&self) -> &ProofBodyForm;
    pub fn range(&self) -> SourceRange;
    pub fn source_span(&self) -> SourceSpan;
}

impl PredicateExpressionBody {
    pub fn id(&self) -> SyntaxNodeId;
    pub fn expression(&self) -> ExprNode;
    pub fn range(&self) -> SourceRange;
}

impl ProofExpressionBody {
    pub fn id(&self) -> SyntaxNodeId;
    pub fn expression(&self) -> ExprNode;
    pub fn range(&self) -> SourceRange;
}

impl PredicateBlock {
    pub fn id(&self) -> SyntaxNodeId;
    pub fn open_brace(&self) -> DelimiterNode<OpenBrace>;
    pub fn statements(&self) -> impl ExactSizeIterator<Item = PredicateStmt> + '_;
    pub fn tail(&self) -> BlockTail;
    pub fn close_brace(&self) -> DelimiterNode<CloseBrace>;
    pub fn range(&self) -> SourceRange;
}

impl ProofBlock {
    pub fn id(&self) -> SyntaxNodeId;
    pub fn open_brace(&self) -> DelimiterNode<OpenBrace>;
    pub fn statements(&self) -> impl ExactSizeIterator<Item = ProofStmt> + '_;
    pub fn tail(&self) -> BlockTail;
    pub fn close_brace(&self) -> DelimiterNode<CloseBrace>;
    pub fn range(&self) -> SourceRange;
}

impl PredicateStmt {
    pub fn id(&self) -> SyntaxNodeId;
    pub fn range(&self) -> SourceRange;
}

impl ProofStmt {
    pub fn id(&self) -> SyntaxNodeId;
    pub fn range(&self) -> SourceRange;
}

impl ErrorStmt {
    pub fn id(&self) -> SyntaxNodeId;
    pub fn range(&self) -> SourceRange;
}

impl BlockTail {
    pub fn id(&self) -> SyntaxNodeId;
    pub fn range(&self) -> SourceRange;
    pub const fn is_authored(&self) -> bool;
}

impl MissingBody {
    pub fn id(&self) -> SyntaxNodeId;
    pub fn range(&self) -> SourceRange;
}
```

Both block accessors always return a `BlockTail`. If no authored tail exists, attachment creates one zero-width `OmittedBlockTail` node. Both brace accessors always return a delimiter node. A missing brace node wraps `MissingToken` and exposes `is_missing() == true`.

## 4. Identity inventory

The following are distinct identity-bearing grammar nodes:

- predicate/proof item;
- each outer attribute and the attached doc block;
- visibility and name;
- generic parameter group and each generic parameter;
- fixed parameter group, each parameter, each parameter pattern, and each parameter type;
- `where` clause and each predicate/type/bound child;
- each `requires` and `ensures` clause and its expression;
- the `PredicateBody`/`ProofBody` wrapper returned by `body()`, with its own directly accessible `id()`;
- expression-body wrapper and its expression;
- block;
- opening brace node;
- closing or missing brace node;
- every statement and all typed descendants;
- authored tail expression or zero-width `OmittedBlockTail`;
- every error or missing semantic child.

Tokens have no ID. The statement-list wrapper has no ID.

An expression body therefore has two independent IDs: the body wrapper and the expression. A one-expression block has five independent top-level body IDs: the body wrapper, block, opening brace, tail expression, and closing brace. They can never compare equal to an expression body.

## 5. `PureLetStmt`

```rust
impl PureLetStmt {
    pub fn id(&self) -> SyntaxNodeId;
    pub fn pattern(&self) -> PatternNode;
    pub fn type_annotation(&self) -> Option<TypeNode>;
    pub fn initializer(&self) -> ExprNode;
    pub fn terminator(&self) -> StatementTerminator;
    pub fn range(&self) -> SourceRange;
}
```

The syntax parser accepts the shared pattern grammar so recovery remains typed. Sema requires:

- an irrefutable pattern;
- no `mut` binding in predicate/proof context;
- a pure initializer;
- no duplicate binding name; and
- no binding named `result`.

Lowering evaluates/resolves the initializer in the existing scope before allocating pattern locals. Bindings become visible only after the initializer. A failed requirement poisons the statement and affected locals; it does not trigger source reparsing.

## 6. `ProofCallStmt`

```rust
impl ProofCallStmt {
    pub fn id(&self) -> SyntaxNodeId;
    pub fn call(&self) -> CallExprNode;
    pub fn terminator(&self) -> StatementTerminator;
    pub fn range(&self) -> SourceRange;
}
```

The surface statement is selected only for a call-shaped expression followed by a statement terminator. Sema resolves the callee through `ProjectSymbolTable` and requires `CallableDeclarationOwner::Proof`. A function or predicate call statement retains this typed shape but is poisoned with `sema.proof.expression_statement_not_proof_call`.

A call at the end of a block without a statement terminator is `BlockTail::Authored`, not `ProofCallStmt`.

## 7. Assertions

`ProofStmt::Assertion` contains the one existing `AssertionStmt` authority. It retains its `AssertionMode` and ordered typed expression conditions.

- `AssertionMode::Prove`: accepted in proof context and never enters runtime-plan.
- `AssertionMode::Check` or `Debug`: structured statement retained, semantic context error, proof non-executable.
- predicate context: `PredicateStmt::Assertion` preserves the one existing typed assertion node as a recovery-only variant; every mode is a semantic context error and the predicate is non-executable.

The condition limit remains 64. Condition order is source order and later becomes zero-based runtime identity only for runtime-capable assertions outside proof context.

## 8. Tail semantics and synthetic HIR

### 8.1 Authored tail

`BlockTail::Authored(expr)` uses the expression's `SyntaxNodeId` as the source-backed HIR key. Its range is exactly the expression range and excludes the closing brace and trailing trivia.

### 8.2 Omitted tail

`OmittedBlockTail` is zero-width at the first byte of the closing brace, or at the missing-close recovery anchor. It has a syntax ID for tooling and reconciliation but is not lowered as a source-backed expression.

After return type resolution:

- Unit proof: allocate `ExprId` with `SyntheticKey { owner: Syntax(block.id()), role: ImplicitUnitTail, ordinal: 0 }`; value is `HirExprKind::Unit`; zero-width span is the omitted-tail anchor; not poisoned.
- non-Unit proof: allocate `ExprId` with `SyntheticKey { owner: Syntax(block.id()), role: MissingRequiredTail, ordinal: 0 }`; value is `HirExprKind::Error`; poisoned; emit `sema.proof.missing_value_tail`.
- predicate: allocate poisoned `HirExprKind::Error` with `SyntheticKey { owner: Syntax(block.id()), role: MissingRequiredTail, ordinal: 0 }`; emit `sema.predicate.missing_boolean_tail`.

The synthetic ordinal is always zero because a block has one tail role.

## 9. Statement and tail ordering

Attachment projects direct identity-bearing children of the statement-list wrapper in source order. It enforces:

1. every terminated child before the final unterminated expression is a statement;
2. at most one tail exists;
3. no statement follows an authored tail;
4. recovery after an apparent tail converts the malformed fragment to `ErrorStatement` and uses an omitted tail;
5. comments/trivia remain lossless but never appear in the typed statement iterator.

The order is immutable and is used by HIR statement arenas and scope construction.

## 10. Poison and executability

Each typed node exposes its attached diagnostics through the owning `ParsedSource`; the node itself does not own a mutable diagnostic vector. `is_recovered()` is derived from snapshot attachment metadata.

A predicate/proof is non-executable when any of the following is present:

- recovered/missing name, generic, parameter, return, `where`, clause, body, delimiter, statement, expression, pattern, or type that affects the declaration;
- invalid clause order;
- assertion context violation;
- impure let/call/clause;
- missing or mismatched required tail;
- duplicate/reserved/poisoned binding;
- unresolved/incorrect proof call; or
- recursive predicate/proof SCC.

Recovered HIR still contains items, scopes, statements, expressions, types, patterns, locals, and diagnostics for tooling. It cannot be admitted to executable sema/codegen/runtime caches.

## 11. Ranges

All ranges are half-open UTF-8 byte ranges in the exact `SourceDocument`.

| Node | Balanced form | Empty form | Missing-close form |
|---|---|---|---|
| block | opening `{` start through closing `}` end | same, with no statements and omitted tail at `}` start | opening `{` start through recovery anchor/EOF |
| open brace | exact `{` token | exact `{` token | zero-width only when opening token itself is missing |
| close brace | exact `}` token | exact `}` token | zero-width at next top-level declaration start or EOF |
| omitted tail | closing `}` start | closing `}` start | missing-close recovery anchor |
| statement | first non-trivia statement token through semicolon, or through last token before logical newline | not applicable | recovered fragment range |
| expression body | `=` start through expression and optional semicolon, excluding newline | missing expression uses zero-width child | through accepted recovery terminator |
| error statement | first unexpected token through balanced logical statement boundary | zero-width only for a wholly missing required statement child | never consumes following clean declaration |

The block range includes braces. The body wrapper range equals the block range for block bodies. A missing body is zero-width at the end of the accepted header.

## 12. Counting

The syntax transaction counts:

- every `PredicateStmt`/`ProofStmt`, including `ErrorStmt`, against the statement budget;
- every nested expression condition/argument/tail against the expression budget;
- every parameter/local pattern descendant against the pattern budget;
- every annotation/return/where type descendant against the type budget;
- every missing/error semantic node against the identity-bearing-node budget;
- every structured diagnostic after deterministic exact deduplication against the diagnostic budget; duplicate emission from one parser event cannot consume the budget twice.

HIR counts are applied independently during lowering. A syntax transaction can succeed and a HIR transaction can still fail atomically on a tighter HIR/module/slot limit.

## 13. Direct builder policy

Production constructors remain crate-private. Tests that need a typed-child/display disagreement use a `#[cfg(test)] pub(crate)` builder in `attachment::test_support` that:

- creates a real in-memory `SourceDocument`;
- constructs an immutable attached test snapshot through the same event builder;
- allows a non-authoritative presentation label to differ from the typed child; and
- never exposes raw IDs or an unchecked production constructor.

Lowering tests prove the typed child controls behavior. No test scans repository text or reparses the display label.
