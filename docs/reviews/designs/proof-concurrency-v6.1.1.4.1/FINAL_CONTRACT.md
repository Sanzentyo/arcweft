# Final contract

## 1. Authority and scope

This contract closes Proof-concurrency v6.1.1.4.1 at Arcweft `main` `ac9ce44fe9423efd85280e26832dd30c725b3b34`. It preserves the accepted qualified HIR database, immutable snapshots, transaction rollback, slot liveness, source-component table, project publication, statement/member owners, the shared callable resolver, and AW-AH-009.4.2 dialogue-application owners. It supplies only the missing final expression/leaf authority and its required consumer migration.

The public authority is `arcweft_lang_hir::expr`. All nested ordinary, Dialogue, and RichText expressions use the same qualified `ExprId` arena. Revision-bound spelling and ranges live only in the source table. No semantic payload contains Rowan nodes, syntax handles, source slices, display strings, or public source spans.

## 2. Closed expression inventory

The final `HirExprKind` has exactly 35 variants:

```text
Unit, Literal, EntityReference, LifetimePath, Path, ShortVariant,
Placeholder, Tuple, BracketSequence, NumericBracketSequence, ArrayRepeat,
Call, Select, Index, Pipe, Try, Await, Thread, Range, Record,
RecordLiteral, Binary, Borrow, Dereference, Closure, Unary, Block,
ComputationBlock, NamedBlock, If, IfLet, Match,
DialogueContentApplication, PostfixBracket, Error
```

Ordinary-expression `DialogueCall` and `MemoBlock` are deleted. `Raw` is never a HIR variant. The retained RichText `DialogueCall` is a tag payload action and is represented by `HirRichTextTagPayload::DialogueCall(ExprId)` whose child is a same-arena `HirExprKind::Call`.

## 3. Result-changing decisions

### 3.1 ID and accepted Dialogue preservation

The exact accepted field types are preserved:

```rust
pub struct HirIdSuffix(Box<str>);
pub struct HirIdFamily(Box<str>);
pub struct HirRelativeId {
    suffix: HirIdSuffix,
    parent_depth: usize,
}
```

The accepted `HirIdRef`, dialogue coordinates, line-plan records, postfix candidate failures, source sites, insertions, call-argument ordinals, and candidate-only `SyntheticRole` values are retained. For each candidate interpretation, the candidate root is ordinal zero and nested candidate-only children use deterministic zero-based preorder per `HirIdKind`; no candidate-only key is reused for a committed selected expression.

### 3.2 Regions and runtime lifetime registry

Type regions and script-visible registry access have disjoint owners. `HirTypeRegion` is used only in HIR type nodes; `HirLifetimeRegistryPath` and `HirLifetimeRegistryAccessMode` are used only for registry expressions/statements. Elision allocates `SyntheticRole::ElidedRegion` ordinal zero under the owning `TypeId`.

### 3.3 Paths

`HirPath` preserves `ImplicitCrate`, `Crate`, `SelfModule`, and `Super { depth }` roots and ordered typed segments. Resolution always receives the immutable `HirSnapshotId` and owning scope. Implicit-root aliases are resolved from that snapshot before crate-root fallback; explicit roots never masquerade as aliases. External-capable project segments remain typed segments. No dotted string is split and no root is collapsed.

### 3.4 Literals

Integers and compact integer sequences use a canonical arbitrary-precision unsigned magnitude. Values above `u128::MAX` are retained exactly; target-width overflow is a checker result, not a lossy HIR flag. Decimal, float, unit-number, and Duration payloads use canonical decimal digits. Duration is an independent literal and type family whose valid HIR value is canonical whole nanoseconds. Float width and IEEE bits are selected by the exact algorithm in `LITERAL_NUMERIC_CONTRACT.md`; NaN and infinity are not literal spellings.

### 3.5 Calls

`HirCallCallee` is either a value `ExprId` or an associated-type receiver rooted at `TypeId`. Dot-member calls use value-first/nominal-second precedence; explicit associated syntax is type-only. The selected type tree is projected directly to the existing `ResolvedAssociatedTypeReceiver` and `CallCallee::AssociatedType`. There is one resolver and no Capacity-only HIR.

### 3.6 Thread

A Thread owns its optional name, attached/detached mode, child `ScopeId`, and ordered typed flow items. It does not store an unexplained block expression. It has no expression tail; its body result is Unit and its expression type is `ThreadHandle<Unit>`. Poisoned Thread HIR is never admitted to the scheduler.

### 3.7 Dialogue and RichText

One `HirDialogueContent` is owned by the dialogue application expression. Its nodes, tags, and arguments use stable composite IDs local to that content. Nested calls and conditions use same-arena `ExprId`s. Authored/inferred start/end forms, text, raw text, escape, ruby, interpolation, controls, marks, line breaks, errors, builtin tags/Fx, registered tags, unresolved tags, argument forms, decoded values, and typed recovery are all exhaustive.

### 3.8 Recovery and rollback

Known syntax families retain their typed HIR variant with poisoned slot state and a typed issue. A node whose family cannot be identified becomes `Error`. Optional block tails synthesize `ImplicitUnitTail`; required missing tails synthesize poisoned `MissingRequiredTail`; missing operands synthesize poisoned `RecoveryOperand`. Every allocation batch is transactional. Exact limits commit; one-over aborts without an ID, source component, scope, local, diagnostic, candidate, retained-result, checked-value, or publication leak.

## 4. Equality, ordering, and deterministic identity

Rust `Eq`/`Ord` are structural over canonical HIR fields and are suitable for in-process maps. They are not a stable artifact hash. Stable cache/fingerprint input is a separately versioned canonical byte stream: one-byte discriminants, fixed-endian integers, ULEB128 lengths, canonical UTF-8 bytes, canonical arbitrary-precision limbs, and ordered child IDs. Source spans, snapshot addresses, allocator addresses, and `std::hash::Hash` output are excluded.

Numeric-value equality is a named semantic operation distinct from structural equality: it compares the arbitrary value plus the checker-selected type and ignores authored radix. Path target equality is a resolver result distinct from structural path equality.

## 5. Public API boundary

Fields are private. Only lowering contexts construct payloads. Public consumers receive immutable accessors. Semantic payloads do not implement public Serde. Cache/bundle codecs consume checked publication records after validation; they never deserialize directly into mutable HIR arenas.

## 6. Completion

All required schemas, source roles, lowering rows, tests, consumer migrations, and deletion gates are specified by this archive. `OPEN_QUESTIONS.md` is exactly `none`.
