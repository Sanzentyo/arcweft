use crate::ast::common::TextRange;
use crate::reference::{BorrowKind, RegionSyntax};
use crate::types::{
    LifetimeName, TypeRef, TypeRefComponentRole, TypeRefNodePath, TypeRefNodeStep, parse_type_ref,
};

#[test]
fn function_components_follow_the_exact_return_boundary_through_grouping() {
    let grouped = "(A -> B)";
    let authored = parse_type_ref(grouped).expect("grouped function type");
    let root = TypeRefNodePath::root();
    assert_eq!(
        authored
            .source()
            .component_at(&root, TypeRefComponentRole::FunctionArrow),
        Some(&TextRange::new(3, 5))
    );
    assert!(
        authored
            .source()
            .component_at(&root, TypeRefComponentRole::FunctionOpen)
            .is_none()
    );

    let grouped_parameters = "((A, B) -> C)";
    let authored = parse_type_ref(grouped_parameters).expect("grouped parameter function type");
    assert_eq!(
        authored
            .source()
            .component_at(&root, TypeRefComponentRole::FunctionOpen),
        Some(&TextRange::new(1, 2))
    );
    assert_eq!(
        authored
            .source()
            .component_at(&root, TypeRefComponentRole::FunctionClose),
        Some(&TextRange::new(6, 7))
    );
    assert_eq!(
        authored
            .source()
            .component_at(&root, TypeRefComponentRole::FunctionArrow),
        Some(&TextRange::new(8, 10))
    );

    let nested = "(A -> B) -> C";
    let authored = parse_type_ref(nested).expect("nested function type");
    let parameter = root.child(TypeRefNodeStep::FunctionParameter(0));
    assert_eq!(
        authored
            .source()
            .component_at(&root, TypeRefComponentRole::FunctionArrow),
        Some(&TextRange::new(9, 11))
    );
    assert_eq!(
        authored
            .source()
            .component_at(&parameter, TypeRefComponentRole::FunctionArrow),
        Some(&TextRange::new(3, 5))
    );
}

#[test]
fn reference_forms_preserve_kind_region_and_operator_ranges() {
    let fixtures = [
        ("&T", BorrowKind::Shared, None, None),
        (
            "&mut T",
            BorrowKind::Mutable,
            None,
            Some(TextRange::new(1, 4)),
        ),
        ("&'a T", BorrowKind::Shared, Some("a"), None),
        (
            "&'a mut T",
            BorrowKind::Mutable,
            Some("a"),
            Some(TextRange::new(4, 7)),
        ),
    ];
    for (source, kind, lifetime, mut_range) in fixtures {
        let TypeRef::Reference(reference) = parse_type_ref(source)
            .expect("reference parses")
            .into_value()
        else {
            panic!("expected reference type");
        };
        assert_eq!(reference.kind(), kind);
        assert_eq!(reference.amp_range(), TextRange::new(0, 1));
        assert_eq!(reference.mut_range(), mut_range);
        assert_eq!(reference.region().name().map(LifetimeName::name), lifetime);
        assert_eq!(reference.range(), TextRange::new(0, source.len()));
    }
}

#[test]
fn trivia_does_not_change_reference_mutability() {
    for source in ["& mut T", "&/* ownership */mut T", "&\nmut T"] {
        let TypeRef::Reference(reference) = parse_type_ref(source)
            .expect("reference parses")
            .into_value()
        else {
            panic!("expected reference type");
        };
        assert_eq!(reference.kind(), BorrowKind::Mutable);
    }
    let TypeRef::Reference(reference) = parse_type_ref("&mutable")
        .expect("reference parses")
        .into_value()
    else {
        panic!("expected reference type");
    };
    assert_eq!(reference.kind(), BorrowKind::Shared);
}

#[test]
fn nested_reference_ranges_use_original_type_offsets() {
    let TypeRef::Reference(outer) = parse_type_ref("  &&mut T  ")
        .expect("nested reference")
        .into_value()
    else {
        panic!("expected outer reference");
    };
    assert_eq!(outer.amp_range(), TextRange::new(2, 3));
    assert_eq!(outer.range(), TextRange::new(2, 9));

    let TypeRef::Reference(inner) = outer.referent() else {
        panic!("expected inner reference");
    };
    assert_eq!(inner.amp_range(), TextRange::new(3, 4));
    assert_eq!(inner.mut_range(), Some(TextRange::new(4, 7)));
    assert_eq!(inner.range(), TextRange::new(3, 9));
}

#[test]
fn references_inside_composite_types_keep_parent_offsets() {
    let TypeRef::Generic { args, .. } =
        parse_type_ref("Vec<&mut T>").expect("generic").into_value()
    else {
        panic!("expected generic");
    };
    let TypeRef::Reference(generic_reference) = &args[0] else {
        panic!("expected generic reference argument");
    };
    assert_eq!(generic_reference.amp_range(), TextRange::new(4, 5));
    assert_eq!(generic_reference.range(), TextRange::new(4, 10));

    let TypeRef::Tuple(items) = parse_type_ref("(&A, &mut B)").expect("tuple").into_value() else {
        panic!("expected tuple");
    };
    let TypeRef::Reference(first) = &items[0] else {
        panic!("expected first tuple reference");
    };
    let TypeRef::Reference(second) = &items[1] else {
        panic!("expected second tuple reference");
    };
    assert_eq!(first.range(), TextRange::new(1, 3));
    assert_eq!(second.range(), TextRange::new(5, 11));

    let TypeRef::Function {
        params,
        return_type,
        ..
    } = parse_type_ref("&A -> &mut B")
        .expect("function type")
        .into_value()
    else {
        panic!("expected function type");
    };
    let TypeRef::Reference(param) = &params[0] else {
        panic!("expected reference parameter");
    };
    let TypeRef::Reference(result) = return_type.as_ref() else {
        panic!("expected reference return");
    };
    assert_eq!(param.range(), TextRange::new(0, 2));
    assert_eq!(result.range(), TextRange::new(6, 12));
}

#[test]
fn invalid_region_order_and_missing_referent_are_typed() {
    let order = parse_type_ref("&mut 'a T").expect_err("invalid order must fail");
    assert_eq!(order.code(), "syntax.type.region_after_mut");
    assert_eq!(order.range(), Some(TextRange::new(5, 7)));

    for source in ["&", "&mut", "&'a"] {
        let error = parse_type_ref(source).expect_err("missing referent must fail");
        assert_eq!(error.code(), "syntax.type.reference_missing_referent");
        assert_eq!(
            error.range(),
            Some(TextRange::new(source.len(), source.len()))
        );
    }
}

#[test]
fn reference_prefix_binds_tighter_than_type_choice() {
    let TypeRef::Choice(alternatives) = parse_type_ref("&A | B")
        .expect("choice parses")
        .into_value()
    else {
        panic!("expected choice type");
    };
    assert!(matches!(alternatives[0], TypeRef::Reference(_)));
    assert!(matches!(alternatives[1], TypeRef::Path(_)));
}

#[test]
fn nominal_path_exposes_structural_heads_without_display_reconstruction() {
    for source in ["domain.Value", "Vec<I32>", "Iterator<Item = I32>"] {
        let authored = parse_type_ref(source).expect("nominal type parses");
        assert!(authored.value().nominal_path().is_some(), "{source}");
    }
    for source in ["!", "(A, B)", "A -> B", "&A", "[A]"] {
        let authored = parse_type_ref(source).expect("non-nominal type parses");
        assert!(authored.value().nominal_path().is_none(), "{source}");
    }
}

#[test]
fn named_reference_region_retains_exact_token() {
    let TypeRef::Reference(reference) = parse_type_ref("&'scene Value")
        .expect("named region parses")
        .into_value()
    else {
        panic!("expected reference type");
    };
    assert!(matches!(
        reference.region(),
        RegionSyntax::Named { range, .. } if *range == TextRange::new(1, 7)
    ));
}
