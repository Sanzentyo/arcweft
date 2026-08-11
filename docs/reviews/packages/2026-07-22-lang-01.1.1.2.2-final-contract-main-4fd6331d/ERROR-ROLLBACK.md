# Structured Errors and Atomic Rollback

## 1. Error context types

Add these sema-owned types. They are used by carrier validation wrappers, accepted inventory admission, Rust metadata projection, and callable projection.

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EnvironmentPublicationItemId {
    AdapterNominal {
        owner: EnvironmentCallableOwner,
        path: TypePath,
    },
    AdapterFunction {
        owner: EnvironmentCallableOwner,
        path: ProjectCallablePath,
        overload: CallableOverloadIndex,
    },
    AdapterMethod {
        owner: EnvironmentCallableOwner,
        method: CallableName,
        overload: CallableOverloadIndex,
    },
    RustType {
        adapter: AdapterPackageId,
        package: RustPackageId,
        rust_item: RustItemPath,
        accepted_path: TypePath,
    },
    RustFunction {
        adapter: AdapterPackageId,
        package: RustPackageId,
        rust_item: RustItemPath,
        callable_path: ProjectCallablePath,
        overload: CallableOverloadIndex,
    },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EnvironmentTypeSiteRoot {
    MethodReceiver,
    Parameter {
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
    Result,
    RustStructTupleField {
        field: u16,
    },
    RustStructRecordField {
        field: String,
    },
    RustEnumTupleField {
        variant: String,
        field: u16,
    },
    RustEnumRecordField {
        variant: String,
        field: String,
    },
    RustNewtypeInner,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EnvironmentTypeSiteStep {
    VecItem,
    SeqItem,
    OptionItem,
    ResultOk,
    ResultError,
    TupleItem(u16),
    NominalArgument(u16),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EnvironmentTypeSite {
    root: EnvironmentTypeSiteRoot,
    steps: Box<[EnvironmentTypeSiteStep]>,
}
```

Every diagnostic contains the exact item and type site:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentPublicationProjectionDiagnostic {
    item: EnvironmentPublicationItemId,
    site: EnvironmentTypeSite,
    primary: SourceSpan,
    related: Box<[EnvironmentPublicationRelatedSource]>,
    kind: EnvironmentPublicationProjectionErrorKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentPublicationRelatedSource {
    label: EnvironmentPublicationRelatedLabel,
    source: SourceSpan,
}
```

Related labels include containing callable/Rust item, first duplicate declaration, accepted declaration, and owner declaration.

## 2. Projection error enum

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnvironmentPublicationProjectionErrorKind {
    DetachedWorld,
    IncompleteWorld {
        missing: AcceptedWorldComponent,
    },
    WorldMismatch {
        expected: ProjectSymbolWorldId,
        actual: ProjectSymbolWorldId,
    },
    RevisionMismatch {
        expected: ProjectSymbolRevision,
        actual: ProjectSymbolRevision,
    },
    CatalogDigestMismatch {
        expected: AcceptedNominalCatalogDigest,
        actual: AcceptedNominalCatalogDigest,
    },
    SourceRevisionMismatch {
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
    UnknownPath {
        requested: AcceptedNominalId,
    },
    InaccessibleExport {
        requested: AcceptedNominalId,
    },
    OwnerMismatch {
        requested: AcceptedNominalId,
        visible: AcceptedNominalId,
    },
    WrongArity {
        nominal: AcceptedNominalId,
        expected: u16,
        actual: usize,
    },
    InvalidAcceptedSemantics {
        nominal: AcceptedNominalId,
        semantics: AcceptedNominalSemanticsKind,
    },
    FreeTypeParameterInCallable {
        index: ArcweftRustTypeParameterIndex,
    },
    UnboundMetadataTypeParameter {
        owner: AcceptedNominalId,
        index: ArcweftRustTypeParameterIndex,
    },
    MetadataOwnerMismatch {
        declaration: AcceptedNominalId,
        package: RustPackageId,
    },
    LimitExceeded {
        kind: NominalResolutionLimitKind,
        observed: u64,
        maximum: u64,
    },
    CallableLimitExceeded {
        kind: CallableBuildLimitKind,
        observed: usize,
        maximum: usize,
    },
    ArithmeticOverflow {
        operation: EnvironmentProjectionArithmetic,
    },
}
```

`InvalidAcceptedSemantics` is reachable only for corrupt internal state. Rust and adapter-native declarations admitted by this contract use `Opaque`. Existing `Exact` and `Character` records remain valid and instantiate through their current semantics.

## 3. Carrier and inventory errors

Malformed typed carriers fail before `ProjectRegistrationFacts` exists:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum AdapterManifestModelError {
    InvalidNominalPath {
        item: AdapterManifestItemId,
        source: Option<AdapterManifestSourceRange>,
        error: AdapterNominalPathError,
    },
    MissingRustPackageMount {
        package: ArcweftRustPackageId,
        item: AdapterManifestItemId,
    },
    DuplicateRustPackageMount {
        package: ArcweftRustPackageId,
        first: AdapterManifestItemId,
        duplicate: AdapterManifestItemId,
    },
    RustPackageMetadataConflict {
        package: ArcweftRustPackageId,
        first_version: String,
        duplicate_version: String,
        first_hash: Option<String>,
        duplicate_hash: Option<String>,
    },
    RustManifestPackageMismatch {
        expected: ArcweftRustPackageId,
        actual: ArcweftRustPackageId,
        item: AdapterManifestItemId,
    },
    AdapterOwnerMismatch {
        expected: AdapterEnvironmentOwnerId,
        actual: AdapterEnvironmentOwnerId,
        item: AdapterManifestItemId,
    },
    DuplicateNominalDeclaration {
        owner: AdapterEnvironmentOwnerId,
        path: AdapterNominalPath,
        first: AdapterManifestItemId,
        duplicate: AdapterManifestItemId,
    },
    TypeGraphLimit {
        item: AdapterManifestItemId,
        site: AdapterTypeSite,
        kind: NominalResolutionLimitKind,
        observed: u64,
        maximum: u64,
    },
}
```

Accepted inventory admission extends the original owned error enum rather than wrapping it in an ad hoc compatibility helper:

```rust
pub enum AcceptedNominalCatalogError {
    // existing variants
    DuplicateExactPath {
        path: TypePath,
        first: AcceptedNominalId,
        duplicate: AcceptedNominalId,
        first_source: Option<SourceSpan>,
        duplicate_source: Option<SourceSpan>,
    },
    OwnerSourceMismatch {
        id: AcceptedNominalId,
        source_owner: AcceptedNominalOwnerId,
        source: Option<SourceSpan>,
    },
    ConflictingVisibility {
        id: AcceptedNominalId,
        visible_source: SourceSpan,
        inaccessible_source: SourceSpan,
    },
}
```

This is a concrete source-reporting correction required by the request. It does not change catalog lookup semantics.

## 4. Report shape and fail-closed rule

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentPublicationProjectionReport {
    diagnostics: Box<[EnvironmentPublicationProjectionDiagnostic]>,
    omitted_diagnostics: u64,
    work: u64,
}
```

A report is returned only on failure. A successful result contains no diagnostic carrier. There is no “partial success” variant.

A failure never creates:

- `TypeKind::Named`;
- `TypeKind::Error`;
- an unchecked callable parameter;
- an untyped method receiver;
- a final publication with missing records; or
- a metadata catalog with missing members.

`CallableValidator::Untyped` remains legal only for the pre-existing explicit untyped method publication model; it is not used to recover a nominal projection failure.

## 5. Exact failure phase table

| Failure | Earliest phase | Structured kind | Primary source | Related source | Transaction result |
|---|---|---|---|---|---|
| empty/control package ID | Rust ABI validation | `ArcweftRustIdentityError` | Rust manifest field when decoded through adapter | Rust item | no source-backed facts |
| empty/invalid path segment | carrier validation | `InvalidNominalPath` | exact path segment | containing item | no source-backed facts |
| missing package mount | adapter manifest ingest | `MissingRustPackageMount` | nominal reference | Rust manifest/package declaration | no source-backed facts |
| duplicate package mount | adapter manifest ingest | `DuplicateRustPackageMount` | duplicate mount | first mount | no source-backed facts |
| package version/hash disagreement | project facts | `RustPackageMetadataConflict` | duplicate package claim | first package claim | no registration request |
| adapter owner disagreement | source-backed facts | `AdapterOwnerMismatch` | owner field/item | manifest header | no registration request |
| duplicate adapter nominal declaration | manifest model | `DuplicateNominalDeclaration` | duplicate declaration | first declaration | no source-backed facts |
| duplicate accepted full path across owners | accepted inventory | `DuplicateExactPath` | duplicate declaration | first declaration | no accepted world |
| visible + inaccessible claim for same ID | accepted inventory | `ConflictingVisibility` | second claim | first claim | no accepted world |
| accepted catalog record limit | accepted inventory | existing catalog limit error | offending declaration | catalog owner | no accepted world |
| detached project/world | projection | `DetachedWorld` | containing type node | containing item | no final registered world |
| missing world component | projection | `IncompleteWorld` | containing type node | containing item | no final registered world |
| input world mismatch | projection | `WorldMismatch` | containing item | bound input source | no final registered world |
| source revision mismatch | registration/projection | `SourceRevisionMismatch` | stale span | current source | no final registered world |
| publication catalog stamp mismatch | callable admission | `CatalogDigestMismatch` | publication item | current world source | no final registered world |
| no exact accepted record | projection | `UnknownPath` | nominal path node | containing item | no publication |
| known private export | projection | `InaccessibleExport` | nominal path node | private declaration | no publication |
| path occupied by another owner | projection | `OwnerMismatch` | nominal owner/path node | visible declaration | no publication |
| generic argument count differs | projection | `WrongArity` | nominal application | accepted declaration | no publication |
| free type parameter in callable | projection | `FreeTypeParameterInCallable` | parameter node | Rust function/type parameter | no publication |
| metadata parameter index absent | metadata projection | `UnboundMetadataTypeParameter` | field/payload node | Rust type declaration | no metadata catalog |
| Rust metadata declaration package differs from owner | metadata projection | `MetadataOwnerMismatch` | Rust type declaration | package declaration | no metadata catalog |
| type node/depth/argument/work limit | projection | `LimitExceeded` | node crossing the limit | containing item | no publication/metadata |
| callable record/group/parameter/effect limit | schema/publication build | `CallableLimitExceeded` | callable item | owner manifest | no callable catalog |
| checked integer conversion fails | any bounded builder | `ArithmeticOverflow` | containing item | none | no final registered world |
| duplicate callable candidate | callable builder | existing typed callable catalog error | duplicate callable | first candidate | no callable catalog |
| exact schema mismatch for authored alias | callable builder | existing checked schema mismatch | authored declaration | environment declaration | no callable catalog |
| persistent environment digest construction fails | final identity | typed digest error | registered-world request | none | no final registered world |

## 6. Deterministic diagnostic order

Diagnostics are ordered by this total key:

1. `EnvironmentCallableOwner` authority rank and typed provider ID;
2. `EnvironmentPublicationItemId`;
3. `EnvironmentTypeSiteRoot`;
4. lexicographic `EnvironmentTypeSiteStep` sequence;
5. stable numeric error-code rank;
6. primary `SourceDocumentIdentity`;
7. primary byte range;
8. related-source identities/ranges.

The error-code rank is an explicit exhaustive `match`; it is not enum declaration order and not a formatted string.

When limits truncate diagnostics:

- the retained prefix is the prefix of this sorted total order;
- `omitted_diagnostics` is the checked count of discarded entries;
- work consumed before truncation remains recorded;
- repeated runs produce byte-identical reports.

## 7. Rollback table

| Candidate state created locally | Next operation | Failure behavior |
|---|---|---|
| validated adapter manifest | source map rendering | discard manifest facts; no project facts |
| source-backed adapter parts | `ProjectRegistrationFacts::try_new` | discard all bound environment inputs |
| linked project symbols/external owners | accepted inventory assembly | discard linked candidate state |
| accepted catalog + visibility index | `AcceptedNominalWorld::try_new` | discard candidate catalog/index |
| accepted nominal world | project callable resolution | discard world and builder |
| project callable records | Rust metadata projection | discard project records and world |
| accepted Rust metadata catalog | environment callable projection | discard metadata and all projected inputs |
| stamped publications | callable builder admission | discard all publications and builder |
| finished callable catalog | environment digest | discard callable catalog and metadata |
| complete `RegisteredTypeCheckEnv` return value | caller commit/swap | only this point replaces previous world |

No phase mutates the previously accepted world.

## 8. Duplicate and collision source requirements

Where source exists, duplicate diagnostics must identify both declarations. This includes:

- two adapter nominal declarations;
- two Rust package mounts;
- two Rust manifests claiming conflicting package metadata;
- two accepted records at one full path;
- two callable candidates with equal final identity.

For generated adapter/Rust facts, both sources are exact spans in deterministic generated documents. For programmatic tests without a source document, `None` is permitted only in the low-level constructor test; integration registration tests must use source-backed facts.

## 9. Detached and incomplete behavior

A detached or incomplete accepted world never projects a nominal publication. It does not:

- accept primitives while omitting nominals;
- build a reduced publication;
- defer the nominal to tooling;
- use an untyped validator;
- use a display name; or
- retain a previous catalog record for that item.

The entire registered-world candidate fails closed.
