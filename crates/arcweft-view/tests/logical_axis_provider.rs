use arcweft_presentation::appearance::{
    ColorScheme, ContrastPreference, EnvironmentRevision, PresentationEnvironment,
    PresentationEnvironmentFieldRevisions, PresentationEnvironmentValues, TextScaleMilli,
};
use arcweft_view::style::{
    ComputedViewStyle, ViewAxisProviderParticipation, ViewBoxAxisHostSeed, ViewBoxAxisMode,
    ViewBoxAxisSeedGeneration, ViewContainerAxis, ViewContainerComparison, ViewContainerPredicate,
    ViewEnvironmentClause, ViewEnvironmentCondition, ViewEnvironmentWrapperIndex,
    ViewEnvironmentWrapperSource, ViewInheritedBoxAxes, ViewInteractionSelector,
    ViewInteractionStateSet, ViewLengthMilli, ViewSpecifiedValue, ViewStyleApplication,
    ViewStyleApplicationTarget, ViewStyleAssignOp, ViewStyleBoundaryFacts, ViewStyleDeclaration,
    ViewStyleNodeFacts, ViewStyleNodeKey, ViewStylePatch, ViewStylePatchId, ViewStylePredicate,
    ViewStyleProgram, ViewStyleResolveContext, ViewStyleResolveError, ViewStyleResolveResult,
    ViewStyleResolver, ViewStyleResolverLimits, ViewStyleRevisionSet, ViewStyleRule,
    ViewStyleScopeId, ViewStyleSelector, ViewStyleSelectorSequence, ViewStyleSheet,
    ViewStyleSheetId, ViewStyleSourceId, ViewStyleToken, ViewStyleTokenId, ViewStyleTraceMode,
    ViewStyleValueKind,
};
use arcweft_view::{ViewElementKind, ViewMountId};

fn environment_with_revision(
    color_scheme: ColorScheme,
    revision: EnvironmentRevision,
) -> PresentationEnvironment {
    PresentationEnvironment::try_from_parts(
        PresentationEnvironmentValues::new(
            color_scheme,
            ContrastPreference::Standard,
            false,
            TextScaleMilli::ONE,
        ),
        revision,
        PresentationEnvironmentFieldRevisions::ZERO,
    )
    .expect("test environment revision is consistent")
}

fn node(mount: u64, instruction: u32) -> ViewStyleNodeKey {
    ViewStyleNodeKey::new(ViewMountId::from_raw(mount), Vec::new(), instruction)
}

fn host_seed(
    key: &ViewStyleNodeKey,
    generation: u64,
    seed: ViewBoxAxisHostSeed,
) -> ViewInheritedBoxAxes {
    let mut current = ViewBoxAxisSeedGeneration::INITIAL;
    for _ in 0..generation {
        current = current.checked_next().unwrap();
    }
    ViewInheritedBoxAxes::for_host_seed(key.mount(), current, seed)
}

#[expect(
    clippy::too_many_arguments,
    reason = "the test seam keeps every required retained provider input explicit"
)]
fn resolve(
    resolver: &mut ViewStyleResolver,
    program: &ViewStyleProgram,
    key: &ViewStyleNodeKey,
    parent_key: Option<&ViewStyleNodeKey>,
    parent: Option<&ComputedViewStyle>,
    inherited_axes: ViewInheritedBoxAxes,
    participation: ViewAxisProviderParticipation,
    applications: &[ViewStyleApplication],
    revisions: ViewStyleRevisionSet,
) -> Result<ViewStyleResolveResult, ViewStyleResolveError> {
    let facts = ViewStyleNodeFacts::new(Some(ViewElementKind::Panel));
    resolve_with_facts(
        resolver,
        program,
        key,
        parent_key,
        parent,
        inherited_axes,
        participation,
        applications,
        revisions,
        &facts,
        &PresentationEnvironment::ENGINE_DEFAULT,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "provider interaction tests must keep every retained shape, fact set, revision, and environment explicit"
)]
fn resolve_with_facts(
    resolver: &mut ViewStyleResolver,
    program: &ViewStyleProgram,
    key: &ViewStyleNodeKey,
    parent_key: Option<&ViewStyleNodeKey>,
    parent: Option<&ComputedViewStyle>,
    inherited_axes: ViewInheritedBoxAxes,
    participation: ViewAxisProviderParticipation,
    applications: &[ViewStyleApplication],
    revisions: ViewStyleRevisionSet,
    facts: &ViewStyleNodeFacts,
    environment: &PresentationEnvironment,
) -> Result<ViewStyleResolveResult, ViewStyleResolveError> {
    resolver.resolve(
        program,
        &ViewStyleResolveContext {
            node_key: key,
            node: facts,
            ancestors: &[],
            applications,
            parent,
            parent_node_key: parent_key,
            inherited_axes,
            axis_provider_participation: participation,
            environment,
            revisions,
            trace: ViewStyleTraceMode::Off,
        },
    )
}

fn axis_program(
    entries: &[(u32, ViewBoxAxisMode)],
) -> (ViewStyleProgram, Vec<ViewStyleApplication>) {
    let patches = entries
        .iter()
        .map(|(id, mode)| {
            ViewStylePatch::new(
                ViewStylePatchId::new(*id),
                vec![
                    ViewStyleDeclaration::new(
                        arcweft_view::style::ViewPropertyKind::BoxAxes,
                        ViewSpecifiedValue::BoxAxes { value: *mode },
                        ViewStyleAssignOp::Replace,
                        ViewStyleSourceId::new(*id),
                    )
                    .unwrap(),
                ],
            )
        })
        .collect::<Vec<_>>();
    let applications = entries
        .iter()
        .map(|(id, _)| {
            ViewStyleApplication::new(
                ViewStyleApplicationTarget::inline(ViewStylePatchId::new(*id)),
                ViewStyleScopeId::new(1),
                0,
                *id,
                ViewStyleBoundaryFacts::SAME_VIEW,
            )
        })
        .collect();
    (
        ViewStyleProgram::try_new(Vec::new(), patches).unwrap(),
        applications,
    )
}

fn axis_rule(
    source: u32,
    predicates: Vec<ViewStylePredicate>,
    value: ViewSpecifiedValue,
) -> ViewStyleRule {
    axis_rule_with_environment(source, predicates, None, value)
}

fn axis_rule_with_environment(
    source: u32,
    predicates: Vec<ViewStylePredicate>,
    environment: Option<ViewEnvironmentCondition>,
    value: ViewSpecifiedValue,
) -> ViewStyleRule {
    ViewStyleRule::new(
        ViewStyleSelector::new(vec![
            ViewStyleSelectorSequence::new(None, Some(ViewElementKind::Panel), None, predicates)
                .unwrap(),
        ])
        .unwrap(),
        environment,
        vec![
            ViewStyleDeclaration::new(
                arcweft_view::style::ViewPropertyKind::BoxAxes,
                value,
                ViewStyleAssignOp::Replace,
                ViewStyleSourceId::new(source),
            )
            .unwrap(),
        ],
        source,
        ViewStyleSourceId::new(source),
    )
    .unwrap()
}

fn named_application(sheet: &str, order: u32) -> ViewStyleApplication {
    ViewStyleApplication::new(
        ViewStyleApplicationTarget::named(ViewStyleSheetId::try_new(sheet).unwrap()),
        ViewStyleScopeId::new(1),
        0,
        order,
        ViewStyleBoundaryFacts::SAME_VIEW,
    )
}

#[test]
fn projection_only_never_registers_a_parent_and_typed_seed_shapes_are_enforced() {
    let program = ViewStyleProgram::default();
    let root = node(1, 0);
    let child = node(1, 1);
    let root_seed = host_seed(&root, 0, ViewBoxAxisHostSeed::Default);
    let mut resolver = ViewStyleResolver::default();
    let projected = resolve(
        &mut resolver,
        &program,
        &root,
        None,
        None,
        root_seed,
        ViewAxisProviderParticipation::ProjectionOnly,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    assert!(matches!(
        resolve(
            &mut resolver,
            &program,
            &child,
            Some(&root),
            Some(&projected),
            projected.axes().inherited_snapshot(),
            ViewAxisProviderParticipation::RetainedPrimary,
            &[],
            ViewStyleRevisionSet::default(),
        ),
        Err(ViewStyleResolveError::AxisProviderMissingParent { node, parent })
            if node == child && parent == root
    ));

    assert!(matches!(
        resolve(
            &mut resolver,
            &program,
            &root,
            None,
            None,
            projected.axes().inherited_snapshot(),
            ViewAxisProviderParticipation::RetainedPrimary,
            &[],
            ViewStyleRevisionSet::default(),
        ),
        Err(ViewStyleResolveError::AxisProviderInvalidRootSeed { node, .. }) if node == root
    ));
    assert!(matches!(
        resolve(
            &mut resolver,
            &program,
            &child,
            Some(&root),
            Some(&projected),
            root_seed,
            ViewAxisProviderParticipation::ProjectionOnly,
            &[],
            ViewStyleRevisionSet::default(),
        ),
        Err(ViewStyleResolveError::AxisProviderInvalidChildSeed { node, .. }) if node == child
    ));
    assert!(matches!(
        resolve(
            &mut resolver,
            &program,
            &child,
            None,
            Some(&projected),
            projected.axes().inherited_snapshot(),
            ViewAxisProviderParticipation::ProjectionOnly,
            &[],
            ViewStyleRevisionSet::default(),
        ),
        Err(ViewStyleResolveError::AxisProviderParentShape { node }) if node == child
    ));
    assert!(matches!(
        resolve(
            &mut resolver,
            &program,
            &child,
            Some(&root),
            None,
            projected.axes().inherited_snapshot(),
            ViewAxisProviderParticipation::ProjectionOnly,
            &[],
            ViewStyleRevisionSet::default(),
        ),
        Err(ViewStyleResolveError::AxisProviderParentShape { node }) if node == child
    ));
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the direct cache evidence keeps primary, projection, provider change, and idempotent cleanup in one scenario"
)]
fn ancestor_change_evicts_descendant_projection_entries_and_mount_cleanup_is_idempotent() {
    let program = ViewStyleProgram::default();
    let mut resolver = ViewStyleResolver::default();
    let root = node(7, 0);
    let child = node(7, 1);
    let root_seed = host_seed(&root, 0, ViewBoxAxisHostSeed::Default);
    let root_computed = resolve(
        &mut resolver,
        &program,
        &root,
        None,
        None,
        root_seed,
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    let child_seed = root_computed.axes().inherited_snapshot();
    let child_computed = resolve(
        &mut resolver,
        &program,
        &child,
        Some(&root),
        Some(&root_computed),
        child_seed,
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    let revision_only_root = resolve(
        &mut resolver,
        &program,
        &root,
        None,
        None,
        root_seed,
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet {
            sheets: 1,
            ..ViewStyleRevisionSet::default()
        },
    )
    .unwrap();
    assert!(!revision_only_root.cache_hit());
    assert_eq!(
        revision_only_root.computed().axes().revision(),
        root_computed.axes().revision()
    );
    for revisions in [
        ViewStyleRevisionSet {
            patches: 1,
            ..ViewStyleRevisionSet::default()
        },
        ViewStyleRevisionSet {
            tokens: 1,
            ..ViewStyleRevisionSet::default()
        },
        ViewStyleRevisionSet {
            applications: 1,
            ..ViewStyleRevisionSet::default()
        },
        ViewStyleRevisionSet {
            interactions: 1,
            ..ViewStyleRevisionSet::default()
        },
        ViewStyleRevisionSet {
            containers: 1,
            ..ViewStyleRevisionSet::default()
        },
    ] {
        let revision_only = resolve(
            &mut resolver,
            &program,
            &root,
            None,
            None,
            root_seed,
            ViewAxisProviderParticipation::RetainedPrimary,
            &[],
            revisions,
        )
        .unwrap();
        assert!(!revision_only.cache_hit());
        assert_eq!(
            revision_only.computed().axes().revision(),
            root_computed.axes().revision()
        );
    }
    let facts = ViewStyleNodeFacts::new(Some(ViewElementKind::Panel));
    let unused_environment_change = resolve_with_facts(
        &mut resolver,
        &program,
        &root,
        None,
        None,
        root_seed,
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
        &facts,
        &environment_with_revision(ColorScheme::Light, EnvironmentRevision::from_value(99)),
    )
    .unwrap();
    assert!(unused_environment_change.cache_hit());
    assert_eq!(
        unused_environment_change.computed().axes().revision(),
        root_computed.axes().revision()
    );
    assert!(
        resolve(
            &mut resolver,
            &program,
            &child,
            Some(&root),
            Some(&root_computed),
            child_seed,
            ViewAxisProviderParticipation::ProjectionOnly,
            &[],
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .cache_hit()
    );
    let projection_revisions = ViewStyleRevisionSet {
        interactions: 1,
        ..ViewStyleRevisionSet::default()
    };
    assert!(
        !resolve(
            &mut resolver,
            &program,
            &child,
            Some(&root),
            Some(&root_computed),
            child_seed,
            ViewAxisProviderParticipation::ProjectionOnly,
            &[],
            projection_revisions,
        )
        .unwrap()
        .cache_hit()
    );
    assert!(
        resolve(
            &mut resolver,
            &program,
            &child,
            Some(&root),
            Some(&root_computed),
            child_seed,
            ViewAxisProviderParticipation::ProjectionOnly,
            &[],
            projection_revisions,
        )
        .unwrap()
        .cache_hit()
    );

    let changed_seed = host_seed(
        &root,
        1,
        ViewBoxAxisHostSeed::Explicit(ViewBoxAxisMode::VerticalRl),
    );
    assert!(
        !resolve(
            &mut resolver,
            &program,
            &root,
            None,
            None,
            changed_seed,
            ViewAxisProviderParticipation::RetainedPrimary,
            &[],
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .cache_hit()
    );
    assert!(
        !resolve(
            &mut resolver,
            &program,
            &child,
            Some(&root),
            Some(&root_computed),
            child_seed,
            ViewAxisProviderParticipation::ProjectionOnly,
            &[],
            projection_revisions,
        )
        .unwrap()
        .cache_hit()
    );

    assert_eq!(resolver.invalidate_mount(root.mount()), 2);
    assert_eq!(resolver.invalidate_mount(root.mount()), 0);
    assert!(
        !resolve(
            &mut resolver,
            &program,
            &root,
            None,
            None,
            root_seed,
            ViewAxisProviderParticipation::RetainedPrimary,
            &[],
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .cache_hit()
    );
    assert_eq!(child_computed.axes().mode(), ViewBoxAxisMode::HorizontalLtr);
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the barrier scenario must retain the old cache evidence across ancestor and local-provider changes"
)]
fn barriers_stop_ancestor_walk_but_their_own_provider_change_reaches_descendants() {
    let (program, applications) = axis_program(&[
        (1, ViewBoxAxisMode::HorizontalLtr),
        (2, ViewBoxAxisMode::VerticalLr),
    ]);
    let mut resolver = ViewStyleResolver::default();
    let root = node(11, 0);
    let barrier = node(11, 1);
    let grandchild = node(11, 2);
    let root_seed = host_seed(&root, 0, ViewBoxAxisHostSeed::Default);
    let root_computed = resolve(
        &mut resolver,
        &program,
        &root,
        None,
        None,
        root_seed,
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    let barrier_computed = resolve(
        &mut resolver,
        &program,
        &barrier,
        Some(&root),
        Some(&root_computed),
        root_computed.axes().inherited_snapshot(),
        ViewAxisProviderParticipation::RetainedPrimary,
        &applications[..1],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    let grandchild_seed = barrier_computed.axes().inherited_snapshot();
    let grandchild_computed = resolve(
        &mut resolver,
        &program,
        &grandchild,
        Some(&barrier),
        Some(&barrier_computed),
        grandchild_seed,
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();

    let changed_root = resolve(
        &mut resolver,
        &program,
        &root,
        None,
        None,
        host_seed(
            &root,
            1,
            ViewBoxAxisHostSeed::Explicit(ViewBoxAxisMode::HorizontalRtl),
        ),
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    assert!(
        !resolve(
            &mut resolver,
            &program,
            &barrier,
            Some(&root),
            Some(&root_computed),
            root_computed.axes().inherited_snapshot(),
            ViewAxisProviderParticipation::ProjectionOnly,
            &applications[..1],
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .cache_hit(),
        "the changed ancestor must evict the barrier node itself"
    );
    let stable_barrier = resolve(
        &mut resolver,
        &program,
        &barrier,
        Some(&root),
        Some(&changed_root),
        changed_root.axes().inherited_snapshot(),
        ViewAxisProviderParticipation::RetainedPrimary,
        &applications[..1],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    assert_eq!(
        stable_barrier.axes().revision(),
        barrier_computed.axes().revision()
    );
    assert!(
        resolve(
            &mut resolver,
            &program,
            &grandchild,
            Some(&barrier),
            Some(&barrier_computed),
            grandchild_seed,
            ViewAxisProviderParticipation::ProjectionOnly,
            &[],
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .cache_hit()
    );

    let changed_barrier = resolve(
        &mut resolver,
        &program,
        &barrier,
        Some(&root),
        Some(&changed_root),
        changed_root.axes().inherited_snapshot(),
        ViewAxisProviderParticipation::RetainedPrimary,
        &applications[1..],
        ViewStyleRevisionSet {
            applications: 1,
            ..ViewStyleRevisionSet::default()
        },
    )
    .unwrap()
    .into_computed();
    assert!(
        !resolve(
            &mut resolver,
            &program,
            &grandchild,
            Some(&barrier),
            Some(&barrier_computed),
            grandchild_seed,
            ViewAxisProviderParticipation::ProjectionOnly,
            &[],
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .cache_hit()
    );
    assert_eq!(changed_barrier.axes().mode(), ViewBoxAxisMode::VerticalLr);
    assert_eq!(
        grandchild_computed.axes().mode(),
        ViewBoxAxisMode::HorizontalLtr
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one retained chain preserves direct cache evidence across local winner removal, addition, and source replacement"
)]
fn local_barrier_transitions_invalidate_only_their_own_descendants() {
    let (program, applications) = axis_program(&[
        (1, ViewBoxAxisMode::HorizontalLtr),
        (2, ViewBoxAxisMode::HorizontalLtr),
    ]);
    let alternate_source = ViewStyleApplication::new(
        ViewStyleApplicationTarget::inline(ViewStylePatchId::new(2)),
        ViewStyleScopeId::new(1),
        0,
        1,
        ViewStyleBoundaryFacts::SAME_VIEW,
    );
    let mut resolver = ViewStyleResolver::default();
    let root = node(14, 0);
    let barrier = node(14, 1);
    let grandchild = node(14, 2);
    let sibling = node(14, 3);
    let other_mount_root = node(15, 0);
    let root_seed = host_seed(&root, 0, ViewBoxAxisHostSeed::Default);
    let root_computed = resolve(
        &mut resolver,
        &program,
        &root,
        None,
        None,
        root_seed,
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    let initial_barrier = resolve(
        &mut resolver,
        &program,
        &barrier,
        Some(&root),
        Some(&root_computed),
        root_computed.axes().inherited_snapshot(),
        ViewAxisProviderParticipation::RetainedPrimary,
        &applications[..1],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    assert_eq!(initial_barrier.axes().mode(), root_computed.axes().mode());
    assert_ne!(
        initial_barrier.axes().revision(),
        root_computed.axes().revision(),
        "an equal-value local winner is still a provider barrier"
    );
    let initial_grandchild = resolve(
        &mut resolver,
        &program,
        &grandchild,
        Some(&barrier),
        Some(&initial_barrier),
        initial_barrier.axes().inherited_snapshot(),
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    let sibling_computed = resolve(
        &mut resolver,
        &program,
        &sibling,
        Some(&root),
        Some(&root_computed),
        root_computed.axes().inherited_snapshot(),
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    let other_seed = host_seed(&other_mount_root, 0, ViewBoxAxisHostSeed::Default);
    resolve(
        &mut resolver,
        &program,
        &other_mount_root,
        None,
        None,
        other_seed,
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap();

    let inherited_barrier = resolve(
        &mut resolver,
        &program,
        &barrier,
        Some(&root),
        Some(&root_computed),
        root_computed.axes().inherited_snapshot(),
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet {
            applications: 1,
            ..ViewStyleRevisionSet::default()
        },
    )
    .unwrap()
    .into_computed();
    assert_eq!(
        inherited_barrier.axes().revision(),
        root_computed.axes().revision()
    );
    assert!(
        !resolve(
            &mut resolver,
            &program,
            &grandchild,
            Some(&barrier),
            Some(&initial_barrier),
            initial_barrier.axes().inherited_snapshot(),
            ViewAxisProviderParticipation::ProjectionOnly,
            &[],
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .cache_hit(),
        "removing a local winner evicts descendants"
    );
    let inherited_grandchild = resolve(
        &mut resolver,
        &program,
        &grandchild,
        Some(&barrier),
        Some(&inherited_barrier),
        inherited_barrier.axes().inherited_snapshot(),
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();

    let restored_barrier = resolve(
        &mut resolver,
        &program,
        &barrier,
        Some(&root),
        Some(&root_computed),
        root_computed.axes().inherited_snapshot(),
        ViewAxisProviderParticipation::RetainedPrimary,
        &applications[..1],
        ViewStyleRevisionSet {
            applications: 2,
            ..ViewStyleRevisionSet::default()
        },
    )
    .unwrap()
    .into_computed();
    assert_eq!(
        restored_barrier.axes().revision(),
        initial_barrier.axes().revision(),
        "revision-set changes do not perturb an unchanged local winner identity"
    );
    assert!(
        !resolve(
            &mut resolver,
            &program,
            &grandchild,
            Some(&barrier),
            Some(&inherited_barrier),
            inherited_barrier.axes().inherited_snapshot(),
            ViewAxisProviderParticipation::ProjectionOnly,
            &[],
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .cache_hit(),
        "adding a local winner evicts descendants before establishing the new barrier"
    );
    resolve(
        &mut resolver,
        &program,
        &grandchild,
        Some(&barrier),
        Some(&restored_barrier),
        restored_barrier.axes().inherited_snapshot(),
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap();

    let replaced_source = resolve(
        &mut resolver,
        &program,
        &barrier,
        Some(&root),
        Some(&root_computed),
        root_computed.axes().inherited_snapshot(),
        ViewAxisProviderParticipation::RetainedPrimary,
        std::slice::from_ref(&alternate_source),
        ViewStyleRevisionSet {
            applications: 3,
            ..ViewStyleRevisionSet::default()
        },
    )
    .unwrap()
    .into_computed();
    assert_eq!(
        replaced_source.axes().mode(),
        restored_barrier.axes().mode()
    );
    assert_ne!(
        replaced_source.axes().revision(),
        restored_barrier.axes().revision(),
        "same-mode winners from different typed sources have different provider identities"
    );
    assert!(
        !resolve(
            &mut resolver,
            &program,
            &grandchild,
            Some(&barrier),
            Some(&restored_barrier),
            restored_barrier.axes().inherited_snapshot(),
            ViewAxisProviderParticipation::ProjectionOnly,
            &[],
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .cache_hit()
    );
    assert!(
        resolve(
            &mut resolver,
            &program,
            &sibling,
            Some(&root),
            Some(&root_computed),
            root_computed.axes().inherited_snapshot(),
            ViewAxisProviderParticipation::ProjectionOnly,
            &[],
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .cache_hit(),
        "a sibling outside the changed subtree remains cached"
    );
    assert!(
        resolve(
            &mut resolver,
            &program,
            &other_mount_root,
            None,
            None,
            other_seed,
            ViewAxisProviderParticipation::RetainedPrimary,
            &[],
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .cache_hit(),
        "a different mount remains indexed and cached"
    );
    assert_eq!(
        initial_grandchild.axes().mode(),
        ViewBoxAxisMode::HorizontalLtr
    );
    assert_eq!(
        sibling_computed.axes().mode(),
        ViewBoxAxisMode::HorizontalLtr
    );
    assert_eq!(
        inherited_grandchild.axes().revision(),
        inherited_barrier.axes().revision()
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "reparenting needs both former and current ancestor changes to prove the reverse edge replacement"
)]
fn reparenting_replaces_the_old_edge_and_only_the_new_parent_invalidates() {
    let program = ViewStyleProgram::default();
    let mut resolver = ViewStyleResolver::default();
    let old_parent = node(16, 0);
    let new_parent = node(16, 1);
    let child = node(16, 2);
    let old_seed = host_seed(&old_parent, 0, ViewBoxAxisHostSeed::Default);
    let new_seed = host_seed(
        &new_parent,
        0,
        ViewBoxAxisHostSeed::Explicit(ViewBoxAxisMode::VerticalRl),
    );
    let old_computed = resolve(
        &mut resolver,
        &program,
        &old_parent,
        None,
        None,
        old_seed,
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    let new_computed = resolve(
        &mut resolver,
        &program,
        &new_parent,
        None,
        None,
        new_seed,
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    resolve(
        &mut resolver,
        &program,
        &child,
        Some(&old_parent),
        Some(&old_computed),
        old_computed.axes().inherited_snapshot(),
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap();
    let reparented = resolve(
        &mut resolver,
        &program,
        &child,
        Some(&new_parent),
        Some(&new_computed),
        new_computed.axes().inherited_snapshot(),
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    assert_eq!(reparented.axes().mode(), ViewBoxAxisMode::VerticalRl);

    resolve(
        &mut resolver,
        &program,
        &old_parent,
        None,
        None,
        host_seed(
            &old_parent,
            1,
            ViewBoxAxisHostSeed::Explicit(ViewBoxAxisMode::HorizontalRtl),
        ),
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap();
    assert!(
        resolve(
            &mut resolver,
            &program,
            &child,
            Some(&new_parent),
            Some(&new_computed),
            new_computed.axes().inherited_snapshot(),
            ViewAxisProviderParticipation::ProjectionOnly,
            &[],
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .cache_hit(),
        "the old parent edge must be absent after reparenting"
    );

    resolve(
        &mut resolver,
        &program,
        &new_parent,
        None,
        None,
        host_seed(
            &new_parent,
            1,
            ViewBoxAxisHostSeed::Explicit(ViewBoxAxisMode::VerticalLr),
        ),
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap();
    assert!(
        !resolve(
            &mut resolver,
            &program,
            &child,
            Some(&new_parent),
            Some(&new_computed),
            new_computed.axes().inherited_snapshot(),
            ViewAxisProviderParticipation::ProjectionOnly,
            &[],
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .cache_hit(),
        "the new parent edge must drive descendant invalidation"
    );
}

#[test]
fn cloned_resolvers_replay_the_same_axis_revision_sequence() {
    let program = ViewStyleProgram::default();
    let root = node(17, 0);
    let mut base = ViewStyleResolver::default();
    resolve(
        &mut base,
        &program,
        &root,
        None,
        None,
        host_seed(&root, 0, ViewBoxAxisHostSeed::Default),
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap();
    let mut left = base.clone();
    let mut right = base;
    for (generation, seed) in [
        (
            1,
            ViewBoxAxisHostSeed::Explicit(ViewBoxAxisMode::HorizontalLtr),
        ),
        (
            2,
            ViewBoxAxisHostSeed::Explicit(ViewBoxAxisMode::VerticalRl),
        ),
        (3, ViewBoxAxisHostSeed::Default),
    ] {
        let inherited = host_seed(&root, generation, seed);
        let left_result = resolve(
            &mut left,
            &program,
            &root,
            None,
            None,
            inherited,
            ViewAxisProviderParticipation::RetainedPrimary,
            &[],
            ViewStyleRevisionSet::default(),
        )
        .unwrap();
        let right_result = resolve(
            &mut right,
            &program,
            &root,
            None,
            None,
            inherited,
            ViewAxisProviderParticipation::RetainedPrimary,
            &[],
            ViewStyleRevisionSet::default(),
        )
        .unwrap();
        assert_eq!(left_result.computed(), right_result.computed());
        assert_eq!(
            left_result.computed().axes().revision(),
            inherited.revision()
        );
    }
}

#[test]
fn invalidation_budget_failure_preserves_provider_and_cache_state() {
    let program = ViewStyleProgram::default();
    let mut resolver = ViewStyleResolver::new(ViewStyleResolverLimits {
        max_axis_invalidation_nodes: 0,
        ..ViewStyleResolverLimits::default()
    });
    let root = node(21, 0);
    let child = node(21, 1);
    let root_seed = host_seed(&root, 0, ViewBoxAxisHostSeed::Default);
    let root_computed = resolve(
        &mut resolver,
        &program,
        &root,
        None,
        None,
        root_seed,
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    resolve(
        &mut resolver,
        &program,
        &child,
        Some(&root),
        Some(&root_computed),
        root_computed.axes().inherited_snapshot(),
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap();

    assert!(matches!(
        resolve(
            &mut resolver,
            &program,
            &root,
            None,
            None,
            host_seed(
                &root,
                1,
                ViewBoxAxisHostSeed::Explicit(ViewBoxAxisMode::VerticalRl),
            ),
            ViewAxisProviderParticipation::RetainedPrimary,
            &[],
            ViewStyleRevisionSet::default(),
        ),
        Err(ViewStyleResolveError::AxisProviderInvalidationBudget { node, limit: 0 })
            if node == root
    ));
    assert!(
        resolve(
            &mut resolver,
            &program,
            &root,
            None,
            None,
            root_seed,
            ViewAxisProviderParticipation::RetainedPrimary,
            &[],
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .cache_hit()
    );
    assert!(
        resolve(
            &mut resolver,
            &program,
            &child,
            Some(&root),
            Some(&root_computed),
            root_computed.axes().inherited_snapshot(),
            ViewAxisProviderParticipation::ProjectionOnly,
            &[],
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .cache_hit()
    );
}

#[test]
fn tracked_self_edge_is_rejected_as_a_cycle_without_mutation() {
    let program = ViewStyleProgram::default();
    let mut resolver = ViewStyleResolver::default();
    let root = node(31, 0);
    let root_seed = host_seed(&root, 0, ViewBoxAxisHostSeed::Default);
    let computed = resolve(
        &mut resolver,
        &program,
        &root,
        None,
        None,
        root_seed,
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    assert!(matches!(
        resolve(
            &mut resolver,
            &program,
            &root,
            Some(&root),
            Some(&computed),
            computed.axes().inherited_snapshot(),
            ViewAxisProviderParticipation::RetainedPrimary,
            &[],
            ViewStyleRevisionSet::default(),
        ),
        Err(ViewStyleResolveError::AxisProviderCycle { node, parent })
            if node == root && parent == root
    ));
    assert!(
        resolve(
            &mut resolver,
            &program,
            &root,
            None,
            None,
            root_seed,
            ViewAxisProviderParticipation::RetainedPrimary,
            &[],
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .cache_hit()
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the typed mismatch and multi-node cycle checks share one retained graph whose atomic state is asserted after each rejection"
)]
fn longer_cycles_and_parent_revision_mismatches_are_atomic_typed_failures() {
    let program = ViewStyleProgram::default();
    let mut resolver = ViewStyleResolver::default();
    let root = node(32, 0);
    let child = node(32, 1);
    let grandchild = node(32, 2);
    let root_seed = host_seed(&root, 0, ViewBoxAxisHostSeed::Default);
    let root_computed = resolve(
        &mut resolver,
        &program,
        &root,
        None,
        None,
        root_seed,
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    let child_seed = root_computed.axes().inherited_snapshot();
    let child_computed = resolve(
        &mut resolver,
        &program,
        &child,
        Some(&root),
        Some(&root_computed),
        child_seed,
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    let grandchild_seed = child_computed.axes().inherited_snapshot();
    let grandchild_computed = resolve(
        &mut resolver,
        &program,
        &grandchild,
        Some(&child),
        Some(&child_computed),
        grandchild_seed,
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();

    let other_revision = host_seed(&root, 1, ViewBoxAxisHostSeed::Default).revision();
    let mismatched = ViewInheritedBoxAxes::from_parent(root_computed.axes().mode(), other_revision);
    assert!(matches!(
        resolve(
            &mut resolver,
            &program,
            &child,
            Some(&root),
            Some(&root_computed),
            mismatched,
            ViewAxisProviderParticipation::RetainedPrimary,
            &[],
            ViewStyleRevisionSet::default(),
        ),
        Err(ViewStyleResolveError::AxisProviderRevisionMismatch {
            node: failed,
            parent,
            expected,
            actual,
        }) if failed == child
            && parent == root
            && expected == root_computed.axes().revision()
            && actual == other_revision
    ));
    assert!(
        resolve(
            &mut resolver,
            &program,
            &child,
            Some(&root),
            Some(&root_computed),
            child_seed,
            ViewAxisProviderParticipation::ProjectionOnly,
            &[],
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .cache_hit()
    );

    assert!(matches!(
        resolve(
            &mut resolver,
            &program,
            &root,
            Some(&grandchild),
            Some(&grandchild_computed),
            grandchild_computed.axes().inherited_snapshot(),
            ViewAxisProviderParticipation::RetainedPrimary,
            &[],
            ViewStyleRevisionSet::default(),
        ),
        Err(ViewStyleResolveError::AxisProviderCycle { node, .. }) if node == root
    ));
    assert!(
        resolve(
            &mut resolver,
            &program,
            &root,
            None,
            None,
            root_seed,
            ViewAxisProviderParticipation::RetainedPrimary,
            &[],
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .cache_hit()
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the failed child commit and successful retry must retain one continuous three-node marker lifecycle"
)]
fn marked_child_resolution_failure_preserves_the_chain_until_a_successful_retry() {
    let patch = ViewStylePatchId::new(77);
    let failure_program = ViewStyleProgram::try_new(
        Vec::new(),
        vec![ViewStylePatch::new(
            patch,
            vec![
                ViewStyleDeclaration::new(
                    arcweft_view::style::ViewPropertyKind::Padding,
                    ViewSpecifiedValue::Length {
                        value: ViewLengthMilli::new(1),
                    },
                    ViewStyleAssignOp::Replace,
                    ViewStyleSourceId::new(77),
                )
                .unwrap(),
            ],
        )],
    )
    .unwrap();
    let application = ViewStyleApplication::new(
        ViewStyleApplicationTarget::inline(patch),
        ViewStyleScopeId::new(1),
        0,
        0,
        ViewStyleBoundaryFacts::SAME_VIEW,
    );
    let mut resolver = ViewStyleResolver::new(ViewStyleResolverLimits {
        max_contributions: 1,
        ..ViewStyleResolverLimits::default()
    });
    let program = ViewStyleProgram::default();
    let root = node(33, 0);
    let child = node(33, 1);
    let grandchild = node(33, 2);
    let root_seed = host_seed(&root, 0, ViewBoxAxisHostSeed::Default);
    let root_computed = resolve(
        &mut resolver,
        &program,
        &root,
        None,
        None,
        root_seed,
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    let child_computed = resolve(
        &mut resolver,
        &program,
        &child,
        Some(&root),
        Some(&root_computed),
        root_computed.axes().inherited_snapshot(),
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    resolve(
        &mut resolver,
        &program,
        &grandchild,
        Some(&child),
        Some(&child_computed),
        child_computed.axes().inherited_snapshot(),
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap();

    let changed_root = resolve(
        &mut resolver,
        &program,
        &root,
        None,
        None,
        host_seed(
            &root,
            1,
            ViewBoxAxisHostSeed::Explicit(ViewBoxAxisMode::VerticalRl),
        ),
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    assert!(matches!(
        resolve(
            &mut resolver,
            &failure_program,
            &child,
            Some(&root),
            Some(&changed_root),
            changed_root.axes().inherited_snapshot(),
            ViewAxisProviderParticipation::RetainedPrimary,
            &[application],
            ViewStyleRevisionSet {
                patches: 1,
                ..ViewStyleRevisionSet::default()
            },
        ),
        Err(ViewStyleResolveError::ContributionBudget { limit: 1 })
    ));

    let retried_child = resolve(
        &mut resolver,
        &program,
        &child,
        Some(&root),
        Some(&changed_root),
        changed_root.axes().inherited_snapshot(),
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    let retried_grandchild = resolve(
        &mut resolver,
        &program,
        &grandchild,
        Some(&child),
        Some(&retried_child),
        retried_child.axes().inherited_snapshot(),
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap()
    .into_computed();
    assert_eq!(
        retried_grandchild.axes().mode(),
        ViewBoxAxisMode::VerticalRl
    );
}

#[test]
fn targeted_node_eviction_preserves_survivor_fifo_order_at_capacity() {
    let program = ViewStyleProgram::default();
    let mut resolver = ViewStyleResolver::new(ViewStyleResolverLimits {
        max_cache_entries: 3,
        ..ViewStyleResolverLimits::default()
    });
    let keys = [node(41, 0), node(42, 0), node(43, 0), node(44, 0)];
    let seeds = keys
        .iter()
        .map(|key| host_seed(key, 0, ViewBoxAxisHostSeed::Default))
        .collect::<Vec<_>>();
    for (key, seed) in keys.iter().zip(seeds.iter().copied()).take(3) {
        assert!(
            !resolve(
                &mut resolver,
                &program,
                key,
                None,
                None,
                seed,
                ViewAxisProviderParticipation::RetainedPrimary,
                &[],
                ViewStyleRevisionSet::default(),
            )
            .unwrap()
            .cache_hit()
        );
    }

    resolve(
        &mut resolver,
        &program,
        &keys[0],
        None,
        None,
        host_seed(&keys[0], 1, ViewBoxAxisHostSeed::Default),
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap();
    resolve(
        &mut resolver,
        &program,
        &keys[3],
        None,
        None,
        seeds[3],
        ViewAxisProviderParticipation::RetainedPrimary,
        &[],
        ViewStyleRevisionSet::default(),
    )
    .unwrap();

    assert!(
        resolve(
            &mut resolver,
            &program,
            &keys[2],
            None,
            None,
            seeds[2],
            ViewAxisProviderParticipation::RetainedPrimary,
            &[],
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .cache_hit()
    );
    assert!(
        !resolve(
            &mut resolver,
            &program,
            &keys[1],
            None,
            None,
            seeds[1],
            ViewAxisProviderParticipation::RetainedPrimary,
            &[],
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .cache_hit()
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the revision-set contract directly exercises each independent inventory and selector input against provider identity"
)]
fn every_revision_set_recomputes_and_provider_identity_follows_the_actual_winner() {
    let make_sheet = |id: &str, rule: ViewStyleRule| {
        ViewStyleSheet::new(
            ViewStyleSheetId::try_new(id).unwrap(),
            Vec::new(),
            vec![rule],
        )
        .unwrap()
    };

    // Sheet inventory: changing the selected sheet changes source identity.
    {
        let program = ViewStyleProgram::try_new(
            vec![
                make_sheet(
                    "style.axis.sheet.first",
                    axis_rule(
                        1,
                        Vec::new(),
                        ViewSpecifiedValue::BoxAxes {
                            value: ViewBoxAxisMode::HorizontalLtr,
                        },
                    ),
                ),
                make_sheet(
                    "style.axis.sheet.second",
                    axis_rule(
                        2,
                        Vec::new(),
                        ViewSpecifiedValue::BoxAxes {
                            value: ViewBoxAxisMode::HorizontalLtr,
                        },
                    ),
                ),
            ],
            Vec::new(),
        )
        .unwrap();
        let key = node(51, 0);
        let seed = host_seed(&key, 0, ViewBoxAxisHostSeed::Default);
        let mut resolver = ViewStyleResolver::default();
        let first = resolve(
            &mut resolver,
            &program,
            &key,
            None,
            None,
            seed,
            ViewAxisProviderParticipation::RetainedPrimary,
            &[named_application("style.axis.sheet.first", 0)],
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .into_computed();
        let changed = resolve(
            &mut resolver,
            &program,
            &key,
            None,
            None,
            seed,
            ViewAxisProviderParticipation::RetainedPrimary,
            &[named_application("style.axis.sheet.second", 0)],
            ViewStyleRevisionSet {
                sheets: 1,
                ..ViewStyleRevisionSet::default()
            },
        )
        .unwrap();
        assert!(!changed.cache_hit());
        assert_eq!(changed.computed().axes().mode(), first.axes().mode());
        assert_ne!(
            changed.computed().axes().revision(),
            first.axes().revision()
        );
    }

    // Patch inventory: changing the selected inline patch changes source identity.
    {
        let (program, applications) = axis_program(&[
            (1, ViewBoxAxisMode::HorizontalLtr),
            (2, ViewBoxAxisMode::VerticalRl),
        ]);
        let key = node(52, 0);
        let seed = host_seed(&key, 0, ViewBoxAxisHostSeed::Default);
        let mut resolver = ViewStyleResolver::default();
        let first = resolve(
            &mut resolver,
            &program,
            &key,
            None,
            None,
            seed,
            ViewAxisProviderParticipation::RetainedPrimary,
            &applications[..1],
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .into_computed();
        let changed = resolve(
            &mut resolver,
            &program,
            &key,
            None,
            None,
            seed,
            ViewAxisProviderParticipation::RetainedPrimary,
            &applications[1..],
            ViewStyleRevisionSet {
                patches: 1,
                ..ViewStyleRevisionSet::default()
            },
        )
        .unwrap();
        assert!(!changed.cache_hit());
        assert_ne!(
            changed.computed().axes().revision(),
            first.axes().revision()
        );
    }

    // Token inventory: same rule/source with a new token value changes only the effective mode.
    {
        let token_id = ViewStyleTokenId::try_new("axis.token.mode").unwrap();
        let token_program = |mode| {
            let token = ViewStyleToken::new(
                token_id.clone(),
                ViewStyleValueKind::BoxAxes,
                ViewSpecifiedValue::BoxAxes { value: mode },
                ViewStyleSourceId::new(1),
            )
            .unwrap();
            ViewStyleProgram::try_new(
                vec![
                    ViewStyleSheet::new(
                        ViewStyleSheetId::try_new("style.axis.sheet.token").unwrap(),
                        vec![token],
                        vec![axis_rule(
                            1,
                            Vec::new(),
                            ViewSpecifiedValue::Token {
                                token: token_id.clone(),
                                value_kind: ViewStyleValueKind::BoxAxes,
                            },
                        )],
                    )
                    .unwrap(),
                ],
                Vec::new(),
            )
            .unwrap()
        };
        let initial_program = token_program(ViewBoxAxisMode::HorizontalLtr);
        let changed_program = token_program(ViewBoxAxisMode::VerticalLr);
        let application = named_application("style.axis.sheet.token", 0);
        let key = node(53, 0);
        let seed = host_seed(&key, 0, ViewBoxAxisHostSeed::Default);
        let mut resolver = ViewStyleResolver::default();
        let first = resolve(
            &mut resolver,
            &initial_program,
            &key,
            None,
            None,
            seed,
            ViewAxisProviderParticipation::RetainedPrimary,
            std::slice::from_ref(&application),
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .into_computed();
        let changed = resolve(
            &mut resolver,
            &changed_program,
            &key,
            None,
            None,
            seed,
            ViewAxisProviderParticipation::RetainedPrimary,
            std::slice::from_ref(&application),
            ViewStyleRevisionSet {
                tokens: 1,
                ..ViewStyleRevisionSet::default()
            },
        )
        .unwrap();
        assert!(!changed.cache_hit());
        assert_eq!(
            changed.computed().axes().mode(),
            ViewBoxAxisMode::VerticalLr
        );
        assert_ne!(
            changed.computed().axes().revision(),
            first.axes().revision()
        );
    }

    // Application inventory: priority is part of the local provider transcript.
    {
        let (program, applications) = axis_program(&[(9, ViewBoxAxisMode::VerticalRl)]);
        let changed_application = ViewStyleApplication::new(
            ViewStyleApplicationTarget::inline(ViewStylePatchId::new(9)),
            ViewStyleScopeId::new(1),
            0,
            99,
            ViewStyleBoundaryFacts::SAME_VIEW,
        );
        let key = node(54, 0);
        let seed = host_seed(&key, 0, ViewBoxAxisHostSeed::Default);
        let mut resolver = ViewStyleResolver::default();
        let first = resolve(
            &mut resolver,
            &program,
            &key,
            None,
            None,
            seed,
            ViewAxisProviderParticipation::RetainedPrimary,
            &applications,
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .into_computed();
        let changed = resolve(
            &mut resolver,
            &program,
            &key,
            None,
            None,
            seed,
            ViewAxisProviderParticipation::RetainedPrimary,
            &[changed_application],
            ViewStyleRevisionSet {
                applications: 1,
                ..ViewStyleRevisionSet::default()
            },
        )
        .unwrap();
        assert!(!changed.cache_hit());
        assert_eq!(changed.computed().axes().mode(), first.axes().mode());
        assert_ne!(
            changed.computed().axes().revision(),
            first.axes().revision()
        );
    }

    // Interaction facts: selector reevaluation adds a local winner.
    {
        let program = ViewStyleProgram::try_new(
            vec![make_sheet(
                "style.axis.sheet.interaction",
                axis_rule(
                    1,
                    vec![ViewStylePredicate::Interaction(
                        ViewInteractionSelector::Hovered,
                    )],
                    ViewSpecifiedValue::BoxAxes {
                        value: ViewBoxAxisMode::VerticalRl,
                    },
                ),
            )],
            Vec::new(),
        )
        .unwrap();
        let application = named_application("style.axis.sheet.interaction", 0);
        let plain = ViewStyleNodeFacts::new(Some(ViewElementKind::Panel));
        let hovered = ViewStyleNodeFacts::new(Some(ViewElementKind::Panel)).with_interactions(
            ViewInteractionStateSet::default().with(ViewInteractionSelector::Hovered),
        );
        let key = node(55, 0);
        let seed = host_seed(&key, 0, ViewBoxAxisHostSeed::Default);
        let mut resolver = ViewStyleResolver::default();
        let first = resolve_with_facts(
            &mut resolver,
            &program,
            &key,
            None,
            None,
            seed,
            ViewAxisProviderParticipation::RetainedPrimary,
            std::slice::from_ref(&application),
            ViewStyleRevisionSet::default(),
            &plain,
            &PresentationEnvironment::ENGINE_DEFAULT,
        )
        .unwrap()
        .into_computed();
        let changed = resolve_with_facts(
            &mut resolver,
            &program,
            &key,
            None,
            None,
            seed,
            ViewAxisProviderParticipation::RetainedPrimary,
            std::slice::from_ref(&application),
            ViewStyleRevisionSet {
                interactions: 1,
                ..ViewStyleRevisionSet::default()
            },
            &hovered,
            &PresentationEnvironment::ENGINE_DEFAULT,
        )
        .unwrap();
        assert!(!changed.cache_hit());
        assert_eq!(
            changed.computed().axes().mode(),
            ViewBoxAxisMode::VerticalRl
        );
        assert_ne!(
            changed.computed().axes().revision(),
            first.axes().revision()
        );
    }

    // Container revision invalidates selector cache even while facts remain unavailable.
    {
        let program = ViewStyleProgram::try_new(
            vec![make_sheet(
                "style.axis.sheet.container",
                axis_rule(
                    1,
                    vec![ViewStylePredicate::Container(ViewContainerPredicate::new(
                        ViewContainerAxis::InlineSize,
                        ViewContainerComparison::GreaterOrEqual,
                        ViewLengthMilli::new(100),
                    ))],
                    ViewSpecifiedValue::BoxAxes {
                        value: ViewBoxAxisMode::VerticalLr,
                    },
                ),
            )],
            Vec::new(),
        )
        .unwrap();
        let application = named_application("style.axis.sheet.container", 0);
        let key = node(56, 0);
        let seed = host_seed(&key, 0, ViewBoxAxisHostSeed::Default);
        let mut resolver = ViewStyleResolver::default();
        let first = resolve(
            &mut resolver,
            &program,
            &key,
            None,
            None,
            seed,
            ViewAxisProviderParticipation::RetainedPrimary,
            std::slice::from_ref(&application),
            ViewStyleRevisionSet::default(),
        )
        .unwrap()
        .into_computed();
        let changed = resolve(
            &mut resolver,
            &program,
            &key,
            None,
            None,
            seed,
            ViewAxisProviderParticipation::RetainedPrimary,
            std::slice::from_ref(&application),
            ViewStyleRevisionSet {
                containers: 1,
                ..ViewStyleRevisionSet::default()
            },
        )
        .unwrap();
        assert!(!changed.cache_hit());
        assert_eq!(
            changed.computed().axes().revision(),
            first.axes().revision()
        );
    }

    // Environment revision and facts select a new rule winner.
    {
        let program = ViewStyleProgram::try_new(
            vec![make_sheet(
                "style.axis.sheet.environment",
                axis_rule_with_environment(
                    1,
                    Vec::new(),
                    Some(
                        ViewEnvironmentCondition::try_new(
                            vec![ViewEnvironmentWrapperSource::new(
                                ViewStyleSourceId::new(10),
                                ViewStyleSourceId::new(10),
                                ViewStyleSourceId::new(10),
                            )],
                            vec![ViewEnvironmentClause::color_scheme(
                                ColorScheme::Dark,
                                ViewEnvironmentWrapperIndex::new(0),
                                ViewStyleSourceId::new(11),
                            )],
                        )
                        .unwrap(),
                    ),
                    ViewSpecifiedValue::BoxAxes {
                        value: ViewBoxAxisMode::HorizontalRtl,
                    },
                ),
            )],
            Vec::new(),
        )
        .unwrap();
        let application = named_application("style.axis.sheet.environment", 0);
        let facts = ViewStyleNodeFacts::new(Some(ViewElementKind::Panel));
        let light =
            environment_with_revision(ColorScheme::Light, EnvironmentRevision::from_value(1));
        let dark = environment_with_revision(ColorScheme::Dark, EnvironmentRevision::from_value(2));
        let key = node(57, 0);
        let seed = host_seed(&key, 0, ViewBoxAxisHostSeed::Default);
        let mut resolver = ViewStyleResolver::default();
        let first = resolve_with_facts(
            &mut resolver,
            &program,
            &key,
            None,
            None,
            seed,
            ViewAxisProviderParticipation::RetainedPrimary,
            std::slice::from_ref(&application),
            ViewStyleRevisionSet::default(),
            &facts,
            &light,
        )
        .unwrap()
        .into_computed();
        let changed = resolve_with_facts(
            &mut resolver,
            &program,
            &key,
            None,
            None,
            seed,
            ViewAxisProviderParticipation::RetainedPrimary,
            std::slice::from_ref(&application),
            ViewStyleRevisionSet::default(),
            &facts,
            &dark,
        )
        .unwrap();
        assert!(!changed.cache_hit());
        assert_eq!(
            changed.computed().axes().mode(),
            ViewBoxAxisMode::HorizontalRtl
        );
        assert_ne!(
            changed.computed().axes().revision(),
            first.axes().revision()
        );
    }
}
