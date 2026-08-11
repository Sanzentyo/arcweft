# Request: Prefix/postfix Try source and propagation contract correction

## Sequence position

This is Lang-01.1.1.1. It follows the independently implemented typed
`AwaitExpr` source slice from Lang-01.1.1 and must be resolved before claiming
completion of Lang-01.1.1 cases A010, A015, and A016 or deleting the current
general prefix-Try surface.

The request is a narrow language and semantic contract correction. It does not
own Stream runtime/wire design, generator execution, callable-catalog
redesign, or function-role deletion.

## Why this correction is required

The returned Lang-01.1.1 package specifies:

```ebnf
TryExpr = Expr "?" ;
```

and assumes an existing typed Try source record with exact `whole`, `operand`,
and `question` ranges. Current production instead has one
`Expr::Try { expr }` node without an owned source record, and that node is
produced by both:

```arcw
try expression
expression?
```

General prefix `try expr` remains canonical in current design documentation
and is used by Agent, CLI, dialogue, and ordinary expression fixtures. The
package neither deletes nor models that live surface. It also requires exact
propagation-boundary/error diagnostics without defining how the enclosing
return boundary and its source range join the expression checker.

These choices cannot be filled in by a compatibility shim or by recovering
operator spelling from source text.

## Required decisions

1. Decide the final status of general prefix `try expr`.
   - Either retain it as a canonical spelling with a typed source variant, or
     remove it and give a complete owner-by-owner migration plan for current
     Agent, CLI, dialogue, documentation, and fixtures.
   - Do not add a deprecated alias, dual AST, or permanent removed-spelling
     diagnostic.
2. Define one final typed Try AST.
   - Give exact fields, ownership, constructors, and accessors.
   - Preserve semantic propagation independently from authored spelling.
   - Define exact source ownership for `whole`, `operand`, and every authored
     operator token.
   - If prefix and postfix forms both remain, use closed typed source variants;
     do not encode the distinction as a Boolean or source string.
3. Define grouping with the already implemented typed Await node.
   - `try await need` and `await? need` remain one `AwaitExpr` with
     `AwaitPropagation::PropagateError` and their current exact Await source.
   - `(await need)?` is a Try node wrapping one
     `AwaitPropagation::PreserveResult` Await node.
   - `await need?` is an Await node whose operand is the selected Try node.
   - State whether any formatter canonicalization is allowed and prove that it
     preserves grouping.
4. Define the propagation boundary.
   - Identify the nearest valid boundary for ordinary functions, closures,
     methods, flow-owned expressions, Agent controllers, and any retained
     prefix-Try contexts.
   - Define behavior when no boundary exists.
   - Define how generic/substituted error types are compared and whether any
     explicit conversion mechanism participates.
   - Do not route propagation through a generator or Stream terminal unless a
     later generator contract explicitly selects that behavior.
5. Define exact diagnostic payloads.
   - `sema.await.propagation_target_missing` must point at the exact
     `try`/attached/postfix operator.
   - `sema.await.error_mismatch` must carry expected and actual error types,
     point at the propagation operator, and relate the enclosing boundary's
     exact return-type/header range.
   - Define corresponding general-Try codes if prefix/postfix Try remains
     broader than Await.
   - Diagnostics must use typed source records and return-boundary facts, not
     source rescanning or function-name rules.
6. Define the accepted semantic join.
   - Specify where the checked return-boundary type and range are stored.
   - Preserve one `CallableDeclarationId` and the existing AW-AH-009.3 accepted
     callable catalog.
   - Do not create a parallel callable catalog, resolver, or execution-facts
     table.

## Required implementation order

1. freeze the final general prefix-Try decision and typed Try/source shapes;
2. add exact Try parser and source-map tests without changing `AwaitExpr`;
3. migrate syntax/HIR/sema/runtime-plan/tooling consumers atomically to the
   selected Try node;
4. add a typed return-propagation boundary carrying type and exact source
   evidence;
5. implement target-missing and error-mismatch diagnostics for Try and Await;
6. update formatter/tooling only after the AST grouping tests are green; and
7. if prefix Try is removed, migrate every current positive use and finish
   with ordinary grammar rejection, with no spelling-specific final node or
   diagnostic.

## Tests to specify

- exact AST and byte ranges for postfix `value?`;
- exact AST and byte ranges for the selected general prefix `try value`
  behavior;
- `try await need`, `await? need`, `(await need)?`, and `await need?`
  grouping and exact nested ranges;
- leading trivia, multiline operands, nested parentheses, UTF-8 before each
  operator, and nonzero parser base offsets;
- malformed missing operand/operator recovery with zero-width insertion
  ranges through ordinary parser recovery;
- `Result<T, E>` propagation into matching, generic-substituted, mismatching,
  non-Result, and missing return boundaries;
- closure, method, ordinary function, flow expression, and Agent controller
  boundary selection;
- stable diagnostic code, typed expected/actual payload, smallest primary
  range, and related boundary range;
- formatter parse-format-parse grouping equivalence for every retained
  spelling;
- direct parser/compiler rejection for any removed spelling, without source
  scans or a compatibility reader; and
- focused syntax/HIR/sema/tooling tests, workspace check/Clippy at the
  reviewable cut, and structural audit if the diagnostic or callable boundary
  is materially changed.

## Constraints

- Consume the existing typed `AwaitExpr`, `AwaitPropagation`,
  `AwaitExprSource`, and `AwaitPropagationSource` without redesigning them
  unless current implementation evidence demonstrates a concrete defect.
- Preserve the accepted AW-AH-009.3 callable identity/catalog and the current
  fixed-point effect analysis.
- Keep `arcweft-lang-syntax` parser/syntax-only and keep semantic boundary
  validation in `arcweft-lang-sema`.
- Do not introduce aliases, dual readers, compatibility AST variants,
  role-name special cases, or source-text recovery in later layers.
- Do not add source gates. Test observable parser, semantic, formatter, and
  diagnostic behavior through typed APIs.
- Do not implement or revise Stream runtime/wire, AWBC versioning, save
  schemas, or generator classification in this request.

## Expected output

Return one implementation-ready contract containing:

- the final prefix/postfix Try surface decision;
- exact typed AST and source-record definitions;
- exact Await/Try grouping and formatter rules;
- exact propagation-boundary ownership and error-compatibility rules;
- typed diagnostics with stable codes and source evidence;
- an owner-by-owner migration/deletion inventory;
- compile-clean implementation cuts; and
- a complete positive, negative, malformed, generic, tooling, and
  no-compatibility test matrix.
