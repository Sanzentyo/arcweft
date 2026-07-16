use arcweft_presentation::appearance::{
    ColorScheme, ContrastPreference, EnvironmentRevision, PresentationEnvironment,
    PresentationEnvironmentField, PresentationEnvironmentFieldRevisions,
    PresentationEnvironmentFieldSet, PresentationEnvironmentValues, SystemColor, TextScaleMilli,
};
use arcweft_view::style::{
    ViewAxisProviderParticipation, ViewBoxAxisHostSeed, ViewBoxAxisSeedGeneration, ViewColorValue,
    ViewContainerAxis, ViewContainerComparison, ViewContainerPredicate, ViewEnvironmentClause,
    ViewEnvironmentCondition, ViewEnvironmentConditionError, ViewInheritedBoxAxes, ViewLengthMilli,
    ViewPropertyKind, ViewRatioMilli, ViewSpecifiedValue, ViewStyleApplication,
    ViewStyleApplicationTarget, ViewStyleAssignOp, ViewStyleBoundaryFacts, ViewStyleDeclaration,
    ViewStyleNodeFacts, ViewStyleNodeKey, ViewStyleProgram, ViewStyleResolveContext,
    ViewStyleResolveResult, ViewStyleResolver, ViewStyleRevisionSet, ViewStyleRule,
    ViewStyleScopeId, ViewStyleSelector, ViewStyleSelectorSequence, ViewStyleSheet,
    ViewStyleSheetId, ViewStyleTraceMode, ViewTextScaleComparison,
};
use arcweft_view::{ViewElementKind, ViewMountId, ViewStyleSourceId};
use serde_json::{Value, json};

fn environment(
    color_scheme: ColorScheme,
    contrast: ContrastPreference,
    reduced_motion: bool,
    text_scale: u16,
) -> PresentationEnvironment {
    PresentationEnvironment::initial(PresentationEnvironmentValues::new(
        color_scheme,
        contrast,
        reduced_motion,
        TextScaleMilli::try_new(text_scale).unwrap(),
    ))
}

fn complete_condition() -> ViewEnvironmentCondition {
    ViewEnvironmentCondition::try_new(
        ViewStyleSourceId::new(1),
        vec![
            ViewEnvironmentClause::text_scale(
                ViewTextScaleComparison::GreaterOrEqual,
                TextScaleMilli::try_new(1_250).unwrap(),
                ViewStyleSourceId::new(5),
            ),
            ViewEnvironmentClause::reduced_motion(true, ViewStyleSourceId::new(4)),
            ViewEnvironmentClause::contrast(ContrastPreference::More, ViewStyleSourceId::new(3)),
            ViewEnvironmentClause::color_scheme(ColorScheme::Dark, ViewStyleSourceId::new(2)),
        ],
    )
    .unwrap()
}

#[test]
fn container_comparison_direct_replacement_round_trips_all_six() {
    for comparison in [
        ViewContainerComparison::Equal,
        ViewContainerComparison::NotEqual,
        ViewContainerComparison::Less,
        ViewContainerComparison::LessOrEqual,
        ViewContainerComparison::Greater,
        ViewContainerComparison::GreaterOrEqual,
    ] {
        let predicate = ViewContainerPredicate::new(
            ViewContainerAxis::InlineSize,
            comparison,
            ViewLengthMilli::new(900),
        );
        let encoded = serde_json::to_value(predicate).unwrap();
        assert_eq!(
            serde_json::from_value::<ViewContainerPredicate>(encoded).unwrap(),
            predicate
        );
        assert_eq!(predicate.comparison(), comparison);
    }
}

#[test]
fn environment_guard_adds_zero_specificity() {
    use arcweft_view::{
        ViewElementKind, ViewStylePredicate, ViewStyleSelector, ViewStyleSelectorSequence,
    };

    let selector = ViewStyleSelector::new(vec![
        ViewStyleSelectorSequence::new(
            None,
            Some(ViewElementKind::Button),
            None,
            Vec::<ViewStylePredicate>::new(),
        )
        .unwrap(),
    ])
    .unwrap();
    let specificity = selector.specificity().unwrap();
    assert_eq!(specificity.predicates(), 0);
    assert_eq!(specificity.elements(), 1);
    assert_eq!(complete_condition().clauses().len(), 4);
}

#[test]
fn condition_try_new_sorts_canonical_field_order() {
    let condition = complete_condition();
    assert_eq!(
        condition
            .clauses()
            .iter()
            .map(|clause| clause.field())
            .collect::<Vec<_>>(),
        vec![
            PresentationEnvironmentField::ColorScheme,
            PresentationEnvironmentField::Contrast,
            PresentationEnvironmentField::ReducedMotion,
            PresentationEnvironmentField::TextScale,
        ]
    );
}

#[test]
fn condition_try_new_rejects_empty_duplicate_and_over_four() {
    assert_eq!(
        ViewEnvironmentCondition::try_new(ViewStyleSourceId::new(1), Vec::new()),
        Err(ViewEnvironmentConditionError::Empty)
    );
    assert_eq!(
        ViewEnvironmentCondition::try_new(
            ViewStyleSourceId::new(1),
            vec![
                ViewEnvironmentClause::reduced_motion(true, ViewStyleSourceId::new(2)),
                ViewEnvironmentClause::reduced_motion(false, ViewStyleSourceId::new(3)),
            ],
        ),
        Err(ViewEnvironmentConditionError::DuplicateField {
            field: PresentationEnvironmentField::ReducedMotion,
        })
    );
    let clauses = (0..5)
        .map(|source| ViewEnvironmentClause::reduced_motion(true, ViewStyleSourceId::new(source)))
        .collect();
    assert_eq!(
        ViewEnvironmentCondition::try_new(ViewStyleSourceId::new(1), clauses),
        Err(ViewEnvironmentConditionError::TooMany { actual: 5, max: 4 })
    );
}

#[test]
fn condition_direct_serde_rejects_empty() {
    assert!(
        serde_json::from_value::<ViewEnvironmentCondition>(json!({
            "source": 1,
            "clauses": []
        }))
        .is_err()
    );
}

#[test]
fn condition_direct_serde_rejects_duplicate() {
    assert!(
        serde_json::from_value::<ViewEnvironmentCondition>(json!({
            "source": 1,
            "clauses": [
                {"field":"reduced_motion","comparison":"equal","value":true,"source":2},
                {"field":"reduced_motion","comparison":"equal","value":false,"source":3}
            ]
        }))
        .is_err()
    );
}

#[test]
fn condition_direct_serde_rejects_over_limit() {
    let clauses = (0..5)
        .map(|source| {
            json!({
                "field":"reduced_motion",
                "comparison":"equal",
                "value":true,
                "source":source
            })
        })
        .collect::<Vec<_>>();
    assert!(
        serde_json::from_value::<ViewEnvironmentCondition>(json!({
            "source": 1,
            "clauses": clauses
        }))
        .is_err()
    );
}

#[test]
fn condition_direct_serde_rejects_noncanonical_order() {
    let mut encoded = serde_json::to_value(complete_condition()).unwrap();
    encoded["clauses"].as_array_mut().unwrap().reverse();
    assert!(serde_json::from_value::<ViewEnvironmentCondition>(encoded).is_err());
}

#[test]
fn condition_direct_serde_rejects_unknown_null_nested_and_wrong_kind() {
    let valid = serde_json::to_value(complete_condition()).unwrap();
    let mut cases = Vec::new();

    let mut unknown = valid.clone();
    unknown
        .as_object_mut()
        .unwrap()
        .insert("unknown".to_owned(), json!(true));
    cases.push(unknown);

    let mut null_value = valid.clone();
    null_value["clauses"][0]["value"] = Value::Null;
    cases.push(null_value);

    let mut nested = valid.clone();
    nested["clauses"][0]["value"] = json!({"nested":"dark"});
    cases.push(nested);

    let mut wrong_kind = valid;
    wrong_kind["clauses"][0]["source"] = json!("2");
    cases.push(wrong_kind);

    for invalid in cases {
        assert!(serde_json::from_value::<ViewEnvironmentCondition>(invalid).is_err());
    }
}

#[test]
fn condition_direct_serde_rejects_out_of_range_text_scale() {
    for invalid in [499, 4_001] {
        assert!(
            serde_json::from_value::<ViewEnvironmentCondition>(json!({
                "source": 1,
                "clauses": [{
                    "field":"text_scale",
                    "comparison":"equal",
                    "value":invalid,
                    "source":2
                }]
            }))
            .is_err()
        );
    }
}

#[test]
fn guard_short_circuit_usage_reads_only_required_fields() {
    let result = complete_condition().matches(environment(
        ColorScheme::Light,
        ContrastPreference::More,
        true,
        1_300,
    ));
    assert!(!result.matched());
    assert_eq!(
        result.usage(),
        PresentationEnvironmentFieldSet::from_field(PresentationEnvironmentField::ColorScheme)
    );
}

#[test]
fn matched_guard_usage_reads_all_fields() {
    let result = complete_condition().matches(environment(
        ColorScheme::Dark,
        ContrastPreference::More,
        true,
        1_300,
    ));
    assert!(result.matched());
    assert_eq!(result.usage(), PresentationEnvironmentFieldSet::ALL);
}

#[test]
fn unrelated_environment_field_change_keeps_cache_entry() {
    let program = guarded_program(
        Some(
            ViewEnvironmentCondition::try_new(
                ViewStyleSourceId::new(0),
                vec![ViewEnvironmentClause::color_scheme(
                    ColorScheme::Dark,
                    ViewStyleSourceId::new(1),
                )],
            )
            .unwrap(),
        ),
        ViewSpecifiedValue::Ratio {
            value: ViewRatioMilli::new(900).unwrap(),
        },
    );
    let initial = environment(
        ColorScheme::Dark,
        ContrastPreference::Standard,
        false,
        1_000,
    );
    let contrast_changed = revised_environment(
        PresentationEnvironmentValues::new(
            ColorScheme::Dark,
            ContrastPreference::More,
            false,
            TextScaleMilli::ONE,
        ),
        PresentationEnvironmentField::Contrast,
    );
    let mut resolver = ViewStyleResolver::default();

    let first = resolve(&mut resolver, &program, &initial);
    let cached = resolve(&mut resolver, &program, &contrast_changed);
    assert!(!first.cache_hit());
    assert!(cached.cache_hit());
    assert!(std::sync::Arc::ptr_eq(
        &first.computed_arc(),
        &cached.computed_arc()
    ));
    assert_eq!(
        cached.environment_usage().selection(),
        PresentationEnvironmentFieldSet::from_field(PresentationEnvironmentField::ColorScheme)
    );
}

#[test]
fn selection_field_change_evicts_cascade_entry() {
    let program = guarded_program(
        Some(
            ViewEnvironmentCondition::try_new(
                ViewStyleSourceId::new(0),
                vec![ViewEnvironmentClause::color_scheme(
                    ColorScheme::Dark,
                    ViewStyleSourceId::new(1),
                )],
            )
            .unwrap(),
        ),
        ViewSpecifiedValue::Ratio {
            value: ViewRatioMilli::new(900).unwrap(),
        },
    );
    let initial = environment(
        ColorScheme::Dark,
        ContrastPreference::Standard,
        false,
        1_000,
    );
    let color_changed = revised_environment(
        PresentationEnvironmentValues::new(
            ColorScheme::Light,
            ContrastPreference::Standard,
            false,
            TextScaleMilli::ONE,
        ),
        PresentationEnvironmentField::ColorScheme,
    );
    let mut resolver = ViewStyleResolver::default();

    let first = resolve(&mut resolver, &program, &initial);
    let changed = resolve(&mut resolver, &program, &color_changed);
    assert!(!first.cache_hit());
    assert!(!changed.cache_hit());
    assert!(!std::sync::Arc::ptr_eq(
        &first.computed_arc(),
        &changed.computed_arc()
    ));
    assert!(changed.computed().is_empty());
}

#[test]
fn projection_only_color_scheme_change_reuses_computed_style() {
    let program = guarded_program(
        None,
        ViewSpecifiedValue::Color {
            value: ViewColorValue::System {
                role: SystemColor::Accent,
            },
        },
    );
    let initial = environment(
        ColorScheme::Dark,
        ContrastPreference::Standard,
        false,
        1_000,
    );
    let color_changed = revised_environment(
        PresentationEnvironmentValues::new(
            ColorScheme::Light,
            ContrastPreference::Standard,
            false,
            TextScaleMilli::ONE,
        ),
        PresentationEnvironmentField::ColorScheme,
    );
    let mut resolver = ViewStyleResolver::default();

    let first = resolve(&mut resolver, &program, &initial);
    let cached = resolve(&mut resolver, &program, &color_changed);
    assert!(cached.cache_hit());
    assert!(std::sync::Arc::ptr_eq(
        &first.computed_arc(),
        &cached.computed_arc()
    ));
    assert_eq!(
        cached.environment_usage().selection(),
        PresentationEnvironmentFieldSet::NONE
    );
    assert_eq!(
        cached.environment_usage().projection(),
        PresentationEnvironmentFieldSet::from_field(PresentationEnvironmentField::ColorScheme)
    );
}

fn revised_environment(
    values: PresentationEnvironmentValues,
    field: PresentationEnvironmentField,
) -> PresentationEnvironment {
    let one = EnvironmentRevision::from_value(1);
    let zero = EnvironmentRevision::ZERO;
    let revisions = PresentationEnvironmentFieldRevisions::new(
        if field == PresentationEnvironmentField::ColorScheme {
            one
        } else {
            zero
        },
        if field == PresentationEnvironmentField::Contrast {
            one
        } else {
            zero
        },
        if field == PresentationEnvironmentField::ReducedMotion {
            one
        } else {
            zero
        },
        if field == PresentationEnvironmentField::TextScale {
            one
        } else {
            zero
        },
    );
    PresentationEnvironment::try_from_parts(values, one, revisions).unwrap()
}

fn guarded_program(
    environment: Option<ViewEnvironmentCondition>,
    value: ViewSpecifiedValue,
) -> ViewStyleProgram {
    let property = if matches!(value, ViewSpecifiedValue::Color { .. }) {
        ViewPropertyKind::BackgroundColor
    } else {
        ViewPropertyKind::Opacity
    };
    let declaration = ViewStyleDeclaration::new(
        property,
        value,
        ViewStyleAssignOp::Replace,
        ViewStyleSourceId::new(3),
    )
    .unwrap();
    let selector = ViewStyleSelector::new(vec![
        ViewStyleSelectorSequence::new(None, Some(ViewElementKind::Button), None, Vec::new())
            .unwrap(),
    ])
    .unwrap();
    let rule = ViewStyleRule::new(
        selector,
        environment,
        vec![declaration],
        0,
        ViewStyleSourceId::new(2),
    )
    .unwrap();
    let sheet = ViewStyleSheet::new(
        ViewStyleSheetId::try_new("style.environment.cache").unwrap(),
        Vec::new(),
        vec![rule],
    )
    .unwrap();
    ViewStyleProgram::try_new(vec![sheet], Vec::new()).unwrap()
}

fn resolve(
    resolver: &mut ViewStyleResolver,
    program: &ViewStyleProgram,
    environment: &PresentationEnvironment,
) -> ViewStyleResolveResult {
    let sheet = ViewStyleSheetId::try_new("style.environment.cache").unwrap();
    let application = ViewStyleApplication::new(
        ViewStyleApplicationTarget::named(sheet),
        ViewStyleScopeId::new(1),
        0,
        0,
        ViewStyleBoundaryFacts::SAME_VIEW,
    );
    let key = ViewStyleNodeKey::new(ViewMountId::from_raw(1), vec![1], 0);
    let facts = ViewStyleNodeFacts::new(Some(ViewElementKind::Button));
    resolver
        .resolve(
            program,
            &ViewStyleResolveContext {
                node_key: &key,
                node: &facts,
                ancestors: &[],
                applications: &[application],
                parent: None,
                parent_node_key: None,
                inherited_axes: ViewInheritedBoxAxes::for_host_seed(
                    key.mount(),
                    ViewBoxAxisSeedGeneration::INITIAL,
                    ViewBoxAxisHostSeed::Default,
                ),
                axis_provider_participation: ViewAxisProviderParticipation::ProjectionOnly,
                environment,
                revisions: ViewStyleRevisionSet::default(),
                trace: ViewStyleTraceMode::Off,
            },
        )
        .unwrap()
}
