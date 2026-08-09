# Proof attached expression semantic-owner decision

## Recovery context

- Recovered into the Proof public-switch worktree: 2026-08-07
- Inspected Git revision:
  `f587e75750d9c5d9b6d8c84e0f098a4cfa80f68b`
- Working tree: dirty Proof public-switch integration
- Validation authority:
  [`2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md`](2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md)

This note restores expression-owner decisions and package-row traceability.
It carries no historical PASS state, test count, structural measurement, or
repository revision forward.

## Final authored owner

The final source-backed expression owner is the attached expression node in
the accepted `ParsedSource`. Its `SyntaxNodeId` is the only authored identity
passed to final HIR lowering.

The parser records one typed semantic projection on that exact node event. It
contains parser-selected facts that cannot be recovered from child ownership,
such as literal value, placeholder role, operator, call form, Thread mode, and
recovery class. Structural children remain attached nodes selected through the
typed role graph; paths, types, and patterns keep their own attached owners.
This projection is not a detached expression tree or second expression arena.

Pratt wrapping may insert an owner before an already completed child, but the
final projection must be installed on that exact event before attachment.
Neither HIR nor a later consumer may infer the family by reading Rowan tokens,
raw text, or byte slices.

## Leaf and composite ownership

The leaf boundary covers Unit, Literal, EntityReference, LifetimePath, Path,
ShortVariant, and Placeholder. Each consumes the lexer/parser-owned typed
value; string splitting and second literal decoders are forbidden. Resolved and
recovered leaf payload details are fixed by
[`2026-07-29-proof-final-hir-leaf-recovery-payload-decision.md`](2026-07-29-proof-final-hir-leaf-recovery-payload-decision.md).

Composite rows retain ordered typed children and their fixed semantic roles:

- tuple, bracket sequence, compact numeric sequence, array repeat, Index, and
  Pipe;
- prefix/postfix Try and ordinary/propagating Await;
- Borrow, Dereference, Unary, Binary, and all Range endpoint forms;
- Record and RecordLiteral with source-ordered fields;
- value Block, If, and generic Error; and
- the accepted Call, Select, Dialogue application, Thread, and remaining final
  families from the later correction chain.

Missing required children use typed `RecoveryOperand` synthetic IDs at their
contract ordinal and poison the known parent. A malformed authored child stays
source-backed; it is not replaced by a synthetic child. Hard depth, byte,
digit, count, and arena limits roll back the full transaction.

Range start/end ordinals remain 0/1 even when one endpoint is omitted. Omission
creates neither an Error child nor a diagnostic. Binary uses the closed
operator vocabulary; `=>` is Implies, `&` is Merge, and `|>` remains Pipe.
Unaccepted shadow operators stay deleted without dedicated historical
diagnostics.

Record fields preserve authored order and exact Whole/Name/separator/Value
components. Missing or duplicate names and missing values remain typed invalid
fields. A missing value has one exact recovery child but no fabricated valid
field value. Record shorthand resolves only through the accepted block-local
timeline and freeze re-derives the chosen Local.

## Blocks, If, and generic recovery

A value Block owns one scope, source-ordered statements, and one authored or
synthetic tail. An omitted tail is a clean Unit at the accepted
`ImplicitUnitTail` key. A missing Let initializer remains its typed Let and
allocates one statement-owned `RecoveryOperand`; known blocks do not collapse
to Error.

If owns Condition, ThenBranch, and ElseBranch at fixed ordinals. A wholly
omitted else is distinct from an authored missing else: omission owns the exact
zero-width else component and one clean synthetic Unit; authored absence owns
the recovery operand at ordinal 2. Nested `else if` remains a typed If child,
and IfLet remains its separate family.

Generic Error is admitted only when no known expression family applies. It has
one typed Recovery component and no semantic child, even if the lossless CST
retains a successfully parsed prefix beneath an error wrapper. A recognized
parent with a poisoned authored child uses roleful `RecoveredChild`; the child
and parent retain their distinct diagnostic obligations.

`PostfixBracket` is bracket-only. Colon-form Dialogue content application is
owned directly by `DialogueContentApplication`; it never passes through a
postfix compatibility carrier.

## Call and Select correction precedence

The historical version of this note left E12 Call and E13 Select pending.
Those exclusions are superseded by the accepted correction chain and its
repository intake:

- [Call source/resolver authority intake](2026-07-30-proof-01-1-1-4-1-1-1-1-2-1-call-authority-return-intake.md);
- [Select source/producer authority intake](2026-07-30-proof-01-1-1-4-1-1-1-1-1-1-select-authority-return-intake.md); and
- [Select central projection/accounting intake](2026-07-30-proof-01-1-1-4-1-1-1-1-1-1-1-select-central-return-intake.md).

Current Call and Select acceptance follows those later contracts and the full
matrix, including recovered argument/member representation, sole attached
source producer, shared resolver authority, deterministic accounting, and
rollback. The prior pending text must not be used to reintroduce a static
Capacity success branch, string callee reader, or alternate Select projection.

## Transaction, deletion, and evidence

Final lowering reserves each `(SyntaxNodeId, Expr)` once and stages payload,
children, poison, diagnostics, and the exact source manifest in one module
transaction. Freeze re-derives family, roles, child order, scopes, synthetic
keys, and source sites from the same attached snapshot. Stale/foreign identity,
payload substitution, limits, or transaction failure publish nothing.

The public switch deletes detached Expr/TypedSyntaxTree readers, source-reparse
helpers, old lowerers, static call dispatch, and all parallel source maps, then
fixes compile fallout against attached syntax and final HIR. Acceptance is the
current exhaustive expression/recovery/limit/identity matrix plus workspace and
structural gates; this document itself awards no row PASS.
