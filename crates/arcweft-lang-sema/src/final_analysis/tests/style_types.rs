use super::*;
use crate::nominal::{
    CheckedTypeReferenceCache, GenericTypeScope, NominalResolutionLimits, ResolvedTypeNodePurpose,
    ResolvedTypeRefOutcome, SelfTypeScope, TypeResolutionInput, TypeResolutionInputError,
    resolve_type_ref,
};
use crate::types::CompileTimeStyleType;
use arcweft_view::style::ViewStyleValueKind;

#[test]
fn style_annotation_facts_reach_the_original_token_values_without_runtime_types() {
    let fixture = fixture(
        "style Native {\n token ink: Color = rgba(1, 2, 3, 255)\n token gap: Length = -1.25px\n Text { color = rgba(4, 5, 6, 255); width = 42px }\n}\n",
        None,
    );
    let report = analyze(&fixture).expect("checked specified token values");
    let project = fixture.project.analysis_view().unwrap();
    let module = project.module(&CanonicalModulePath::crate_root()).unwrap();
    let style = module
        .items()
        .find_map(|(_, item)| match item.kind() {
            HirItemKind::Style(style) => Some(style),
            _ => None,
        })
        .unwrap();
    for (token, kind) in style
        .tokens()
        .iter()
        .zip([ViewStyleValueKind::Color, ViewStyleValueKind::Length])
    {
        let root = token.value_type().unwrap();
        assert_eq!(report.ty(root), None, "the annotation is semantic-only");
        let annotation = report.type_resolution(root).unwrap();
        let [node] = annotation.outcome().product().nodes() else {
            panic!("one annotation node");
        };
        assert_eq!(node.purpose(), ResolvedTypeNodePurpose::StyleAnnotation);
        let value = report.expression(token.value()).unwrap();
        assert_eq!(
            value.value_type(),
            Some(&TypeKind::CompileTimeStyle(CompileTimeStyleType::Value(
                kind
            )))
        );
        assert!(
            matches!(value.resolution(), CheckedExpressionResolution::StyleValue(value) if value.kind() == kind)
        );
        assert!(value.effects().is_empty());
    }
}

#[test]
fn style_token_annotations_resolve_every_native_kind_in_the_original_owner() {
    let tokens = ViewStyleValueKind::ALL
        .iter()
        .enumerate()
        .map(|(index, kind)| format!("token t{index}: {} = 0\n", kind.source_name()))
        .collect::<String>();
    let fixture = fixture(&format!("style Native {{\n{tokens}}}\n"), None);
    let project = fixture.project.analysis_view().unwrap();
    let module = project.module(&CanonicalModulePath::crate_root()).unwrap();
    let (owner, style) = module
        .items()
        .find_map(|(owner, item)| match item.kind() {
            HirItemKind::Style(style) => Some((owner, style)),
            _ => None,
        })
        .unwrap();
    assert_eq!(style.tokens().len(), ViewStyleValueKind::ALL.len());
    let generics = GenericTypeScope::empty();
    for (ordinal, (token, kind)) in style
        .tokens()
        .iter()
        .zip(ViewStyleValueKind::ALL)
        .enumerate()
    {
        let root = token.value_type().unwrap();
        let input = TypeResolutionInput::accepted(
            root,
            module,
            fixture.project.view(),
            &fixture.symbols,
            fixture.registered.environment().nominal_world(),
            &generics,
            SelfTypeScope::Absent,
            NominalResolutionLimits::PRODUCTION,
        )
        .unwrap()
        .with_style_token_annotation(owner, u32::try_from(ordinal).unwrap())
        .unwrap();
        let report = resolve_type_ref(&input).unwrap();
        let ResolvedTypeRefOutcome::Complete(product) = report.outcome() else {
            panic!("{kind:?}: {report:?}");
        };
        assert!(report.diagnostics().is_empty());
        assert_eq!(
            product.recovered(),
            &TypeKind::CompileTimeStyle(CompileTimeStyleType::Value(*kind))
        );
        let [node] = product.nodes() else {
            panic!("one exact annotation root");
        };
        assert_eq!(node.node(), root);
        assert_eq!(node.purpose(), ResolvedTypeNodePurpose::StyleAnnotation);
        assert!(!node.purpose().has_runtime_type());
        assert!(node.source().project().is_some());
    }
}

#[test]
fn style_annotation_cache_and_input_preserve_the_declaring_token_scope() {
    let fixture = fixture(
        "style First { token fonts: FontFamilyList = []; token size: Length = 12px }\nstyle Other { token fonts: FontFamilyList = [] }\n",
        None,
    );
    let project = fixture.project.analysis_view().unwrap();
    let module = project.module(&CanonicalModulePath::crate_root()).unwrap();
    let styles = module
        .items()
        .filter_map(|(owner, item)| match item.kind() {
            HirItemKind::Style(style) => Some((owner, style)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(first, first_style), (other, _)] = styles.as_slice() else {
        panic!("two original styles");
    };
    let root = first_style.tokens()[0].value_type().unwrap();
    let generics = GenericTypeScope::empty();
    let input = || {
        TypeResolutionInput::accepted(
            root,
            module,
            fixture.project.view(),
            &fixture.symbols,
            fixture.registered.environment().nominal_world(),
            &generics,
            SelfTypeScope::Absent,
            NominalResolutionLimits::PRODUCTION,
        )
        .unwrap()
    };
    assert!(matches!(
        input().with_style_token_annotation(*other, 0),
        Err(TypeResolutionInputError::InvalidStyleTokenAnnotation { .. })
    ));
    assert!(matches!(
        input().with_style_token_annotation(*first, 1),
        Err(TypeResolutionInputError::InvalidStyleTokenAnnotation { .. })
    ));
    assert!(matches!(
        input().with_style_token_annotation(*first, u32::MAX),
        Err(TypeResolutionInputError::InvalidStyleTokenAnnotation { .. })
    ));
    let mut cache = CheckedTypeReferenceCache::default();
    let specified = input().with_style_token_annotation(*first, 0).unwrap();
    let accepted = cache.resolve(&specified).unwrap();
    assert!(matches!(
        accepted.outcome(),
        ResolvedTypeRefOutcome::Complete(_)
    ));
    let ordinary = cache.resolve(&input()).unwrap();
    assert!(
        matches!(ordinary.outcome(), ResolvedTypeRefOutcome::Poisoned(_)),
        "Style annotation names are not nominal aliases"
    );
    assert!(Arc::ptr_eq(&accepted, &cache.resolve(&specified).unwrap()));
    assert!(Arc::ptr_eq(&ordinary, &cache.resolve(&input()).unwrap()));
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.hits(), 2);
}

#[test]
fn style_annotation_rejects_unknown_or_structural_types_without_a_nominal_fallback() {
    for ty in ["Unknown", "Vec<i32>", "(Color, Length)"] {
        let fixture = fixture(&format!("style Native {{ token value: {ty} = 0 }}\n"), None);
        let project = fixture.project.analysis_view().unwrap();
        let module = project.module(&CanonicalModulePath::crate_root()).unwrap();
        let (owner, style) = module
            .items()
            .find_map(|(owner, item)| match item.kind() {
                HirItemKind::Style(style) => Some((owner, style)),
                _ => None,
            })
            .unwrap();
        let root = style.tokens()[0].value_type().unwrap();
        let generics = GenericTypeScope::empty();
        let input = TypeResolutionInput::accepted(
            root,
            module,
            fixture.project.view(),
            &fixture.symbols,
            fixture.registered.environment().nominal_world(),
            &generics,
            SelfTypeScope::Absent,
            NominalResolutionLimits::PRODUCTION,
        )
        .unwrap()
        .with_style_token_annotation(owner, 0)
        .unwrap();
        let report = resolve_type_ref(&input).unwrap();
        assert!(
            matches!(report.outcome(), ResolvedTypeRefOutcome::Poisoned(_)),
            "{ty}"
        );
        let [diagnostic] = report.diagnostics() else {
            panic!("one annotation diagnostic");
        };
        assert_eq!(
            diagnostic.kind().code().as_str(),
            "sema.style.type_annotation"
        );
        assert!(diagnostic.to_source_diagnostic().is_some());
    }
}
