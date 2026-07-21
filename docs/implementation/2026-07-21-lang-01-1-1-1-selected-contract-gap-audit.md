# Lang-01.1.1.1 selected-contract gap audit

## Purpose and authority

This note records the implementation gap for the Lang-01.1.1.1 contract
selected by
[`2026-07-21-redelivered-contract-adjudication.md`](2026-07-21-redelivered-contract-adjudication.md).
It is an implementation-state audit, not a second language design.

The selected authority is:

- package: `arcweft-lang-01.1.1.1-final-contract-c957a61e4a0b.zip`;
- SHA-256:
  `024A13F98A7F46764A79CCBBD8F7ED317C30A4F5E24332E6AE1E2FF7B2A7E18C`;
- package status: final, with no open design questions;
- acceptance inventory: 124 required test rows and 8 conditional formatter
  rows.

The package replaces the earlier implementation-ready delivery. Its contract
is the source of truth except where a package example uses a language spelling
that has since been removed by the independently selected ordinary-function
and generator model. Such an example is translated to the current model; it
does not restore the removed spelling.

## Selected semantic contract

Both `try expression` and `expression?` remain canonical authored forms. They
have the same propagation semantics, and neither form is rewritten or
canonicalized to the other.

The syntax representation must atomically replace `Expr::Try { expr }` with
`Expr::Try(TryExpr)`. The source model is:

```rust
pub enum TryOperatorSource {
    PrefixTry { try_keyword: TextRange },
    PostfixQuestion { question: TextRange },
}

pub struct TryExprSource {
    whole: TextRange,
    operand: TextRange,
    operator: TryOperatorSource,
}

pub struct TryExpr {
    operand: Box<Expr>,
    source: TryExprSource,
}
```

Constructors remain crate-private and public access is through intentional
accessors, including operator `range()` and `TryExpr::into_parts`. Prefix
binding power is 90 and postfix binding power is 100. The parser must retain
exact whole, operand, and operator ranges.

The strict and lossless expression paths must share the ordinary grammar.
Dialogue-specific `has_try_prefix`, `strip_prefix("try ")`, source-end
mutation, and Try reconstruction are deleted. Recovery must not produce an
executable Try node when the operand or operator is missing and must not report
a bare operand as a successful Try expression.

Semantic analysis replaces `expected_returns` with one lexical
`return_propagation_frames` stack. The selected evidence model is:

```rust
pub enum PropagationBoundaryKind {
    Function,
    Closure,
    Method,
    Flow,
}

pub enum CheckedReturnType {
    Known(TypeKind),
    Unconstrained,
}

pub struct PropagationBoundaryEvidence {
    pub kind: PropagationBoundaryKind,
    pub declaration: Option<CallableDeclarationId>,
    pub checked_return: CheckedReturnType,
    pub header: SourceSpan,
    pub result: Option<SourceSpan>,
}

pub struct PropagationBarrierEvidence {
    pub owner: SourceSpan,
}

pub enum PropagationTargetEvidence {
    Boundary(PropagationBoundaryEvidence),
    GeneratorTerminal(PropagationBarrierEvidence),
}
```

The internal stack uses private `ReturnPropagationFrame::Boundary` and
`ReturnPropagationFrame::GeneratorTerminal` variants. Lookup examines only the
nearest lexical frame and never skips an inner boundary or terminal. Flow
headers and result annotations require `FlowSignatureSource`; closures require
`ClosureExprSource { whole, header, result, body }`; methods reuse the existing
`FunctionSignatureSource`. Synthetic HIR must refer to a real synthetic
`SourceDocument` and valid `SourceSpan`s. A detached `TextRange` fallback is
not accepted as ready HIR.

Try applies the general `Result` and `Option` propagation rules. Propagating
Await requires a compatible Result boundary. Preserve-result Await does not
perform boundary lookup. Compatibility is checked directionally with
`expected.accepts(actual)` after existing type resolution and substitution.
There is no implicit `From`, `Into`, `ArcError`, or Option-to-Result
conversion. Explicit `map_err`, `context`, and `ok_or` remain valid because
they change the operand type before propagation.

Only these four new type-check error kinds and stable codes are introduced:

- `sema.await.propagation_target_missing`;
- `sema.await.error_mismatch`;
- `sema.try.propagation_target_missing`;
- `sema.try.error_mismatch`.

The operator is the smallest primary range. A declared result annotation is
the preferred related range, with the callable header as the fallback. The
contract does not authorize a parallel callable catalog, resolver, diagnostic
catalog, detached evidence model, compatibility reader, source gate, CSS
path, or Takumi path.

## Ordinary-function and generator interpretation

The selected project direction has one ordinary `fn` surface. References in
the package matrix to `task fn`, `dialogue fn`, or `stream fn` are therefore
interpreted as follows:

- a function used by task, dialogue, or Agent execution remains a normal
  `PropagationBoundaryKind::Function` with the shared callable declaration
  identity;
- no `FunctionKind` branch, role attribute, alias, removed-keyword diagnostic,
  or compatibility parser is added for those former spellings;
- an ordinary `fn -> Stream<T, E>` is a generator terminal only when the final
  own-scope `yield` classification says that its body is a generator;
- a function that immediately returns or forwards a `Stream<T, E>` value
  without own-scope `yield` remains an ordinary function boundary;
- retained `stream {}` and `seq {}` generator expressions form terminals if
  those expressions remain part of the final grammar;
- a package row involving the old `source` owner is transitional. It may test
  the current owner while that owner exists, but it must not preserve or
  recreate `source` solely for this contract. After Lang-01.3 source
  elimination, the row is removed or translated to the final Stream owner.

The propagation implementation must consume the final typed generator fact.
It must not infer a terminal from a removed declaration spelling.

## Acceptance status

### Implemented substrate

The following reusable substrate already exists, but does not by itself close
the corresponding package rows:

- typed Await syntax and source ranges, including the one-node representation
  for propagating `try await` and `await?`;
- callable declaration identity and callable signature `SourceSpan`s;
- the shared registered callable catalog;
- directional `TypeKind::accepts` compatibility;
- basic parsing of both Try spellings and some grouping and precedence cases;
- exhaustive old-shape Try handling in several HIR, semantic, runtime-plan,
  verifier, CLI, and Agent consumers.

### Partially implemented

The parser accepts important positive cases, and existing semantic code handles
some Result and Option operands. However, those paths still use the old Try
shape, incomplete source evidence, and the old expected-return stack. Existing
tooling consumers preserve behavior only for `Expr::Try { expr }`; they are not
evidence that the selected typed boundary has been implemented. Several
malformed-input cases are rejected, but the package's exact recovery ranges,
prefix-depth behavior, and no-false-positive conditions are not yet covered as
one coherent contract.

### Missing acceptance work

The selected package remains incomplete until all of the following are done:

- introduce `TryOperatorSource`, `TryExprSource`, and `TryExpr` exactly once,
  then migrate every exhaustive consumer in the same compiling cut;
- preserve exact prefix and postfix source ranges and precedence in strict,
  lossless, fragment, incremental, and recovery parsing;
- remove the dialogue-owned Try recognizer, wrapper, string stripping, and
  source-range reconstruction;
- add Flow, closure, and method result/header source evidence and require real
  synthetic documents for synthetic HIR;
- replace `expected_returns` with the final nearest-frame propagation stack;
- apply the boundary contract to ordinary functions, closures, methods, flows,
  generators, Try, propagating Await, and preserve-result Await;
- reject missing propagation targets and incompatible error envelopes through
  exactly the four selected `TypeCheckErrorKind` variants;
- keep ordinary non-propagatable operand errors in the existing type-error
  family instead of adding an extra propagation diagnostic catalog;
- migrate HIR, project index, symbol, style, runtime-plan, verifier, Agent REPL,
  CLI, LSP, and all other exhaustive Expr consumers;
- add the package's positive, negative, range, ambiguity, nesting, nearest-
  boundary, synthetic-source, diagnostic, and tooling tests;
- correct the conflicting design documents identified below;
- run the required focused, workspace, structural, and Tier 2 validation.

No formatter subsystem currently owns this syntax. The eight formatter rows
are conditional and do not justify creating a broad formatter implementation.

## Rejected older WIP

The Jujutsu workspace `D:\git\arcweft-ws-lang-01-1-1` contains change
`b0bc0461786d` (`Implement Lang-01.1.1 correction and direct suspension
slice`). It must not be cherry-picked or merged as a unit.

That WIP predates the selected final contract and conflicts with it in material
ways:

- it models `TryExprSource` as a prefix/postfix enum instead of separating
  `TryOperatorSource` from the `TryExprSource` struct;
- it redesigns Await even though the selected contract preserves the typed
  Await model already present on current `main`;
- it retains dialogue-specific Try peeling, wrapping, and source-end mutation;
- it permits detached `TextRange` propagation evidence;
- it uses different boundary, target, and checked-return types;
- it introduces task, thread, source, and other barriers that are not the
  selected single `GeneratorTerminal` model;
- it adds a separate propagation diagnostic catalog and additional operand and
  invalid-target codes instead of the four selected type-check diagnostics;
- it retains `task fn`, `dialogue fn`, `stream fn`, and `FunctionKind`;
- it is based on an older tree and changes View files that have since moved or
  been deleted.

Its parser/range test ideas, consumer inventory, nearest-frame algorithm,
source-evidence work, and direct-suspension/AWBC implementation may be used as
review material. They must be rewritten against the selected types, current
file ownership, ordinary-function model, and current Await implementation.
The old change is not a compatibility authority.

## Dependency and implementation order

The required order is:

1. Finish and validate the active View integration cut so this work starts
   from one stable owner graph.
2. Land the isolated AW-AH-009.3 callable-catalog correction so callable IDs,
   accepted declarations, and source evidence have one authority.
3. Settle the Lang-01.1 ordinary-function role deletion and typed own-scope
   generator classification. The Try syntax migration may be prepared in
   parallel, but semantic terminals must consume this final fact.
4. Atomically introduce the selected Try AST/source types, migrate all
   exhaustive consumers, and delete the dialogue-specific parser path.
5. Add Flow/closure/method source evidence, the SourceSpan-only propagation
   frame stack, Try/Await rules, and exactly four diagnostics.
6. Reconcile tooling, documentation, focused tests, workspace validation,
   structural audit, and Tier 2 expectations.
7. Only after this selected contract is stable, selectively rebase the useful
   Lang-01.1 direct-suspension and AWBC pieces from the rejected WIP.

Do not land an intermediate semantic implementation that treats
`FunctionKind::Stream` as the final generator authority. That would encode the
removed surface into the new boundary and create another migration layer.

## Validation requirements

Focused validation must cover the complete affected path:

- all syntax library and integration tests, including strict, lossless,
  fragment, incremental, malformed, recovery-range, and ambiguity cases;
- all HIR and semantic tests, including nested boundaries, generator
  terminals, Try/Option/Result, Await propagation, exact codes, and primary and
  related spans;
- runtime-plan and verifier tests for every migrated Expr consumer;
- Agent REPL, CLI, and LSP focused diagnostics and snapshot tests;
- synthetic-HIR readiness tests that use a real synthetic source document.

The reviewable cut then requires:

```bash
cargo check --workspace --all-targets --all-features
just fmt-check
just clippy
just test-workspace
just test-doc
cargo +nightly -Zscript tools/structure-audit.rs --root .
just test-tier2
```

`just test-tier2` is mandatory for this cut. The change spans public syntax,
HIR, semantic, runtime-plan/verifier, CLI, Agent, and LSP contracts and affects
the runtime/Agent integration boundary. Stale MCP or Agent expectations,
resource identifiers, semantic identities, and authored View geometry must be
updated to the current production contract. Production aliases or duplicate
paths must not be added merely to satisfy stale slow tests.

The feature combination should remain stable across the validation cut, as
required by the repository test-execution policy. Failures and skipped tests
must be recorded rather than hidden.

## Documentation conflicts to reconcile

The current Result/Option documentation describes implicit `From` conversion
and Option-to-ArcResult convenience. That conflicts with the selected explicit
conversion rule and must be corrected.

Current documentation also presents `try await` as preferred or canonical over
`await?`. The selected contract makes both authored spellings canonical and
semantically equal, so preference or canonicalization language must be
removed.

Examples that still use `task fn`, `dialogue fn`, or `stream fn` must be
rewritten as ordinary `fn` examples with the final typed execution or generator
classification. Documentation must not preserve a removed spelling merely to
mirror a package test row.

## Completion boundary

This audit does not complete Lang-01.1.1.1 and does not authorize implementation
from the rejected WIP. Completion requires the exact selected types and
semantics, deletion of the old Try representation and dialogue special path,
ordinary-function/generator integration, migration of all current consumers,
the required negative and span tests, documentation reconciliation, workspace
validation, structural audit, and successful Tier 2 reconciliation.

A temporarily compiling subset, acceptance of both surface spellings without
typed source ownership, or basic Result propagation without the final lexical
frame and diagnostics is not completion. Any intentionally excluded item must
remain an explicit implementation non-goal or follow-up request; it must not be
silently counted as implemented.
