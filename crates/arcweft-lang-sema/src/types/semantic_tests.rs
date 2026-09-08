//! Focused semantic-tag tests for standard checked fields.

use super::{
    CharacterField, CompileTimeCallableType, CompileTimeFxType, CompileTimeScalarKind,
    CompileTimeScalarType, GenericParameterOwnerId, GenericTypeParameterId,
    LanguageIntrinsicGenericOwner, ProgressField, StyleCallableId, TypeKind, ViewCallableId,
};
use crate::env::nominal::{AcceptedNominalId, AcceptedNominalOwnerId};
use arcweft_core::pattern::RuntimeSemanticTypeIdentityEncoder;
use arcweft_core::{
    pattern::RuntimeCheckedType,
    value::{RuntimeSignedIntWidth, RuntimeUnsignedIntWidth},
};
use arcweft_lang_syntax::{
    ast::{
        module_path::ModulePathRoot,
        symbol_path::{ProjectSymbolPath, ProjectSymbolSegment},
    },
    types::TypePath,
};
use std::collections::BTreeSet;

#[test]
fn standard_field_semantic_tags_are_unique() {
    let progress = ProgressField::ALL
        .iter()
        .copied()
        .map(ProgressField::semantic_tag)
        .collect::<BTreeSet<_>>();
    let character = CharacterField::ALL
        .iter()
        .copied()
        .map(CharacterField::semantic_tag)
        .collect::<BTreeSet<_>>();

    assert_eq!(progress.len(), ProgressField::ALL.len());
    assert_eq!(character.len(), CharacterField::ALL.len());
}

#[test]
fn language_intrinsic_generic_owner_tags_and_type_digests_are_unique() {
    let tags = LanguageIntrinsicGenericOwner::ALL
        .iter()
        .copied()
        .map(LanguageIntrinsicGenericOwner::semantic_tag)
        .collect::<BTreeSet<_>>();
    let digests = LanguageIntrinsicGenericOwner::ALL
        .iter()
        .copied()
        .map(|owner| {
            TypeKind::generic_parameter(GenericTypeParameterId::new(
                GenericParameterOwnerId::LanguageIntrinsic(owner),
                0,
            ))
            .semantic_identity_digest()
            .expect("stable fixture type")
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(tags.len(), LanguageIntrinsicGenericOwner::ALL.len());
    assert_eq!(digests.len(), LanguageIntrinsicGenericOwner::ALL.len());
}

#[test]
fn runtime_primitive_digests_use_the_core_checked_type_authority() {
    let pairs = [
        (TypeKind::Never, RuntimeCheckedType::Never),
        (TypeKind::Unit, RuntimeCheckedType::Unit),
        (TypeKind::Bool, RuntimeCheckedType::Bool),
        (
            TypeKind::I8,
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I8),
        ),
        (
            TypeKind::I16,
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I16),
        ),
        (
            TypeKind::I32,
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I32),
        ),
        (
            TypeKind::I64,
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I64),
        ),
        (
            TypeKind::I128,
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I128),
        ),
        (
            TypeKind::ISize,
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::ISize),
        ),
        (
            TypeKind::U8,
            RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U8),
        ),
        (
            TypeKind::U16,
            RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U16),
        ),
        (
            TypeKind::U32,
            RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U32),
        ),
        (
            TypeKind::U64,
            RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U64),
        ),
        (
            TypeKind::U128,
            RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U128),
        ),
        (
            TypeKind::USize,
            RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::USize),
        ),
        (TypeKind::F32, RuntimeCheckedType::F32),
        (TypeKind::F64, RuntimeCheckedType::F64),
        (TypeKind::String, RuntimeCheckedType::String),
        (TypeKind::Char, RuntimeCheckedType::Char),
        (TypeKind::Bytes, RuntimeCheckedType::Bytes),
        (TypeKind::Duration, RuntimeCheckedType::Duration),
        (TypeKind::Progress, RuntimeCheckedType::Progress),
    ];

    let mut digests = BTreeSet::new();
    for (source, checked) in &pairs {
        let source = source
            .semantic_identity_digest()
            .expect("stable fixture type");
        assert_eq!(
            source.as_bytes(),
            checked.semantic_identity_digest().as_bytes()
        );
        assert!(
            digests.insert(source),
            "duplicate primitive identity for {checked:?}"
        );
    }
}

#[test]
fn compile_time_callable_and_meta_type_identities_are_typed_and_distinct() {
    let view = TypeKind::CompileTimeCallable(CompileTimeCallableType::View(ViewCallableId::Text));
    let style =
        TypeKind::CompileTimeCallable(CompileTimeCallableType::Style(StyleCallableId::Rgba));
    let meta = TypeKind::MetaType(Box::new(TypeKind::I32));
    let meta_changed = TypeKind::MetaType(Box::new(TypeKind::String));

    let digests = [
        view.semantic_identity_digest()
            .expect("stable fixture type"),
        style
            .semantic_identity_digest()
            .expect("stable fixture type"),
        meta.semantic_identity_digest()
            .expect("stable fixture type"),
        meta_changed
            .semantic_identity_digest()
            .expect("stable fixture type"),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    assert_eq!(digests.len(), 4);

    assert_eq!(view.source_label(), "CompileTimeCallable<View::Text>");
    assert_eq!(style.source_label(), "CompileTimeCallable<Style::rgba>");
    assert_eq!(meta.source_label(), "MetaType<i32>");
    assert!(view.accepts(&view));
    assert!(!view.accepts(&style));
    assert!(meta.accepts(&meta));
    assert!(!meta.accepts(&meta_changed));
    assert!(view.stable_ordering(&style).is_lt());
}

fn accepted_scalar(name: &str, kind: CompileTimeScalarKind) -> TypeKind {
    let path: TypePath = ProjectSymbolPath::new(
        ModulePathRoot::ImplicitCrate,
        name.split('.')
            .map(|segment| ProjectSymbolSegment::try_new(segment).expect("test segment")),
    )
    .expect("test path")
    .into();
    TypeKind::CompileTimeScalar(CompileTimeScalarType::new(
        AcceptedNominalId::new(AcceptedNominalOwnerId::Standard, path),
        kind,
    ))
}

#[test]
fn compile_time_scalar_digest_uses_tag_91_and_exact_declaration_kind() {
    let scalar = accepted_scalar("Milli", CompileTimeScalarKind::Milli);
    let mut expected = RuntimeSemanticTypeIdentityEncoder::new();
    expected.write_tag(91);
    expected.write_u8(0); // standard accepted owner
    expected.write_u8(0); // implicit-crate type path root
    expected.write_len(1);
    expected.write_str("Milli");
    expected.write_u8(CompileTimeScalarKind::Milli.semantic_tag());

    assert_eq!(
        scalar
            .semantic_identity_digest()
            .expect("stable fixture type")
            .as_bytes(),
        expected.finish().as_bytes()
    );
}

#[test]
fn compile_time_scalar_kinds_and_declarations_are_closed_typed_identities() {
    let scalars = CompileTimeScalarKind::ALL
        .into_iter()
        .map(|kind| accepted_scalar("Scalar", kind))
        .collect::<Vec<_>>();
    let digests = scalars
        .iter()
        .map(|ty| ty.semantic_identity_digest().expect("stable scalar type"))
        .collect::<BTreeSet<_>>();
    assert_eq!(digests.len(), CompileTimeScalarKind::ALL.len());

    for scalar in &scalars {
        assert!(scalar.accepts(scalar));
        assert_eq!(scalar.first_mismatch(scalar), None);
        assert!(!scalar.contains_nominal_poison());
    }
    for pair in scalars.windows(2) {
        assert!(pair[0].stable_ordering(&pair[1]).is_lt());
        assert!(!pair[0].accepts(&pair[1]));
        assert!(pair[0].first_mismatch(&pair[1]).is_some());
    }

    let wrong_declaration = accepted_scalar("Other", CompileTimeScalarKind::Milli);
    let wrong_kind = accepted_scalar("Scalar", CompileTimeScalarKind::Ratio);
    assert!(!scalars[0].accepts(&wrong_declaration));
    assert!(!scalars[0].accepts(&wrong_kind));
    assert_ne!(
        scalars[0]
            .semantic_identity_digest()
            .expect("stable fixture type"),
        wrong_declaration
            .semantic_identity_digest()
            .expect("stable fixture type")
    );
    assert_ne!(
        scalars[0]
            .semantic_identity_digest()
            .expect("stable fixture type"),
        wrong_kind
            .semantic_identity_digest()
            .expect("stable fixture type")
    );
}

#[test]
fn compile_time_scalar_is_a_leaf_for_generic_substitution_and_nominal_visits() {
    let scalar = accepted_scalar("Milli", CompileTimeScalarKind::Milli);
    let parameter = GenericTypeParameterId::new(
        GenericParameterOwnerId::Detached(super::DetachedGenericOwnerId::new(41)),
        0,
    );
    let substitutions = [(parameter, TypeKind::String)].into_iter().collect();

    assert_eq!(scalar.substitute_type_parameters(&substitutions), scalar);
    assert!(
        super::TypeGenericUseCollector::collect(&scalar)
            .expect("scalar generic scan")
            .types()
            .is_empty()
    );

    let mut visited = 0_u8;
    super::visit_project_nominals(super::ScopedTypeView::at_root(&scalar), &mut |_, _| {
        visited = visited.saturating_add(1);
        Ok::<_, core::convert::Infallible>(())
    })
    .expect("scalar nominal visit");
    assert_eq!(visited, 0);
}

#[test]
fn abstract_fx_is_expected_only_and_accepts_exact_producer_types() {
    let abstract_fx = CompileTimeFxType::Abstract;
    let builtin = CompileTimeFxType::Builtin(
        arcweft_presentation::fx::BuiltinFxCallableRowId::WaveGlyphTransform,
    );
    let registered = CompileTimeFxType::Registered(
        arcweft_presentation::fx::FxId::try_new("test", "wave").expect("valid Fx id"),
    );

    assert!(abstract_fx.accepts(&builtin));
    assert!(abstract_fx.accepts(&registered));
    assert!(!builtin.accepts(&abstract_fx));
    assert!(!registered.accepts(&abstract_fx));
    assert_ne!(builtin, registered);
}
