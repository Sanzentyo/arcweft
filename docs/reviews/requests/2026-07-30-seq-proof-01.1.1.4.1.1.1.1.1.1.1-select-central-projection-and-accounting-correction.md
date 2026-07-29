# Design request: Proof-concurrency 01.1.1.4.1.1.1.1.1.1.1 Select central projection and accounting correction

- Date: 2026-07-30
- Sequence: Proof-concurrency v6.1.1.4.1.1.1.1.1.1.1, continuing the rejected
  Select source/diagnostic/producer return
- Kind: independently throwable, GitHub-only, design-only replacement
- Production changes: prohibited
- Required archive name:
  `arcweft-proof-concurrency-v6.1.1.4.1.1.1.1.1.1.1-select-central-projection-and-accounting-correction-final-contract.zip`

## Assignment

Prepare a corrected, decision-complete, standalone replacement package for the
E13 Select portion of Proof-concurrency v6.1.1 on the latest GitHub `main` of:

`https://github.com/Sanzentyo/arcweft`

Read repository `AGENTS.md`, this complete request, the primary E13 request,
both rejected-return intake notes, the previous continuation request, every
accepted predecessor archive referenced there, and the current parser,
attachment, final-HIR, source-query, diagnostic, and limit owners.

Required repository documents include:

- `docs/reviews/requests/2026-07-28-seq-proof-01.1.1.4.1.1.1.1.1-select-recovered-member-schema-correction.md`;
- `docs/implementation/2026-07-29-proof-01-1-1-4-1-1-1-1-1-select-return-intake.md`;
- `docs/reviews/requests/2026-07-29-seq-proof-01.1.1.4.1.1.1.1.1.1-select-source-diagnostic-and-producer-authority-correction.md`;
- `docs/implementation/2026-07-30-proof-01-1-1-4-1-1-1-1-1-1-select-authority-return-intake.md`;
- `docs/reviews/designs/proof-concurrency-v6.1.1.4.1/arcweft-proof-concurrency-v6.1.1.4.1-final-hir-semantic-leaf-expression-payload-correction-final-contract.zip`;
- `docs/reviews/designs/proof-concurrency-v6.1.1.4.1.1/arcweft-proof-concurrency-v6.1.1.4.1.1-source-owner-and-semantic-consistency-correction-final-contract.zip`;
- `docs/reviews/designs/proof-concurrency-v6.1.1.4.1.1.1/arcweft-proof-concurrency-v6.1.1.4.1.1.1-synthetic-role-owner-admission-correction-final-contract.zip`; and
- `docs/reviews/designs/proof-concurrency-v6.1.1.4.1.1.1.1/arcweft-proof-concurrency-v6.1.1.4.1.1.1.1-tail-owner-and-generator-evidence-correction-final-contract.zip`.

Open predecessor ZIP members directly. Do not infer their contents from file
names or summaries. Record actual member hashes and precedence. This request is
GitHub-only and reproduces every new result-changing decision below; do not
require the rejected external ZIP.

This is a design-only assignment. Do not edit production code, create a patch,
branch, PR, implementation overlay, or extracted workspace tree.

## Rejected archive identity

The mechanically valid but implementation-rejected return had:

```text
name:
arcweft-proof-concurrency-v6.1.1.4.1.1.1.1.1.1-select-source-diagnostic-and-producer-authority-correction-final-contract.zip

SHA-256:
531396561540A87D63A12861511B334494DB28A813D630C095323292E6CAB141
```

Do not rename or lightly patch that package. Return a complete replacement
whose matrices are derived from the final owners fixed here.

## Repository-fixed decisions

These decisions are closed. Do not present them as alternatives and do not
restore a superseded branch to make an old row pass.

### 1. Final Select payload has only reachable member states

The exact final schema is:

```rust
pub enum HirSelectedMember {
    Name(HirName),
    Missing,
}

pub struct HirSelectExpr {
    target: ExprId,
    member: HirSelectedMember,
}
```

Fields remain private. Construction and accessors live on the original owner.
Use ordinary structural equality, hashing, and ordering consistent with the
contained accepted types. Final HIR stores no source spelling, range, revision,
syntax handle, delimiter flag, sentinel, or synthetic member ID.

Delete `HirSelectedMember::Invalid` and every attached `ErrorNode`/Invalid
member branch. Current Select grammar admits a `NameReference` or creates a
zero-width `MissingName`; it does not consume a non-name token as an invalid
member. Parser-admitted names use the same identifier predicate as `HirName`.
An impossible conversion failure is an attachment/lowering invariant hard
failure with complete transaction rollback, not a publishable recovery value.

Leading `.member` remains `ShortVariant`, so missing Select target remains
unreachable and creates no synthetic child.

### 2. Select uses the central attached expression owner

The central expression projection is a protected integration boundary and may
not yet be present as landed production code on the GitHub revision used by the
designer. Its complete E13-facing shape is reproduced here so the assignment
does not depend on unpublished files. The eventual implementation extends that
single parser-owned projection directly:

```rust
pub enum SyntaxSelectedMember {
    Name(SyntaxName),
    Missing,
}

pub enum ExpressionProjection {
    // Preserve every existing variant unchanged.
    Select(SyntaxSelectedMember),
}
```

Use the existing `PendingExpressionProjection`, projected start event, and
`AttachedExpressionNode`. Add the exact central expression-component role
needed for `SelectedMember`; reuse the existing `Target` role. `Name` carries
the parser-validated typed name. `Missing` corresponds to the exact zero-width
`MissingName` node/range after any consumed trivia.

Do not create a public or independent `AttachedSelectExpr` that walks CST,
another Select projection database, an extension trait, or a fallback reader.
If a convenience view is necessary, it must be a non-owning method of the
central attached expression record and may only destructure its already stored
projection.

The parser event that recognizes the dot is the delimiter authority. The
lossless CST and the Select `Whole` span retain the exact dot token. Do not scan
source text to rediscover it, and do not add a public delimiter source role.

### 3. `?.` is Try followed by dot Select

`target?.member` has the final semantic shape:

```text
Path(target)
-> Try { form: PostfixQuestion, operand: target }
-> Select { target: try_id, member: Name(member) }
```

`target?.` has the same inner Try and an outer Select with `Missing` member.
Delete the provisional combined `?.` Select/`OptionalDot` path. The final
lexer/parser must expose `?` to the accepted postfix Try transaction and `.` to
the ordinary Select transaction. Do not add an optional flag to
`HirSelectExpr`, and do not erase the Try identity during lowering.

Provide exact byte ranges and source roles for both nodes. For
`target?.member`, the outer Select `Target` is the complete inner Try span,
while the Try separately owns `Operand` and `Operator` components.

### 4. Use real poisoned-target producers

Compact `..` is one range token. Therefore `target..member`, `target..`, and
any compact repeated-dot generator are not Select evidence.

Use and directly verify tokenization-safe current-language producers such as:

```text
target. .member
target. .
```

The first must produce an inner missing-member Select used as the authored,
poisoned target of an outer `Name` Select. The second must produce two distinct
missing-member Select roots. If exact current parser evidence disproves either
literal, replace it with another ordinary current-grammar spelling and record
the evidence; do not add syntax, change `..` range tokenization, hand-construct
an attached node, or call the row reachable without a real `ParsedSource`
producer.

Include whitespace and comment-trivia variants. The `MissingName` insertion is
the parser-owned zero-width range after consumed trivia, not a range rebuilt at
the preceding dot end.

### 5. Exact root poison and diagnostic obligations

Use this precedence for the singular Select root issue:

1. if the authored target is poisoned:

   ```rust
   HirRecoveryIssue::InvalidExpression(
       HirExpressionRecoveryIssue::RecoveredChild {
           role: HirExprSourceRole::Target,
       },
   )
   ```

2. otherwise a `Missing` member uses
   `HirRecoveryIssue::MissingOperand { role: SelectedMember }`;
3. otherwise the Select root is clean.

`MissingOperand { role: Target }` is not an authored-child propagation issue.
It remains reserved for a missing synthetic operand, which E13 cannot produce.

Recovery diagnostic identity remains one qualified `SyntheticOwner`, not an
`(owner, role)` pair:

- `Name`: no Select-root member diagnostic;
- `Missing`: exactly one Select-root diagnostic, primary
  `SelectedMember`, at the attached insertion;
- poisoned authored target: the target child/root retains its own terminal
  diagnostic; the outer Select adds no target-propagation diagnostic; and
- poisoned target plus `Missing`: the descendant owner and outer Select owner
  each retain one diagnostic, in descendant-before-ancestor order.

Freeze derives the independent member obligation from `HirSelectedMember`, not
only from the singular root poison. Retry uses the same qualified owners and
deduplicates rather than appending. Diagnostic rendering derives the missing
member message from the Select payload and `SelectedMember` site; it never
parses source or copies the child's arbitrary issue.

### 6. Exact source roles and query order

For each Select root:

```text
Whole             slot-owned exact Select span
Target            required Span of the authored target expression
SelectedMember    Span for Name; Insertion for Missing
Recovery          role not applicable
all other roles   role not applicable
```

Owner poison changes `owner_status`, not component presence.

Preserve the accepted validation precedence exactly:

```text
owner module/kind/liveness
-> role applicability
-> role ordinal
-> expected document ID
-> source revision
-> retained source length
-> committed presence and owner status
```

Include combined-failure tests proving that an inapplicable role or bad
ordinal wins before wrong document/revision/length. Do not add an E13 source
map, cached outcome, raw range reader, fallback, or second query API.

### 7. Exact syntax and HIR accounting

The attached Select parser creates `MissingName` but does not emit a
`SyntaxEvent::Diagnostic` for it. Do not add a redundant parser diagnostic
solely to reproduce the rejected package. The accepted grammar has no separate
parser-recovery-record counter.

Use these E13 diagnostic deltas:

```text
case                                syntax diag   HIR diag   final delta
clean target.member                 0             0          0
missing member: target.             0             1          1
poison target + Name                0             1          1
poison target + Missing             0             2          2
clean target?.member                0             0          0
missing member: target?.            0             1          1
```

The `target?.*` expression/slot total includes `Path + Try + Select`. Direct dot
Select uses `Path + Select`. The two real poisoned-target fixtures use
`Path + inner Select + outer Select`. No E13 case allocates a synthetic
descendant.

For final `HirLimit::Diagnostics` (`1,024` inclusive), isolate exact and
one-over rows with retained syntax/HIR prefill:

- delta one: prefill `1,023` commits at `1,024`; prefill `1,024` observes
  `1,025` and rolls back;
- delta two: prefill `1,022` commits at `1,024`; prefill `1,023` observes
  `1,025` and rolls back; and
- clean delta zero: an existing `1,024` remains valid.

The grammar's diagnostic limit is also `1,024`, but MissingName does not charge
it. Test syntax diagnostics with an actual `SyntaxEvent::Diagnostic` producer.
Delete every E13 128/129 diagnostic/recovery row inherited from the obsolete
detached expression parser.

Derive repeated-Select exact/one-over evidence from a tokenization-safe
`ParsedSource` generator and the actual final `SyntaxLimit::Expressions`,
`IdentityBearingNodes`, accepted source bytes, and HIR limits. State which
limit is reached first and leave earlier limits available. Do not use compact
dots, injected counters, hand-built events, or the deleted detached-reader
constant as end-to-end evidence.

Test `HirLimit::Expressions` and `HirLimit::TotalSlotsPerModule` independently,
with the non-target budget left available. State deterministic preflight
order. Give checked deltas for every direct, Try+Select, and nested Select case,
including complete source-component and slot-metadata staging. Distinguish
family-local component deltas from the full expression-tree transaction total.

`HirLimit::NameBytes` remains transaction/context-owned. It preflights the
valid authored spelling before the unchanged
`HirName::try_new(Box<str>) -> Result<HirName, HirNameInvariantError>` call.
Give valid exact/one-over and Missing-zero rows. Do not introduce
`HirWorkBudget`, `HirNameConstructionError`, or an E13-specific name
constructor. Deleted Invalid rows do not count as evidence.

### 8. Atomic lowering and deletion switch

Specify one transaction from:

```text
ParsedSource
-> central ExpressionProjection::Select
-> central AttachedExpressionNode
-> staged final HIR/source/diagnostic/work transaction
-> freeze
-> source query
-> commit or complete rollback
```

Preflight source identity, attachment invariants, name bytes, diagnostics,
expression/slot/component totals, and checked arithmetic before publication.
A hard failure publishes no root/child ID, source site, component, poison,
diagnostic, scope, candidate/result fact, or work fact. Retrying the same
source/snapshot/module identity obtains the same qualified IDs and diagnostic
owners.

Provide a path/symbol-level migration inventory from current `main`. The one
coherent public authority switch must migrate every real consumer and delete:

- the old `HirSelectExpr { member: HirName }` constructor, accessor, and match
  arms;
- the provisional combined `?.` Select/`OptionalDot` path;
- any standalone attached Select CST reader;
- HIR-facing detached `Expr::Select`/`SelectExpr` readers and source fallbacks;
- old final-HIR lowering, source, diagnostic, and cache readers;
- sema, verifier, runtime-plan, compiler, LSP, formatter, Agent/debug, project,
  and cache consumers that still read the obsolete member shape; and
- invalid-member variants/tests that have no producer.

A parser-internal clean carrier may survive only when it remains crate-private
and no final-HIR, sema, compiler, or tooling consumer can reach it. Do not fix
an obsolete reader's missing-member behavior; delete its consumer during the
switch and repair compile errors toward the central final owner.

## Required evidence and matrices

Return complete, mutually consistent matrices for at least:

- literal producer/token/CST/projection geometry for clean dot, missing dot,
  Try+Select clean/missing, tokenization-safe poisoned-target clean/missing,
  trivia/comment insertion, compact `..` range, leading ShortVariant, and
  non-name trailing recovery;
- final payload, child IDs, root poison, member obligation, diagnostic owners
  and order for every reachable combination;
- explicit unreachable evidence for missing target and invalid member;
- `Whole`, `Target`, and `SelectedMember` success; `Recovery` and another
  inapplicable role; role/source/liveness combined failures in normative order;
- exact/one-over syntax expressions, identity nodes, source bytes, name bytes,
  final HIR diagnostics, expressions, total slots, and any source-component
  limit actually present;
- publication rollback at every preflight stage, retry identity, diagnostic
  deduplication, and work-order perturbation; and
- compile-time/API evidence through owned visibility or compile-fail tests,
  never source-text search.

Every end-to-end row must exercise the real chain:

```text
ParsedSource
-> parser-owned projection
-> attached expression
-> final HIR transaction
-> source/diagnostic query
-> commit or rollback
```

Direct constructor tests may prove private invariants only. Label every
unreachable or delegated row explicitly; do not present a hand-constructed
node, token injection, or obsolete reader as final producer evidence.

## Constraints

- Preserve all accepted non-E13 schemas and matrices, except the already
  accepted Try rows must be restated where `?.` reaches them.
- Do not redesign the global source query, diagnostic owner, `HirName`, or
  limit owners merely to preserve a rejected E13 row.
- Do not fabricate a name, `ExprId`, syntax node, range, diagnostic, token
  split, counter, source spelling, or sentinel.
- Do not add aliases, wrappers, extension traits, compatibility shims, dual
  readers, source reparsing, source gates, CSS/Takumi paths, or permanent
  removed-syntax-specific diagnostics.
- Do not restore or repair detached HIR, static Capacity string dispatch,
  `SpeakerLine`, `ContentCall`, or stringly `HirDialogue` paths.
- Continue deletion-driven migration: remove obsolete APIs/readers in the
  coherent local switch and repair all resulting call sites toward the final
  owner.

## Required return

Return exactly one archive named:

```text
arcweft-proof-concurrency-v6.1.1.4.1.1.1.1.1.1.1-select-central-projection-and-accounting-correction-final-contract.zip
```

Put every sidecar inside the ZIP. Include README, this complete request copy,
the primary request copy, both rejected intake copies, `FINAL_STATUS.md`,
`OPEN_QUESTIONS.md`, predecessor precedence and direct member/hash audit,
repository evidence, complete affected attached/final Rust schemas,
producer/CST/projection geometry, source/query table, diagnostic/poison/freeze
table, work/limit/rollback tables, concrete consumer deletion inventory,
complete focused test matrices, traceability, validation report, and manifest.
Do not require adjacent summary, status, or hash files.

Use `READY_FOR_IMPLEMENTATION` only if `OPEN_QUESTIONS.md` is exactly `none`,
every claimed producer runs through current `ParsedSource`, `?.` retains its
Try identity, the central projection is the sole attached owner, all exact and
one-over rows are independently reachable, the query/poison/diagnostic rules
match the accepted owners, and the complete deletion switch is actionable.
Otherwise return the same correctly named ZIP with `NOT_READY` and exact
unresolved decisions.
