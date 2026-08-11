# Final predicate and proof grammar

## 1. Lexical conventions

The grammar uses the existing Arcweft lexer, identifier, lifetime, attribute, documentation, visibility, path, expression, pattern, type, generic-bound, and `where` vocabularies. The new declaration keywords are `predicate` and `proof`. Neither declaration accepts an entity-reference token or another authored artifact identifier.

A logical line ends at a physical newline or EOF when the delimiter nesting depth is zero. Newlines inside `()`, `[]`, `{}`, or `<>` do not terminate a clause. A depth-zero semicolon also terminates a clause/body expression. All real bytes, including terminators, trivia, comments, and recovery tokens, remain in the lossless tree.

## 2. Normative EBNF

```ebnf
PredicateDeclaration =
    OuterPrefix* Visibility? "predicate" Identifier
    GenericParameterGroup? FixedParameterGroup
    WhereClause? RequiresClause* EnsuresClause*
    PredicateBody ;

ProofDeclaration =
    OuterPrefix* Visibility? "proof" Identifier
    GenericParameterGroup? FixedParameterGroup
    ReturnType? WhereClause? RequiresClause* EnsuresClause*
    ProofBody ;

OuterPrefix = OuterAttribute | DocCommentLine+ ;

Visibility = "pub" | "pub" "(" "crate" ")" | "pub" "(" "super" ")" ;

GenericParameterGroup =
    "<" (GenericParameter ("," GenericParameter)* ","?)? ">" ;

GenericParameter = LifetimeParameter | TypeParameter ;
LifetimeParameter = LifetimeToken ;
TypeParameter = Identifier (":" TraitBound ("+" TraitBound)*)? ;

FixedParameterGroup =
    "(" (Parameter ("," Parameter)* ","?)? ")" ;

Parameter = Pattern ":" Type ;

ReturnType = "->" Type ;

WhereClause =
    "where" WherePredicate ("," WherePredicate)* ","? ;
WherePredicate = Type ":" TraitBound ("+" TraitBound)* ;

RequiresClause = "requires" Expression ClauseTerminator ;
EnsuresClause  = "ensures"  Expression ClauseTerminator ;
ClauseTerminator = ";" | LogicalLineEnd ;

PredicateBody = ExpressionBody | PredicateBlock ;
ProofBody     = ExpressionBody | ProofBlock ;

ExpressionBody = "=" Expression BodyTerminator ;
BodyTerminator = ";" | LogicalLineEnd | EndOfFile ;

PredicateBlock = "{" PredicateStatement* BlockTail? "}" ;
ProofBlock     = "{" ProofStatement* BlockTail? "}" ;
BlockTail      = Expression ;

PredicateStatement = PureLetStatement | ErrorStatement ;
(* A structurally recognized assertion is attached as recovery-only
   PredicateStmt::Assertion and is not part of the clean grammar. *)
ProofStatement =
      PureLetStatement
    | ProofCallStatement
    | ProveAssertionStatement
    | ErrorStatement ;

PureLetStatement =
    "let" Pattern (":" Type)? "=" Expression StatementTerminator ;

ProofCallStatement = CallExpression StatementTerminator ;

ProveAssertionStatement =
    "assert" "." "prove" "(" AssertionArguments? ")"
    StatementTerminator ;

AssertionArguments = Expression ("," Expression)* ","? ;
StatementTerminator = ";" | LogicalLineEnd ;
```

`Pattern`, `Type`, `Expression`, `CallExpression`, `TraitBound`, attributes, and documentation are parsed by the same full-source grammar event pipeline used everywhere else. There is no signature text, clause text, or body text parser.

## 3. Declaration shape

### 3.1 Prefixes and visibility

Consecutive `///` lines form one identity-bearing `DocBlock` attached to the following declaration. Each `#[...]` outer attribute is separately identity-bearing and remains in source order. Documentation and attributes preceding an item that cannot own them receive ordinary attachment diagnostics and are not silently moved.

Visibility follows existing Arcweft meaning:

- no modifier: module-private;
- `pub`: package export;
- `pub(crate)`: package-visible but not externally exported;
- `pub(super)`: parent-module-visible.

### 3.2 Names and identity

The declaration name is exactly one ordinary identifier. The following forms are not names:

- `@proof.x` or any entity-reference token;
- a dotted path;
- a string;
- an integer; or
- an omitted token.

A missing ordinary name produces an identity-bearing `MissingName` node and a recovered, non-executable declaration only when the parser can synchronize at a valid fixed parameter group. An entity reference immediately after `proof` prevents declaration commitment and produces an ordinary `ErrorItem`, so removed `proof @...` syntax never creates a `ProofItem`.

### 3.3 Generics and fixed parameters

There is at most one generic parameter group and exactly one fixed parameter group. The fixed group may be empty as `()` and is never omitted in a clean declaration.

A parameter uses the full typed pattern grammar and requires a type. Defaults, variadic/rest parameters, receiver syntax, multiple curried groups, and untyped names are not accepted. Pattern irrefutability, duplicate names, `_`, poison, and local allocation are semantic/lowering rules in `SCOPES_LOCALS_CAPTURES.md`.

Each predicate and each proof accepts at most 64 parameters. The sixty-fourth succeeds. Encountering a sixty-fifth parameter is a fatal syntax transaction limit failure and publishes no parse snapshot.

### 3.4 `where`

There is at most one `where` clause. It appears after the optional proof return type and before any `requires`/`ensures` clause. It contains at most 256 predicates and preserves authored order. A trailing comma is accepted.

Generic parameters are limited to 256 per declaration. Exact limits commit; one over is fatal and atomic.

## 4. Return rules

### 4.1 Predicate

A predicate has an implicit `Bool` return type. `->` after the fixed parameter group is always rejected as current grammar:

- the arrow and following type are retained in a `ReturnType` recovery node;
- diagnostic `syntax.predicate.return_not_allowed` covers the arrow through the recovered type;
- the declaration is recovered and non-executable;
- HIR still records the synthetic `Bool` return type so tooling has a complete shape.

No authored type can replace the implicit `Bool`.

The predicate body must produce `Bool`. A block must have an authored tail expression; absence yields an `OmittedBlockTail`, a poisoned synthetic HIR missing-tail expression, and `sema.predicate.missing_boolean_tail`. An authored non-Boolean tail yields `sema.predicate.tail_must_be_bool` on the tail span.

### 4.2 Proof

Omitting `-> Type` gives the proof the `Unit` return type. An expression body must then type-check as `Unit`. A block may omit its tail; lowering creates a synthetic Unit expression keyed by the block.

An explicit return type is fully resolved by sema. If the resolved type is `Unit`, the same omitted-tail rule applies. If it is non-`Unit`, an expression body or authored block tail is required. An omitted block tail remains structurally present as `OmittedBlockTail`, lowers to a poisoned synthetic missing-tail expression, and reports `sema.proof.missing_value_tail` at the insertion point. A mismatched authored tail reports `sema.proof.tail_type_mismatch` on the tail expression.

An alias that resolves to `Unit` is treated as Unit; the syntax parser does not guess from spelling.

## 5. Contracts

### 5.1 Allowed clauses

Only `requires` and `ensures` are declaration clauses in this cut. The previous proof-body/contract spellings `invariant`, `assume`, `check`, `use`, and trusted-axiom references are not proof clauses and have no dedicated historical node.

Every clause contains one typed expression. Clauses do not own strings that are reparsed later.

### 5.2 Order and count

All `requires` clauses precede all `ensures` clauses. A `requires` after the first `ensures` is retained as a recovered `RequiresClause`, reports `syntax.contract.invalid_clause_order`, and makes the declaration non-executable.

The combined number of `requires` plus `ensures` clauses is at most 64. The exact total succeeds. One over is a fatal syntax limit failure with full transaction rollback.

### 5.3 Clause scope

- generic parameters and fixed parameters are visible in both clause families;
- earlier clauses do not introduce names for later clauses;
- the synthetic postcondition local `result` is visible only in `ensures`;
- for a predicate, `result: Bool`;
- for a proof, `result` has the resolved return type, including `Unit`;
- `result` is not visible in `requires` or the body;
- a parameter or local pattern that binds `result` reports `sema.binding.result_reserved` and is poisoned.

Clauses are pure. Purity is checked by sema through the same typed expression graph as body expressions.

## 6. Body and statement rules

### 6.1 Expression body

`= Expression` is an `ExpressionBody` node with a child expression. Its range begins at `=` and ends after the expression and optional semicolon, excluding the terminating newline. It has no block, brace, statement list, or implicit tail node.

### 6.2 Predicate block

A predicate block contains zero or more pure `let` statements and a required Boolean tail expression. Assertions and standalone expression statements are not admitted as clean `PredicateStatement` variants.

An authored `assert.prove/check/debug` is still parsed once by the common assertion grammar so diagnostics can point to the structured statement. Attachment exposes it as the recovery-only `PredicateStmt::Assertion(AssertionStmt)` variant and sema emits `sema.predicate.assertion_not_allowed`. It is never a clean `PredicateStatement`, never executable, and is not reparsed or wrapped as a string.

Predicate calls are ordinary pure call expressions in initializers, conditions, and the tail.

### 6.3 Proof block

A proof block admits:

- pure immutable `let` statements;
- call-shaped expression statements, represented as `ProofCallStatement` and later required to resolve to a proof;
- the existing typed `assert.prove(...)` statement; and
- recovered error statements.

`assert.check` and `assert.debug` retain their common typed `AssertionStmt` shape, then report `sema.proof.runtime_assertion_not_allowed`; they never become runtime guards from proof context.

An expression statement that does not resolve to a proof reports `sema.proof.expression_statement_not_proof_call`. An impure initializer or call reports `sema.proof.impure_let` or the existing purity diagnostic at the exact typed child.

### 6.4 Newline and semicolon distinction

At block delimiter depth zero:

- a completed `let`, proof-call, or assertion before `;` or logical newline is a statement;
- the final expression before `}` with no statement terminator is the block tail;
- a final call followed by `;` is a proof-call statement and does not satisfy a non-Unit tail;
- a final call without `;` is the tail expression, not a proof-call statement;
- comments and whitespace do not change the distinction.

This rule makes a one-expression block observably different from `= Expression` and from a one-statement block.

## 7. Names, qualification, imports, and collisions

A callable declaration's canonical identity is package plus canonical module path plus ordinary name and callable owner kind. Surface source never authors that identity.

Unqualified lookup uses the current module, explicit imports, aliases, and visible glob imports through the existing `ProjectSymbolTable`. Qualified lookup uses the existing project symbol path rules. Predicates and proofs do not gain a separate namespace.

Functions, predicates, and proofs collide on the same ordinary binding name within a module. There is no signature overloading. A second declaration with the same ordinary name is a duplicate even when its parameter types or callable kind differ.

Imports and aliases can target a function, predicate, or proof under the same rules and visibility checks as other callables. An alias collision is reported by the current project symbol authority.

## 8. Call and recursion policy

- predicates are pure Boolean callables and may be called from pure expressions where a `Bool` value is accepted;
- proofs may be referenced only by `ProofCallStatement` in proof context during cut 01.1;
- proofs are not runtime callables and never lower to runtime-plan call effects;
- functions cannot invoke proofs as value expressions;
- forward references are allowed through project registration;
- any direct or mutual recursion strongly connected component containing a predicate or proof is rejected by `sema.callable.recursive_contract` on each participating call edge;
- ordinary function-only recursion retains its existing policy.

This deterministic no-recursion rule avoids defining proof fixed points or recursive logical predicates before later proof-kernel cuts.

## 9. Recovery contract

### 9.1 Header synchronization

The parser uses these depth-zero synchronization tokens, in order:

1. `(` for the required fixed parameter group;
2. `->`, `where`, `requires`, `ensures`, `=`, or `{` after parameters;
3. a top-level declaration keyword at indentation zero; or
4. EOF.

It never consumes a following clean top-level declaration merely to complete the malformed item.

### 9.2 Missing and malformed cases

| Case | Typed/lossless result | Primary range | Executable |
|---|---|---|---|
| missing ordinary name with recoverable `()` | `MissingName` inside recovered declaration | zero-width before `(` | no |
| entity-style name after `proof` | ordinary `ErrorItem`; no `ProofItem` | entity-reference token | no |
| malformed generic parameter | `ErrorNode` within `GenericParameterGroup` | offending balanced fragment | no |
| missing `>` | zero-width `CloseAngleNode`/`MissingToken` before `(` | insertion point | no |
| missing parameter group | `MissingTokenNode`/`FixedParameterGroup` recovery | before first header continuation/body token | no |
| missing `)` | zero-width `CloseParenNode` before next header boundary | insertion point | no |
| parameter missing pattern/type | `MissingPattern` or `MissingType` | insertion point | no |
| malformed `where` predicate | `ErrorType`/`ErrorNode`, synchronize at comma or clause/body | offending fragment | no |
| clause after body token | body begins; later tokens recover as following grammar | first unexpected token | declaration no |
| `requires` after `ensures` | recovered typed clause | `requires` keyword through clause | no |
| missing clause expression | `MissingExpression` | before terminator | no |
| missing body | `MissingBody` | end of accepted header | no |
| missing opening brace after block decision | zero-width `OpenBraceNode` | insertion point | no |
| missing closing brace | zero-width `CloseBraceNode` at next top-level boundary/EOF | insertion point | no |
| malformed statement | `ErrorStatement` with typed recovered descendants | one logical statement fragment | no |
| missing required tail | `OmittedBlockTail` plus semantic diagnostic | close-brace start/recovery anchor | no |

All recovery nodes are queryable. A declaration containing any poison that affects its signature, clauses, body structure, or required tail is non-executable.

### 9.3 Removed forms

There is no spelling-specific production branch or diagnostic code for:

- `proof @...`;
- `trusted axiom`;
- old proof clauses;
- `calc`; or
- the deleted borrow block.

They flow through the same unexpected-item, unexpected-statement, unresolved-name, missing-token, and synchronization machinery as any other text outside current grammar. Tests assert public typed outcomes and executability, never repository source spellings.

## 10. Inclusive limits and commit point

The parser increments counters when it emits the corresponding start event:

- a parameter count at `Parameter` start, including recovered parameters;
- a clause count at `RequiresClause`/`EnsuresClause` start;
- statement/expression/type/pattern counts at their semantic grammar node starts;
- diagnostic count after deterministic structured sort/exact deduplication;
- identity count after tree construction for every identity-bearing node, including missing/error nodes.

Event/node limits are checked before appending the one-over event or allocating the one-over identity. Diagnostics are normalized first and then compared with the inclusive diagnostic maximum, so duplicate emission cannot consume the budget twice. At exact maximum, parsing, attachment, and commit succeed. At one over, the transaction discards its document revision, generation, event vector, tree, identities, attachments, diagnostics, statistics, and caches.

## 11. Canonical examples

```arcw
/// True when both endpoints are ordered.
pub predicate ordered<T>(pair: (T, T), cmp: Comparator<T>)
where T: Ord
requires cmp.is_total()
ensures result == (cmp.compare(pair.0, pair.1) <= 0)
{
    let (left, right): (T, T) = pair
    cmp.compare(left, right) <= 0
}
```

```arcw
pub proof preserve_order<T>(pair: (T, T), cmp: Comparator<T>) -> Bool
where T: Ord
requires cmp.is_total()
ensures result
{
    let ok: Bool = ordered(pair, cmp)
    prove_comparator_total(cmp)
    assert.prove(ok)
    ok
}
```

```arcw
proof record_fact(value: Bool)()
```

The final example is malformed because exactly one fixed parameter group follows the name and `record_fact(value: Bool)()` contains two groups. It recovers at the second group and is non-executable. A valid Unit proof is:

```arcw
proof record_fact(value: Bool) {
    assert.prove(value)
}
```
