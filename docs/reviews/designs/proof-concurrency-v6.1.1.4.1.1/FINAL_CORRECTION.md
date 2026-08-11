# Final source-owner and semantic-consistency correction

## 1. Authority

This contract is Proof-concurrency v6.1.1.4.1.1. It normatively supersedes only the contradictory source-owner, pathless-pattern, Duration, overflow, synthetic-owner, limit, and traceability rows in the retained v6.1.1.4.1 ZIP. All uncontradicted decisions remain in force: one qualified expression arena, the 35-expression inventory, the 12-pattern inventory, AW-AH-009.4.2 Dialogue/ID outer shapes, same-arena RichText children, one shared callable resolver, the accepted Thread body, and deletion-driven public migration.

Repository `main` was inspected at `5018912852a45e96f48735767021bf858ffcd493`. The intervening cleanup commits remove obsolete facades and do not select any of the missing payloads addressed here.

## 2. Single typed source authority

The sole public source query is:

```rust
impl HirModule {
    pub fn source_site<'a>(
        &'a self,
        expected_source: &SourceDocumentIdentity,
        query: HirSourceQuery,
    ) -> Result<HirSourceLookup<'a>, HirSourceQueryError>;
}
```

`HirSourceQuery` is an enum whose three variants pair the correct typed owner and role: `Expr { owner: ExprId, role: HirExprSourceRole }`, `Pattern { owner: PatternId, role: HirPatternSourceRole }`, and `Type { owner: TypeId, role: HirTypeSourceRole }`. The same enum is the exact non-`Whole` component-map key. It is not an untyped owner and cannot express a Pattern role for an `ExprId`.

The old `expr_source_site` method is **superseded and deleted**, not wrapped. There is no overload, compatibility alias, parallel reader, or vector-position fallback.

A lookup returns `Present(Span|Insertion)` or `AbsentOptional`, together with `Clean|Poisoned` owner status. Foreign IDs produce typed `IdResolveError::WrongModule`; stale source identity/revision/length is rejected by `HirSourceQueryError`; a rolled-back transaction returns no public owner ID and publishes no source key.

## 3. Pathless variant patterns

The final variant payload is:

```rust
pub struct HirVariantPattern {
    head: HirVariantPatternHead,
    name: HirName,
    payload: Option<PatternId>,
}

pub enum HirVariantPatternHead {
    Qualified(HirPath),
    Unqualified(HirUnqualifiedVariantForm),
}

pub enum HirUnqualifiedVariantForm {
    DotShorthand,
    BareExpectedType,
}
```

`.Foo` lowers to `Unqualified(DotShorthand)`. `Some`, `None`, `Ok`, and `Err` lower to `Unqualified(BareExpectedType)`. They remain unresolved until expected-type variant resolution; HIR neither hard-codes Option/Result nor creates an empty path. A qualified source uses `Qualified(HirPath)` and retains all root semantics.

The optional payload is a same-module `PatternId` whose kind is Tuple or Record. It inherits the variant pattern's lexical scope; no extra scope is created. Unknown expected types and unknown variants are sema diagnostics over clean HIR. Malformed known variant syntax remains `HirPatternKind::Variant` with typed poison. Only unclassifiable pattern syntax becomes `Error`.

## 4. Duration identity

`HirDurationValue` structurally contains both a canonical `HirDurationSemanticValue` and the normalized authored unit. Its derived `Eq`, `Hash`, and `Ord` include both fields. Therefore `1s` and `1000ms` are structurally different HIR values.

`HirDurationSemanticValue` contains only whole nanoseconds and separately derives `Eq`, `Hash`, and `Ord`. Checker comparison, constant folding, checked-value caches, runtime-plan lowering, verifier comparison, and the checked artifact fingerprint use `HirDurationValue::semantic_value()`. Therefore `1s` and `1000ms` are semantically equal and have the same checked-value fingerprint.

The authored unit is semantic HIR payload used for typed diagnostics and structural/incremental identity. Its source range and exact spelling remain revision-bound source components. No custom `Eq` or `Hash` implementation ignores a field, so each type obeys the Rust `Eq`/`Hash`/`Ord` consistency laws.

## 5. Overflow phase

A valid HIR Float contains a canonical decimal and optional explicit width. Width conversion happens in `arcweft-lang-sema::literal`; a result that would round to infinity returns `FloatLiteralCheckResult::Rejected(FloatLiteralCheckError::WidthOverflow { ... })`. No bits are published.

A valid HIR Duration contains arbitrary-precision whole nanoseconds. Runtime admission happens in the same checker owner; a value above `u64::MAX` returns `DurationLiteralCheckResult::Rejected(DurationLiteralCheckError::RuntimeRangeOverflow { ... })`. No `LogicalDuration` or runtime constant is published.

`HirFloatIssue::WidthOverflow` and `HirDurationIssue::RuntimeRangeOverflow` are deleted. They are not retained as unreachable compatibility variants. Hard byte/digit/count limit failures are lowering transaction errors and likewise do not become invalid HIR payload variants.

## 6. Elided-region owner

`SyntheticOwner` retains every accepted typed owner and adds exactly `Type(TypeId)`. `SyntheticKey` stores `SyntheticOwner`, `SyntheticRole`, and `u32` ordinal. `SyntheticKey::try_new` validates only the role's typed owner-kind and ordinal policy without probing an arena slot; the owning HIR transaction then resolves that typed owner against its module/snapshot before staging the key.

For an elided type region:

```text
owner   = SyntheticOwner::Type(reference_type_id)
role    = SyntheticRole::ElidedRegion
ordinal = 0
source  = HirSourceQuery::Type { owner: reference_type_id,
          role: HirTypeSourceRole::Region(ElisionInsertion) }
```

The insertion point is the parser-owned elision anchor between `&` and the next region/mutability/referent token, revision-bound to the exact source document. `HirElidedRegion::try_new(reference_type_id, key)` additionally requires `key.owner() == SyntheticOwner::Type(reference_type_id)`. Equality/order/hash include the typed owner variant. Stable fingerprint bytes encode owner-kind discriminant, module, slot, role discriminant, and ordinal. A foreign, not-yet-live, or retired owner is rejected by the typed owner resolver before staging. The current private raw-owner field is replaced directly; no raw-owner accessor, wrapper, or dual key remains.

## 7. Limits

The existing `HirLimit` enum is the single HIR limit owner. This correction adds source/leaf/path/registry variants and their exact inclusive maxima through the original `HirLimit::maximum()` implementation. `HirLowerError::Limit(HirLimitError { limit, observed, maximum })` owns all hard failures. `observed` and `maximum` are `usize` after checked conversion.

Every hard limit is preflighted before any public slot, source row, scope, diagnostic, candidate, checked value, or publication record is committed. Exact succeeds; one-over aborts the complete transaction. Callable and RichText limits retain their own accepted owners and are not merged into `HirLimit`.

## 8. Matrix and tests

`LOWERING_MATRIX.tsv` is the complete corrected 82-row matrix, not a delta. `TEST_MATRIX.tsv` is the complete corrected matrix with dedicated source, rollback, pathless-variant, Duration-identity, checker-overflow, synthetic-owner, limit, and migration rows. `SUBTEST_REGISTRY.tsv` defines all 164 previously absent references as named subtests under `T-SOURCE-01` or `T-ROLLBACK-01`.

Fieldless valid variants are not tested by inventing impossible field mutations. Their negative tests attempt to publish an inapplicable child/component role or unexpected child and assert typed rejection with no invalid constructor.

## 9. Readiness

All result-changing decisions in the v6.1.1.4.1.1 request are closed. `OPEN_QUESTIONS.md` is exactly `none`. The affected public HIR switch is `READY_FOR_IMPLEMENTATION` subject to the deletion-driven implementation order in this archive.
