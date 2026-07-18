use std::{collections::HashSet, sync::Arc};

use arcweft_character::id::CharacterId;
use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
    symbol::{
        CallableDeclarationId, CallableDeclarationOwner, CallablePackageId,
        ExternalDeclarationSeed, ProjectDirectBinding, ProjectExternalDeclarations,
        ProjectSymbolRevision, ProjectSymbolTable, ProjectSymbolWorldId,
    },
};
use arcweft_lang_syntax::{
    ast::{
        common::Visibility,
        module_path::{CanonicalModulePath, ModulePathRoot},
        symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
    },
    parser::parse_source,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use crate::{
    canonicalization::SemanticScopeId, checker::TypeExpressionId, effect_row::EffectRow,
    types::TypeKind,
};

use super::limits::{CatalogBuildWork, ResolverWork};
use super::{
    AdapterPackageId, AgentIntrinsicSignatureId, BuiltinCallableId, CallPoison,
    CallableArgumentIndex, CallableArgumentPolicy, CallableArgumentSlotIndex,
    CallableAuthorityRank, CallableBuildLimitError, CallableCandidateId, CallableCatalogBuildError,
    CallableCatalogError, CallableDiagnosticCode, CallableDocumentation, CallableEffectSchema,
    CallableGroupIndex, CallableGroupKind, CallableIdentityError, CallableIndexKind,
    CallableInstantiation, CallableLimits, CallableLookupKey, CallableName, CallableOverloadIndex,
    CallableParameter, CallableParameterCoordinate, CallableParameterGroup, CallableParameterIndex,
    CallableParameterPassing, CallableParameterPresence, CallableParameterSource,
    CallableParameterType, CallablePath, CallablePathError, CallableQueryLimitError,
    CallableScalarError, CallableScalarKind, CallableSchemaError, CallableSignatureSchema,
    CallableSource, CallableValidator, CharacterOwnerSource, CurriedCallableId, DataLastCallableId,
    DialogueCallableId, DialogueCalleeIdentity, EnvironmentCallableId, EnvironmentCallableKind,
    EnvironmentCallableOwner, EnvironmentDeclarationOrdinal, FloatWidth, FunctionValueOrdinal,
    FunctionValueSignatureId, FxCallableSignatureId, FxResolution, LanguageCallableFamily,
    LexicalBindingIndex, LocalCallableId, NonEmptyCallableSet, NonEmptyResolvedCandidates,
    PresentationCallableId, ProjectCallablePath, ProjectNameBinding, ReceiverMethodKey,
    ReductionConstructorKind, RegisteredCallableCatalogBuilder, ResolveCallError, ResolvedCallable,
    ResolvedCharacterOwner, ResolvedFunctionValue, RustItemPath, SemanticParameter,
    SemanticParameterGroup, SemanticSignature, SemanticSignatureError, SemanticSignatureHelp,
    SemanticSignatureIndex, SignatureOrigin, SignatureWorkReport, SpreadArgumentPolicy,
    StandardEnvironmentId, StdFloatCallableId, StdFloatOperation, TraitImplementationIndex,
    UnknownNamedArgumentPolicy,
};

fn name(value: &str) -> CallableName {
    CallableName::try_new(value).expect("valid callable name")
}

fn path(segments: &[&str]) -> CallablePath {
    CallablePath::try_new(segments.iter().map(|segment| name(segment)))
        .expect("valid callable path")
}

fn index(value: usize) -> CallableParameterIndex {
    CallableParameterIndex::try_from_usize(value).expect("parameter index")
}

fn group(value: usize) -> CallableGroupIndex {
    CallableGroupIndex::try_from_usize(value).expect("group index")
}

fn limits(groups: usize, parameters: usize, work: u64) -> CallableLimits {
    CallableLimits::for_test(32, groups, parameters, 32, 256, 256, 128, work, work)
}

fn project_binding_limits(max_path_segments: usize, work: u64) -> CallableLimits {
    CallableLimits::for_test(max_path_segments, 16, 128, 32, 256, 256, 128, work, work)
}

fn project_binding_path(segments: impl IntoIterator<Item = String>) -> ProjectSymbolPath {
    ProjectSymbolPath::new(
        ModulePathRoot::ImplicitCrate,
        segments
            .into_iter()
            .map(|segment| ProjectSymbolSegment::try_new(segment).expect("valid project segment")),
    )
    .expect("test project binding path is non-empty")
}

fn external_binding_project(
    bindings: impl IntoIterator<Item = (String, ProjectSymbolPath)>,
) -> (HirProject, ProjectSymbolTable) {
    let source = " ";
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://callable-catalog-tests/src/main.arcw")
                .expect("document id"),
            SourceName::path("src/main.arcw"),
            source,
        )
        .expect("source document"),
    );
    let parsed = parse_source(source);
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let hir = lower_document_to_hir(&document, parsed.typed_tree()).expect("lowered empty HIR");
    let project = HirProject::new(
        "callable-catalog-tests",
        [HirProjectModule::try_new(
            CanonicalModulePath::crate_root(),
            document.identity().clone(),
            hir,
        )
        .expect("root module")],
    )
    .expect("HIR project");
    let declaration = document
        .span(SourceRange::new(0, source.len()))
        .expect("declaration span");
    let seeds = bindings
        .into_iter()
        .map(|(canonical, path)| {
            let binding = ProjectDirectBinding::try_new(
                CanonicalModulePath::crate_root(),
                path,
                Some(Visibility::Public),
                declaration.clone(),
                false,
            )
            .expect("typed direct binding");
            ExternalDeclarationSeed::try_new(
                SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), canonical)
                    .expect("opaque canonical path"),
                Some(Visibility::Public),
                declaration.clone(),
                vec![binding],
            )
            .expect("external declaration seed")
        })
        .collect::<Vec<_>>();
    let package = CallablePackageId::try_new("callable-catalog-tests").expect("package");
    let world = ProjectSymbolWorldId::try_new(
        package,
        document.identity().id().clone(),
        "typed-project-bindings",
    )
    .expect("world");
    let revision =
        ProjectSymbolRevision::try_for_documents([document.identity()]).expect("project revision");
    let externals = ProjectExternalDeclarations::try_new(world, revision, seeds)
        .expect("external declarations");
    let symbols = ProjectSymbolTable::link(&project, &externals)
        .expect("typed project bindings link")
        .into_table();
    (project, symbols)
}

struct ResolvedFixture {
    base: CallableCandidateId,
    origin: SignatureOrigin,
    authority: Option<CallableAuthorityRank>,
}

fn multi_group_schema(group_count: usize) -> Arc<CallableSignatureSchema> {
    let groups = (0..group_count)
        .map(|index| {
            CallableParameterGroup::try_new(
                group(index),
                if index == 0 {
                    CallableGroupKind::Initial
                } else {
                    CallableGroupKind::Curried
                },
                Vec::new(),
                &limits(group_count, 4, 20),
            )
            .expect("valid empty parameter group")
        })
        .collect();
    Arc::new(
        CallableSignatureSchema::try_new(
            groups,
            TypeKind::String,
            CallableEffectSchema::fixed(EffectRow::default()),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::Reject,
            ),
            CallableValidator::Ordinary,
            &limits(group_count, 4, 20),
        )
        .expect("valid multi-group schema"),
    )
}

fn project_fixture(name_value: &str) -> ResolvedFixture {
    let package = CallablePackageId::try_new("game").expect("package");
    let module = CanonicalModulePath::crate_root();
    let declaration = CallableDeclarationId::try_new(
        package.clone(),
        module.clone(),
        CallableDeclarationOwner::Function,
        name_value,
    )
    .expect("project declaration");
    ResolvedFixture {
        base: CallableCandidateId::Project(declaration.clone()),
        origin: SignatureOrigin::Project {
            declaration,
            path: ProjectCallablePath::new(package, module, path(&[name_value])),
        },
        authority: Some(CallableAuthorityRank::Project),
    }
}

fn standard_fixture() -> ResolvedFixture {
    let owner = StandardEnvironmentId::Core;
    let id = EnvironmentCallableId::new(
        EnvironmentCallableOwner::Standard(owner),
        EnvironmentCallableKind::Function,
        CallableLookupKey::Free(path(&["standard_curried"])),
        CallableOverloadIndex::try_from_usize(0).expect("overload"),
    );
    ResolvedFixture {
        base: CallableCandidateId::Environment(id.clone()),
        origin: SignatureOrigin::Standard {
            owner,
            id: id.clone(),
        },
        authority: Some(CallableAuthorityRank::Standard),
    }
}

fn adapter_fixture() -> ResolvedFixture {
    let package = AdapterPackageId::try_new("adapter.test").expect("adapter package");
    let id = EnvironmentCallableId::new(
        EnvironmentCallableOwner::Adapter(package.clone()),
        EnvironmentCallableKind::Function,
        CallableLookupKey::Free(path(&["adapter_curried"])),
        CallableOverloadIndex::try_from_usize(0).expect("overload"),
    );
    ResolvedFixture {
        base: CallableCandidateId::Environment(id.clone()),
        origin: SignatureOrigin::Adapter {
            package,
            id: id.clone(),
        },
        authority: Some(CallableAuthorityRank::Adapter),
    }
}

fn data_last_candidate() -> CallableCandidateId {
    let parameter = CallableParameter::try_new(
        index(0),
        Some(name("value")),
        CallableParameterType::Exact(TypeKind::String),
        CallableParameterPassing::PositionalOrNamed,
        CallableParameterPresence::Required,
        None,
        None,
    )
    .expect("data-last parameter");
    let schema = CallableSignatureSchema::try_new(
        vec![
            CallableParameterGroup::try_new(
                group(0),
                CallableGroupKind::Initial,
                vec![parameter],
                &limits(2, 4, 20),
            )
            .expect("data-last group"),
        ],
        TypeKind::Unit,
        CallableEffectSchema::fixed(EffectRow::default()),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            SpreadArgumentPolicy::Reject,
        ),
        CallableValidator::Ordinary,
        &limits(2, 4, 20),
    )
    .expect("data-last schema");
    let base = CallableCandidateId::Local(LocalCallableId::new(
        SemanticScopeId(77),
        LexicalBindingIndex::try_from_usize(0).expect("binding index"),
    ));
    CallableCandidateId::DataLast(
        DataLastCallableId::try_new(base, group(0), index(0), &schema).expect("valid data-last ID"),
    )
}

fn assert_curried_one_over(fixture: ResolvedFixture) {
    let schema = multi_group_schema(1);
    let one_over = CallableGroupIndex::try_from_usize(schema.groups().len())
        .expect("one-over group index fits");
    let curried = CurriedCallableId::try_new(fixture.base.clone(), one_over)
        .expect("nonzero curried identity");
    assert_eq!(
        ResolvedCallable::try_new(
            CallableCandidateId::Curried(curried),
            fixture.origin,
            schema,
            CallableInstantiation::Curried {
                base: fixture.base.clone(),
                group: one_over,
            },
            Vec::new(),
            fixture.authority,
            &limits(2, 4, 20),
        ),
        Err(ResolveCallError::InvalidCallGroup {
            candidate: fixture.base,
            group: one_over,
        })
    );
}

#[test]
fn callable_scalar_invariants() {
    assert_eq!(name("valid_name").as_str(), "valid_name");
    assert_eq!(
        CallableName::try_new(""),
        Err(CallableScalarError::Empty {
            kind: CallableScalarKind::CallableName,
        })
    );
    assert_eq!(
        CallableName::try_new("bad.name"),
        Err(CallableScalarError::ContainsSeparator {
            kind: CallableScalarKind::CallableName,
            byte: 3,
            separator: '.',
        })
    );
    assert_eq!(
        CallableName::try_new("bad\nname"),
        Err(CallableScalarError::Control {
            kind: CallableScalarKind::CallableName,
            byte: 3,
        })
    );
    assert!(matches!(
        AdapterPackageId::try_new("adapter id"),
        Err(CallableScalarError::ContainsSeparator {
            kind: CallableScalarKind::AdapterPackageId,
            separator: ' ',
            ..
        })
    ));
    assert!(matches!(
        AdapterPackageId::try_new(""),
        Err(CallableScalarError::Empty {
            kind: CallableScalarKind::AdapterPackageId,
        })
    ));
    assert!(matches!(
        AdapterPackageId::try_new("adapter/path"),
        Err(CallableScalarError::ContainsSeparator {
            kind: CallableScalarKind::AdapterPackageId,
            separator: '/',
            ..
        })
    ));
    assert!(matches!(
        AdapterPackageId::try_new("adapter\nid"),
        Err(CallableScalarError::Control {
            kind: CallableScalarKind::AdapterPackageId,
            ..
        })
    ));
    assert_eq!(
        RustItemPath::try_new("crate::module::function<T> ")
            .expect("provenance keeps Rust punctuation and spaces")
            .as_str(),
        "crate::module::function<T> "
    );
    assert!(matches!(
        RustItemPath::try_new(""),
        Err(CallableScalarError::Empty {
            kind: CallableScalarKind::RustItemPath,
        })
    ));
    assert!(matches!(
        RustItemPath::try_new("crate::item\n"),
        Err(CallableScalarError::Control {
            kind: CallableScalarKind::RustItemPath,
            ..
        })
    ));
}

#[test]
fn callable_index_invariants() {
    assert_eq!(
        CallableParameterIndex::try_from_usize(u16::MAX as usize)
            .expect("max")
            .get(),
        u16::MAX as usize
    );
    assert_eq!(
        CallableParameterIndex::try_from_usize(u16::MAX as usize + 1),
        Err(CallableScalarError::IndexOverflow {
            kind: CallableIndexKind::Parameter,
            value: u16::MAX as usize + 1,
        })
    );
    assert_eq!(
        CallableGroupIndex::try_from_usize(u16::MAX as usize)
            .expect("group max")
            .get(),
        u16::MAX as usize
    );
    assert!(matches!(
        CallableGroupIndex::try_from_usize(u16::MAX as usize + 1),
        Err(CallableScalarError::IndexOverflow {
            kind: CallableIndexKind::Group,
            ..
        })
    ));
    assert_eq!(
        CallableOverloadIndex::try_from_usize(u16::MAX as usize)
            .expect("overload max")
            .get(),
        u16::MAX as usize
    );
    assert!(matches!(
        CallableOverloadIndex::try_from_usize(u16::MAX as usize + 1),
        Err(CallableScalarError::IndexOverflow {
            kind: CallableIndexKind::Overload,
            ..
        })
    ));
    assert_eq!(
        CallableArgumentIndex::try_from_usize(u16::MAX as usize)
            .expect("argument max")
            .get(),
        u16::MAX as usize
    );
    assert!(matches!(
        CallableArgumentIndex::try_from_usize(u16::MAX as usize + 1),
        Err(CallableScalarError::IndexOverflow {
            kind: CallableIndexKind::Argument,
            ..
        })
    ));
    assert_eq!(
        CallableArgumentSlotIndex::try_from_usize(u16::MAX as usize)
            .expect("slot max")
            .get(),
        u16::MAX as usize
    );
    assert!(matches!(
        CallableArgumentSlotIndex::try_from_usize(u16::MAX as usize + 1),
        Err(CallableScalarError::IndexOverflow {
            kind: CallableIndexKind::ArgumentSlot,
            ..
        })
    ));
    assert_eq!(
        LexicalBindingIndex::try_from_usize(u32::MAX as usize)
            .expect("lexical max")
            .get(),
        u32::MAX as usize
    );
    assert_eq!(
        FunctionValueOrdinal::try_from_usize(u32::MAX as usize)
            .expect("function value max")
            .get(),
        u32::MAX as usize
    );
    if usize::BITS > u32::BITS {
        assert!(matches!(
            LexicalBindingIndex::try_from_usize(u32::MAX as usize + 1),
            Err(CallableScalarError::IndexOverflow {
                kind: CallableIndexKind::LexicalBinding,
                ..
            })
        ));
        assert!(matches!(
            FunctionValueOrdinal::try_from_usize(u32::MAX as usize + 1),
            Err(CallableScalarError::IndexOverflow {
                kind: CallableIndexKind::FunctionValue,
                ..
            })
        ));
    }
}

#[test]
fn callable_additional_index_invariants() {
    assert_eq!(
        EnvironmentDeclarationOrdinal::try_from_usize(u32::MAX as usize)
            .expect("declaration ordinal max")
            .get(),
        u32::MAX as usize
    );
    assert_eq!(
        TraitImplementationIndex::try_from_usize(u32::MAX as usize)
            .expect("trait implementation max")
            .get(),
        u32::MAX as usize
    );
    assert_eq!(
        SemanticSignatureIndex::try_from_usize(u16::MAX as usize)
            .expect("signature index max")
            .get(),
        u16::MAX as usize
    );
    assert_eq!(
        SemanticSignatureIndex::try_from_usize(u16::MAX as usize + 1),
        Err(SemanticSignatureError::ActiveSignatureOutOfBounds)
    );
    if usize::BITS > u32::BITS {
        assert!(EnvironmentDeclarationOrdinal::try_from_usize(u32::MAX as usize + 1).is_err());
        assert!(TraitImplementationIndex::try_from_usize(u32::MAX as usize + 1).is_err());
    }
}

#[test]
fn callable_path_exact_limit_and_one_over() {
    assert_eq!(
        CallablePath::try_new(Vec::<CallableName>::new()),
        Err(CallablePathError::Empty)
    );
    let exact_limits = CallableLimits::for_test(3, 16, 128, 32, 256, 256, 128, 100, 100);
    let exact = CallablePath::try_new_with_limits(
        (0..3).map(|value| name(&format!("p{value}"))),
        &exact_limits,
    )
    .expect("exact path limit");
    assert_eq!(exact.len(), 3);
    assert_eq!(
        CallablePath::try_new_with_limits(
            (0..4).map(|value| name(&format!("p{value}"))),
            &exact_limits,
        ),
        Err(CallablePathError::TooManySegments {
            actual: 4,
            limit: 3,
        })
    );
}

#[test]
fn typed_project_binding_path_limit_is_fail_closed() {
    let binding = project_binding_path((0..3).map(|index| format!("segment-{index}")));
    let (project, symbols) = external_binding_project([("adapter.long".to_owned(), binding)]);
    let mut builder = RegisteredCallableCatalogBuilder::new(project_binding_limits(2, 100));

    assert_eq!(
        builder.add_project_bindings(&project, &symbols, |_| Some(TypeKind::I32)),
        Err(CallableCatalogBuildError::Limit(
            CallableBuildLimitError::PathSegments {
                actual: 3,
                limit: 2,
            }
        ))
    );
}

#[test]
fn typed_project_binding_work_charges_one_row_plus_each_segment() {
    let fixture = || {
        external_binding_project([(
            "adapter.viewport".to_owned(),
            project_binding_path(["adapter".to_owned(), "viewport".to_owned()]),
        )])
    };
    let (project, symbols) = fixture();
    let mut exact = RegisteredCallableCatalogBuilder::new(project_binding_limits(32, 3));
    exact
        .add_project_bindings(&project, &symbols, |_| Some(TypeKind::I32))
        .expect("one row plus two segments consumes exactly three units");

    let (project, symbols) = fixture();
    let mut one_under = RegisteredCallableCatalogBuilder::new(project_binding_limits(32, 2));
    assert_eq!(
        one_under.add_project_bindings(&project, &symbols, |_| Some(TypeKind::I32)),
        Err(CallableCatalogBuildError::Limit(
            CallableBuildLimitError::Work {
                requested: 2,
                consumed: 1,
                limit: 2,
            }
        ))
    );
}

#[test]
fn typed_project_binding_without_registered_type_is_fail_closed() {
    let (project, symbols) = external_binding_project([(
        "adapter.viewport".to_owned(),
        project_binding_path(["adapter".to_owned(), "viewport".to_owned()]),
    )]);
    let expected = symbols
        .scope_bindings()
        .next()
        .expect("external scope binding")
        .2
        .clone();
    let mut builder = RegisteredCallableCatalogBuilder::new(project_binding_limits(32, 100));

    assert_eq!(
        builder.add_project_bindings(&project, &symbols, |_| None),
        Err(CallableCatalogBuildError::MissingProjectBindingType { target: expected })
    );
}

#[test]
fn typed_project_binding_collision_rejects_the_complete_catalog() {
    let shared = project_binding_path(["shared".to_owned()]);
    let (project, symbols) = external_binding_project([
        ("adapter.first".to_owned(), shared.clone()),
        ("adapter.second".to_owned(), shared),
    ]);
    let mut types = [TypeKind::I32, TypeKind::I64].into_iter();
    let mut builder = RegisteredCallableCatalogBuilder::new(project_binding_limits(32, 100));
    builder
        .add_project_bindings(&project, &symbols, |_| types.next())
        .expect("typed rows stage before collision validation");

    let error = builder
        .finish()
        .expect_err("different targets at one typed path reject the catalog");
    let CallableCatalogBuildError::ProjectBindingCollision {
        path,
        first: ProjectNameBinding::NonCallable { ty: first_type, .. },
        second: ProjectNameBinding::NonCallable {
            ty: second_type, ..
        },
    } = error
    else {
        panic!("unexpected catalog error: {error:?}");
    };
    assert_eq!(
        path.path()
            .segments()
            .iter()
            .map(CallableName::as_str)
            .collect::<Vec<_>>(),
        ["shared"]
    );
    assert_ne!(first_type, second_type);
}

#[test]
fn identical_typed_project_bindings_at_the_same_path_are_accepted() {
    let shared = project_binding_path(["shared".to_owned()]);
    let (project, symbols) = external_binding_project([
        ("adapter.first".to_owned(), shared.clone()),
        ("adapter.second".to_owned(), shared),
    ]);
    let key = ProjectCallablePath::new(
        project.package().clone(),
        CanonicalModulePath::crate_root(),
        path(&["shared"]),
    );
    let mut builder = RegisteredCallableCatalogBuilder::new(project_binding_limits(32, 100));
    builder
        .add_project_bindings(&project, &symbols, |_| Some(TypeKind::I32))
        .expect("identical typed rows stage successfully");

    let catalog = builder
        .finish()
        .expect("identical bindings at one typed path are idempotent");
    assert!(matches!(
        catalog.project().binding(&key),
        Some(ProjectNameBinding::NonCallable {
            ty: TypeKind::I32,
            ..
        })
    ));
}

#[test]
fn reversed_typed_external_facts_produce_identical_project_catalogs() {
    let viewport = (
        "adapter.viewport".to_owned(),
        project_binding_path(["adapter".to_owned(), "viewport".to_owned()]),
    );
    let mode = (
        "adapter.mode".to_owned(),
        project_binding_path(["adapter".to_owned(), "mode".to_owned()]),
    );
    let build = |bindings| {
        let (project, symbols) = external_binding_project(bindings);
        let rows = symbols
            .scope_bindings()
            .map(|(module, path, target)| (module.clone(), path.clone(), target.clone()))
            .collect::<Vec<_>>();
        let mut builder = RegisteredCallableCatalogBuilder::new(project_binding_limits(32, 100));
        builder
            .add_project_bindings(&project, &symbols, |_| Some(TypeKind::I32))
            .expect("typed project bindings");
        (
            rows,
            builder
                .finish()
                .expect("complete deterministic callable catalog"),
        )
    };
    let forward = build([viewport.clone(), mode.clone()]);
    let reverse = build([mode, viewport]);

    assert_eq!(forward, reverse);
    assert_eq!(forward.0.len(), 2);
}

#[test]
fn builtin_identity_table_and_near_misses() {
    let cases = [
        (&["fallback"][..], BuiltinCallableId::InlineFailureFallback),
        (
            &["InlineFailure", "fallback"][..],
            BuiltinCallableId::InlineFailureFallback,
        ),
        (&["panic"][..], BuiltinCallableId::Panic),
        (&["fail"][..], BuiltinCallableId::Fail),
        (&["bail"][..], BuiltinCallableId::Bail),
        (&["ensure"][..], BuiltinCallableId::Ensure),
        (&["assert"][..], BuiltinCallableId::Assert),
        (&["debug_assert"][..], BuiltinCallableId::DebugAssert),
        (&["rgb"][..], BuiltinCallableId::Rgb),
        (&["sin"][..], BuiltinCallableId::Sin),
        (&["cos"][..], BuiltinCallableId::Cos),
        (
            &["Reduction", "unchanged"][..],
            BuiltinCallableId::Reduction(ReductionConstructorKind::Unchanged),
        ),
    ];
    for (segments, expected) in cases {
        assert_eq!(BuiltinCallableId::resolve(&path(segments)), Some(expected));
    }
    assert_eq!(BuiltinCallableId::resolve(&path(&["Panic"])), None);
    assert_eq!(
        BuiltinCallableId::resolve(&path(&["std", "f32", "to_f32"])),
        None
    );
    assert!(BuiltinCallableId::resolve(&path(&["std", "f32", "to_f64"])).is_some());
}

#[test]
fn builtin_extended_identity_and_schema_table_is_typed() {
    let direct = [
        (vec!["vec2"], "Vec2"),
        (vec!["vec3"], "Vec3"),
        (vec!["vec4"], "Vec4"),
        (vec!["math", "matmul_f32"], "MatrixF32"),
        (vec!["math", "matrix_add_f32"], "MatrixF32"),
        (vec!["math", "matmul_f64"], "MatrixF64"),
        (vec!["math", "matrix_add_f64"], "MatrixF64"),
        (vec!["math", "tensor_add_f32"], "TensorF32"),
        (vec!["math", "tensor_add_f64"], "TensorF64"),
    ];
    for (segments, result) in direct {
        let id = BuiltinCallableId::resolve(&path(&segments)).expect("builtin identity");
        assert_eq!(
            id.signature_schema().result(),
            &TypeKind::Named(result.to_owned())
        );
    }
    assert_eq!(
        BuiltinCallableId::resolve(&path(&["event", "emit"]))
            .expect("event emit")
            .signature_schema()
            .result(),
        &TypeKind::Unit
    );

    let operations = [
        ("abs", StdFloatOperation::Abs),
        ("floor", StdFloatOperation::Floor),
        ("ceil", StdFloatOperation::Ceil),
        ("round", StdFloatOperation::Round),
        ("trunc", StdFloatOperation::Trunc),
        ("fract", StdFloatOperation::Fract),
        ("sqrt", StdFloatOperation::Sqrt),
        ("sin", StdFloatOperation::Sin),
        ("cos", StdFloatOperation::Cos),
        ("tan", StdFloatOperation::Tan),
        ("exp", StdFloatOperation::Exp),
        ("exp2", StdFloatOperation::Exp2),
        ("ln", StdFloatOperation::Ln),
        ("log2", StdFloatOperation::Log2),
        ("log10", StdFloatOperation::Log10),
        ("powf", StdFloatOperation::Powf),
        ("atan2", StdFloatOperation::Atan2),
        ("mul_add", StdFloatOperation::MulAdd),
        ("is_nan", StdFloatOperation::IsNan),
        ("is_infinite", StdFloatOperation::IsInfinite),
        ("is_finite", StdFloatOperation::IsFinite),
        ("is_sign_positive", StdFloatOperation::IsSignPositive),
        ("is_sign_negative", StdFloatOperation::IsSignNegative),
        ("to_bits", StdFloatOperation::ToBits),
        ("from_bits", StdFloatOperation::FromBits),
    ];
    for (name, operation) in operations {
        for (width_name, width) in [("f32", FloatWidth::F32), ("f64", FloatWidth::F64)] {
            let expected = BuiltinCallableId::StdFloat(
                StdFloatCallableId::try_new(width, operation).expect("supported float pair"),
            );
            assert_eq!(
                BuiltinCallableId::resolve(&path(&["std", width_name, name])),
                Some(expected)
            );
        }
    }
    assert!(BuiltinCallableId::resolve(&path(&["std", "f32", "to_f64"])).is_some());
    assert!(BuiltinCallableId::resolve(&path(&["std", "f64", "to_f32"])).is_some());
}

#[test]
fn fx_identity_table_is_closed() {
    let cases = [
        ("style", FxCallableSignatureId::Style),
        ("text", FxCallableSignatureId::Text),
        ("color", FxCallableSignatureId::Color),
        ("transform", FxCallableSignatureId::Transform),
        ("mask", FxCallableSignatureId::Mask),
        ("filter", FxCallableSignatureId::Filter),
        ("shader", FxCallableSignatureId::Shader),
        ("transition", FxCallableSignatureId::Transition),
        ("conditional", FxCallableSignatureId::Conditional),
        ("stack", FxCallableSignatureId::Stack),
    ];
    for (member, expected) in cases {
        assert_eq!(
            FxCallableSignatureId::resolve(&path(&["Fx", member])),
            FxResolution::Known(expected)
        );
    }
    assert_eq!(
        FxCallableSignatureId::resolve(&path(&["Fx", "unknown"])),
        FxResolution::UnknownMember {
            member: name("unknown"),
        }
    );
    assert!(matches!(
        FxCallableSignatureId::resolve(&path(&["Fx", "stack", "nested"])),
        FxResolution::InvalidNestedPath { .. }
    ));
}

#[test]
fn agent_identity_table_is_complete() {
    let cases = [
        (&["expect"][..], AgentIntrinsicSignatureId::Expect),
        (&["deny"][..], AgentIntrinsicSignatureId::Deny),
        (&["checkpoint"][..], AgentIntrinsicSignatureId::Checkpoint),
        (&["note"][..], AgentIntrinsicSignatureId::Note),
        (&["attach"][..], AgentIntrinsicSignatureId::Attach),
        (
            &["choice_action"][..],
            AgentIntrinsicSignatureId::ChoiceAction,
        ),
        (&["viewport"][..], AgentIntrinsicSignatureId::Viewport),
        (&["layer"][..], AgentIntrinsicSignatureId::Layer),
        (&["object"][..], AgentIntrinsicSignatureId::Object),
        (&["capture"][..], AgentIntrinsicSignatureId::Capture),
        (
            &["read_resource"][..],
            AgentIntrinsicSignatureId::ReadResource,
        ),
        (&["entity_meta"][..], AgentIntrinsicSignatureId::EntityMeta),
        (
            &["project_neighbors"][..],
            AgentIntrinsicSignatureId::ProjectNeighbors,
        ),
        (&["signal"][..], AgentIntrinsicSignatureId::Signal),
        (&["metric"][..], AgentIntrinsicSignatureId::Metric),
        (&["state_path"][..], AgentIntrinsicSignatureId::StatePath),
        (
            &["observation_path"][..],
            AgentIntrinsicSignatureId::ObservationPath,
        ),
        (&["state"][..], AgentIntrinsicSignatureId::State),
        (&["observation"][..], AgentIntrinsicSignatureId::Observation),
        (&["diagnostics"][..], AgentIntrinsicSignatureId::Diagnostics),
        (&["exists"][..], AgentIntrinsicSignatureId::Exists),
        (
            &["action_enabled"][..],
            AgentIntrinsicSignatureId::ActionEnabled,
        ),
        (&["all"][..], AgentIntrinsicSignatureId::All),
        (&["any"][..], AgentIntrinsicSignatureId::Any),
        (&["not"][..], AgentIntrinsicSignatureId::Not),
        (&["wait"][..], AgentIntrinsicSignatureId::Wait),
        (
            &["advance_text"][..],
            AgentIntrinsicSignatureId::AdvanceText,
        ),
        (
            &["viewport_point"][..],
            AgentIntrinsicSignatureId::ViewportPoint,
        ),
        (
            &["pointer", "click"][..],
            AgentIntrinsicSignatureId::PointerClick,
        ),
        (&["invoke"][..], AgentIntrinsicSignatureId::Invoke),
        (&["rag", "query"][..], AgentIntrinsicSignatureId::RagQuery),
    ];
    for (segments, expected) in cases {
        assert_eq!(
            AgentIntrinsicSignatureId::resolve(&path(segments)),
            Some(expected)
        );
    }
    assert_eq!(
        AgentIntrinsicSignatureId::resolve(&path(&["pointer_click"])),
        None
    );
}

#[test]
fn family_schemas_preserve_validator_result_effect_and_structural_owner() {
    let conditional = FxCallableSignatureId::Conditional.signature_schema();
    assert_eq!(conditional.result(), &TypeKind::Named("Fx".to_owned()));
    assert_eq!(conditional.groups()[0].parameters().len(), 3);
    assert_eq!(
        conditional.validator(),
        &CallableValidator::Fx(FxCallableSignatureId::Conditional)
    );

    let capture = AgentIntrinsicSignatureId::Capture.signature_schema();
    assert_eq!(capture.groups()[0].parameters().len(), 4);
    assert!(
        capture
            .effects()
            .declared()
            .concrete()
            .iter()
            .any(|effect| effect.as_str() == "agent.capture")
    );
    assert_eq!(
        capture.validator(),
        &CallableValidator::Agent(AgentIntrinsicSignatureId::Capture)
    );

    let unchanged = ReductionConstructorKind::Unchanged.signature_schema();
    assert_eq!(
        unchanged.result(),
        &TypeKind::Named("Reduction<_>".to_owned())
    );
    assert_eq!(unchanged.groups()[0].parameters().len(), 1);
    assert_eq!(
        unchanged.validator(),
        &CallableValidator::ReductionConstructor(ReductionConstructorKind::Unchanged)
    );

    let character = CharacterId::try_new("character.alice").expect("character id");
    let owner =
        ResolvedCharacterOwner::new(character.clone(), CharacterOwnerSource::EntityReference);
    let show = super::schema::presentation_schema(PresentationCallableId::Show, Some(&owner))
        .expect("show schema");
    assert_eq!(
        show.result(),
        &TypeKind::presentation_handle("CharacterSurface")
    );
    assert_eq!(
        show.groups()[0].parameters()[1].ty(),
        &CallableParameterType::Exact(TypeKind::character_look(character.clone()))
    );

    let speaker = super::schema::dialogue_schema(
        DialogueCallableId::SpeakerLine,
        &DialogueCalleeIdentity::Speaker {
            character: character.clone(),
        },
    )
    .expect("dialogue schema");
    assert_eq!(
        speaker.groups()[0].parameters()[3].ty(),
        &CallableParameterType::Exact(TypeKind::character_look(character))
    );
    assert_eq!(speaker.groups()[0].parameters().len(), 14);
}

#[test]
fn presentation_paths_are_exact_and_receiver_keys_are_structural() {
    let cases = [
        (&["view"][..], PresentationCallableId::View),
        (&["menu"][..], PresentationCallableId::Menu),
        (&["overlay"][..], PresentationCallableId::Overlay),
        (&["bg"][..], PresentationCallableId::Background),
        (&["image"][..], PresentationCallableId::Image),
        (
            &["player_viewport"][..],
            PresentationCallableId::PlayerViewport,
        ),
        (&["show"][..], PresentationCallableId::Show),
        (&["ref", "bg"][..], PresentationCallableId::RefBackground),
        (&["ref", "show"][..], PresentationCallableId::RefShow),
        (
            &["clear", "bg"][..],
            PresentationCallableId::ClearBackground,
        ),
        (&["hide"][..], PresentationCallableId::Hide),
    ];
    for (segments, expected) in cases {
        assert_eq!(
            PresentationCallableId::resolve(&path(segments)),
            Some(expected)
        );
    }
    assert_eq!(PresentationCallableId::resolve(&path(&["Ref", "bg"])), None);

    let key = ReceiverMethodKey::new(TypeKind::Vec(Box::new(TypeKind::String)), name("map"));
    let mut keys = HashSet::new();
    keys.insert(key.clone());
    assert!(keys.contains(&ReceiverMethodKey::new(
        TypeKind::Vec(Box::new(TypeKind::String)),
        name("map"),
    )));
    assert!(!keys.contains(&ReceiverMethodKey::new(
        TypeKind::Named("Vec<String>".to_owned()),
        name("map"),
    )));
}

#[test]
fn dialogue_identity_table_is_complete() {
    let character = CharacterId::try_new("character.alice").expect("character id");
    assert_eq!(
        DialogueCallableId::resolve(&DialogueCalleeIdentity::Speaker {
            character: character.clone(),
        }),
        DialogueCallableId::SpeakerLine
    );
    assert_eq!(
        DialogueCallableId::resolve(&DialogueCalleeIdentity::SpeakerPreset { character }),
        DialogueCallableId::SpeakerLine
    );
    assert_eq!(
        DialogueCallableId::resolve(&DialogueCalleeIdentity::Content {
            path: path(&["dialogue", "opening"]),
        }),
        DialogueCallableId::ContentCall
    );
}

#[test]
fn non_empty_result_wrappers_reject_empty_inputs() {
    assert_eq!(
        NonEmptyCallableSet::try_new(Vec::new(), &limits(2, 4, 20)),
        Err(CallableCatalogError::EmptyCandidateSet)
    );
    assert_eq!(
        NonEmptyResolvedCandidates::try_new(Vec::new(), &limits(2, 4, 20)),
        Err(ResolveCallError::InvalidResolvedCallable)
    );
}

#[test]
fn resolved_callable_validates_origin_family_and_function_value_type() {
    let builtin = super::CallableCandidateId::Builtin(BuiltinCallableId::Panic);
    assert_eq!(
        ResolvedCallable::try_new(
            builtin.clone(),
            SignatureOrigin::Language {
                family: LanguageCallableFamily::Agent,
            },
            Arc::new(BuiltinCallableId::Panic.signature_schema()),
            CallableInstantiation::None,
            Vec::new(),
            None,
            &limits(2, 4, 20),
        ),
        Err(ResolveCallError::InvalidResolvedCallable)
    );
    assert_eq!(
        ResolvedCallable::try_new(
            builtin.clone(),
            SignatureOrigin::Language {
                family: LanguageCallableFamily::Builtin,
            },
            Arc::new(BuiltinCallableId::Panic.signature_schema()),
            CallableInstantiation::Curried {
                base: builtin.clone(),
                group: group(1),
            },
            Vec::new(),
            None,
            &limits(2, 4, 20),
        ),
        Err(ResolveCallError::InvalidResolvedCallable)
    );
    let resolved_builtin = ResolvedCallable::try_new(
        builtin,
        SignatureOrigin::Language {
            family: LanguageCallableFamily::Builtin,
        },
        Arc::new(BuiltinCallableId::Panic.signature_schema()),
        CallableInstantiation::None,
        Vec::new(),
        None,
        &limits(2, 4, 20),
    )
    .expect("matching builtin origin");
    let candidates =
        NonEmptyResolvedCandidates::try_new(vec![resolved_builtin.clone()], &limits(2, 4, 20))
            .expect("non-empty resolved candidates");
    assert_eq!(candidates.len().get(), 1);
    assert_eq!(candidates.first(), &resolved_builtin);

    let function_id = FunctionValueSignatureId::new(
        TypeExpressionId::from_index(7),
        FunctionValueOrdinal::try_from_usize(0).expect("function ordinal"),
    );
    let function_callable = ResolvedCallable::try_new(
        super::CallableCandidateId::FunctionValue(function_id.clone()),
        SignatureOrigin::FunctionValue {
            id: function_id.clone(),
        },
        Arc::new(BuiltinCallableId::Panic.signature_schema()),
        CallableInstantiation::None,
        Vec::new(),
        None,
        &limits(2, 4, 20),
    )
    .expect("function-value callable");
    assert_eq!(
        ResolvedFunctionValue::try_new(
            function_id,
            function_callable,
            TypeKind::String,
            None,
            None,
            group(0),
        ),
        Err(ResolveCallError::InvalidResolvedCallable)
    );
}

#[test]
fn curried_id_accepts_nonzero_without_schema() {
    let base = CallableCandidateId::Builtin(BuiltinCallableId::Panic);
    let next_group = group(1);
    let curried =
        CurriedCallableId::try_new(base.clone(), next_group).expect("structural curried ID");
    assert_eq!(curried.base(), &base);
    assert_eq!(curried.next_group(), next_group);
}

#[test]
fn curried_id_rejects_initial_group() {
    let base = CallableCandidateId::Builtin(BuiltinCallableId::Panic);
    let initial = group(0);
    assert_eq!(
        CurriedCallableId::try_new(base.clone(), initial),
        Err(CallableIdentityError::InvalidCurriedGroup {
            base: Box::new(base),
            group: initial,
        })
    );
}

#[test]
fn curried_id_rejects_curried_base() {
    let base = CallableCandidateId::Builtin(BuiltinCallableId::Panic);
    let wrapped = CallableCandidateId::Curried(
        CurriedCallableId::try_new(base, group(1)).expect("first curried ID"),
    );
    assert_eq!(
        CurriedCallableId::try_new(wrapped.clone(), group(2)),
        Err(CallableIdentityError::InvalidCurriedBase {
            base: Box::new(wrapped),
        })
    );
}

#[test]
fn curried_id_rejects_data_last_base() {
    let wrapped = data_last_candidate();
    assert_eq!(
        CurriedCallableId::try_new(wrapped.clone(), group(1)),
        Err(CallableIdentityError::InvalidCurriedBase {
            base: Box::new(wrapped),
        })
    );
}

#[test]
fn curried_id_wrapper_error_precedes_initial_group_error() {
    let base = CallableCandidateId::Builtin(BuiltinCallableId::Panic);
    let curried = CallableCandidateId::Curried(
        CurriedCallableId::try_new(base, group(1)).expect("first curried ID"),
    );
    for wrapped in [curried, data_last_candidate()] {
        assert_eq!(
            CurriedCallableId::try_new(wrapped.clone(), group(0)),
            Err(CallableIdentityError::InvalidCurriedBase {
                base: Box::new(wrapped),
            })
        );
    }
}

#[test]
fn resolved_curried_accepts_exact_multi_group_schema() {
    let ResolvedFixture {
        base,
        origin,
        authority,
    } = project_fixture("curried_ok");
    let schema = multi_group_schema(2);
    let next_group = group(1);
    let expected_group = schema.group(next_group).expect("group 1");
    let curried = CurriedCallableId::try_new(base.clone(), next_group).expect("curried ID");
    let expected_id = CallableCandidateId::Curried(curried.clone());
    let expected_instantiation = CallableInstantiation::Curried {
        base: base.clone(),
        group: next_group,
    };
    let resolved = ResolvedCallable::try_new(
        expected_id.clone(),
        origin,
        Arc::clone(&schema),
        expected_instantiation.clone(),
        Vec::new(),
        authority,
        &limits(2, 4, 20),
    )
    .expect("canonical curried product");
    assert_eq!(resolved.id(), &expected_id);
    assert_eq!(resolved.instantiation(), &expected_instantiation);
    assert!(std::ptr::eq(schema.as_ref(), resolved.schema()));
    assert!(std::ptr::eq(
        expected_group,
        resolved
            .schema()
            .group(next_group)
            .expect("published group 1")
    ));
}

#[test]
fn resolved_curried_rejects_project_one_over_group() {
    assert_curried_one_over(project_fixture("project_one_over"));
}

#[test]
fn resolved_curried_rejects_standard_one_over_group() {
    assert_curried_one_over(standard_fixture());
}

#[test]
fn resolved_curried_rejects_adapter_one_over_group() {
    assert_curried_one_over(adapter_fixture());
}

#[test]
fn resolved_curried_rejects_base_id_representation() {
    let ResolvedFixture {
        base,
        origin,
        authority,
    } = project_fixture("base_representation");
    assert_eq!(
        ResolvedCallable::try_new(
            base.clone(),
            origin,
            multi_group_schema(2),
            CallableInstantiation::Curried {
                base,
                group: group(1),
            },
            Vec::new(),
            authority,
            &limits(2, 4, 20),
        ),
        Err(ResolveCallError::InvalidResolvedCallable)
    );
}

#[test]
fn resolved_curried_rejects_mismatched_base() {
    let ResolvedFixture {
        base,
        origin,
        authority,
    } = project_fixture("curried_base_a");
    let other = project_fixture("curried_base_b").base;
    let curried = CurriedCallableId::try_new(base, group(1)).expect("curried ID");
    assert_eq!(
        ResolvedCallable::try_new(
            CallableCandidateId::Curried(curried),
            origin,
            multi_group_schema(2),
            CallableInstantiation::Curried {
                base: other,
                group: group(1),
            },
            Vec::new(),
            authority,
            &limits(2, 4, 20),
        ),
        Err(ResolveCallError::InvalidResolvedCallable)
    );
}

#[test]
fn resolved_curried_rejects_mismatched_group() {
    let ResolvedFixture {
        base,
        origin,
        authority,
    } = project_fixture("curried_group_mismatch");
    let curried = CurriedCallableId::try_new(base.clone(), group(1)).expect("curried ID");
    assert_eq!(
        ResolvedCallable::try_new(
            CallableCandidateId::Curried(curried),
            origin,
            multi_group_schema(3),
            CallableInstantiation::Curried {
                base,
                group: group(2),
            },
            Vec::new(),
            authority,
            &limits(3, 4, 20),
        ),
        Err(ResolveCallError::InvalidResolvedCallable)
    );
}

#[test]
fn resolved_curried_rejects_non_curried_instantiation() {
    let ResolvedFixture {
        base,
        origin,
        authority,
    } = project_fixture("curried_non_curried_instantiation");
    let curried = CurriedCallableId::try_new(base, group(1)).expect("curried ID");
    assert_eq!(
        ResolvedCallable::try_new(
            CallableCandidateId::Curried(curried),
            origin,
            multi_group_schema(2),
            CallableInstantiation::None,
            Vec::new(),
            authority,
            &limits(2, 4, 20),
        ),
        Err(ResolveCallError::InvalidResolvedCallable)
    );
}

#[test]
fn resolved_curried_rejects_corrupt_world_prebuilt_candidate() {
    let ResolvedFixture {
        base,
        origin,
        authority,
    } = project_fixture("corrupt_curried");
    let missing_group = group(1);
    let curried =
        CurriedCallableId::try_new(base.clone(), missing_group).expect("structural curried ID");
    assert_eq!(
        ResolvedCallable::try_new(
            CallableCandidateId::Curried(curried),
            origin,
            multi_group_schema(1),
            CallableInstantiation::Curried {
                base: base.clone(),
                group: missing_group,
            },
            Vec::new(),
            authority,
            &limits(2, 4, 20),
        ),
        Err(ResolveCallError::InvalidCallGroup {
            candidate: base,
            group: missing_group,
        })
    );
}

#[test]
fn invalid_call_group_has_stable_diagnostic_code() {
    assert_eq!(
        ResolveCallError::InvalidCallGroup {
            candidate: CallableCandidateId::Builtin(BuiltinCallableId::Panic),
            group: group(1),
        }
        .code(),
        CallableDiagnosticCode::InvalidCallGroup
    );
}

#[test]
fn data_last_ids_enforce_context_free_coordinates() {
    let parameter = |parameter_index, passing| {
        CallableParameter::try_new(
            index(parameter_index),
            Some(name(&format!("p{parameter_index}"))),
            CallableParameterType::Exact(TypeKind::String),
            passing,
            CallableParameterPresence::Required,
            None,
            None,
        )
        .expect("data-last parameter")
    };
    let data_last_schema = CallableSignatureSchema::try_new(
        vec![
            CallableParameterGroup::try_new(
                group(0),
                CallableGroupKind::Initial,
                vec![
                    parameter(0, CallableParameterPassing::PositionalOrNamed),
                    parameter(1, CallableParameterPassing::PositionalOrNamed),
                ],
                &limits(2, 4, 20),
            )
            .expect("data-last group"),
        ],
        TypeKind::Unit,
        CallableEffectSchema::fixed(EffectRow::default()),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            SpreadArgumentPolicy::Reject,
        ),
        CallableValidator::Ordinary,
        &limits(2, 4, 20),
    )
    .expect("data-last schema");
    let local = super::CallableCandidateId::Local(LocalCallableId::new(
        SemanticScopeId(1),
        LexicalBindingIndex::try_from_usize(0).expect("binding index"),
    ));
    assert!(matches!(
        DataLastCallableId::try_new(local.clone(), group(0), index(0), &data_last_schema),
        Err(CallableIdentityError::DataLastReceiverNotFinal { .. })
    ));
    let valid = DataLastCallableId::try_new(local.clone(), group(0), index(1), &data_last_schema)
        .expect("final parameter is a valid data-last receiver");
    assert_eq!(valid.receiver_parameter(), index(1));

    let rest_schema = CallableSignatureSchema::try_new(
        vec![
            CallableParameterGroup::try_new(
                group(0),
                CallableGroupKind::Initial,
                vec![parameter(0, CallableParameterPassing::RestPositional)],
                &limits(2, 4, 20),
            )
            .expect("rest group"),
        ],
        TypeKind::Unit,
        CallableEffectSchema::fixed(EffectRow::default()),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            SpreadArgumentPolicy::Reject,
        ),
        CallableValidator::Ordinary,
        &limits(2, 4, 20),
    )
    .expect("rest schema");
    assert!(matches!(
        DataLastCallableId::try_new(local, group(0), index(0), &rest_schema),
        Err(CallableIdentityError::DataLastReceiverIsRest { .. })
    ));
}

#[test]
fn schema_rejects_gaps_duplicate_names_and_invalid_rest() {
    let parameter = |index_value, parameter_name: &str| {
        CallableParameter::try_new(
            index(index_value),
            Some(name(parameter_name)),
            CallableParameterType::Exact(TypeKind::String),
            CallableParameterPassing::PositionalOrNamed,
            CallableParameterPresence::Required,
            None,
            None,
        )
        .expect("parameter")
    };
    assert!(matches!(
        CallableParameterGroup::try_new(
            group(0),
            CallableGroupKind::Initial,
            vec![parameter(1, "gap")],
            &limits(2, 4, 20),
        ),
        Err(CallableSchemaError::NonContiguousParameter { .. })
    ));
    assert!(matches!(
        CallableParameterGroup::try_new(
            group(0),
            CallableGroupKind::Initial,
            vec![parameter(0, "same"), parameter(1, "same")],
            &limits(2, 4, 20),
        ),
        Err(CallableSchemaError::DuplicateParameterName { .. })
    ));
    let rest = CallableParameter::try_new(
        index(0),
        Some(name("rest")),
        CallableParameterType::Unchecked,
        CallableParameterPassing::RestPositional,
        CallableParameterPresence::Defaulted,
        None,
        None,
    );
    assert!(matches!(
        rest,
        Err(CallableSchemaError::InvalidDefaultedRest { .. })
    ));

    let rest_parameter = |parameter_index| {
        CallableParameter::try_new(
            index(parameter_index),
            Some(name(&format!("rest{parameter_index}"))),
            CallableParameterType::Unchecked,
            CallableParameterPassing::RestPositional,
            CallableParameterPresence::Optional,
            None,
            None,
        )
        .expect("rest parameter")
    };
    assert!(matches!(
        CallableParameterGroup::try_new(
            group(0),
            CallableGroupKind::Initial,
            vec![rest_parameter(0), rest_parameter(1)],
            &limits(2, 4, 20),
        ),
        Err(CallableSchemaError::InvalidRestParameter { .. })
    ));
}

#[test]
fn schema_rejects_empty_groups_and_mismatched_source_coordinates() {
    assert_eq!(
        CallableSignatureSchema::try_new(
            Vec::new(),
            TypeKind::Unit,
            CallableEffectSchema::fixed(EffectRow::default()),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::Reject,
            ),
            CallableValidator::Ordinary,
            &limits(2, 4, 20),
        ),
        Err(CallableSchemaError::EmptyGroups)
    );

    let document = SourceDocument::try_new(
        SourceDocumentId::try_new("schema-source-coordinate").expect("document id"),
        SourceName::Memory,
        "value: String",
    )
    .expect("document");
    let whole = document
        .span(SourceRange::new(0, document.text().len()))
        .expect("parameter span");
    let source = CallableParameterSource::try_new(group(1), index(0), whole, None, None, None)
        .expect("source evidence");
    let parameter = CallableParameter::try_new(
        index(0),
        Some(name("value")),
        CallableParameterType::Exact(TypeKind::String),
        CallableParameterPassing::PositionalOrNamed,
        CallableParameterPresence::Required,
        None,
        Some(source),
    )
    .expect("parameter validates its own index");
    assert!(matches!(
        CallableParameterGroup::try_new(
            group(0),
            CallableGroupKind::Initial,
            vec![parameter],
            &limits(2, 4, 20),
        ),
        Err(CallableSchemaError::SourceCoordinateMismatch { .. })
    ));
}

#[test]
fn schema_enforces_contiguous_groups_and_semantic_equality_ignores_docs() {
    let parameter_with_doc = |documentation: &str| {
        CallableParameter::try_new(
            index(0),
            Some(name("value")),
            CallableParameterType::Exact(TypeKind::String),
            CallableParameterPassing::PositionalOrNamed,
            CallableParameterPresence::Required,
            Some(Arc::from(documentation)),
            None,
        )
        .expect("parameter")
    };
    let schema = |documentation: &str| {
        let initial = CallableParameterGroup::try_new(
            group(0),
            CallableGroupKind::Initial,
            vec![parameter_with_doc(documentation)],
            &limits(2, 4, 20),
        )
        .expect("initial group");
        CallableSignatureSchema::try_new(
            vec![initial],
            TypeKind::Bool,
            CallableEffectSchema::fixed(EffectRow::default()),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::Reject,
            ),
            CallableValidator::Ordinary,
            &limits(2, 4, 20),
        )
        .expect("schema")
    };
    let baseline = schema("first docs");
    assert!(baseline.semantic_eq(&schema("other docs")));

    let semantic_variant = |effects, validator| {
        let initial = CallableParameterGroup::try_new(
            group(0),
            CallableGroupKind::Initial,
            vec![parameter_with_doc("docs")],
            &limits(2, 4, 20),
        )
        .expect("initial group");
        CallableSignatureSchema::try_new(
            vec![initial],
            TypeKind::Bool,
            CallableEffectSchema::fixed(effects),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::Reject,
            ),
            validator,
            &limits(2, 4, 20),
        )
        .expect("schema variant")
    };
    assert!(!baseline.semantic_eq(&semantic_variant(
        EffectRow::closed(crate::effects::EffectSet::default()),
        CallableValidator::Ordinary,
    )));
    assert!(!baseline.semantic_eq(&semantic_variant(
        EffectRow::default(),
        CallableValidator::Untyped,
    )));

    let bad_group = CallableParameterGroup::try_new(
        group(1),
        CallableGroupKind::Curried,
        Vec::new(),
        &limits(3, 4, 20),
    )
    .expect("group itself valid");
    assert!(matches!(
        CallableSignatureSchema::try_new(
            vec![bad_group],
            TypeKind::Unit,
            CallableEffectSchema::fixed(EffectRow::default()),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::Reject,
            ),
            CallableValidator::Ordinary,
            &limits(3, 4, 20),
        ),
        Err(CallableSchemaError::NonContiguousGroup { .. })
    ));
}

#[test]
fn inclusive_work_limits_do_not_mutate_on_failure() {
    assert_eq!(
        super::CallableCatalogBuildError::WorkOverflow.code(),
        super::CallableDiagnosticCode::ResourceExhausted
    );
    assert_eq!(
        super::CallableCatalogBuildError::InvalidSchema(CallableSchemaError::EmptyGroups).code(),
        super::CallableDiagnosticCode::CorruptCallableCatalog
    );
    let mut build = CatalogBuildWork::new(3);
    build.charge(3).expect("exact build work");
    assert_eq!(build.consumed(), 3);
    assert_eq!(build.remaining(), 0);
    assert_eq!(build.limit(), 3);
    assert_eq!(
        build.charge(1),
        Err(CallableBuildLimitError::Work {
            requested: 1,
            consumed: 3,
            limit: 3,
        })
    );
    assert_eq!(build.consumed(), 3);

    let mut query = ResolverWork::new(2);
    query.charge(2).expect("exact query work");
    assert_eq!(
        query.charge(1),
        Err(CallableQueryLimitError::Work {
            requested: 1,
            consumed: 2,
            limit: 2,
        })
    );
    assert_eq!(query.consumed(), 2);
    assert_eq!(query.remaining(), 0);
    assert_eq!(query.limit(), 2);

    let report =
        SignatureWorkReport::try_new(1, 1, 1, 0, 0, &limits(2, 4, 3)).expect("exact total work");
    assert_eq!(report.total_work(), Ok(3));
    assert!(matches!(
        SignatureWorkReport::try_new(1, 1, 2, 0, 0, &limits(2, 4, 3)),
        Err(CallableQueryLimitError::Work { .. })
    ));
}

fn semantic_signature(source: Option<CallableSource>) -> SemanticSignature {
    let coordinate = CallableParameterCoordinate::new(group(0), index(0));
    let parameter = SemanticParameter::try_new(
        coordinate,
        "value: String",
        Some(name("value")),
        CallableParameterType::Exact(TypeKind::String),
        CallableParameterPassing::PositionalOrNamed,
        CallableParameterPresence::Required,
        None,
        None,
    )
    .expect("semantic parameter");
    let semantic_group = SemanticParameterGroup::try_new(
        group(0),
        CallableGroupKind::Initial,
        vec![parameter],
        &limits(2, 4, 20),
    )
    .expect("semantic group");
    SemanticSignature::try_new(
        super::CallableCandidateId::Builtin(BuiltinCallableId::Panic),
        Vec::new(),
        SignatureOrigin::Language {
            family: LanguageCallableFamily::Builtin,
        },
        Arc::from("panic(value: String) -> Never"),
        vec![semantic_group],
        TypeKind::Never,
        EffectRow::default(),
        CallableDocumentation::missing(),
        source,
        group(0),
        CallPoison::Clean,
        &limits(2, 4, 20),
    )
    .expect("semantic signature")
}

#[test]
fn semantic_signature_help_enforces_active_indices_and_source_identity() {
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new("signature-test").expect("document id"),
        SourceName::Memory,
        "panic(value)",
    )
    .expect("document");
    let call_span = document
        .span(SourceRange::new(0, document.text().len()))
        .expect("call span");
    let report =
        SignatureWorkReport::try_new(1, 1, 1, 0, 0, &limits(2, 4, 20)).expect("work report");
    let zero = SemanticSignatureIndex::try_from_usize(0).expect("signature index");

    assert_eq!(
        SemanticSignatureHelp::try_new(
            document.identity().clone(),
            call_span.clone(),
            Vec::new(),
            zero,
            None,
            Vec::new(),
            report,
            &limits(2, 4, 20),
        ),
        Err(SemanticSignatureError::EmptySignatures)
    );
    assert_eq!(
        SemanticSignatureHelp::try_new(
            document.identity().clone(),
            call_span.clone(),
            vec![semantic_signature(None)],
            SemanticSignatureIndex::try_from_usize(1).expect("representable index"),
            None,
            Vec::new(),
            report,
            &limits(2, 4, 20),
        ),
        Err(SemanticSignatureError::ActiveSignatureOutOfBounds)
    );
    assert_eq!(
        SemanticSignatureHelp::try_new(
            document.identity().clone(),
            call_span.clone(),
            vec![semantic_signature(None)],
            zero,
            Some(CallableParameterCoordinate::new(group(0), index(1))),
            Vec::new(),
            report,
            &limits(2, 4, 20),
        ),
        Err(SemanticSignatureError::ActiveParameterOutOfBounds)
    );

    let other = SourceDocument::try_new(
        SourceDocumentId::try_new("other-signature-test").expect("document id"),
        SourceName::Memory,
        "panic(value)",
    )
    .expect("other document");
    let other_signature = other
        .span(SourceRange::new(0, other.text().len()))
        .expect("other signature span");
    let source = CallableSource::try_new(None, Some(other_signature), None, None, Vec::new())
        .expect("callable source");
    assert_eq!(
        SemanticSignatureHelp::try_new(
            document.identity().clone(),
            call_span.clone(),
            vec![semantic_signature(Some(source))],
            zero,
            None,
            Vec::new(),
            report,
            &limits(2, 4, 20),
        ),
        Err(SemanticSignatureError::SourceIdentityMismatch)
    );

    let help = SemanticSignatureHelp::try_new(
        document.identity().clone(),
        call_span,
        vec![semantic_signature(None)],
        zero,
        Some(CallableParameterCoordinate::new(group(0), index(0))),
        Vec::new(),
        report,
        &limits(2, 4, 20),
    )
    .expect("valid semantic signature help");
    assert_eq!(help.active_signature(), zero);
    assert_eq!(
        help.active_parameter(),
        Some(CallableParameterCoordinate::new(group(0), index(0)))
    );
}
