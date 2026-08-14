# Decision 07 — compile-clean generation/domain/context/validator order

The retry phase order is replaced. The final generation owner lands before any validator type that borrows it. Checked nominal lookup is additionally bound to one non-forgeable construction domain; a generation-wide validator would be too broad.

## Final API

Owner: `arcweft_core::plan::admission` for generation/context; checked behavior remains on the existing `RuntimeCheckedType` inherent implementation in `arcweft_core::pattern`.

```rust
pub struct AdmittedRuntimeGeneration {
    // private accepted declaration, project roots, producer roots,
    // nominal layouts/closures, and catalog declarations
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimeCheckedValueContext<'generation> {
    generation: &'generation AdmittedRuntimeGeneration,
    domain: RuntimeNominalRecordAdmissionDomain<'generation>,
}

impl<'generation> RuntimeNominalRecordAdmissionDomain<'generation> {
    #[must_use]
    pub fn checked_values(self) -> RuntimeCheckedValueContext<'generation>;
}

impl RuntimeCheckedValueContext<'_> {
    pub fn validate(
        self,
        expected: &RuntimeCheckedType,
        value: &RuntimeValue,
    ) -> Result<(), RuntimeCheckedTypeError>;

    pub(crate) fn lookup_nominal(
        self,
        nominal: &RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
    ) -> Result<AdmittedRuntimeNominalLayout<'_>, RuntimeNominalCatalogLookupError>;
}

impl RuntimeCheckedType {
    pub fn validate_value(
        &self,
        value: &RuntimeValue,
        context: RuntimeCheckedValueContext<'_>,
    ) -> Result<(), RuntimeCheckedTypeError>;

    pub(crate) fn matches_non_authoritative_pattern(
        &self,
        value: &RuntimeValue,
    ) -> bool;
}
```

The context constructor is private to `RuntimeNominalRecordAdmissionDomain::checked_values`. Project domains are issued only by `AdmittedRuntimePlan::project_nominal_domain(site)` or the plan-paired AWBC product's exact domain/site resolver; producer domains are the retained admitted producer-shape borrows. The domain and generation share the same `'generation` lifetime. A caller cannot combine a domain from one generation with another generation, select an unscoped project root, or create a context from a generation identity scalar.

`RuntimeCheckedValueContext` is non-Serde, non-Default, has private fields, no public constructor, no `Deref`, no `into_inner`, no generation-wide nominal lookup, and no owned clone. It may be `Copy` only because its retained domain is already the accepted non-exclusive borrowed authority; copying preserves identical site/producer and generation provenance.

`RuntimeCheckedValueValidator<'generation>` is crate-private implementation state. Its sole constructor takes `RuntimeCheckedValueContext`, initializes `remaining_work=65_536`, `depth=0`, and root paths, and is called only by `RuntimeCheckedValueContext::validate`. It is never public API.

## Compile-clean cuts

1. Add final project/producer root errors, facts, declaration DTOs, and `AdmittedRuntimeGeneration` with private fields and admission constructor. No validator refers to a missing type.
2. Add final `RuntimeNominalRecordAdmissionDomain` issuance from admitted plan/product and retained producer shape; add domain-bound `RuntimeCheckedValueContext`. No validation caller is migrated yet.
3. Extend the legitimate `RuntimeValuePath`, add non-Serde `RuntimeCheckedTypePath`, complete shapes, structured checked errors, and private validator state.
4. Add inherent `RuntimeCheckedType::validate_value` and migrate one core nominal-tree caller through an exact final domain. In the same cut, make public `accepts_value` unavailable to authority callers and retain only the crate-private non-authoritative matcher.
5. Add final plan type declarations, wrappers, paths, coordinates, and private wire DTOs; no placeholder root-use table is introduced.
6. Update runtime-plan lowering to emit the mandatory fields from accepted semantic facts; compile until every current owner row in `RUNTIME_PLAN_SITE_RESOLUTION.csv` is handled.
7. Add raw-plan admission and opaque `AdmittedRuntimePlan`; migrate plan execution entry points or keep them disabled until the admitted wrapper is available.
8. Replace AWBC runtime type/constant/pattern owners, add exact sites/origins/domain operands, codec/verifier changes, and pair/standalone admission in one schema-version-1 cut.
9. Migrate VM/fiber/product/executor/session/hot-swap/player APIs to admitted wrappers; delete raw execution constructors in the same cut.
10. Add the dialogue-owned catalog bridge and migrate dialogue schema construction; remove the runtime-driver bridge name, caller generation scalar, and unsupported Character-to-View error in the same cut.
11. Add standard role rows/registry through current `TypeCheckEnv`/`AcceptedNominalWorld`/registrar; delete the nonexistent environment API from design and all relevant `Named` success paths in the same cut.
12. Migrate restore/replay/View/save/patch/diagnostics to final paths and domain-bound checked contexts; delete dialogue-local paths and boolean authority checks.
13. Run codec goldens/tamper matrices/compile-fail tests, then delete stale fixtures, aliases, root-use rows, generic slot helpers, and old readers before workspace gates.

Every numbered cut builds without a dummy admitted generation, generation-wide nominal validator, public validator constructor, nullable authority, temporary resolver, defaulted field, or boolean success fallback.
