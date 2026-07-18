use arcweft_presentation::appearance::{
    ColorScheme, ContrastPreference, EnvironmentRevision, PresentationColor,
    PresentationEnvironment, PresentationEnvironmentFieldRevisions, PresentationEnvironmentValues,
    TextScaleMilli,
};
use arcweft_view::ViewFlexDirection;
use arcweft_view::ViewPartLocalName;
use arcweft_view::ViewPartName;
use arcweft_view::style::{
    ComputedViewStyleBuilder, ComputedViewStyleRevision, ViewAxisProviderParticipation,
    ViewBoxAxisHostSeed, ViewBoxAxisSeedGeneration, ViewColorValue, ViewElementState,
    ViewElementStateSet, ViewEnvironmentClause, ViewEnvironmentCondition,
    ViewEnvironmentWrapperIndex, ViewEnvironmentWrapperSource, ViewInheritedBoxAxes,
    ViewInteractionSelector, ViewInteractionStateSet, ViewPropertyKind, ViewSpecifiedValue,
    ViewStyleApplication, ViewStyleApplicationTarget, ViewStyleAssignOp, ViewStyleBoundaryFacts,
    ViewStyleCombinator, ViewStyleContribution, ViewStyleContributionSource, ViewStyleDeclaration,
    ViewStyleNodeFacts, ViewStyleNodeKey, ViewStylePatch, ViewStylePatchId, ViewStylePredicate,
    ViewStylePriority, ViewStyleProgram, ViewStyleResolveContext, ViewStyleResolveError,
    ViewStyleResolver, ViewStyleResolverLimits, ViewStyleRevisionSet, ViewStyleRule,
    ViewStyleScopeId, ViewStyleSelector, ViewStyleSelectorSequence, ViewStyleSheet,
    ViewStyleSheetId, ViewStyleSourceId, ViewStyleTraceEntry, ViewStyleTraceMode,
    ViewStyleTraceRejection,
};
use arcweft_view::{ViewElementKind, ViewMountId};

fn environment(color_scheme: ColorScheme) -> PresentationEnvironment {
    environment_with_revision(color_scheme, false, EnvironmentRevision::ZERO)
}

fn environment_with_revision(
    color_scheme: ColorScheme,
    reduced_motion: bool,
    revision: EnvironmentRevision,
) -> PresentationEnvironment {
    PresentationEnvironment::try_from_parts(
        PresentationEnvironmentValues::new(
            color_scheme,
            ContrastPreference::Standard,
            reduced_motion,
            TextScaleMilli::ONE,
        ),
        revision,
        PresentationEnvironmentFieldRevisions::ZERO,
    )
    .expect("test environment revision is consistent")
}

fn color(red: u8, green: u8, blue: u8) -> ViewSpecifiedValue {
    ViewSpecifiedValue::Color {
        value: ViewColorValue::Literal {
            color: PresentationColor::rgb(red, green, blue),
        },
    }
}

fn declaration(
    property: ViewPropertyKind,
    value: ViewSpecifiedValue,
    source: u32,
) -> ViewStyleDeclaration {
    ViewStyleDeclaration::new(
        property,
        value,
        ViewStyleAssignOp::Replace,
        ViewStyleSourceId::new(source),
    )
    .unwrap()
}

fn selector(element: ViewElementKind, predicates: Vec<ViewStylePredicate>) -> ViewStyleSelector {
    ViewStyleSelector::new(vec![
        ViewStyleSelectorSequence::new(None, Some(element), None, predicates).unwrap(),
    ])
    .unwrap()
}

fn rule(
    source_order: u32,
    predicates: Vec<ViewStylePredicate>,
    value: ViewSpecifiedValue,
) -> ViewStyleRule {
    ViewStyleRule::new(
        selector(ViewElementKind::Button, predicates),
        None,
        vec![declaration(
            ViewPropertyKind::BackgroundColor,
            value,
            source_order,
        )],
        source_order,
        ViewStyleSourceId::new(source_order),
    )
    .unwrap()
}

fn rule_with_selector(
    source_order: u32,
    selector: ViewStyleSelector,
    value: ViewSpecifiedValue,
) -> ViewStyleRule {
    ViewStyleRule::new(
        selector,
        None,
        vec![declaration(
            ViewPropertyKind::BackgroundColor,
            value,
            source_order,
        )],
        source_order,
        ViewStyleSourceId::new(source_order),
    )
    .unwrap()
}

fn sheet(id: &str, rules: Vec<ViewStyleRule>) -> ViewStyleSheet {
    ViewStyleSheet::new(ViewStyleSheetId::try_new(id).unwrap(), Vec::new(), rules).unwrap()
}

fn guarded_rule(
    source_order: u32,
    predicates: Vec<ViewStylePredicate>,
    environment: ViewEnvironmentCondition,
    value: ViewSpecifiedValue,
) -> ViewStyleRule {
    ViewStyleRule::new(
        selector(ViewElementKind::Button, predicates),
        Some(environment),
        vec![declaration(
            ViewPropertyKind::BackgroundColor,
            value,
            source_order,
        )],
        source_order,
        ViewStyleSourceId::new(source_order),
    )
    .unwrap()
}

fn application(id: &str, depth: u16, order: u32) -> ViewStyleApplication {
    ViewStyleApplication::new(
        ViewStyleApplicationTarget::named(ViewStyleSheetId::try_new(id).unwrap()),
        ViewStyleScopeId::new(u64::from(order)),
        depth,
        order,
        ViewStyleBoundaryFacts::SAME_VIEW,
    )
}

fn context<'a>(
    key: &'a ViewStyleNodeKey,
    node: &'a ViewStyleNodeFacts,
    ancestors: &'a [ViewStyleNodeFacts],
    applications: &'a [ViewStyleApplication],
    parent: Option<&'a arcweft_view::style::ComputedViewStyle>,
    environment: &'a PresentationEnvironment,
    trace: ViewStyleTraceMode,
) -> ViewStyleResolveContext<'a> {
    let inherited_axes = parent.map_or_else(
        || {
            ViewInheritedBoxAxes::for_host_seed(
                key.mount(),
                ViewBoxAxisSeedGeneration::INITIAL,
                ViewBoxAxisHostSeed::Default,
            )
        },
        |parent| parent.axes().inherited_snapshot(),
    );
    ViewStyleResolveContext {
        node_key: key,
        node,
        ancestors,
        applications,
        parent,
        parent_node_key: None,
        inherited_axes,
        axis_provider_participation: ViewAxisProviderParticipation::ProjectionOnly,
        environment,
        revisions: ViewStyleRevisionSet {
            sheets: 1,
            patches: 1,
            tokens: 1,
            applications: 1,
            interactions: 1,
            containers: 1,
        },
        trace,
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "the child projection test seam adds the required retained parent identity"
)]
fn child_context<'a>(
    key: &'a ViewStyleNodeKey,
    node: &'a ViewStyleNodeFacts,
    ancestors: &'a [ViewStyleNodeFacts],
    applications: &'a [ViewStyleApplication],
    parent_key: &'a ViewStyleNodeKey,
    parent: &'a arcweft_view::style::ComputedViewStyle,
    environment: &'a PresentationEnvironment,
    trace: ViewStyleTraceMode,
) -> ViewStyleResolveContext<'a> {
    let mut context = context(key, node, ancestors, applications, None, environment, trace);
    context.parent = Some(parent);
    context.parent_node_key = Some(parent_key);
    context.inherited_axes = parent.axes().inherited_snapshot();
    context
}

fn node_key(mount: u64, path: Vec<u64>, instruction: u32) -> ViewStyleNodeKey {
    ViewStyleNodeKey::new(ViewMountId::from_raw(mount), path, instruction)
}

#[test]
fn simultaneous_states_use_rule_source_order_instead_of_fixed_state_order() {
    let hovered = ViewStylePredicate::Interaction(ViewInteractionSelector::Hovered);
    let pressed = ViewStylePredicate::Interaction(ViewInteractionSelector::Pressed);
    let program = ViewStyleProgram::try_new(
        vec![sheet(
            "style.states",
            vec![
                rule(10, vec![hovered], color(0x11, 0x22, 0x33)),
                rule(20, vec![pressed], color(0xaa, 0xbb, 0xcc)),
            ],
        )],
        Vec::new(),
    )
    .unwrap();
    let node = ViewStyleNodeFacts::new(Some(ViewElementKind::Button)).with_interactions(
        ViewInteractionStateSet::default()
            .with(ViewInteractionSelector::Hovered)
            .with(ViewInteractionSelector::Pressed),
    );
    let key = node_key(1, vec![2], 3);
    let applications = [application("style.states", 1, 0)];
    let environment = environment(ColorScheme::Light);
    let computed = ViewStyleResolver::default()
        .resolve(
            &program,
            &context(
                &key,
                &node,
                &[],
                &applications,
                None,
                &environment,
                ViewStyleTraceMode::Off,
            ),
        )
        .unwrap()
        .into_computed();

    assert_eq!(
        computed.value(ViewPropertyKind::BackgroundColor),
        Some(&color(0xaa, 0xbb, 0xcc))
    );
}

#[test]
fn deeper_scope_then_later_application_choose_the_winner() {
    let program = ViewStyleProgram::try_new(
        vec![
            sheet("style.ancestor", vec![rule(1, Vec::new(), color(1, 2, 3))]),
            sheet(
                "style.local_first",
                vec![rule(1, Vec::new(), color(4, 5, 6))],
            ),
            sheet(
                "style.local_last",
                vec![rule(1, Vec::new(), color(7, 8, 9))],
            ),
        ],
        Vec::new(),
    )
    .unwrap();
    let applications = [
        application("style.ancestor", 1, 100),
        application("style.local_first", 2, 3),
        application("style.local_last", 2, 4),
    ];
    let node = ViewStyleNodeFacts::new(Some(ViewElementKind::Button));
    let key = node_key(1, Vec::new(), 0);
    let environment = environment(ColorScheme::Dark);
    let computed = ViewStyleResolver::default()
        .resolve(
            &program,
            &context(
                &key,
                &node,
                &[],
                &applications,
                None,
                &environment,
                ViewStyleTraceMode::Off,
            ),
        )
        .unwrap()
        .into_computed();

    assert_eq!(
        computed.value(ViewPropertyKind::BackgroundColor),
        Some(&color(7, 8, 9))
    );
}

#[test]
fn inheritance_copies_only_the_canonical_inherited_property_set() {
    let mut parent = ComputedViewStyleBuilder::default();
    for (property, value, order) in [
        (ViewPropertyKind::Color, color(20, 30, 40), 1),
        (ViewPropertyKind::BackgroundColor, color(50, 60, 70), 2),
    ] {
        assert!(parent.apply(ViewStyleContribution::new(
            property,
            value,
            ViewStyleAssignOp::Replace,
            ViewStylePriority::new(1, 1, 0, 0, 0, order),
            ViewStyleContributionSource::Patch {
                patch: ViewStylePatchId::new(1),
                declaration: ViewStyleSourceId::new(order),
            },
        )));
    }
    let parent = parent.finish(ComputedViewStyleRevision::new(10));
    let program = ViewStyleProgram::default();
    let parent_key = node_key(1, Vec::new(), 0);
    let key = node_key(1, Vec::new(), 1);
    let node = ViewStyleNodeFacts::new(Some(ViewElementKind::Panel));
    let environment = environment(ColorScheme::Light);
    let child = ViewStyleResolver::default()
        .resolve(
            &program,
            &child_context(
                &key,
                &node,
                &[],
                &[],
                &parent_key,
                &parent,
                &environment,
                ViewStyleTraceMode::Off,
            ),
        )
        .unwrap()
        .into_computed();

    assert_eq!(
        child.value(ViewPropertyKind::Color),
        Some(&color(20, 30, 40))
    );
    assert_eq!(child.value(ViewPropertyKind::BackgroundColor), None);
}

#[test]
fn environment_and_element_states_match_together_and_cache_key_tracks_environment() {
    let program = ViewStyleProgram::try_new(
        vec![sheet(
            "style.environment",
            vec![guarded_rule(
                1,
                vec![ViewStylePredicate::ElementState(
                    ViewElementState::FocusVisible,
                )],
                ViewEnvironmentCondition::try_new(
                    vec![ViewEnvironmentWrapperSource::new(
                        ViewStyleSourceId::new(30),
                        ViewStyleSourceId::new(30),
                        ViewStyleSourceId::new(30),
                    )],
                    vec![
                        ViewEnvironmentClause::color_scheme(
                            ColorScheme::Dark,
                            ViewEnvironmentWrapperIndex::new(0),
                            ViewStyleSourceId::new(31),
                        ),
                        ViewEnvironmentClause::reduced_motion(
                            true,
                            ViewEnvironmentWrapperIndex::new(0),
                            ViewStyleSourceId::new(32),
                        ),
                    ],
                )
                .unwrap(),
                color(9, 9, 9),
            )],
        )],
        Vec::new(),
    )
    .unwrap();
    let node = ViewStyleNodeFacts::new(Some(ViewElementKind::Button))
        .with_element_states(ViewElementStateSet::default().with(ViewElementState::FocusVisible));
    let key = node_key(8, vec![13], 21);
    let applications = [application("style.environment", 1, 0)];
    let dark =
        environment_with_revision(ColorScheme::Dark, true, EnvironmentRevision::from_value(1));
    let light =
        environment_with_revision(ColorScheme::Light, true, EnvironmentRevision::from_value(2));
    let mut resolver = ViewStyleResolver::default();
    let first = resolver
        .resolve(
            &program,
            &context(
                &key,
                &node,
                &[],
                &applications,
                None,
                &dark,
                ViewStyleTraceMode::Off,
            ),
        )
        .unwrap();
    let cached = resolver
        .resolve(
            &program,
            &context(
                &key,
                &node,
                &[],
                &applications,
                None,
                &dark,
                ViewStyleTraceMode::Off,
            ),
        )
        .unwrap();
    let changed = resolver
        .resolve(
            &program,
            &context(
                &key,
                &node,
                &[],
                &applications,
                None,
                &light,
                ViewStyleTraceMode::Off,
            ),
        )
        .unwrap();

    assert!(!first.cache_hit());
    assert!(cached.cache_hit());
    assert!(!changed.cache_hit());
    assert_eq!(
        first.computed().value(ViewPropertyKind::BackgroundColor),
        Some(&color(9, 9, 9))
    );
    assert_eq!(
        changed.computed().value(ViewPropertyKind::BackgroundColor),
        None
    );
}

#[test]
fn inline_patch_uses_authored_application_layer_and_full_trace_is_deterministic() {
    let patch_id = ViewStylePatchId::new(7);
    let program = ViewStyleProgram::try_new(
        vec![sheet(
            "style.base",
            vec![rule(1, Vec::new(), color(1, 1, 1))],
        )],
        vec![ViewStylePatch::new(
            patch_id,
            vec![declaration(
                ViewPropertyKind::BackgroundColor,
                color(2, 2, 2),
                2,
            )],
        )],
    )
    .unwrap();
    let applications = [
        application("style.base", 1, 0),
        ViewStyleApplication::new(
            ViewStyleApplicationTarget::inline(patch_id),
            ViewStyleScopeId::new(2),
            1,
            1,
            ViewStyleBoundaryFacts::SAME_VIEW,
        ),
    ];
    let node = ViewStyleNodeFacts::new(Some(ViewElementKind::Button));
    let key = node_key(3, vec![4], 5);
    let environment = environment(ColorScheme::Light);
    let resolution = ViewStyleResolver::default()
        .resolve(
            &program,
            &context(
                &key,
                &node,
                &[],
                &applications,
                None,
                &environment,
                ViewStyleTraceMode::Full,
            ),
        )
        .unwrap();

    assert_eq!(
        resolution
            .computed()
            .value(ViewPropertyKind::BackgroundColor),
        Some(&color(2, 2, 2))
    );
    assert_eq!(resolution.trace().entries().len(), 2);
    assert!(matches!(
        &resolution.trace().entries()[1],
        ViewStyleTraceEntry::Contribution {
            property: ViewPropertyKind::BackgroundColor,
            accepted: true,
            ..
        }
    ));
}

#[test]
fn trace_modes_keep_off_empty_reconstruct_winners_and_bypass_full_cache() {
    let program = ViewStyleProgram::try_new(
        vec![sheet(
            "style.trace-modes",
            vec![rule(1, Vec::new(), color(9, 8, 7))],
        )],
        Vec::new(),
    )
    .unwrap();
    let applications = [application("style.trace-modes", 1, 0)];
    let node = ViewStyleNodeFacts::new(Some(ViewElementKind::Button));
    let key = node_key(30, vec![1], 2);
    let environment = environment(ColorScheme::Light);
    let mut resolver = ViewStyleResolver::default();

    let off = resolver
        .resolve(
            &program,
            &context(
                &key,
                &node,
                &[],
                &applications,
                None,
                &environment,
                ViewStyleTraceMode::Off,
            ),
        )
        .unwrap();
    let winners = resolver
        .resolve(
            &program,
            &context(
                &key,
                &node,
                &[],
                &applications,
                None,
                &environment,
                ViewStyleTraceMode::Winners,
            ),
        )
        .unwrap();
    let full = resolver
        .resolve(
            &program,
            &context(
                &key,
                &node,
                &[],
                &applications,
                None,
                &environment,
                ViewStyleTraceMode::Full,
            ),
        )
        .unwrap();

    assert!(!off.cache_hit());
    assert!(off.trace().is_empty());
    assert!(winners.cache_hit());
    assert!(matches!(
        winners.trace().entries(),
        [ViewStyleTraceEntry::Winner {
            property: ViewPropertyKind::BackgroundColor,
            ..
        }]
    ));
    assert!(!full.cache_hit());
    assert!(matches!(
        full.trace().entries(),
        [ViewStyleTraceEntry::Contribution {
            property: ViewPropertyKind::BackgroundColor,
            accepted: true,
            ..
        }]
    ));
    assert_eq!(off.computed(), winners.computed());
    assert_eq!(off.computed(), full.computed());
}

#[test]
fn inline_patch_rejects_a_property_that_does_not_apply_to_the_node_element() {
    let patch_id = ViewStylePatchId::new(8);
    let declaration_source = ViewStyleSourceId::new(80);
    let program = ViewStyleProgram::try_new(
        Vec::new(),
        vec![ViewStylePatch::new(
            patch_id,
            vec![
                ViewStyleDeclaration::new(
                    ViewPropertyKind::FlexDirection,
                    ViewSpecifiedValue::FlexDirection {
                        value: ViewFlexDirection::Column,
                    },
                    ViewStyleAssignOp::Replace,
                    declaration_source,
                )
                .unwrap(),
            ],
        )],
    )
    .unwrap();
    let applications = [ViewStyleApplication::new(
        ViewStyleApplicationTarget::inline(patch_id),
        ViewStyleScopeId::new(1),
        1,
        0,
        ViewStyleBoundaryFacts::SAME_VIEW,
    )];
    let node = ViewStyleNodeFacts::new(Some(ViewElementKind::Button));
    let key = node_key(10, Vec::new(), 1);
    let environment = environment(ColorScheme::Light);
    let resolution = ViewStyleResolver::default()
        .resolve(
            &program,
            &context(
                &key,
                &node,
                &[],
                &applications,
                None,
                &environment,
                ViewStyleTraceMode::Full,
            ),
        )
        .unwrap();

    assert_eq!(
        resolution.computed().value(ViewPropertyKind::FlexDirection),
        None
    );
    assert_eq!(
        resolution.trace().entries(),
        [ViewStyleTraceEntry::PatchRejected {
            patch: patch_id,
            declaration: declaration_source,
            reason: ViewStyleTraceRejection::PropertyNotApplicable,
        }]
    );
}

#[test]
fn cache_distinguishes_no_parent_from_a_revision_zero_parent() {
    let mut parent = ComputedViewStyleBuilder::default();
    assert!(parent.apply(ViewStyleContribution::new(
        ViewPropertyKind::Color,
        color(3, 4, 5),
        ViewStyleAssignOp::Replace,
        ViewStylePriority::new(1, 1, 0, 0, 0, 0),
        ViewStyleContributionSource::Inherited,
    )));
    let parent = parent.finish(ComputedViewStyleRevision::new(0));
    let program = ViewStyleProgram::default();
    let parent_key = node_key(11, Vec::new(), 0);
    let key = node_key(11, Vec::new(), 1);
    let node = ViewStyleNodeFacts::new(Some(ViewElementKind::Panel));
    let environment = environment(ColorScheme::Light);
    let mut resolver = ViewStyleResolver::default();

    let without_parent = resolver
        .resolve(
            &program,
            &context(
                &key,
                &node,
                &[],
                &[],
                None,
                &environment,
                ViewStyleTraceMode::Off,
            ),
        )
        .unwrap();
    let with_parent = resolver
        .resolve(
            &program,
            &child_context(
                &key,
                &node,
                &[],
                &[],
                &parent_key,
                &parent,
                &environment,
                ViewStyleTraceMode::Off,
            ),
        )
        .unwrap();

    assert!(!with_parent.cache_hit());
    assert_eq!(
        without_parent.computed().value(ViewPropertyKind::Color),
        None
    );
    assert_eq!(
        with_parent.computed().value(ViewPropertyKind::Color),
        Some(&color(3, 4, 5))
    );
}

#[test]
fn nested_boundary_exposes_only_the_direct_root_or_an_explicit_exported_part() {
    let exported = ViewPartName::try_new("public.action").unwrap();
    let root_rule = rule(1, Vec::new(), color(1, 2, 3));
    let exported_rule = rule_with_selector(
        2,
        ViewStyleSelector::new(vec![
            ViewStyleSelectorSequence::new(None, None, Some(exported.clone()), Vec::new()).unwrap(),
        ])
        .unwrap(),
        color(4, 5, 6),
    );
    let program = ViewStyleProgram::try_new(
        vec![sheet("style.boundary", vec![root_rule, exported_rule])],
        Vec::new(),
    )
    .unwrap();
    let environment = environment(ColorScheme::Light);

    let resolve = |key: u64, node: &ViewStyleNodeFacts, boundary: ViewStyleBoundaryFacts| {
        let applications = [ViewStyleApplication::new(
            ViewStyleApplicationTarget::named(ViewStyleSheetId::try_new("style.boundary").unwrap()),
            ViewStyleScopeId::new(1),
            1,
            0,
            boundary,
        )];
        ViewStyleResolver::default()
            .resolve(
                &program,
                &context(
                    &node_key(key, Vec::new(), 1),
                    node,
                    &[],
                    &applications,
                    None,
                    &environment,
                    ViewStyleTraceMode::Off,
                ),
            )
            .unwrap()
            .into_computed()
    };

    let root = resolve(
        1,
        &ViewStyleNodeFacts::new(Some(ViewElementKind::Button)),
        ViewStyleBoundaryFacts::nested_view(1, false, true),
    );
    assert_eq!(
        root.value(ViewPropertyKind::BackgroundColor),
        Some(&color(1, 2, 3))
    );

    let private = resolve(
        2,
        &ViewStyleNodeFacts::new(Some(ViewElementKind::Button)).with_parts(
            Some(ViewPartLocalName::try_new("private.action").unwrap()),
            None,
        ),
        ViewStyleBoundaryFacts::nested_view(1, false, false),
    );
    assert_eq!(private.value(ViewPropertyKind::BackgroundColor), None);

    let public = resolve(
        3,
        &ViewStyleNodeFacts::new(Some(ViewElementKind::Button)).with_parts(
            Some(ViewPartLocalName::try_new("private.action").unwrap()),
            Some(exported),
        ),
        ViewStyleBoundaryFacts::nested_view(1, true, false),
    );
    assert_eq!(
        public.value(ViewPropertyKind::BackgroundColor),
        Some(&color(4, 5, 6))
    );

    let transitive_root = resolve(
        4,
        &ViewStyleNodeFacts::new(Some(ViewElementKind::Button)),
        ViewStyleBoundaryFacts::nested_view(2, false, true),
    );
    assert_eq!(
        transitive_root.value(ViewPropertyKind::BackgroundColor),
        None
    );
}

#[test]
fn resolver_rejects_specificity_that_cannot_be_represented_exactly() {
    let predicates = vec![
        ViewStylePredicate::Interaction(ViewInteractionSelector::Hovered);
        usize::from(u16::MAX) + 1
    ];
    let program = ViewStyleProgram::try_new(
        vec![sheet(
            "style.specificity",
            vec![rule(1, predicates, color(1, 1, 1))],
        )],
        Vec::new(),
    )
    .unwrap();
    let limits = ViewStyleResolverLimits {
        max_selector_steps: usize::from(u16::MAX) + 2,
        ..ViewStyleResolverLimits::default()
    };
    let node = ViewStyleNodeFacts::new(Some(ViewElementKind::Button)).with_interactions(
        ViewInteractionStateSet::default().with(ViewInteractionSelector::Hovered),
    );
    let key = node_key(12, Vec::new(), 1);
    let applications = [application("style.specificity", 1, 0)];
    let environment = environment(ColorScheme::Light);
    let error = ViewStyleResolver::new(limits)
        .resolve(
            &program,
            &context(
                &key,
                &node,
                &[],
                &applications,
                None,
                &environment,
                ViewStyleTraceMode::Off,
            ),
        )
        .unwrap_err();

    assert_eq!(
        error,
        ViewStyleResolveError::SelectorSpecificityBudget {
            limit: usize::from(u16::MAX),
        }
    );
}

#[test]
fn exported_part_does_not_expose_private_child_ancestry_to_structural_selectors() {
    let exported = ViewPartName::try_new("public.action").unwrap();
    let structural = ViewStyleSelector::new(vec![
        ViewStyleSelectorSequence::new(
            None,
            Some(ViewElementKind::Panel),
            None,
            vec![ViewStylePredicate::Interaction(
                ViewInteractionSelector::Hovered,
            )],
        )
        .unwrap(),
        ViewStyleSelectorSequence::new(
            Some(ViewStyleCombinator::Descendant),
            None,
            Some(exported.clone()),
            Vec::new(),
        )
        .unwrap(),
    ])
    .unwrap();
    let program = ViewStyleProgram::try_new(
        vec![sheet(
            "style.private_ancestry",
            vec![rule_with_selector(1, structural, color(8, 8, 8))],
        )],
        Vec::new(),
    )
    .unwrap();
    let application = ViewStyleApplication::new(
        ViewStyleApplicationTarget::named(
            ViewStyleSheetId::try_new("style.private_ancestry").unwrap(),
        ),
        ViewStyleScopeId::new(1),
        1,
        0,
        ViewStyleBoundaryFacts::nested_view(1, true, false),
    );
    let private_ancestor = ViewStyleNodeFacts::new(Some(ViewElementKind::Panel))
        .with_interactions(
            ViewInteractionStateSet::default().with(ViewInteractionSelector::Hovered),
        )
        .with_active_scopes(vec![ViewStyleScopeId::new(1)]);
    let node = ViewStyleNodeFacts::new(Some(ViewElementKind::Button)).with_parts(
        Some(ViewPartLocalName::try_new("private.action").unwrap()),
        Some(exported),
    );
    let key = node_key(13, Vec::new(), 1);
    let environment = environment(ColorScheme::Light);
    let computed = ViewStyleResolver::default()
        .resolve(
            &program,
            &context(
                &key,
                &node,
                &[private_ancestor],
                &[application],
                None,
                &environment,
                ViewStyleTraceMode::Off,
            ),
        )
        .unwrap()
        .into_computed();

    assert_eq!(computed.value(ViewPropertyKind::BackgroundColor), None);
}

#[test]
fn specificity_sequence_traversal_consumes_the_global_selector_budget() {
    let long_selector = ViewStyleSelector::new(vec![
        ViewStyleSelectorSequence::new(None, Some(ViewElementKind::Panel), None, Vec::new())
            .unwrap(),
        ViewStyleSelectorSequence::new(
            Some(ViewStyleCombinator::Descendant),
            Some(ViewElementKind::Panel),
            None,
            Vec::new(),
        )
        .unwrap(),
        ViewStyleSelectorSequence::new(
            Some(ViewStyleCombinator::Descendant),
            Some(ViewElementKind::Panel),
            None,
            Vec::new(),
        )
        .unwrap(),
        ViewStyleSelectorSequence::new(
            Some(ViewStyleCombinator::Descendant),
            Some(ViewElementKind::Button),
            None,
            Vec::new(),
        )
        .unwrap(),
    ])
    .unwrap();
    let program = ViewStyleProgram::try_new(
        vec![sheet(
            "style.selector_budget",
            vec![
                rule_with_selector(1, long_selector.clone(), color(1, 1, 1)),
                rule_with_selector(2, long_selector, color(2, 2, 2)),
            ],
        )],
        Vec::new(),
    )
    .unwrap();
    let limits = ViewStyleResolverLimits {
        max_selector_steps: 5,
        ..ViewStyleResolverLimits::default()
    };
    let node = ViewStyleNodeFacts::new(Some(ViewElementKind::Panel));
    let key = node_key(14, Vec::new(), 1);
    let applications = [application("style.selector_budget", 1, 0)];
    let environment = environment(ColorScheme::Light);

    assert_eq!(
        ViewStyleResolver::new(limits)
            .resolve(
                &program,
                &context(
                    &key,
                    &node,
                    &[],
                    &applications,
                    None,
                    &environment,
                    ViewStyleTraceMode::Off,
                ),
            )
            .unwrap_err(),
        ViewStyleResolveError::SelectorBudget { limit: 5 }
    );
}
