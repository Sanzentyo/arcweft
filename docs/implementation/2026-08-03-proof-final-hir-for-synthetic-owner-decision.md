# Proof final-HIR `for` synthetic owner decision

- Decision date: 2026-08-03
- Recorded and inspected: 2026-08-03
- Inspected Git revision: `9591e1f7db4884cb796cf098ab027c9bd3155cf2`
- Working tree: dirty protected Proof-concurrency integration WIP; the final-HIR
  files inspected below are not independently accepted or committed evidence
- Status: schema decision complete; production implementation not performed in
  this documentation cut

## Decision

The final expression inventory gains exactly one semantic family with two
payload variants:

```rust
pub enum HirExprKind {
    // The existing 36 families remain unchanged.
    ForSynthetic(HirForSyntheticExpr),
}

pub enum HirForSyntheticExpr {
    Iterator { source: ExprId },
    NextValue { iterator: ExprId },
}
```

This is the narrow correction to the accepted 36-family inventory. The final
inventory has 37 families after this addition. `Iterator` and `NextValue` are
not authored calls, paths, placeholders, errors, or lexical locals.

`Iterator { source }` is the semantic value produced by applying the accepted
`IntoIterator` evidence to the authored `for` source. Its checked type is
`IntoIterator::IntoIter`.

`NextValue { iterator }` is the successful `Item` value obtained from the
accepted `Iterator::next` evidence. The `Option::None` result and the resulting
loop exit are control semantics of `HirForStmt`; they are not represented by a
fabricated expression child. Pattern checking and binding consume the
successful `Item` value.

## Contract gap closed here

The accepted ordinary-Flow F10 row requires:

- `HirForStmt` with `source`, `iterator`, and `next_value` expression IDs;
- statement-owned `ForIterator` and `ForNextValue` synthetic children;
- allocation and source-order freeze for both children.

The accepted typed synthetic-key contract fixes both roles to
`SyntheticOwner::Stmt`, ordinal `0`. The accepted tail-owner contract fixes the
normal insertion anchors to the end of `in` and the start of the body opening
brace.

Those contracts did not select a `HirExprKind` payload for either expression.
The accepted leaf package and the current WIP `HirExprKind` exhaustively list
36 families without an iterator temporary, a next-value temporary, or a
general desugaring-value family. The current WIP `HirForStmt` consequently
stores two `ExprId`s for which no clean production payload can be constructed.

Using `Call`, `Path`, `Placeholder`, or `Error` would not close the gap:

- `Call` would fabricate an authored callee, delimiter, argument, and source
  surface and would incorrectly enter ordinary-call accounting;
- `Path` would fabricate a source or lexical name for a non-lexical value;
- `Placeholder` is reserved for authored partial-application and pipe-left
  placeholders;
- `Error` would poison every otherwise valid `for` statement.

This decision therefore deliberately amends the expression inventory instead
of guessing one of those payloads.

## Identity and payload freeze

For one source-backed `HirForStmt` with ID `for_stmt`, the only admitted
synthetic expression keys are:

```text
SyntheticKey(Stmt(for_stmt), ForIterator, 0)
SyntheticKey(Stmt(for_stmt), ForNextValue, 0)
```

The committed graph must satisfy all of the following:

```text
kind(for_stmt.iterator)   = ForSynthetic::Iterator
iterator.source           = for_stmt.source
kind(for_stmt.next_value) = ForSynthetic::NextValue
next_value.iterator       = for_stmt.iterator
```

Both IDs, both payload edges, the `HirForStmt`, and its body scope must belong
to the same HIR module and accepted snapshot. A substituted key, wrong role,
nonzero ordinal, duplicate child, foreign/stale ID, reversed edge, extra
synthetic child, or mismatch between the statement fields and payload edges is
an invariant or transaction failure. It is never repaired by rediscovering a
child from source text.

## Scope and visibility

The authored source, `Iterator`, and `NextValue` expressions use the enclosing
statement scope. The two synthetic values are semantic dataflow nodes, not
names, and create no `LocalId`, scope member, or user-visible binding.

The `for` body owns one child body scope. Pattern locals are allocated in
pattern preorder into that body scope and are visible only while checking and
executing the body. They are not visible in the source expression, the
iterator conversion, the next operation, a sibling statement, or after the
loop.

## Evaluation order

Runtime and checked-semantic evaluation is fixed as follows:

1. evaluate `HirForStmt::source` once in the enclosing scope;
2. apply accepted `IntoIterator` evidence once and establish `iterator`;
3. at the start of each iteration, apply accepted `Iterator::next` evidence;
4. exit the loop on `None`;
5. expose the successful `Item` as `next_value` and apply the accepted pattern
   policy;
6. initialize the pattern locals and execute the body in its child scope;
7. return to step 3.

The final HIR remains pre-sema. It records the two typed semantic operations,
but the checker owns conformance selection and the exact witness facts. A
missing or ambiguous `IntoIterator`/`Iterator` implementation is a semantic
failure, not parser recovery and not an HIR `Error` payload.

## Source ownership

Both synthetic expressions have insertion sites, never fabricated spans:

- `ForIterator`: the end of the attached `in` token;
- `ForNextValue`: the start of the attached body opening brace;
- when `in` is recovered, `ForIterator` uses the typed required-`in` insertion
  owned by the attached `ForStatement`;
- when the body is missing, `ForNextValue` uses the typed `MissingBody`
  insertion; an unclosed body still uses its authored opening brace.

The attached owner must expose those token or insertion components directly.
Lowering must not scan, slice, or reparse source text to recover either anchor.
Slot/source-index freeze checks the exact key, insertion metadata, document,
revision, and payload edge before publication. Synthetic expressions publish
no authored component rows.

## Recovery and poison

Missing or malformed authored pattern, source, and body components retain
their existing typed recovery owners. The two synthetic nodes are still
allocated so the F10 identity graph remains complete.

- structural poison in `source` propagates to `iterator`;
- structural poison in `iterator` propagates to `next_value`;
- trait-resolution failure is retained by sema facts and does not rewrite HIR
  structural poison;
- no default iterator, item, path, call target, or source spelling is invented.

The statement-level primary recovery order is:

```text
Pattern
Source
Iterator
NextValue
MissingBody
body children in source order
UnclosedBody
```

Child terminal diagnostics stay on their child owners. `HirForStmt` retains
only the roleful recovered-child issue selected by this order. A limit,
cancellation, panic, stale/foreign input, source-freeze mismatch, or invariant
failure aborts the transaction and publishes no poison or partial graph.

## Accounting and transaction boundary

One admitted `for` statement charges:

- one source-backed `StmtId`;
- the authored source expression and its ordinary descendants;
- exactly two additional HIR expression slots for `ForIterator` and
  `ForNextValue`;
- one body scope;
- the pattern, pattern locals, and body children under their existing limits;
- exact slot insertion metadata for both synthetic expression identities.

The two synthetic expressions do not charge syntax-expression nodes, authored
call arguments, call type arguments, or ordinary-call resolver candidates.
The checker performs one `for`-iteration trait-resolution transaction and
derives the `IntoIter`, `Item`, and witness evidence from it; consumers must not
repeat that resolution independently for each synthetic node.

HIR expression and total-slot limits include both synthetic expression slots.
All counts and exact source manifests are preflighted before reservation. An
exact limit may commit; the first one-over attempt rolls back the statement,
synthetic keys, scopes, locals, source rows, diagnostics, semantic facts,
cache facts, and invalidation facts together.

## Required implementation evidence

Implementation of this decision is not complete until focused tests prove:

- exact construction of both payload variants and both statement-owned keys;
- the payload-edge and same-module/snapshot freeze rules;
- enclosing-scope ownership and absence from lexical local lookup;
- source-once, conversion-once, next-per-iteration evaluation behavior;
- standard built-in, explicit witness, and identity-`IntoIterator` cases;
- missing/malformed pattern and source, missing and unclosed body, and the
  primary recovery order;
- the recovered-`in` and missing-body insertion anchors;
- source poison propagation without converting semantic trait failures into
  HIR recovery;
- substituted, duplicate, reordered, stale, foreign, wrong-owner, and
  wrong-ordinal rejection;
- exact and one-over expression/total-slot accounting with complete rollback;
- one checker-owned iteration resolution and no ordinary-call dispatch;
- source-query, accepted-project, compiler/runtime-plan, LSP, and cache identity
  parity; and
- compile-fail evidence that unchecked construction and compatibility accessors
  are unavailable.

Focused HIR and sema tests come first. Workspace check, strict Clippy,
workspace tests, applicable Tier 2 tests, structural audit, and deletion audit
belong to the coherent public authority-switch cut, not to this documentation
decision.

## Migration and non-goals

Implementation must add the final payload and migrate every consumer directly.
It must not introduce:

- a synthetic authored `HirCallExpr` or a second call resolver;
- a hidden path/local spelling for either value;
- source-string reconstruction or a source gate;
- an alias, wrapper, compatibility shim, dual reader, side table, or fallback;
- a legacy `HirFor` adapter; or
- a temporary `Error`/`Placeholder` success path.

Once the final attached-HIR/project authority is switched, obsolete detached
`for` carriers and their consumers are deleted in the same compiling migration
cut. This note does not authorize changes to unrelated loop families, runtime
assertion codecs, save/replay, Stream, resource, manifest, Dialogue runtime, or
TTS.

## Evidence inspected

- `docs/reviews/packages/arcweft-proof-concurrency-v6.1.1.2.1.1.1-ordinary-flow-evidence-schema-redelivery-correction-final-contract.zip`,
  SHA-256 `BDC55671E7D4F8CDB3D07D8EC004672C90E14DEA88A47E63D8189E585BB3E4DF`;
- `docs/reviews/designs/proof-concurrency-v6.1.1.4.1/arcweft-proof-concurrency-v6.1.1.4.1-final-hir-semantic-leaf-expression-payload-correction-final-contract.zip`,
  SHA-256 `61E2EE166BFF158FE83DCF1484B7B9380A81F60D865377503400D27D238CC708`;
- `docs/reviews/designs/proof-concurrency-v6.1.1.4.1.1.1.1/arcweft-proof-concurrency-v6.1.1.4.1.1.1.1-tail-owner-and-generator-evidence-correction-final-contract.zip`,
  SHA-256 `69DC42FC7C985FED638D08D694ED301291A50AF3CEFA7117321D4219BE7E6471`;
- `crates/arcweft-lang-hir/src/expr.rs`;
- `crates/arcweft-lang-hir/src/stmt/thread.rs`;
- `crates/arcweft-lang-hir/src/final_lowering/statement_lowering.rs`; and
- the attached `Loop`, `While`, `WhileLet`, and `For` syntax owners in the
  protected working tree.

Performed validation was a read-only schema, package, and consumer search. No
Rust code was changed and no compile, test, Clippy, Tier 2, or structural-audit
command was run for this documentation-only decision.
