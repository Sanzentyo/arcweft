use arcweft_lang_syntax::{
    ast::module_path::{CanonicalModulePath, ModuleSegment},
    types::{TypePath, TypeRef, parse_type_ref},
};

use super::{
    NominalRecordLiteralPolicy, TypeCheckEnv,
    identity::EnvironmentBindingId,
    nominal::{
        AcceptedNominalCatalog, AcceptedNominalCatalogError, AcceptedNominalId,
        AcceptedNominalOrigin, AcceptedNominalOwnerId, AcceptedNominalRecord,
        AcceptedNominalSemantics, OpenNominalArity, OpenNominalEnvironment, OpenNominalPattern,
        OpenNominalPatternError, OpenNominalRule, OpenNominalRuleId, OpenNominalScope,
        RustPackageId,
    },
};
use crate::nominal::{AcceptedNominalCatalogLimitKind, AcceptedNominalCatalogLimits};
use crate::types::TypeKind;

fn path(source: &str) -> TypePath {
    let authored = parse_type_ref(source).expect("test path parses");
    let TypeRef::Path(path) = authored.value() else {
        panic!("test fixture is a direct path")
    };
    path.clone()
}

fn standard_record(source: &str, semantics: AcceptedNominalSemantics) -> AcceptedNominalRecord {
    AcceptedNominalRecord::try_new(
        AcceptedNominalId::new(AcceptedNominalOwnerId::Standard, path(source)),
        0,
        semantics,
        AcceptedNominalOrigin::Domain,
        None,
    )
    .expect("test accepted nominal record")
}

fn rule(
    owner: &str,
    ordinal: u32,
    scope: OpenNominalScope,
    pattern: OpenNominalPattern,
    arity: OpenNominalArity,
) -> Result<OpenNominalRule, AcceptedNominalCatalogError> {
    OpenNominalRule::try_new(
        OpenNominalRuleId::new(
            EnvironmentBindingId::try_new(owner).expect("environment owner"),
            ordinal,
        ),
        scope,
        pattern,
        arity,
        None,
    )
}

#[test]
fn typed_ids_expose_owned_identity_without_inverse_display_parsing() {
    let package = RustPackageId::try_new("adapter-types").expect("package id");
    let id = AcceptedNominalId::new(
        AcceptedNominalOwnerId::RustPackage(package.clone()),
        path("adapter.Widget"),
    );

    assert_eq!(id.owner(), &AcceptedNominalOwnerId::RustPackage(package));
    assert_eq!(id.canonical_path(), &path("adapter.Widget"));
    assert_eq!(id.source_label(), "rust:adapter-types::adapter.Widget");

    let rule_id = OpenNominalRuleId::new(
        EnvironmentBindingId::try_new("adapter.test").expect("environment owner"),
        7,
    );
    assert_eq!(rule_id.owner().as_str(), "adapter.test");
    assert_eq!(rule_id.ordinal(), 7);
}

#[test]
fn exact_catalog_is_ordered_and_digest_is_insertion_independent() {
    let alpha = standard_record(
        "domain.Alpha",
        AcceptedNominalSemantics::Exact(TypeKind::Duration),
    );
    let beta = standard_record("domain.Beta", AcceptedNominalSemantics::Opaque);
    let first = AcceptedNominalCatalog::try_new(
        [beta.clone(), alpha.clone()],
        [],
        AcceptedNominalCatalogLimits::PRODUCTION,
    )
    .expect("catalog");
    let second = AcceptedNominalCatalog::try_new(
        [alpha, beta],
        [],
        AcceptedNominalCatalogLimits::PRODUCTION,
    )
    .expect("catalog");

    assert_eq!(first.digest(), second.digest());
    assert_eq!(
        first
            .exact_records()
            .map(|record| record.id().canonical_path().canonical_string())
            .collect::<Vec<_>>(),
        ["domain.Alpha", "domain.Beta"]
    );
}

#[test]
fn duplicate_exact_path_is_rejected_deterministically() {
    let environment = AcceptedNominalRecord::try_new(
        AcceptedNominalId::new(
            AcceptedNominalOwnerId::Environment(
                EnvironmentBindingId::try_new("z.owner").expect("owner"),
            ),
            path("domain.Value"),
        ),
        0,
        AcceptedNominalSemantics::Opaque,
        AcceptedNominalOrigin::Adapter,
        None,
    )
    .expect("record");
    let rust = AcceptedNominalRecord::try_new(
        AcceptedNominalId::new(
            AcceptedNominalOwnerId::RustPackage(
                RustPackageId::try_new("a-owner").expect("package"),
            ),
            path("domain.Value"),
        ),
        0,
        AcceptedNominalSemantics::Opaque,
        AcceptedNominalOrigin::RustExport,
        None,
    )
    .expect("record");

    let left = AcceptedNominalCatalog::try_new(
        [environment.clone(), rust.clone()],
        [],
        AcceptedNominalCatalogLimits::PRODUCTION,
    )
    .expect_err("duplicate path");
    let right = AcceptedNominalCatalog::try_new(
        [rust, environment],
        [],
        AcceptedNominalCatalogLimits::PRODUCTION,
    )
    .expect_err("duplicate path");
    assert_eq!(left, right);
    assert!(matches!(
        left,
        AcceptedNominalCatalogError::DuplicateExactPath { path: duplicate, .. }
            if duplicate == path("domain.Value")
    ));
}

#[test]
fn exact_records_reject_reserved_paths_and_nonzero_exact_arity() {
    let reserved = AcceptedNominalRecord::try_new(
        AcceptedNominalId::new(AcceptedNominalOwnerId::Standard, path("Ref")),
        0,
        AcceptedNominalSemantics::Opaque,
        AcceptedNominalOrigin::Standard,
        None,
    );
    assert!(matches!(
        reserved,
        Err(AcceptedNominalCatalogError::ReservedPath { path: reserved })
            if reserved == path("Ref")
    ));

    assert!(matches!(
        AcceptedNominalRecord::try_new(
            AcceptedNominalId::new(AcceptedNominalOwnerId::Standard, path("ArcError")),
            1,
            AcceptedNominalSemantics::Exact(TypeKind::Named("ArcError".to_owned())),
            AcceptedNominalOrigin::Domain,
            None,
        ),
        Err(AcceptedNominalCatalogError::InvalidArity {
            minimum: 1,
            maximum: 1,
            ..
        })
    ));
}

#[test]
fn open_patterns_reject_global_or_unbounded_namespaces() {
    for pattern in [
        OpenNominalPattern::Exact(path("Ref")),
        OpenNominalPattern::Namespace {
            prefix: path("Ref"),
            min_tail_segments: 1,
            max_tail_segments: 1,
        },
    ] {
        assert!(matches!(
            rule(
                "adapter.reserved",
                0,
                OpenNominalScope::AcceptedWorld,
                pattern,
                OpenNominalArity::Exact(1),
            ),
            Err(AcceptedNominalCatalogError::InvalidOpenPattern {
                reason: OpenNominalPatternError::ReservedPath,
                ..
            })
        ));
    }

    let zero_tail = rule(
        "adapter.test",
        0,
        OpenNominalScope::AcceptedWorld,
        OpenNominalPattern::Namespace {
            prefix: path("adapter"),
            min_tail_segments: 0,
            max_tail_segments: 1,
        },
        OpenNominalArity::Exact(0),
    );
    assert!(matches!(
        zero_tail,
        Err(AcceptedNominalCatalogError::InvalidOpenPattern {
            reason: OpenNominalPatternError::ZeroTail,
            ..
        })
    ));

    let too_deep = rule(
        "adapter.test",
        1,
        OpenNominalScope::AcceptedWorld,
        OpenNominalPattern::Namespace {
            prefix: path("adapter"),
            min_tail_segments: 1,
            max_tail_segments: 17,
        },
        OpenNominalArity::Exact(0),
    );
    assert!(matches!(
        too_deep,
        Err(AcceptedNominalCatalogError::InvalidOpenPattern {
            reason: OpenNominalPatternError::TailMaximumExceeded {
                maximum: 17,
                allowed: 16
            },
            ..
        })
    ));
}

#[test]
fn overlapping_open_rules_are_rejected_but_disjoint_arity_is_allowed() {
    let broad = rule(
        "adapter.test",
        0,
        OpenNominalScope::ModuleSubtree(module("game")),
        OpenNominalPattern::Namespace {
            prefix: path("adapter"),
            min_tail_segments: 1,
            max_tail_segments: 4,
        },
        OpenNominalArity::Inclusive {
            minimum: 0,
            maximum: 1,
        },
    )
    .expect("rule");
    let nested = rule(
        "adapter.test",
        1,
        OpenNominalScope::ExactModule(module("game.ui")),
        OpenNominalPattern::Namespace {
            prefix: path("adapter.ui"),
            min_tail_segments: 1,
            max_tail_segments: 2,
        },
        OpenNominalArity::Exact(1),
    )
    .expect("rule");
    assert!(matches!(
        AcceptedNominalCatalog::try_new(
            [],
            [nested, broad.clone()],
            AcceptedNominalCatalogLimits::PRODUCTION,
        ),
        Err(AcceptedNominalCatalogError::OverlappingOpenRules { .. })
    ));

    let disjoint_arity = rule(
        "adapter.test",
        2,
        OpenNominalScope::ModuleSubtree(module("game")),
        broad.pattern().clone(),
        OpenNominalArity::Exact(2),
    )
    .expect("rule");
    AcceptedNominalCatalog::try_new(
        [],
        [broad, disjoint_arity],
        AcceptedNominalCatalogLimits::PRODUCTION,
    )
    .expect("disjoint arity rules do not collide");
}

#[test]
fn duplicate_open_rule_identity_is_rejected_even_for_disjoint_patterns() {
    let first = rule(
        "adapter.test",
        4,
        OpenNominalScope::AcceptedWorld,
        OpenNominalPattern::Exact(path("adapter.First")),
        OpenNominalArity::Exact(0),
    )
    .expect("rule");
    let duplicate = rule(
        "adapter.test",
        4,
        OpenNominalScope::DetachedOnly,
        OpenNominalPattern::Exact(path("adapter.Second")),
        OpenNominalArity::Exact(2),
    )
    .expect("rule");

    assert!(matches!(
        AcceptedNominalCatalog::try_new(
            [],
            [duplicate, first],
            AcceptedNominalCatalogLimits::PRODUCTION,
        ),
        Err(AcceptedNominalCatalogError::OverlappingOpenRules { first, second })
            if first == second && first.ordinal() == 4
    ));
}

#[test]
fn open_lookup_accepts_only_explicit_scope_pattern_and_arity() {
    let open = rule(
        "adapter.open",
        0,
        OpenNominalScope::ExactModule(module("game.ui")),
        OpenNominalPattern::Namespace {
            prefix: path("adapter.ui"),
            min_tail_segments: 1,
            max_tail_segments: 2,
        },
        OpenNominalArity::Exact(1),
    )
    .expect("rule");
    let catalog = AcceptedNominalCatalog::try_new(
        [],
        [open.clone()],
        AcceptedNominalCatalogLimits::PRODUCTION,
    )
    .expect("catalog");

    assert_eq!(
        catalog
            .matching_open_rule(
                OpenNominalEnvironment::Accepted,
                Some(&module("game.ui")),
                &path("adapter.ui.Widget"),
                1,
            )
            .map(OpenNominalRule::id),
        Some(open.id())
    );
    assert!(
        catalog
            .matching_open_rule(
                OpenNominalEnvironment::Accepted,
                Some(&module("game.other")),
                &path("adapter.ui.Widget"),
                1,
            )
            .is_none()
    );
    assert!(
        catalog
            .matching_open_rule(
                OpenNominalEnvironment::Accepted,
                Some(&module("game.ui")),
                &path("adapter.unknown.Widget"),
                1,
            )
            .is_none()
    );
    assert!(
        catalog
            .matching_open_rule(
                OpenNominalEnvironment::Accepted,
                Some(&module("game.ui")),
                &path("adapter.ui.Widget"),
                0,
            )
            .is_none()
    );
    assert!(catalog.exact(&path("adapter.ui.Unknown")).is_none());
}

#[test]
fn catalog_limits_and_typecheck_environment_updates_are_atomic() {
    let limits = AcceptedNominalCatalogLimits::try_new(1, 1).expect("small limits");
    let alpha = standard_record("domain.Alpha", AcceptedNominalSemantics::Opaque);
    let beta = standard_record("domain.Beta", AcceptedNominalSemantics::Opaque);
    assert!(matches!(
        AcceptedNominalCatalog::try_new([alpha, beta], [], limits),
        Err(AcceptedNominalCatalogError::Limit {
            kind: AcceptedNominalCatalogLimitKind::ExactRecords,
            observed: 2,
            maximum: 1,
        })
    ));

    let first_open = rule(
        "adapter.limit",
        0,
        OpenNominalScope::AcceptedWorld,
        OpenNominalPattern::Exact(path("adapter.First")),
        OpenNominalArity::Exact(0),
    )
    .expect("rule");
    let second_open = rule(
        "adapter.limit",
        1,
        OpenNominalScope::AcceptedWorld,
        OpenNominalPattern::Exact(path("adapter.Second")),
        OpenNominalArity::Exact(0),
    )
    .expect("rule");
    assert!(matches!(
        AcceptedNominalCatalog::try_new([], [first_open, second_open], limits),
        Err(AcceptedNominalCatalogError::Limit {
            kind: AcceptedNominalCatalogLimitKind::OpenRules,
            observed: 2,
            maximum: 1,
        })
    ));

    let env = TypeCheckEnv::default();
    let before = env.nominal_catalog().digest();
    let env = env
        .try_with_nominal_record(standard_record(
            "domain.Duration",
            AcceptedNominalSemantics::Exact(TypeKind::Duration),
        ))
        .expect("record is accepted atomically");
    assert_ne!(before, env.nominal_catalog().digest());
    assert!(
        env.nominal_catalog()
            .exact(&path("domain.Duration"))
            .is_some()
    );
}

#[test]
fn standard_environment_projects_domain_and_structural_nominals_exactly() {
    let environment = TypeCheckEnv::standard();
    for (name, semantics) in [
        ("DataFormat", TypeKind::DataFormat),
        ("DataShape", TypeKind::DataShape),
        ("AgentValue", TypeKind::AgentValue),
        ("TextCluster", TypeKind::TextCluster),
        ("Duration", TypeKind::Duration),
        ("DebugStatePath", TypeKind::DebugStatePath),
        ("ObservationFieldPath", TypeKind::ObservationFieldPath),
        ("VirtualPath", TypeKind::Named("VirtualPath".to_owned())),
        ("ArcError", TypeKind::Named("ArcError".to_owned())),
        ("ReducerError", TypeKind::Named("ReducerError".to_owned())),
        ("AgentError", TypeKind::Named("AgentError".to_owned())),
    ] {
        let record = environment
            .nominal_catalog()
            .exact(&path(name))
            .expect("standard domain atom is exact accepted evidence");
        assert_eq!(record.origin(), AcceptedNominalOrigin::Domain);
        assert_eq!(
            record.semantics(),
            &AcceptedNominalSemantics::Exact(semantics)
        );
        assert_eq!(record.arity(), 0);
    }

    let reduction = environment
        .nominal_catalog()
        .exact(&path("Reduction"))
        .expect("Reduction is accepted as a typed generic family");
    assert_eq!(reduction.origin(), AcceptedNominalOrigin::Domain);
    assert_eq!(reduction.semantics(), &AcceptedNominalSemantics::Opaque);
    assert_eq!(reduction.arity(), 1);

    let dialogue = environment
        .nominal_catalog()
        .exact(&path("DialogueContent"))
        .expect("standard structural nominal is exact accepted evidence");
    assert_eq!(dialogue.origin(), AcceptedNominalOrigin::NominalRecord);
    assert!(
        environment
            .nominal_records()
            .contains_key("DialogueContent")
    );

    let transform = environment
        .nominal_catalog()
        .exact(&path("Transform2D"))
        .expect("Transform2D is an exact standard record");
    assert_eq!(transform.origin(), AcceptedNominalOrigin::NominalRecord);
    assert_eq!(
        transform.semantics(),
        &AcceptedNominalSemantics::Exact(TypeKind::Named("Transform2D".to_owned()))
    );
    let transform_fields = environment
        .nominal_records()
        .get("Transform2D")
        .expect("Transform2D publishes its typed field inventory");
    assert_eq!(transform_fields.len(), 10);
    assert_eq!(
        transform_fields.get("rotation"),
        Some(&TypeKind::Named("Angle".to_owned()))
    );
    assert_eq!(
        environment.nominal_record_literal_policy("Transform2D"),
        NominalRecordLiteralPolicy::DefaultMissing
    );
}

#[test]
fn rust_export_publishes_typed_package_and_exact_path_atomically() {
    let package = RustPackageId::try_new("truck_game").expect("package");
    let environment = TypeCheckEnv::default()
        .try_with_rust_type_export(package.clone(), path("Rank"))
        .expect("Rust export");
    let record = environment
        .nominal_catalog()
        .exact(&path("Rank"))
        .expect("Rust export exact record");

    assert_eq!(
        record.id().owner(),
        &AcceptedNominalOwnerId::RustPackage(package)
    );
    assert_eq!(record.origin(), AcceptedNominalOrigin::RustExport);
    assert_eq!(record.semantics(), &AcceptedNominalSemantics::Opaque);
}

#[test]
fn world_only_scopes_are_validated_for_the_selected_environment() {
    let accepted = rule(
        "adapter.accepted",
        0,
        OpenNominalScope::AcceptedWorld,
        OpenNominalPattern::Exact(path("adapter.Accepted")),
        OpenNominalArity::Exact(0),
    )
    .expect("rule");
    let catalog =
        AcceptedNominalCatalog::try_new([], [accepted], AcceptedNominalCatalogLimits::PRODUCTION)
            .expect("catalog");
    catalog
        .validate_scopes_for(OpenNominalEnvironment::Accepted)
        .expect("accepted-world rule");
    assert!(matches!(
        catalog.validate_scopes_for(OpenNominalEnvironment::Detached),
        Err(AcceptedNominalCatalogError::InvalidScope {
            scope: OpenNominalScope::AcceptedWorld,
            ..
        })
    ));
}

fn module(source: &str) -> CanonicalModulePath {
    CanonicalModulePath::from_segments(
        source
            .split('.')
            .map(|segment| ModuleSegment::new(segment).expect("module segment")),
    )
}
