# Compiler-local View rows and persistent bundle projection

## 1. Cut 3 compiler-local row

```rust
pub struct CompilerLocalViewMatchCatalogRow {
    pub checked_match: CheckedMatchRef,
    pub program: ViewProgramId,
    pub accepted_revision: AcceptedViewProgramRevision,
    pub site: ViewMatchSiteId,
    pub checked_match_semantic: CheckedMatchSemanticDigest,
    pub view_admission: CheckedViewMatchAdmissionDigest,
    pub need_admission: CheckedNeedProducerAdmissionDigest,
    pub ownership: OwnershipEvidenceDigest,
    pub resource_dependency: Option<ResourceDependencyDigest>,
}
```

This row is keyed by the exact compiler generation and exists only inside the
compiler catalog. `checked_match` permits exact lookup against
`FinalSemanticAnalysis`. It is never encoded into a bundle or runtime
snapshot.

Cut 3 depends only on Cut 1/2 semantic/admission products and current View
identity types. It does not depend on `NeedProducerContractDigest`,
`TaskPlanSemanticDigest`, `RuntimeValueDigest`, `TaskSpec`, `TaskExecution` or
any snapshot row introduced in Cut 4/5.

## 2. Stable View site

```text
domain = "arcweft.view.match-site.v1\0"
ViewProgramId semantic bytes
AcceptedDeclarationSemanticId
CheckedExpressionChildRolePath for the Match site
```

The path contains closed source-order roles/accepted field identities.
`AcceptedViewProgramRevision`, raw HIR IDs, spans and source spelling are
excluded. As a semantic coordinate digest, all-zero is valid and absence is
`Option<ViewMatchSiteId>`.

## 3. Cut 5 persistent row

```rust
pub struct AcceptedViewMatchBundleRowV1 {
    pub version: ViewMatchBundleRowVersion,
    pub program: ViewProgramIdProjection,
    pub accepted_revision: AcceptedViewProgramRevisionProjection,
    pub site: ViewMatchSiteIdProjection,
    pub checked_match: CheckedMatchSemanticDigestProjection,
    pub view_admission: CheckedViewMatchAdmissionDigestProjection,
    pub need_admission: CheckedNeedProducerAdmissionDigestProjection,
    pub ownership: OwnershipEvidenceDigestProjection,
    pub producer_contract: NeedProducerContractDigest,
    pub payload_type: RuntimeTypeSemanticDigest,
    pub plan: TaskPlanSemanticDigest,
    pub arguments: RuntimeValueDigest,
    pub resource_dependency: Option<ResourceDependencyDigestProjection>,
}
```

The version constructor accepts exactly one.

Projection newtypes exist to maintain legitimate dependency direction. Each is
a strict byte projection of the named accepted owner. It cannot be constructed
from arbitrary debug/source strings.

## 4. Forbidden persistent content

The row and its nested projections contain none of:

- `CheckedMatchRef`;
- `ExprId`, `PatternId`, `LocalId`, `HirSnapshotId` or `SourceSpan`;
- a compiler-only certificate object;
- a copied `CompilerLocalViewMatchCatalogRow`;
- source/debug spelling;
- a whole generic Serde payload.

The package validator scans both the field list and machine data for these
names.

## 5. Publication join

`AcceptedViewMatchBundleRowV1::project_from_joined_products` receives:

```rust
pub struct AcceptedViewMatchBundleInputs<'a> {
    pub compiler: &'a CompilerLocalViewMatchCatalogRow,
    pub current_revision: &'a AcceptedViewProgramRevision,
    pub awbc: &'a AcceptedAwbcViewTaskProduct,
    pub producer: &'a AcceptedNeedProducerProduct,
    pub runtime_type: &'a RuntimeCheckedType,
    pub arguments: &'a RuntimeValue,
    pub resource: Option<&'a AcceptedResourceDependency>,
}
```

Validation order:

1. compiler row exact snapshot/catalog ownership;
2. current program/revision equality;
3. `ViewMatchSiteId` and checked Match semantic digest;
4. View admission/Need admission/ownership digest equality;
5. AWBC task-plan semantic digest recomputation;
6. producer family (`ViewMatchSubscription`), contract and site equality;
7. runtime payload type digest;
8. canonical argument digest;
9. resource dependency projection;
10. strict version-1 row construction.

The constructor copies only validated identity/digest projections. It does not
hash the join to mint another semantic identity.

## 6. Runtime validation

Bundle loading:

1. strict row decode;
2. validate `ViewProgramIdProjection`;
3. validate revision projection against the accepted current revision catalog;
4. validate site/checked/admission/ownership projections against compiler
   products retained in the accepted bundle build;
5. validate producer contract, payload type, plan and argument digest against
   the AWBC/runtime product;
6. publish the row to the View runtime catalog.

Revision is not producer identity. A replacement may map an old accepted
revision/site to a new accepted revision/site only after the complete semantic
and task product checks in the replacement transaction.

## 7. Structural absence tests

- serializing a compiler-local row is impossible because it has no bundle
  codec implementation;
- adding `CheckedMatchRef`/`ExprId`/`HirSnapshotId`/`SourceSpan` to the machine
  bundle field list fails the package validator;
- changing Cut 3 dependencies to any Cut 4 task/digest type fails the package
  validator;
- a bundle row with equal debug labels but a mismatched checked Match digest
  fails;
- a bundle row cannot replace accepted revision validation with a new hash.
