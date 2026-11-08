use super::*;
use crate::final_analysis::CheckedStyleOperand;
use crate::nominal::{
    CheckedTypeReferenceCache, GenericTypeScope, NominalResolutionLimits, ResolvedTypeNodePurpose,
    ResolvedTypeRefOutcome, SelfTypeScope, TypeResolutionInput, TypeResolutionInputError,
    resolve_type_ref,
};
use crate::types::CompileTimeStyleType;
use arcweft_view::style::ViewStyleValueKind;

#[test]
fn unannotated_style_token_facts_keep_original_initializers_without_fabricated_annotations() {
    let fixture = fixture(
        "style A {\n token amount = 0.5\n token alias = token(amount)\n token face = .Normal\n Panel { opacity = token(alias); font-style = token(face) }\n}\nstyle B {\n token face = .Normal\n Panel { blend-mode = token(face) }\n}\n",
        None,
    );
    let report = analyze(&fixture).unwrap();
    let project = fixture.project.analysis_view().unwrap();
    let module = project.module(&CanonicalModulePath::crate_root()).unwrap();
    let styles = module
        .items()
        .filter_map(|(_, item)| match item.kind() {
            HirItemKind::Style(style) => Some(style),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(styles.len(), 2);
    for (style, kinds) in styles.into_iter().zip([
        vec![
            ViewStyleValueKind::Ratio,
            ViewStyleValueKind::Ratio,
            ViewStyleValueKind::FontStyle,
        ],
        vec![ViewStyleValueKind::BlendMode],
    ]) {
        for (token, kind) in style.tokens().iter().zip(kinds) {
            assert!(
                token.value_type().is_none(),
                "original HIR has no annotation"
            );
            let checked = report.expression(token.value()).unwrap();
            assert_eq!(
                checked.value_type(),
                Some(&TypeKind::CompileTimeStyle(CompileTimeStyleType::Value(
                    kind
                )))
            );
            assert!(checked.effects().is_empty());
            assert!(
                matches!(checked.resolution(), CheckedExpressionResolution::StyleValue(value) if value.kind() == kind)
            );
        }
    }
}

#[test]
fn style_composite_operands_keep_exact_roles_without_becoming_runtime_values() {
    let fixture = fixture(
        "style Composite {\n token shadows: ShadowList = [shadow(color = system_color(.CanvasText))]\n token filters: FilterList = [blur(1px), brightness(1), contrast(2), opacity(25%)]\n token transitions: Transition = [transition(\"color\", 1.25s, delay = 1ms)]\n token image: Resource = resource(\"image.background\")\n}\n",
        None,
    );
    let report = analyze(&fixture).unwrap();
    let mut roles = BTreeMap::new();
    for (_, checked) in report.expressions() {
        if let CheckedExpressionResolution::StyleOperand(operand) = checked.resolution() {
            let ty = TypeKind::CompileTimeStyle(operand.style_type());
            assert_eq!(checked.value_type(), Some(&ty));
            assert!(!ty.accepts(&TypeKind::Named(ty.source_label())));
            assert!(checked.effects().is_empty());
            *roles.entry(operand.style_type()).or_insert(0) += 1;
        }
    }
    assert_eq!(
        roles,
        BTreeMap::from([
            (CompileTimeStyleType::Shadow, 1),
            (CompileTimeStyleType::Filter, 4),
            (CompileTimeStyleType::Transition, 1),
            (CompileTimeStyleType::SystemColor, 1),
            (CompileTimeStyleType::Property, 1),
            (CompileTimeStyleType::Duration, 2),
            (CompileTimeStyleType::ResourceId, 1),
        ])
    );
}

#[test]
fn style_numeric_operands_preserve_signed_minimum_magnitudes_without_runtime_types() {
    let fixture = fixture(
        "style Numeric {\n token integer_min: Integer = -2147483648\n token length_min: Length = -2147483.648px\n token angle_min: Angle = -2147483.648deg\n}\n",
        None,
    );
    let report = analyze(&fixture).unwrap();
    let numbers = report
        .expressions()
        .filter_map(|(_, checked)| match checked.resolution() {
            CheckedExpressionResolution::StyleOperand(CheckedStyleOperand::Number(number)) => {
                Some((number, checked))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(numbers.len(), 3);
    for (number, checked) in numbers {
        assert_eq!(number.magnitude(), 1 << 31);
        assert_eq!(
            checked.value_type(),
            Some(&TypeKind::CompileTimeStyle(
                CompileTimeStyleType::NumericLiteral(number.kind())
            ))
        );
        assert!(checked.effects().is_empty());
        assert!(number.specified_value(false).is_err());
        assert!(number.specified_value(true).is_ok());
    }
}

#[test]
fn style_construction_operands_retain_the_original_sheet_and_declaration() {
    let fixture = fixture(
        "style A {\n token font.stack: FontFamilyList = [\"Yu Gothic\", system_font(.Ui)]\n token ink: Color = system_color(.CanvasText)\n Text { font-family = token(font.stack); color = token(ink) }\n}\nstyle B { token font.stack: FontFamilyList = [system_font(.Serif)] }\n",
        None,
    );
    let report = analyze(&fixture).unwrap();
    let project = fixture.project.analysis_view().unwrap();
    let module = project.module(&CanonicalModulePath::crate_root()).unwrap();
    let (owner, style) = module
        .items()
        .find_map(|(owner, item)| match item.kind() {
            HirItemKind::Style(style) if style.tokens().len() == 2 => Some((owner, style)),
            _ => None,
        })
        .unwrap();
    let mut references = 0;
    let mut operands = 0;
    for (expression, _) in module.expressions() {
        let Some(checked) = report.expression(expression) else {
            continue;
        };
        if let CheckedExpressionResolution::StyleOperand(operand) = checked.resolution() {
            operands += 1;
            assert_eq!(
                checked.value_type(),
                Some(&TypeKind::CompileTimeStyle(operand.style_type()))
            );
            assert!(checked.effects().is_empty());
            if let CheckedStyleOperand::Token(reference) = operand {
                references += 1;
                assert_eq!(reference.style(), owner);
                let token = &style.tokens()[reference.ordinal() as usize];
                assert_eq!(reference.initializer(), token.value());
                assert_eq!(
                    reference.id().public_id(),
                    &arcweft_id::PublicId::try_from(token).unwrap()
                );
            }
        }
    }
    assert_eq!(references, 2);
    assert_eq!(operands, 8);
}

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
