# Implementation and deletion order

All public replacement work after independently compiling private substrate is one unmerged compiling series.

1. **Typed synthetic identity substrate.** Add `SyntheticOwner` behavior to the original identity owner, including `Type(TypeId)`, `kind()`, `module()`, and the original `SyntheticRole` owner/ordinal policy. Replace the private raw-owner field in `SyntheticKey`; migrate private constructors/tests. Do not expose a raw conversion.
2. **Typed source index substrate.** Add `HirSourceQuery`, Pattern/Type role enums, lookup/result/error records, required-role validation, and the single private `source_site` implementation. Stage source entries with the same arena transaction.
3. **Elided type region.** Add the final TypeId-owned region record with `SyntheticOwner::Type`, role ElidedRegion, ordinal zero, and typed insertion source role. Replace the deleted provisional reference island directly; do not restore it.
4. **Pathless pattern carrier.** Replace the provisional Variant payload with `HirVariantPattern` and its qualified/unqualified head. Lower `.Foo`, Some/None/Ok/Err without empty paths or early enum hard-coding.
5. **Literal and limit substrate.** Extend the original `HirLimit::maximum()` owner. Add canonical Duration semantic value and checker result/error records. Remove hard-limit and checker-overflow variants from HIR issue enums.
6. **Lowering/source transaction.** Connect attached syntax directly to final Expr/Pattern/Type owners, preflight all limits, stage source keys, and validate role applicability/required components before commit.
7. **Sema/checker.** Use `HirDurationSemanticValue` for semantic comparison/cache/fingerprint. Emit Float WidthOverflow and Duration RuntimeRangeOverflow only from `arcweft-lang-sema::literal`. Connect pathless variants to expected-type resolution and the shared variant catalog.
8. **Consumer migration.** Migrate verifier, runtime-plan, compiler, LSP, formatter, Agent/debug, persistent cache, and project publication to `HirModule::source_site` and typed records. No consumer may derive identity from vector position or reopen syntax text.
9. **Deletion in the same public switch.** Delete `expr_source_site`, any parallel source map, raw `SyntheticKey` owner access, `HirLeafLimits`, `HirFloatIssue::WidthOverflow`, `HirDurationIssue::RuntimeRangeOverflow`, `HirStringIssue::DecodedByteLimitExceeded`, digit-limit issue variants, the all-variants-required `path:HirPath` shape, old literal/path readers, detached syntax clones, and every obsolete match arm.
10. **Validation.** Run focused HIR/syntax/sema/checker/source-query/rollback tests; workspace check/strict Clippy; normal workspace tests; Tier 2 when runtime/render/Agent paths are touched; compile-fail public API tests; structural audit. The residual-reader review is one-off review evidence and never a checked-in source gate.

No step may introduce an alias, wrapper, extension trait, V2 carrier, compatibility codec, dual reader, source-string parser, source gate, CSS/Takumi path, or removed-syntax-specific diagnostic.
