use arcweft_lang_syntax::{
    ast::{common::TextRange, module_path::ModuleSegment},
    types::{TypeRefNodeStep, parse_type_ref},
};

use crate::{
    env::TypeCheckEnv,
    types::{
        ArrayLength, DetachedTypeOwnerId, GenericTypeOwnerId, GenericTypeParameterId, TypeKind,
    },
};

use super::{
    GenericTypeBinding, GenericTypeScope, NominalResolutionLimits, NominalTypeDiagnosticKind,
    ResolvedTypeRefOutcome, SelfTypeScope, StructuralTypeNodeKind, TypeNameResolution,
    TypeResolutionFailure, TypeResolutionInput, TypeSourceEvidence, resolve_type_ref,
};

fn resolve_detached(
    source: &str,
    environment: &TypeCheckEnv,
    generics: &GenericTypeScope,
    self_scope: SelfTypeScope,
    limits: NominalResolutionLimits,
) -> super::TypeResolutionReport {
    let authored = parse_type_ref(source).expect("test type parses");
    resolve_type_ref(&TypeResolutionInput::detached(
        &authored,
        None,
        environment,
        generics,
        self_scope,
        limits,
    ))
    .expect("detached input is internally valid")
}

fn nested_option_type(wrappers: usize) -> String {
    let mut source = "i32".to_owned();
    for _ in 0..wrappers {
        source = format!("Option<{source}>");
    }
    source
}

#[test]
fn production_recursive_depth_limit_is_inclusive_and_poisoned_at_one_over() {
    let environment = TypeCheckEnv::new();
    let generics = GenericTypeScope::empty();
    let exact = resolve_detached(
        &nested_option_type(255),
        &environment,
        &generics,
        SelfTypeScope::Absent,
        NominalResolutionLimits::PRODUCTION,
    );
    assert!(exact.diagnostics().is_empty());

    let one_over = resolve_detached(
        &nested_option_type(256),
        &environment,
        &generics,
        SelfTypeScope::Absent,
        NominalResolutionLimits::PRODUCTION,
    );
    assert!(matches!(
        one_over.diagnostics(),
        [diagnostic]
            if matches!(
                diagnostic.kind(),
                NominalTypeDiagnosticKind::Limit {
                    kind: super::NominalResolutionLimitKind::RecursiveTypeDepth,
                    observed: 257,
                    maximum: 256,
                }
            )
    ));
}

#[test]
fn recursive_builtin_resolution_preserves_detached_failure_and_structure() {
    let report = resolve_detached(
        "Result<i32, (Missing, bool)>",
        &TypeCheckEnv::new(),
        &GenericTypeScope::empty(),
        SelfTypeScope::Absent,
        NominalResolutionLimits::PRODUCTION,
    );
    let ResolvedTypeRefOutcome::Detached(detached) = report.outcome() else {
        panic!("one unavailable project name makes the report detached")
    };
    let TypeKind::Result { ok, error } = detached.product().recovered() else {
        panic!("outer builtin shape must survive a nested unavailable name")
    };
    assert_eq!(ok.as_ref(), &TypeKind::I32);
    let TypeKind::Tuple(items) = error.as_ref() else {
        panic!("nested tuple shape must survive")
    };
    assert!(matches!(
        items.as_slice(),
        [TypeKind::Error(_), TypeKind::Bool]
    ));
    assert!(report.diagnostics().is_empty());
    assert_eq!(detached.unavailable().len(), 1);
    assert_eq!(
        detached.product().nodes().len(),
        5,
        "every structural node has one fact"
    );
    assert!(detached.product().nodes().iter().any(|node| {
        matches!(
            node.outcome(),
            TypeNameResolution::Structural(StructuralTypeNodeKind::Tuple)
        )
    }));
}

#[test]
fn associated_binding_node_retains_its_recovered_semantic_type() {
    let report = resolve_detached(
        "Iterator<Item = Vec<i32>>",
        &TypeCheckEnv::new(),
        &GenericTypeScope::empty(),
        SelfTypeScope::Absent,
        NominalResolutionLimits::PRODUCTION,
    );
    let product = report.outcome().product();
    let binding = product
        .nodes()
        .iter()
        .find(|node| node.node().steps() == [TypeRefNodeStep::AssociatedBinding(0)])
        .expect("associated binding has one indexed node fact");
    assert_eq!(
        binding.recovered(),
        Some(&TypeKind::Vec(Box::new(TypeKind::I32)))
    );
}

#[test]
fn generic_precedence_uses_typed_owner_identity() {
    let parameter = GenericTypeParameterId::new(
        GenericTypeOwnerId::Detached(DetachedTypeOwnerId::new(41)),
        0,
    );
    let generics = GenericTypeScope::try_new([GenericTypeBinding::new(
        parameter.clone(),
        ModuleSegment::new("Duration").expect("generic name"),
        TypeSourceEvidence::detached(TextRange::new(0, 8)),
    )])
    .expect("generic scope");
    let report = resolve_detached(
        "Duration",
        &TypeCheckEnv::new(),
        &generics,
        SelfTypeScope::Absent,
        NominalResolutionLimits::PRODUCTION,
    );
    let ResolvedTypeRefOutcome::Complete(product) = report.outcome() else {
        panic!("generic shadowing is complete")
    };
    assert_eq!(
        product.recovered(),
        &TypeKind::GenericParam(parameter.clone())
    );
    assert!(matches!(
        product.nodes()[0].outcome(),
        TypeNameResolution::Generic(actual) if actual == &parameter
    ));
}

#[test]
fn array_wrong_length_kind_is_contextual_and_retains_outer_shape() {
    let report = resolve_detached(
        "Array<u8, String>",
        &TypeCheckEnv::new(),
        &GenericTypeScope::empty(),
        SelfTypeScope::Absent,
        NominalResolutionLimits::PRODUCTION,
    );
    let ResolvedTypeRefOutcome::Poisoned(poisoned) = report.outcome() else {
        panic!("wrong array length kind is authoritative poison")
    };
    let TypeKind::Array { item, len } = poisoned.product().recovered() else {
        panic!("array recovery keeps its typed outer shape")
    };
    assert_eq!(item.as_ref(), &TypeKind::U8);
    assert!(matches!(len, ArrayLength::Error(_)));
    assert!(matches!(
        report.diagnostics(),
        [diagnostic]
            if matches!(
                diagnostic.kind(),
                NominalTypeDiagnosticKind::WrongArgumentKind { argument: 1, .. }
            )
    ));
    let length_node = poisoned
        .product()
        .nodes()
        .iter()
        .find(|node| node.node().steps() == [TypeRefNodeStep::GenericArgument(1)])
        .expect("array length node fact");
    assert!(matches!(
        length_node.outcome(),
        TypeNameResolution::Failed(TypeResolutionFailure::WrongArgumentKind { argument: 1, .. })
    ));
}

#[test]
fn entity_family_arguments_are_contextual_typed_facts() {
    let report = resolve_detached(
        "SpeakerPreset<Character>",
        &TypeCheckEnv::new(),
        &GenericTypeScope::empty(),
        SelfTypeScope::Absent,
        NominalResolutionLimits::PRODUCTION,
    );
    let ResolvedTypeRefOutcome::Complete(product) = report.outcome() else {
        panic!("a contextual entity family is a complete built-in application")
    };
    assert_eq!(
        product.recovered(),
        &TypeKind::SpeakerPreset(crate::types::EntityKind::Character)
    );
    assert!(matches!(
        product.nodes()[0].outcome(),
        TypeNameResolution::EntityFamily(crate::types::EntityKind::Character)
    ));
    assert!(matches!(
        product.nodes()[1].outcome(),
        TypeNameResolution::Builtin(super::BuiltinTypeConstructor::SpeakerPreset)
    ));

    let bare = resolve_detached(
        "Character",
        &TypeCheckEnv::new(),
        &GenericTypeScope::empty(),
        SelfTypeScope::Absent,
        NominalResolutionLimits::PRODUCTION,
    );
    assert!(matches!(
        bare.outcome(),
        ResolvedTypeRefOutcome::Detached(_)
    ));

    let invalid = resolve_detached(
        "Speaker<String>",
        &TypeCheckEnv::new(),
        &GenericTypeScope::empty(),
        SelfTypeScope::Absent,
        NominalResolutionLimits::PRODUCTION,
    );
    let ResolvedTypeRefOutcome::Poisoned(poisoned) = invalid.outcome() else {
        panic!("a value type is not an entity-family argument")
    };
    assert!(matches!(poisoned.product().recovered(), TypeKind::Error(_)));
    assert!(matches!(
        invalid.diagnostics(),
        [diagnostic]
            if matches!(
                diagnostic.kind(),
                NominalTypeDiagnosticKind::WrongArgumentKind {
                    argument: 0,
                    expected: super::TypeArgumentExpectation::EntityFamily,
                    actual: super::TypeArgumentKind::Type(TypeKind::String),
                    ..
                }
            )
    ));
}

#[test]
fn ref_entity_family_projection_preserves_typed_nodes_ranges_and_work() {
    for (source, family) in [
        ("Ref<Character>", crate::types::EntityKind::Character),
        ("Ref<Flow>", crate::types::EntityKind::Flow),
    ] {
        let report = resolve_detached(
            source,
            &TypeCheckEnv::new(),
            &GenericTypeScope::empty(),
            SelfTypeScope::Absent,
            NominalResolutionLimits::PRODUCTION,
        );
        let ResolvedTypeRefOutcome::Complete(product) = report.outcome() else {
            panic!("{source} is complete in a detached world")
        };
        assert_eq!(product.recovered(), &TypeKind::entity_ref(family.clone()));
        assert_eq!(report.work_charged(), 2);
        assert_eq!(product.nodes().len(), 2);
        let child = &product.nodes()[0];
        let root = &product.nodes()[1];
        assert!(matches!(
            child.outcome(),
            TypeNameResolution::EntityFamily(actual) if actual == &family
        ));
        assert!(matches!(
            root.outcome(),
            TypeNameResolution::Builtin(super::BuiltinTypeConstructor::Ref)
        ));
        assert_eq!(root.source().local(), TextRange::new(0, source.len()));
        assert_eq!(
            root.terminal_source().expect("Ref terminal").local(),
            TextRange::new(0, 3)
        );
        assert_eq!(child.source().local(), TextRange::new(4, source.len() - 1));
    }
}

#[test]
fn ref_projection_rejects_wrong_argument_categories_without_fallback() {
    let cases = [
        (
            "Ref<String>",
            super::TypeArgumentKind::Type(TypeKind::String),
        ),
        ("Ref<3>", super::TypeArgumentKind::ConstInt(3)),
        (
            "Ref<Option<String>>",
            super::TypeArgumentKind::Type(TypeKind::Option(Box::new(TypeKind::String))),
        ),
        (
            "Ref<Speaker<Character>>",
            super::TypeArgumentKind::Type(TypeKind::Speaker(crate::types::EntityKind::Character)),
        ),
    ];
    for (source, expected_actual) in cases {
        let report = resolve_detached(
            source,
            &TypeCheckEnv::new(),
            &GenericTypeScope::empty(),
            SelfTypeScope::Absent,
            NominalResolutionLimits::PRODUCTION,
        );
        let ResolvedTypeRefOutcome::Poisoned(poisoned) = report.outcome() else {
            panic!("{source} must be an authoritative wrong-kind failure")
        };
        assert!(matches!(poisoned.product().recovered(), TypeKind::Error(_)));
        assert!(matches!(
            report.diagnostics(),
            [diagnostic]
                if matches!(
                    diagnostic.kind(),
                    NominalTypeDiagnosticKind::WrongArgumentKind {
                        target: super::TypeArityTarget::Builtin(
                            super::BuiltinTypeConstructor::Ref
                        ),
                        argument: 0,
                        expected: super::TypeArgumentExpectation::EntityFamily,
                        actual,
                    } if actual == &expected_actual
                )
        ));
        let child = poisoned
            .product()
            .nodes()
            .iter()
            .find(|node| node.node().steps() == [TypeRefNodeStep::GenericArgument(0)])
            .expect("Ref argument node");
        assert!(matches!(
            child.outcome(),
            TypeNameResolution::Failed(TypeResolutionFailure::WrongArgumentKind { .. })
        ));
    }
}

#[test]
fn ref_projection_retains_arity_and_detached_unavailable_evidence() {
    let bare = resolve_detached(
        "Ref",
        &TypeCheckEnv::new(),
        &GenericTypeScope::empty(),
        SelfTypeScope::Absent,
        NominalResolutionLimits::PRODUCTION,
    );
    assert!(matches!(
        bare.diagnostics(),
        [diagnostic]
            if matches!(
                diagnostic.kind(),
                NominalTypeDiagnosticKind::WrongArity {
                    target: super::TypeArityTarget::Builtin(
                        super::BuiltinTypeConstructor::Ref
                    ),
                    expected: super::TypeArityExpectation::Exact(1),
                    actual: 0,
                }
            )
    ));
    assert_eq!(
        bare.work_charged(),
        2,
        "the existing accounting charges the visited node and its diagnostic"
    );

    let excess = resolve_detached(
        "Ref<Character, String>",
        &TypeCheckEnv::new(),
        &GenericTypeScope::empty(),
        SelfTypeScope::Absent,
        NominalResolutionLimits::PRODUCTION,
    );
    assert!(matches!(
        excess.diagnostics(),
        [diagnostic]
            if matches!(
                diagnostic.kind(),
                NominalTypeDiagnosticKind::WrongArity { actual: 2, .. }
            )
    ));
    assert_eq!(
        excess.work_charged(),
        4,
        "the existing accounting charges three nodes and the arity diagnostic"
    );

    let unknown = resolve_detached(
        "Ref<Missing>",
        &TypeCheckEnv::new(),
        &GenericTypeScope::empty(),
        SelfTypeScope::Absent,
        NominalResolutionLimits::PRODUCTION,
    );
    let ResolvedTypeRefOutcome::Detached(detached) = unknown.outcome() else {
        panic!("an unavailable detached argument stays detached")
    };
    assert!(unknown.diagnostics().is_empty());
    assert_eq!(detached.unavailable().len(), 1);
    assert!(matches!(
        detached
            .product()
            .nodes()
            .last()
            .map(super::ResolvedTypeNode::outcome),
        Some(TypeNameResolution::Builtin(
            super::BuiltinTypeConstructor::Ref
        ))
    ));
}

#[test]
fn self_absence_uses_exact_head_and_diagnostic_cap_is_deterministic() {
    let production = NominalResolutionLimits::PRODUCTION;
    let limits = NominalResolutionLimits::try_new(
        production.type_nodes_per_reference(),
        production.recursive_type_depth(),
        production.generic_arguments_per_application(),
        production.alias_expansion_depth(),
        production.alias_expansion_nodes(),
        2,
        production.related_labels_per_diagnostic(),
        production.work_per_reference(),
    )
    .expect("smaller diagnostic cap is valid");
    let report = resolve_detached(
        "(Self, Self, Self)",
        &TypeCheckEnv::new(),
        &GenericTypeScope::empty(),
        SelfTypeScope::Absent,
        limits,
    );
    assert_eq!(report.diagnostics().len(), 2);
    assert_eq!(report.omitted_diagnostics(), 1);
    assert_eq!(
        report.diagnostics()[0].primary().local(),
        TextRange::new(1, 5)
    );
    assert_eq!(
        report.diagnostics()[1].primary().local(),
        TextRange::new(7, 11)
    );
    assert!(report.diagnostics().iter().all(|diagnostic| matches!(
        diagnostic.kind(),
        NominalTypeDiagnosticKind::SelfUnavailable
    )));
}

#[test]
fn production_reference_diagnostic_cap_is_inclusive_and_counts_omissions() {
    let maximum = usize::from(NominalResolutionLimits::PRODUCTION.diagnostics_per_type_reference());
    let exact = format!(
        "({})",
        std::iter::repeat_n("Self", maximum)
            .collect::<Vec<_>>()
            .join(", ")
    );
    let exact = resolve_detached(
        &exact,
        &TypeCheckEnv::new(),
        &GenericTypeScope::empty(),
        SelfTypeScope::Absent,
        NominalResolutionLimits::PRODUCTION,
    );
    assert_eq!(exact.diagnostics().len(), maximum);
    assert_eq!(exact.omitted_diagnostics(), 0);

    let one_over = format!(
        "({})",
        std::iter::repeat_n("Self", maximum + 1)
            .collect::<Vec<_>>()
            .join(", ")
    );
    let one_over = resolve_detached(
        &one_over,
        &TypeCheckEnv::new(),
        &GenericTypeScope::empty(),
        SelfTypeScope::Absent,
        NominalResolutionLimits::PRODUCTION,
    );
    assert_eq!(one_over.diagnostics().len(), maximum);
    assert_eq!(one_over.omitted_diagnostics(), 1);
    assert!(one_over.diagnostics().iter().all(|diagnostic| matches!(
        diagnostic.kind(),
        NominalTypeDiagnosticKind::SelfUnavailable
    )));
}

#[test]
fn catalog_work_overflow_poisons_the_smallest_scanned_node() {
    let production = NominalResolutionLimits::PRODUCTION;
    let limits = NominalResolutionLimits::try_new(
        production.type_nodes_per_reference(),
        production.recursive_type_depth(),
        production.generic_arguments_per_application(),
        production.alias_expansion_depth(),
        production.alias_expansion_nodes(),
        1,
        1,
        2,
    )
    .expect("two units cover the minimum diagnostic budget");
    let report = resolve_detached(
        "Option<Duration>",
        &TypeCheckEnv::standard(),
        &GenericTypeScope::empty(),
        SelfTypeScope::Absent,
        limits,
    );
    let ResolvedTypeRefOutcome::Poisoned(poisoned) = report.outcome() else {
        panic!("catalog scanning beyond the budget poisons the reference")
    };
    assert!(matches!(
        poisoned.product().recovered(),
        TypeKind::Option(inner) if matches!(inner.as_ref(), TypeKind::Error(_))
    ));
    assert!(matches!(
        report.diagnostics(),
        [diagnostic]
            if matches!(diagnostic.kind(), NominalTypeDiagnosticKind::WorkOverflow { .. })
    ));
    let duration = poisoned
        .product()
        .nodes()
        .iter()
        .find(|node| node.node().steps() == [TypeRefNodeStep::GenericArgument(0)])
        .expect("duration child fact");
    assert!(matches!(
        duration.outcome(),
        TypeNameResolution::Failed(TypeResolutionFailure::WorkOverflow { .. })
    ));
    assert_eq!(report.work_charged(), 2);
}
