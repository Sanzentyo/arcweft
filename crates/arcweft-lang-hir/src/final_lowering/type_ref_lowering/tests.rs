use std::sync::Arc;

use arcweft_lang_syntax::ast::module_path::{CanonicalModulePath, ModulePathRoot};
use arcweft_lang_syntax::ast::symbol_path::{ProjectSymbolPath, ProjectSymbolSegment};
use arcweft_lang_syntax::attachment::{AttachedTypeFamily, AttachedTypeRefNode, TypedItemNode};
use arcweft_lang_syntax::incremental::{ParsedSource, SyntaxDatabase};
use arcweft_lang_syntax::types::{
    TypePath, TypeRefComponentRole, TypeRefNodeStep, TypeRefRegionPart,
};
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceEdit, SourceName, SourceRange};

use super::*;
use crate::database::HirDatabase;
use crate::diagnostic::{HirDiagnostic, HirRecoveryPrimary};
use crate::identity::{HirLimit, HirTypedId};
use crate::leaf::{HirPathRoot, HirPathSegment, HirTypeRegion, HirTypeRegionIssue};
use crate::lowering::{HirLimitError, HirModuleKey, LoweringRequest};
use crate::module::{HirModule, HirModuleStatus};
use crate::scope::{HirScope, HirScopeKind, HirScopeOwner};
use crate::slot::HirOrigin;
use crate::source_index::{
    HirSourceOwnerStatus, HirSourcePresence, HirSourceQueryError, HirTypeRegionSourcePart,
};
use crate::symbol::CallablePackageId;

fn alias_document(document_id: &str, type_source: &str) -> (SourceName, Arc<SourceDocument>) {
    let name = SourceName::path(format!("proof/type-lowering/{document_id}.arcw"));
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!(
                "arcweft-test://lang-hir/type-lowering/{document_id}.arcw"
            ))
            .expect("type-lowering document ID"),
            name.clone(),
            format!("type Values = {type_source}\n"),
        )
        .expect("type-lowering source"),
    );
    (name, document)
}

fn parsed_type(document_id: &str, type_source: &str) -> ParsedSource {
    parsed_alias_type(document_id, type_source)
}

fn parsed_alias_type(document_id: &str, type_source: &str) -> ParsedSource {
    let name = SourceName::path(format!("proof/type-lowering/{document_id}.arcw"));
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!(
                "arcweft-test://lang-hir/type-lowering/{document_id}.arcw"
            ))
            .expect("type-lowering document ID"),
            name.clone(),
            format!("type Values = {type_source}\n"),
        )
        .expect("type-alias source"),
    );
    SyntaxDatabase::try_new()
        .expect("type-lowering syntax database")
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("attached type-alias source parses")
}

fn attached_type(parsed: &ParsedSource) -> AttachedTypeRefNode {
    attached_alias_type(parsed)
}

fn attached_alias_type(parsed: &ParsedSource) -> AttachedTypeRefNode {
    let item = parsed
        .items()
        .expect("type-alias item inventory")
        .into_iter()
        .next()
        .expect("type-alias declaration");
    let TypedItemNode::TypeAlias(alias) = item else {
        panic!("expected type-alias item family");
    };
    alias
        .semantics()
        .expect("attached type-alias declaration")
        .target()
        .clone()
}

fn module_key(parsed: &ParsedSource) -> HirModuleKey {
    HirModuleKey::new(
        CallablePackageId::try_new("proof-type-lowering-tests").expect("package ID"),
        CanonicalModulePath::crate_root(),
        parsed.document().identity().clone(),
    )
}

fn stage<'source>(
    database: &HirDatabase,
    parsed: &'source ParsedSource,
) -> StagedHirModuleTransaction<'source> {
    super::super::stage_unpublished_module_for_invariant_test(
        database,
        LoweringRequest::try_new(module_key(parsed), parsed).expect("lowering request"),
        crate::lowering::HirLoweringControl::new(),
    )
    .expect("staged HIR module")
}

fn allocate_module_scope(
    transaction: &mut StagedHirModuleTransaction<'_>,
    parsed: &ParsedSource,
) -> ScopeId {
    let module = transaction.snapshot_id().module();
    let root = parsed.root_syntax();
    let site = HirSourceSite::Span(root.source_span().clone());
    let (slots, arenas) = transaction.storage_mut();
    arenas
        .scopes()
        .allocate_source(
            slots,
            root.id(),
            site,
            HirScope::try_new(
                module,
                HirScopeKind::Module,
                None,
                HirScopeOwner::Module(module),
                Box::new([]),
                Box::new([]),
            )
            .expect("module scope"),
        )
        .expect("module scope allocation")
}

fn lower_and_publish(parsed: &ParsedSource) -> (Arc<HirModule>, TypeId) {
    let attached = attached_type(parsed);
    lower_attached_and_publish(parsed, &attached)
}

fn lower_attached_and_publish(
    parsed: &ParsedSource,
    attached: &AttachedTypeRefNode,
) -> (Arc<HirModule>, TypeId) {
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, parsed);
    let scope = allocate_module_scope(&mut transaction, parsed);
    let owner = transaction
        .lower_attached_type(attached, scope)
        .expect("attached type lowering");
    let module = transaction
        .finish(&mut database)
        .expect("type module publication")
        .into_module();
    (module, owner)
}

fn resolved_type(module: &HirModule, owner: TypeId) -> &HirType {
    module
        .arenas()
        .types()
        .resolve(module.slots(), owner)
        .expect("published type payload")
}

fn type_source_positions(
    module: &HirModule,
    owner: TypeId,
    attached: &AttachedTypeRefNode,
) -> (SourceRange, usize) {
    let expected_whole = attached.whole_source_span();
    let whole = module
        .source_site(
            expected_whole.source(),
            HirSourceQuery::Type {
                owner,
                role: HirTypeSourceRole::Whole,
            },
        )
        .expect("whole type source query");
    let HirSourcePresence::Present(HirSourceSite::Span(published_whole)) = whole.presence() else {
        panic!("type must publish its whole source span");
    };
    assert_eq!(published_whole.source(), expected_whole.source());
    assert_eq!(published_whole.range(), expected_whole.range());

    let expected_elision = attached
        .component(TypeRefComponentRole::Region(
            TypeRefRegionPart::ElisionInsertion,
        ))
        .expect("attached elision component");
    let elision = module
        .source_site(
            expected_whole.source(),
            HirSourceQuery::Type {
                owner,
                role: HirTypeSourceRole::Region(HirTypeRegionSourcePart::ElisionInsertion),
            },
        )
        .expect("elision source query");
    let HirSourcePresence::Present(HirSourceSite::Insertion(published_elision)) =
        elision.presence()
    else {
        panic!("elided region must publish an insertion source site");
    };
    assert_eq!(published_elision.source_identity(), expected_whole.source());
    assert_eq!(published_elision.offset(), expected_elision.range().start());
    (expected_whole.range(), published_elision.offset())
}

fn kind_matches_family(kind: &HirTypeKind, family: AttachedTypeFamily) -> bool {
    matches!(
        (kind, family),
        (HirTypeKind::Never, AttachedTypeFamily::Never)
            | (HirTypeKind::ConstInt(_), AttachedTypeFamily::ConstInt)
            | (HirTypeKind::Path(_), AttachedTypeFamily::Path)
            | (HirTypeKind::Tuple(_), AttachedTypeFamily::Tuple)
            | (HirTypeKind::Function(_), AttachedTypeFamily::Function)
            | (HirTypeKind::Choice(_), AttachedTypeFamily::Choice)
            | (HirTypeKind::Generic(_), AttachedTypeFamily::Generic)
            | (HirTypeKind::TraitBound(_), AttachedTypeFamily::TraitBound)
            | (HirTypeKind::Projection(_), AttachedTypeFamily::Projection)
            | (HirTypeKind::Reference(_), AttachedTypeFamily::Reference)
            | (HirTypeKind::Slice(_), AttachedTypeFamily::Slice)
            | (HirTypeKind::Recovery(_), AttachedTypeFamily::Recovery)
    )
}

#[test]
fn all_twelve_attached_type_families_lower_into_the_final_type_arena() {
    let cases = [
        ("never", "!", AttachedTypeFamily::Never),
        ("const-int", "32", AttachedTypeFamily::ConstInt),
        ("path", "crate.model.Value", AttachedTypeFamily::Path),
        ("tuple", "(A, B)", AttachedTypeFamily::Tuple),
        ("function", "(A, B) -> C", AttachedTypeFamily::Function),
        ("choice", "A | B", AttachedTypeFamily::Choice),
        ("generic", "Vec<A>", AttachedTypeFamily::Generic),
        (
            "trait-bound",
            "Iterator<Item = A>",
            AttachedTypeFamily::TraitBound,
        ),
        (
            "projection",
            "(A | B)::Item",
            AttachedTypeFamily::Projection,
        ),
        ("reference", "&'scene mut A", AttachedTypeFamily::Reference),
        ("slice", "[A]", AttachedTypeFamily::Slice),
        ("recovery", "[A; 32]", AttachedTypeFamily::Recovery),
    ];

    for (document_id, source, expected_family) in cases {
        let parsed = parsed_type(document_id, source);
        let attached = attached_type(&parsed);
        assert_eq!(attached.family(), expected_family, "fixture `{source}`");
        let (module, owner) = lower_and_publish(&parsed);
        let payload = resolved_type(&module, owner);
        assert!(
            kind_matches_family(payload.kind(), expected_family),
            "attached family and HIR payload differ for `{source}`: {:?}",
            payload.kind()
        );
        assert_eq!(payload.scope().module(), module.module_id());
        if expected_family == AttachedTypeFamily::Recovery {
            assert_eq!(module.status(), HirModuleStatus::Recovered);
            assert!(payload.is_poisoned());
        } else {
            assert_eq!(module.status(), HirModuleStatus::Clean);
            assert!(!payload.is_poisoned());
        }
    }
}

#[test]
fn parent_type_is_reserved_before_exact_attached_children() {
    let parsed = parsed_type("child-identity", "Pair<A, Vec<B>>");
    let attached = attached_type(&parsed);
    let attached_children = attached.children().expect("attached type children");
    let (module, owner) = lower_and_publish(&parsed);
    let HirTypeKind::Generic(generic) = resolved_type(&module, owner).kind() else {
        panic!("fixture must lower as a generic type");
    };
    assert_eq!(generic.arguments().len(), attached_children.len());

    for child in &attached_children {
        let TypeRefNodeStep::GenericArgument(ordinal) = child.step() else {
            panic!("root child must be a generic argument");
        };
        let child_id = generic.arguments()[usize::from(ordinal)];
        assert!(owner.raw().slot() < child_id.raw().slot());
        assert!(matches!(
            module.slots().resolve(child_id).expect("child slot").origin(),
            HirOrigin::Source(key) if key.syntax() == child.node().id()
        ));
    }
}

#[test]
fn path_projection_preserves_roots_and_external_project_segments() {
    for (document_id, source, expected) in [
        ("crate-root", "crate.Value", HirPathRoot::Crate),
        ("self-root", "self.Value", HirPathRoot::SelfModule),
        (
            "super-root",
            "super.super.Value",
            HirPathRoot::Super { depth: 2 },
        ),
        ("implicit-root", "Value", HirPathRoot::ImplicitCrate),
    ] {
        let parsed = parsed_type(document_id, source);
        let (module, owner) = lower_and_publish(&parsed);
        let HirTypeKind::Path(path) = resolved_type(&module, owner).kind() else {
            panic!("attached fixture `{source}` must lower as a path");
        };
        assert_eq!(path.root(), expected);
        assert!(matches!(
            path.segments(),
            [HirPathSegment::Identifier(segment)] if segment.as_str() == "Value"
        ));
    }

    // Current authored TypeRef grammar admits identifier segments only. This
    // direct typed projection supplements the attached production cases above
    // for the accepted external-project segment branch.
    let project_path = ProjectSymbolPath::new(
        ModulePathRoot::Crate,
        [
            ProjectSymbolSegment::try_new("hero-pack").unwrap(),
            ProjectSymbolSegment::try_new("Character").unwrap(),
        ],
    )
    .unwrap();
    let projected = project_type_path(&TypePath::from(project_path)).unwrap();
    assert!(matches!(
        projected.segments(),
        [HirPathSegment::ProjectSymbol(project), HirPathSegment::Identifier(name)]
            if project.as_str() == "hero-pack" && name.as_str() == "Character"
    ));
}

#[test]
fn named_and_elided_regions_publish_exact_typed_source_roles() {
    let named_parsed = parsed_type("named-region", "&'scene mut Value");
    let named_attached = attached_type(&named_parsed);
    let (named_module, named_owner) = lower_and_publish(&named_parsed);
    let HirTypeKind::Reference(named) = resolved_type(&named_module, named_owner).kind() else {
        panic!("named fixture must lower as reference");
    };
    assert!(matches!(
        named.region(),
        Some(HirTypeRegion::Named(region)) if region.name().as_str() == "scene"
    ));
    for (syntax_role, hir_role) in [
        (
            TypeRefComponentRole::Region(TypeRefRegionPart::Whole),
            HirTypeSourceRole::Region(HirTypeRegionSourcePart::Whole),
        ),
        (
            TypeRefComponentRole::Region(TypeRefRegionPart::NamedApostrophe),
            HirTypeSourceRole::Region(HirTypeRegionSourcePart::NamedApostrophe),
        ),
        (
            TypeRefComponentRole::Region(TypeRefRegionPart::NamedName),
            HirTypeSourceRole::Region(HirTypeRegionSourcePart::NamedName),
        ),
    ] {
        let lookup = named_module
            .source_site(
                named_parsed.document().identity(),
                HirSourceQuery::Type {
                    owner: named_owner,
                    role: hir_role,
                },
            )
            .expect("named region source query");
        assert_eq!(lookup.owner_status(), HirSourceOwnerStatus::Clean);
        assert_eq!(
            lookup.presence(),
            HirSourcePresence::Present(&HirSourceSite::Span(
                named_attached
                    .component(syntax_role)
                    .expect("named region attached component")
            ))
        );
    }

    let elided_parsed = parsed_type("elided-region", "&Value");
    let elided_attached = attached_type(&elided_parsed);
    let (elided_module, elided_owner) = lower_and_publish(&elided_parsed);
    let HirTypeKind::Reference(elided) = resolved_type(&elided_module, elided_owner).kind() else {
        panic!("elided fixture must lower as reference");
    };
    let Some(HirTypeRegion::Elided(region)) = elided.region() else {
        panic!("reference must retain an elided region");
    };
    assert_eq!(region.owner_type(), elided_owner);
    assert_eq!(region.key().owner(), SyntheticOwner::Type(elided_owner));
    assert_eq!(region.key().role(), SyntheticRole::ElidedRegion);
    assert_eq!(region.key().ordinal(), 0);
    assert!(
        elided_module
            .slots()
            .contains_key_only_synthetic(region.key())
    );
    assert_eq!(elided_module.slots().key_only_synthetic_keys().count(), 1);

    let lookup = elided_module
        .source_site(
            elided_parsed.document().identity(),
            HirSourceQuery::Type {
                owner: elided_owner,
                role: HirTypeSourceRole::Region(HirTypeRegionSourcePart::ElisionInsertion),
            },
        )
        .expect("elided region source query");
    let HirSourcePresence::Present(HirSourceSite::Insertion(insertion)) = lookup.presence() else {
        panic!("elided region must publish an insertion source site");
    };
    assert_eq!(
        insertion.offset(),
        elided_attached
            .component(TypeRefComponentRole::Region(
                TypeRefRegionPart::ElisionInsertion,
            ))
            .expect("elision component")
            .range()
            .start()
    );
}

#[test]
fn recovery_uses_the_exact_type_poison_and_recovery_source() {
    let parsed = parsed_type("recovery-evidence", "[Value; 4]");
    let attached = attached_type(&parsed);
    let (module, owner) = lower_and_publish(&parsed);
    let payload = resolved_type(&module, owner);
    assert!(matches!(
        (payload.kind(), payload.state()),
        (
            HirTypeKind::Recovery(error),
            HirPoisonState::Poisoned(HirRecoveryIssue::InvalidType(issue))
        ) if error.issue() == *issue && *issue == HirGenericTypeIssue::UnclassifiedSyntax
    ));
    let lookup = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Type {
                owner,
                role: HirTypeSourceRole::Recovery,
            },
        )
        .expect("recovery source query");
    assert_eq!(lookup.owner_status(), HirSourceOwnerStatus::Poisoned);
    assert_eq!(
        lookup.presence(),
        HirSourcePresence::Present(&HirSourceSite::Span(
            attached
                .component(TypeRefComponentRole::Recovery)
                .expect("attached recovery component")
        ))
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one named-region recovery matrix proves poison state and exact source authority together"
)]
fn invalid_named_regions_publish_known_reference_poison_and_exact_source_authority() {
    for (case, invalid_name) in [("leading-digit", "9"), ("unicode-digit", "a١")] {
        let type_source = format!("&'{invalid_name} mut Value");
        let (name, document) = alias_document(case, &type_source);
        let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
        let parsed = syntax
            .parse_initial(
                SourceSnapshotId::initial(name),
                document,
                arcweft_lang_syntax::parser::ParseOptions::default(),
            )
            .expect("recoverable named-region source");
        let attached = attached_type(&parsed);
        assert_eq!(attached.family(), AttachedTypeFamily::Reference, "{case}");

        let (module, owner) = lower_and_publish(&parsed);
        let payload = resolved_type(&module, owner);
        let HirTypeKind::Reference(reference) = payload.kind() else {
            panic!("invalid named region must retain its known reference family");
        };
        assert_eq!(reference.region(), None);
        let _ = resolved_type(&module, reference.referent());
        assert!(matches!(
            payload.state(),
            HirPoisonState::Poisoned(HirRecoveryIssue::InvalidTypeRegion(
                HirTypeRegionIssue::InvalidNamedRegion
            ))
        ));
        assert_eq!(module.status(), HirModuleStatus::Recovered);

        for (syntax_role, hir_role) in [
            (
                TypeRefComponentRole::Region(TypeRefRegionPart::Whole),
                HirTypeSourceRole::Region(HirTypeRegionSourcePart::Whole),
            ),
            (
                TypeRefComponentRole::Region(TypeRefRegionPart::NamedApostrophe),
                HirTypeSourceRole::Region(HirTypeRegionSourcePart::NamedApostrophe),
            ),
            (
                TypeRefComponentRole::Region(TypeRefRegionPart::NamedName),
                HirTypeSourceRole::Region(HirTypeRegionSourcePart::NamedName),
            ),
        ] {
            let lookup = module
                .source_site(
                    parsed.document().identity(),
                    HirSourceQuery::Type {
                        owner,
                        role: hir_role,
                    },
                )
                .expect("poisoned named-region source query");
            assert_eq!(lookup.owner_status(), HirSourceOwnerStatus::Poisoned);
            assert_eq!(
                lookup.presence(),
                HirSourcePresence::Present(&HirSourceSite::Span(
                    attached
                        .component(syntax_role)
                        .expect("named-region source component"),
                ))
            );
        }
        let elision_role = HirTypeSourceRole::Region(HirTypeRegionSourcePart::ElisionInsertion);
        assert_eq!(
            module.source_site(
                parsed.document().identity(),
                HirSourceQuery::Type {
                    owner,
                    role: elision_role,
                },
            ),
            Err(HirSourceQueryError::TypeRoleNotApplicable {
                owner,
                role: elision_role,
            })
        );

        let name_role = HirTypeSourceRole::Region(HirTypeRegionSourcePart::NamedName);
        let recovery = module
            .diagnostics()
            .iter()
            .find_map(|diagnostic| match diagnostic {
                HirDiagnostic::Recovery(recovery)
                    if recovery.owner() == SyntheticOwner::Type(owner) =>
                {
                    Some(recovery)
                }
                _ => None,
            })
            .expect("invalid named-region recovery diagnostic");
        assert_eq!(
            recovery.primary_role(),
            HirRecoveryPrimary::query(HirSourceQuery::Type {
                owner,
                role: name_role,
            })
        );
        assert_eq!(
            recovery.primary(),
            &HirSourceSite::Span(
                attached
                    .component(TypeRefComponentRole::Region(TypeRefRegionPart::NamedName))
                    .expect("invalid named-region primary"),
            )
        );

        let revised = syntax
            .reparse(
                &parsed,
                &[SourceEdit::new(
                    parsed
                        .document()
                        .span(SourceRange::new(0, 0))
                        .expect("revision insertion"),
                    " ",
                )],
                arcweft_lang_syntax::parser::ParseOptions::default(),
            )
            .expect("revised source");
        assert!(matches!(
            module.source_site(
                revised.document().identity(),
                HirSourceQuery::Type {
                    owner,
                    role: name_role,
                },
            ),
            Err(HirSourceQueryError::StaleSourceRevision { .. })
        ));

        let foreign = parsed_type(&format!("{case}-foreign"), "Value");
        let (_, foreign_owner) = lower_and_publish(&foreign);
        assert!(matches!(
            module.source_site(
                parsed.document().identity(),
                HirSourceQuery::Type {
                    owner: foreign_owner,
                    role: HirTypeSourceRole::Whole,
                },
            ),
            Err(HirSourceQueryError::TypeResolve {
                error: crate::identity::IdResolveError::WrongModule { .. },
                ..
            })
        ));
    }
}

#[test]
fn invalid_named_region_reuses_rolled_back_ids_but_name_one_over_is_fatal() {
    let maximum = HirLimit::NameBytes.maximum();
    let exact = parsed_type(
        "invalid-named-region-rollback",
        &format!("&'{} Value", "9".repeat(maximum)),
    );
    let attached = attached_type(&exact);
    assert_eq!(attached.family(), AttachedTypeFamily::Reference);
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut dropped = stage(&database, &exact);
    let dropped_scope = allocate_module_scope(&mut dropped, &exact);
    let dropped_owner = dropped
        .lower_attached_type(&attached, dropped_scope)
        .expect("known-family recovery lowers transactionally");
    drop(dropped);
    assert!(database.current(&module_key(&exact)).is_none());

    let mut replacement = stage(&database, &exact);
    let replacement_scope = allocate_module_scope(&mut replacement, &exact);
    let replacement_owner = replacement
        .lower_attached_type(&attached, replacement_scope)
        .expect("replacement known-family recovery");
    assert_eq!(replacement_scope, dropped_scope);
    assert_eq!(replacement_owner, dropped_owner);
    assert_eq!(
        replacement
            .finish(&mut database)
            .expect("recovered publication")
            .into_module()
            .status(),
        HirModuleStatus::Recovered,
    );

    let one_over = parsed_type(
        "invalid-named-region-name-one-over",
        &format!("&'{} Value", "9".repeat(maximum + 1)),
    );
    let attached = attached_type(&one_over);
    assert_eq!(attached.family(), AttachedTypeFamily::Reference);
    let mut transaction = stage(&database, &one_over);
    let scope = allocate_module_scope(&mut transaction, &one_over);
    assert_eq!(
        transaction.lower_attached_type(&attached, scope),
        Err(HirLowerFailure::Limit(HirLimitError::with_maximum(
            HirLimit::NameBytes,
            maximum + 1,
            maximum,
        )))
    );
    assert!(transaction.finish(&mut database).is_err());
    assert!(database.current(&module_key(&one_over)).is_none());
}

#[test]
fn retained_elided_type_reuses_type_and_key_identity_across_revision() {
    const INSERTED_TRIVIA: &str = "// retained trivia\n";

    let (name, document) = alias_document("retained-elision", "&Value");
    let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("initial source");
    let revised = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial
                    .document()
                    .span(SourceRange::new(0, 0))
                    .expect("edit insertion"),
                INSERTED_TRIVIA,
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("revised source");
    let initial_attached = attached_type(&initial);
    let revised_attached = revised
        .attached_type_ref(initial_attached.id())
        .expect("retained attached type in revised snapshot");
    assert_eq!(initial_attached.id(), revised_attached.id());

    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut first = stage(&database, &initial);
    let first_scope = allocate_module_scope(&mut first, &initial);
    let first_id = first
        .lower_attached_type(&initial_attached, first_scope)
        .expect("initial type lowering");
    let first_module = first
        .finish(&mut database)
        .expect("initial publication")
        .into_module();
    let HirTypeKind::Reference(first_reference) = resolved_type(&first_module, first_id).kind()
    else {
        panic!("fixture must lower as reference");
    };
    let Some(HirTypeRegion::Elided(first_region)) = first_reference.region() else {
        panic!("fixture must have elided region");
    };
    let (initial_whole_range, initial_elision_offset) =
        type_source_positions(&first_module, first_id, &initial_attached);

    let mut second = stage(&database, &revised);
    let second_scope = allocate_module_scope(&mut second, &revised);
    let second_id = second
        .lower_attached_type(&revised_attached, second_scope)
        .expect("revised type lowering");
    assert_eq!(second_scope, first_scope);
    assert_eq!(second_id, first_id);
    let second_module = second
        .finish(&mut database)
        .expect("revised publication")
        .into_module();
    let HirTypeKind::Reference(second_reference) = resolved_type(&second_module, second_id).kind()
    else {
        panic!("fixture must remain a reference");
    };
    let Some(HirTypeRegion::Elided(second_region)) = second_reference.region() else {
        panic!("fixture must retain elision");
    };
    assert_eq!(second_region.key(), first_region.key());
    assert!(
        second_module
            .slots()
            .contains_key_only_synthetic(second_region.key())
    );
    assert_eq!(second_module.slots().key_only_synthetic_keys().count(), 1);

    assert_ne!(initial.document().identity(), revised.document().identity());
    let (revised_whole_range, revised_elision_offset) =
        type_source_positions(&second_module, second_id, &revised_attached);
    assert_eq!(
        revised_whole_range.start(),
        initial_whole_range.start() + INSERTED_TRIVIA.len()
    );
    assert_eq!(
        revised_whole_range.end(),
        initial_whole_range.end() + INSERTED_TRIVIA.len()
    );
    assert_eq!(
        revised_elision_offset,
        initial_elision_offset + INSERTED_TRIVIA.len()
    );
}

#[test]
fn dropped_elided_lowering_reuses_unpublished_ids_without_publishing_state() {
    let parsed = parsed_type("drop-rollback", "&Value");
    let attached = attached_type(&parsed);
    let mut database = HirDatabase::try_new().expect("HIR database");

    let mut dropped = stage(&database, &parsed);
    let dropped_scope = allocate_module_scope(&mut dropped, &parsed);
    let dropped_type = dropped
        .lower_attached_type(&attached, dropped_scope)
        .expect("dropped type lowering");
    drop(dropped);
    assert!(database.current(&module_key(&parsed)).is_none());

    let mut replacement = stage(&database, &parsed);
    let replacement_scope = allocate_module_scope(&mut replacement, &parsed);
    let replacement_type = replacement
        .lower_attached_type(&attached, replacement_scope)
        .expect("replacement type lowering");
    assert_eq!(replacement_scope, dropped_scope);
    assert_eq!(replacement_type, dropped_type);
    let module = replacement
        .finish(&mut database)
        .expect("replacement publication")
        .into_module();
    assert_eq!(module.slots().key_only_synthetic_keys().count(), 1);
}

#[test]
fn stale_attached_type_and_foreign_scope_poison_the_whole_transaction() {
    let (name, document) = alias_document("stale-type", "Value");
    let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("initial source");
    let revised = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial
                    .document()
                    .span(SourceRange::new(0, 0))
                    .expect("edit insertion"),
                " ",
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("revised source");
    let initial_attached = attached_type(&initial);
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut stale = stage(&database, &revised);
    let stale_scope = allocate_module_scope(&mut stale, &revised);
    assert!(matches!(
        stale.lower_attached_type(&initial_attached, stale_scope),
        Err(HirLowerFailure::StaleSource { .. })
    ));
    assert!(stale.finish(&mut database).is_err());
    assert!(database.current(&module_key(&revised)).is_none());

    let parsed = parsed_type("foreign-scope", "Value");
    let attached = attached_type(&parsed);
    let foreign_database = HirDatabase::try_new().expect("foreign database");
    let mut foreign_transaction = stage(&foreign_database, &parsed);
    let foreign_scope = allocate_module_scope(&mut foreign_transaction, &parsed);
    let mut local = stage(&database, &parsed);
    assert!(matches!(
        local.lower_attached_type(&attached, foreign_scope),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidArenaCommit
        ))
    ));
    assert!(local.finish(&mut database).is_err());
    assert!(database.current(&module_key(&parsed)).is_none());
}

fn path_with_segments(count: usize, segment: &str) -> String {
    std::iter::repeat_n(segment, count)
        .collect::<Vec<_>>()
        .join(".")
}

fn assert_lowering_limit(document_id: &str, type_source: &str, expected: HirLimit) {
    let parsed = parsed_type(document_id, type_source);
    let attached = attached_type(&parsed);
    assert_eq!(attached.family(), AttachedTypeFamily::Path);
    assert_attached_lowering_limit(&parsed, &attached, expected);
}

fn assert_attached_lowering_limit(
    parsed: &ParsedSource,
    attached: &AttachedTypeRefNode,
    expected: HirLimit,
) {
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, parsed);
    let scope = allocate_module_scope(&mut transaction, parsed);
    let error = transaction
        .lower_attached_type(attached, scope)
        .expect_err("one-over limit must fail");
    let HirLowerFailure::Limit(error) = error else {
        panic!("expected {expected:?} limit, got {error:?}");
    };
    assert_eq!(error.limit(), expected);
    assert_eq!(error.maximum(), expected.maximum());
    assert!(error.observed() > error.maximum());
    assert!(transaction.finish(&mut database).is_err());
    assert!(database.current(&module_key(parsed)).is_none());
}

#[test]
fn path_limits_accept_exact_values_and_rollback_one_over() {
    let exact_segments = path_with_segments(HirLimit::PathSegments.maximum(), "S");
    let exact_parsed = parsed_type("path-segments-exact", &exact_segments);
    let (module, owner) = lower_and_publish(&exact_parsed);
    let HirTypeKind::Path(path) = resolved_type(&module, owner).kind() else {
        panic!("exact path fixture must remain a path");
    };
    assert_eq!(path.segments().len(), HirLimit::PathSegments.maximum());
    assert_lowering_limit(
        "path-segments-one-over",
        &path_with_segments(HirLimit::PathSegments.maximum() + 1, "S"),
        HirLimit::PathSegments,
    );

    let exact_name = format!("N{}", "a".repeat(HirLimit::NameBytes.maximum() - 1));
    let exact_parsed = parsed_type("name-bytes-exact", &exact_name);
    let (module, owner) = lower_and_publish(&exact_parsed);
    assert!(matches!(
        resolved_type(&module, owner).kind(),
        HirTypeKind::Path(_)
    ));
    assert_lowering_limit(
        "name-bytes-one-over",
        &format!("N{}", "a".repeat(HirLimit::NameBytes.maximum())),
        HirLimit::NameBytes,
    );

    let semantic_segment = format!("N{}", "a".repeat(HirLimit::NameBytes.maximum() - 1));
    let exact_semantic = path_with_segments(
        HirLimit::PathSemanticBytes.maximum() / HirLimit::NameBytes.maximum(),
        &semantic_segment,
    );
    let exact_parsed = parsed_type("path-semantic-bytes-exact", &exact_semantic);
    let (module, owner) = lower_and_publish(&exact_parsed);
    assert!(matches!(
        resolved_type(&module, owner).kind(),
        HirTypeKind::Path(_)
    ));
    assert_lowering_limit(
        "path-semantic-bytes-one-over",
        &format!("{exact_semantic}.S"),
        HirLimit::PathSemanticBytes,
    );
}

#[test]
fn function_effect_names_accept_the_exact_byte_limit_and_rollback_one_over() {
    let maximum = HirLimit::NameBytes.maximum();
    let exact_effect = format!("E{}", "a".repeat(maximum - 1));
    let exact = format!("(Input, Context) -> Value effects {{ {exact_effect} }}");
    let parsed = parsed_alias_type("function-effect-name-exact", &exact);
    let attached = attached_alias_type(&parsed);
    assert_eq!(attached.family(), AttachedTypeFamily::Function);
    let (module, owner) = lower_attached_and_publish(&parsed, &attached);
    let HirTypeKind::Function(function) = resolved_type(&module, owner).kind() else {
        panic!("exact effect fixture must remain a function type");
    };
    assert_eq!(
        function
            .effects()
            .expect("effect row")
            .effects()
            .first()
            .expect("one effect")
            .as_str(),
        exact_effect
    );

    let one_over_effect = format!("E{}", "a".repeat(maximum));
    let one_over = parsed_alias_type(
        "function-effect-name-one-over",
        &format!("(Input, Context) -> Value effects {{ {one_over_effect} }}"),
    );
    let attached = attached_alias_type(&one_over);
    assert_eq!(attached.family(), AttachedTypeFamily::Function);
    assert_attached_lowering_limit(&one_over, &attached, HirLimit::NameBytes);
}
