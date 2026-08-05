use std::sync::Arc;

use arcweft_id::DeclarationIdentityFamily;
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::attachment::source_file::SourceFileEntryNode;
use arcweft_lang_syntax::attachment::{
    AttachedCallableParameter, AttachedCallableParameterKind, TypedItemNode,
};
use arcweft_lang_syntax::incremental::{ParsedSource, SyntaxDatabase};
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceEdit, SourceName, SourceRange};

use crate::database::HirDatabase;
use crate::diagnostic::{HirDiagnostic, HirRecoveryPrimary};
use crate::expr::{HirCallArgument, HirCallValue, HirRecoveredName};
use crate::identity::{
    ExprId, HirLimit, HirTypedId, LocalGeneration, ScopeId, SyntheticKey, SyntheticOwner,
    SyntheticRole,
};
use crate::item::{
    HirActionDeclaration, HirCharacterAssignmentState, HirCharacterSurfaceAlias,
    HirDeclarationMemberKind, HirDeclarationMemberPoisonState, HirItem, HirItemIssue, HirItemKind,
    HirItemPoisonState, HirItemPrefix, HirModuleDeclaration, HirParameterKind, HirPublicIdOrigin,
    HirRequiredName, HirRetainedName, HirRetainedPublicId,
};
use crate::leaf::{HirName, HirPath, HirPathRoot, HirPathSegment, HirPathValue};
use crate::lower::{HirInvariantFailure, HirLowerFailure, HirModuleKey, LoweringRequest};
use crate::module::HirModule;
use crate::scope::{HirLocal, HirLocalKind, HirScope, HirScopeKind, HirScopeOwner};
use crate::slot::HirOrigin;
use crate::source_index::HirSourceSite;
use crate::symbol::CallablePackageId;

use super::super::StagedHirModuleTransaction;
use super::nominal::preflight_nominal_members;
use super::preflight_source_file_inventory;
use super::retained::preflight_character_members;

mod activity;
mod entry;
mod extern_capability;
mod function;
mod layer;
mod metric;
mod predicate;
mod proof;
mod resource;
mod source;
mod style;
mod style_freeze;
mod test_bench;
mod trait_impl;
mod view;

fn source_document(document_id: &str, name: &SourceName, source: &str) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(document_id).unwrap(),
            name.clone(),
            source,
        )
        .unwrap(),
    )
}

fn parse(document_id: &str, source: &str) -> ParsedSource {
    let name = SourceName::path("proof/source-file-items.arcw");
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    syntax
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(document_id, &name, source),
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap_or_else(|error| panic!("{document_id}: syntax parse failed: {error:?}"))
}

fn module_key(parsed: &ParsedSource) -> HirModuleKey {
    HirModuleKey::new(
        CallablePackageId::try_new("proof-source-file-item-tests").unwrap(),
        CanonicalModulePath::crate_root(),
        parsed.document().identity().id().clone(),
    )
}

fn stage<'source>(
    database: &HirDatabase,
    parsed: &'source ParsedSource,
    key: &HirModuleKey,
) -> StagedHirModuleTransaction<'source> {
    database
        .stage_final_hir(LoweringRequest::try_new(key.clone(), parsed).unwrap())
        .unwrap()
}

fn lower(database: &mut HirDatabase, parsed: &ParsedSource, key: &HirModuleKey) -> Arc<HirModule> {
    let tree = parsed.tree();
    let mut transaction = stage(database, parsed, key);
    transaction.lower_attached_source_file_items(&tree).unwrap();
    transaction.finish(database).unwrap().into_module()
}

fn resolve_item(module: &HirModule, ordinal: usize) -> &HirItem {
    let id = module.source_ordered_items()[ordinal];
    module.arenas().items().resolve(module.slots(), id).unwrap()
}

#[test]
fn retained_identity_acceptance_matrix_keeps_one_typed_public_id() {
    for (ordinal, (spelling, expected_origin)) in [
        ("character Alice {}\n", HirPublicIdOrigin::DerivedFromName),
        (
            "character @character.Alice Alice {}\n",
            HirPublicIdOrigin::Explicit,
        ),
        (
            "character @character:.Alice Alice {}\n",
            HirPublicIdOrigin::Explicit,
        ),
        ("character @.Alice Alice {}\n", HirPublicIdOrigin::Explicit),
    ]
    .into_iter()
    .enumerate()
    {
        let parsed = parse(
            &format!("arcweft-test://retained/identity-matrix-{ordinal}"),
            spelling,
        );
        assert!(parsed.diagnostics().is_empty(), "{spelling:?}");
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let module = lower(&mut database, &parsed, &key);
        let HirItemKind::Character(character) = resolve_item(&module, 0).kind() else {
            panic!("retained identity matrix must lower a Character item")
        };
        let public_id = character
            .header()
            .public_id()
            .resolved()
            .expect("accepted identity must retain a public ID");
        assert_eq!(public_id.as_str(), "character.Alice");
        assert_eq!(
            character.header().public_id().origin(),
            Some(expected_origin)
        );
    }

    for (ordinal, spelling) in [
        "character @view.Alice Alice {}\n",
        "character @view:.Alice Alice {}\n",
    ]
    .into_iter()
    .enumerate()
    {
        let parsed = parse(
            &format!("arcweft-test://retained/identity-matrix-wrong-family-{ordinal}"),
            spelling,
        );
        assert!(
            parsed
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == "syntax.declaration.wrong_family_id")
        );
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let module = lower(&mut database, &parsed, &key);
        let HirItemKind::Character(character) = resolve_item(&module, 0).kind() else {
            panic!("wrong-family identity matrix must retain the Character item")
        };
        assert_eq!(character.header().public_id().resolved(), None);
    }
}

fn action_callable_scope(module: &HirModule, ordinal: usize) -> ScopeId {
    let HirItemKind::Action(action) = resolve_item(module, ordinal).kind() else {
        panic!("source-ordered item {ordinal} must be an Action")
    };
    action.callable_scope()
}

fn path_spellings(path: &HirPath) -> Vec<&str> {
    path.segments()
        .iter()
        .map(|segment| match segment {
            HirPathSegment::Identifier(name) => name.as_str(),
            HirPathSegment::ProjectSymbol(symbol) => symbol.as_str(),
        })
        .collect()
}

fn resolved_path(path: &HirPathValue) -> &HirPath {
    path.as_resolved().expect("clean path projection")
}

fn assert_item_slot_whole(
    module: &HirModule,
    parsed: &ParsedSource,
    owner: crate::identity::ItemId,
) {
    let metadata = module.slots().resolve(owner).unwrap();
    let HirOrigin::Source(origin) = metadata.origin() else {
        panic!("authored item must retain a source-backed slot")
    };
    let attached = parsed
        .tree()
        .items()
        .unwrap()
        .into_iter()
        .find(|item| item.id() == origin.syntax())
        .expect("item slot syntax must resolve in the accepted ParsedSource");
    assert_eq!(
        metadata.source_site(),
        &HirSourceSite::Span(attached.source_span())
    );
}

fn assert_source_backed_child<I: HirTypedId>(module: &HirModule, owner: I) {
    let metadata = module.slots().resolve(owner).unwrap();
    assert!(matches!(metadata.origin(), HirOrigin::Source(_)));
    assert!(matches!(metadata.source_site(), HirSourceSite::Span(_)));
}

fn assert_item_owner_whole_recovery(module: &HirModule, owner: crate::identity::ItemId) {
    let recovery = module
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            HirDiagnostic::Recovery(diagnostic)
                if diagnostic.owner() == SyntheticOwner::Item(owner) =>
            {
                Some(diagnostic)
            }
            HirDiagnostic::Syntax(_) | HirDiagnostic::Recovery(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(recovery.len(), 1);
    assert_eq!(
        recovery[0].primary_role(),
        HirRecoveryPrimary::owner_whole(SyntheticOwner::Item(owner))
    );
    assert_eq!(
        recovery[0].primary(),
        module.slots().resolve(owner).unwrap().source_site()
    );
}

#[test]
fn predicate_and_proof_reject_function_only_default_and_rest_without_lowering_the_default() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-fixed-only-parameter-surface",
        concat!(
            "predicate fixed_only(defaulted: Int = 1, rest: ...Int) = true\n",
            "proof fixed_only(defaulted: Int = 1, rest: ...Int) = ()\n",
        ),
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "the shared typed parameter grammar must remain lossless: {:?}",
        parsed.diagnostics()
    );

    let mut default_syntax = Vec::new();
    {
        let mut record_surface = |parameters: &[AttachedCallableParameter]| {
            let [defaulted, rest] = parameters else {
                panic!("fixed-only callable fixture must retain two typed parameters")
            };
            assert!(matches!(
                defaulted.kind(),
                AttachedCallableParameterKind::Fixed
            ));
            let default = defaulted.default().expect("typed authored default");
            assert!(!default.has_recovery());
            assert!(!defaulted.has_recovery());
            assert!(matches!(
                rest.kind(),
                AttachedCallableParameterKind::Rest { .. }
            ));
            assert!(rest.default().is_none());
            assert!(!rest.has_recovery());
            default_syntax.push(default.value().id());
        };
        for item in parsed.tree().items().unwrap() {
            match item {
                TypedItemNode::Predicate(node) => {
                    let attached = node.semantics().unwrap();
                    record_surface(attached.parameter_group().parameters());
                }
                TypedItemNode::Proof(node) => {
                    let attached = node.semantics().unwrap();
                    record_surface(attached.parameter_group().parameters());
                }
                _ => {}
            }
        }
    }
    assert_eq!(default_syntax.len(), 2);

    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    for ordinal in 0..2 {
        let item = resolve_item(&module, ordinal);
        assert_eq!(
            item.state(),
            &HirItemPoisonState::Poisoned(HirItemIssue::MalformedHeader)
        );
        let parameters = match item.kind() {
            HirItemKind::Predicate(predicate) => predicate.parameters(),
            HirItemKind::Proof(proof) => proof.parameters(),
            _ => panic!("fixed-only callable fixture changed item family"),
        };
        let [defaulted, rest] = parameters else {
            panic!("fixed-only final HIR must retain two typed parameters")
        };
        for parameter in [defaulted, rest] {
            assert_eq!(parameter.kind(), HirParameterKind::Fixed);
            assert!(parameter.default().is_none());
            assert_source_backed_child(&module, parameter.pattern());
            assert_source_backed_child(&module, parameter.ty());
            for local in parameter.locals() {
                let metadata = module.slots().resolve(*local).unwrap();
                let HirOrigin::Synthetic(key) = metadata.origin() else {
                    panic!("pattern binding locals must retain synthetic semantic identity")
                };
                assert_eq!(key.owner(), SyntheticOwner::Pattern(parameter.pattern()));
                assert_eq!(key.role(), SyntheticRole::DestructuredBinding);
                assert_eq!(key.ordinal(), 0);
                assert!(matches!(
                    metadata.source_site(),
                    HirSourceSite::Insertion(_)
                ));

                let local = module
                    .arenas()
                    .locals()
                    .resolve(module.slots(), *local)
                    .unwrap();
                let pattern = module
                    .arenas()
                    .patterns()
                    .resolve(module.slots(), parameter.pattern())
                    .unwrap();
                assert_eq!(local.kind(), HirLocalKind::Parameter);
                assert_eq!(local.pattern(), Some(parameter.pattern()));
                assert_eq!(local.scope(), pattern.scope());
            }
        }
    }
    for syntax in default_syntax {
        assert_eq!(
            module.slots().prepared_source_owner::<ExprId>(syntax),
            None,
            "Predicate/Proof defaults must not acquire executable ExprIds"
        );
    }
}

#[test]
fn clean_nominal_items_publish_typed_payloads_inline_members_and_exact_sources() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-nominal-clean",
        concat!(
            "type Alias<T: Format> = Result<T, Error> where T: Clone\n",
            "struct Record<T> where T: Clone {\n",
            "    /// Stored value\n",
            "    value: T,\n",
            "}\n",
            "enum Choice<T> where T: Clone {\n",
            "    /// No value\n",
            "    Empty,\n",
            "    Value T,\n",
            "}\n",
        ),
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    assert_eq!(module.source_ordered_items().len(), 3);

    let alias_owner = module.source_ordered_items()[0];
    let alias_item = resolve_item(&module, 0);
    assert_eq!(alias_item.state(), &HirItemPoisonState::Clean);
    let HirItemKind::TypeAlias(alias) = alias_item.kind() else {
        panic!("final TypeAlias item")
    };
    assert!(matches!(
        alias.name(),
        HirRequiredName::Resolved(name) if name.as_str() == "Alias"
    ));
    assert_eq!(alias.generic_parameters().len(), 1);
    assert_eq!(alias.generic_parameters()[0].bounds().len(), 1);
    assert_eq!(alias.where_predicates().len(), 1);
    assert_eq!(alias.where_predicates()[0].bounds().len(), 1);
    assert!(
        !module
            .slots()
            .resolve(alias.target())
            .unwrap()
            .is_poisoned()
    );
    assert!(alias_item.members().is_empty());
    assert!(module.declaration_members().arena(alias_owner).is_none());
    assert_item_slot_whole(&module, &parsed, alias_owner);
    assert_source_backed_child(&module, alias.target());

    let record_owner = module.source_ordered_items()[1];
    let record_item = resolve_item(&module, 1);
    assert_eq!(record_item.state(), &HirItemPoisonState::Clean);
    let HirItemKind::Struct(record) = record_item.kind() else {
        panic!("final Struct item")
    };
    assert_eq!(record.fields().len(), 1);
    assert_eq!(
        record.fields()[0]
            .name()
            .resolved()
            .expect("field name")
            .as_str(),
        "value"
    );
    assert_eq!(
        record.fields()[0]
            .documentation()
            .expect("field documentation")
            .markdown(),
        "Stored value"
    );
    assert!(
        !module
            .slots()
            .resolve(record.fields()[0].ty())
            .unwrap()
            .is_poisoned()
    );
    assert!(record_item.members().is_empty());
    assert!(module.declaration_members().arena(record_owner).is_none());
    assert_item_slot_whole(&module, &parsed, record_owner);
    assert_source_backed_child(&module, record.fields()[0].ty());

    let choice_owner = module.source_ordered_items()[2];
    let choice_item = resolve_item(&module, 2);
    assert_eq!(choice_item.state(), &HirItemPoisonState::Clean);
    let HirItemKind::Enum(choice) = choice_item.kind() else {
        panic!("final Enum item")
    };
    assert_eq!(choice.variants().len(), 2);
    assert!(choice.variants()[0].payload().is_none());
    assert!(choice.variants()[1].payload().is_some());
    assert_eq!(
        choice.variants()[0]
            .documentation()
            .expect("variant documentation")
            .markdown(),
        "No value"
    );
    assert!(choice_item.members().is_empty());
    assert!(module.declaration_members().arena(choice_owner).is_none());
    assert_item_slot_whole(&module, &parsed, choice_owner);
    assert_source_backed_child(
        &module,
        choice.variants()[1].payload().expect("typed enum payload"),
    );
}

#[test]
fn clean_nominal_attribute_retains_typed_path_arguments_and_source_owned_children() {
    let attribute_source = "#[tool.link(first, level = .soft, rest...)]";
    let source = format!("{attribute_source}\npub struct Record {{}}\n");
    let parsed = parse(
        "arcweft-test://proof/final-hir-nominal-attribute-clean",
        &source,
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let owner = module.source_ordered_items()[0];
    let item = resolve_item(&module, 0);
    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    assert_eq!(
        item.prefix().visibility(),
        Some(crate::item::HirVisibility::Public)
    );
    let [attribute] = item.prefix().attributes() else {
        panic!("one retained nominal attribute")
    };
    assert_eq!(attribute.path().root(), HirPathRoot::ImplicitCrate);
    assert_eq!(path_spellings(attribute.path()), ["tool", "link"]);
    let [
        HirCallArgument::Positional {
            value: HirCallValue::Present { .. },
        },
        HirCallArgument::Named {
            name: HirRecoveredName::Valid(name),
            value: HirCallValue::Present { .. },
            ..
        },
        HirCallArgument::Spread {
            value: HirCallValue::Present { .. },
            ..
        },
    ] = attribute.arguments()
    else {
        panic!("typed positional, named, and spread arguments")
    };
    assert_eq!(name.as_str(), "level");

    let argument_owners = attribute
        .arguments()
        .iter()
        .map(HirCallArgument::value)
        .collect::<Vec<ExprId>>();
    assert_eq!(argument_owners.len(), 3);
    assert_eq!(
        module
            .arenas()
            .expressions()
            .try_iter(module.slots())
            .unwrap()
            .count(),
        argument_owners.len(),
        "attributes allocate only their authored value expressions, never a Call/callee owner"
    );
    assert!(argument_owners.iter().all(|owner| matches!(
        module.slots().resolve(*owner).unwrap().origin(),
        HirOrigin::Source(_)
    )));
    for argument in argument_owners {
        assert_source_backed_child(&module, argument);
    }
    assert_item_slot_whole(&module, &parsed, owner);
}

#[test]
fn recovered_nominal_attribute_is_omitted_without_shifting_the_next_source_owner() {
    let invalid = "#[first(name = one, name = two)]";
    let retained = "#[second(value)]";
    let source = format!("{invalid}\n{retained}\nstruct Record {{}}\n");
    let parsed = parse(
        "arcweft-test://proof/final-hir-nominal-attribute-recovery",
        &source,
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let owner = module.source_ordered_items()[0];
    let item = resolve_item(&module, 0);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::Recovery)
    );
    let [attribute] = item.prefix().attributes() else {
        panic!("only the clean second attribute is retained")
    };
    assert_eq!(path_spellings(attribute.path()), ["second"]);
    assert_eq!(
        module
            .arenas()
            .expressions()
            .try_iter(module.slots())
            .unwrap()
            .count(),
        3,
        "both rejected argument values and the retained value remain source-owned expressions"
    );
    let expression_owners = module
        .arenas()
        .expressions()
        .try_iter(module.slots())
        .unwrap()
        .map(|(owner, _)| owner)
        .collect::<Vec<_>>();
    for expression in expression_owners {
        assert_source_backed_child(&module, expression);
    }
    assert_item_slot_whole(&module, &parsed, owner);
    assert_item_owner_whole_recovery(&module, owner);
}

#[test]
fn nominal_attribute_recovery_matrix_omits_invalid_payloads_without_fabricated_call_owners() {
    let cases = [
        ("missing-value", "#[broken(name =)]", 0),
        ("positional-after-named", "#[broken(name = one, two)]", 2),
        ("spread-not-last", "#[broken(rest..., tail)]", 2),
        ("duplicate-name", "#[broken(name = one, name = two)]", 2),
        ("poisoned-child", "#[broken(@)]", 1),
    ];

    for (case, attribute, expected_expressions) in cases {
        let source = format!("{attribute}\nstruct Record {{}}\n");
        let parsed = parse(
            &format!("arcweft-test://proof/final-hir-nominal-attribute-{case}"),
            &source,
        );
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let module = lower(&mut database, &parsed, &key);
        let owner = module.source_ordered_items()[0];
        let item = resolve_item(&module, 0);
        assert_eq!(
            item.state(),
            &HirItemPoisonState::Poisoned(HirItemIssue::Recovery),
            "{case}: {:?}",
            parsed.diagnostics(),
        );
        assert!(item.prefix().attributes().is_empty(), "{case}");
        assert_eq!(
            module
                .arenas()
                .expressions()
                .try_iter(module.slots())
                .unwrap()
                .count(),
            expected_expressions,
            "{case}: only authored value expressions may own ExprIds",
        );
        let expression_owners = module
            .arenas()
            .expressions()
            .try_iter(module.slots())
            .unwrap()
            .map(|(owner, _)| owner)
            .collect::<Vec<_>>();
        for expression in expression_owners {
            assert_source_backed_child(&module, expression);
        }
        assert_item_slot_whole(&module, &parsed, owner);
        assert_item_owner_whole_recovery(&module, owner);
    }
}

#[test]
fn nominal_prefix_recovery_reports_the_source_first_attribute_before_visibility() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-nominal-prefix-recovery-order",
        "#[broken(name = one, name = two)]\npub(other) struct Record {}\n",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let item = resolve_item(&module, 0);

    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::Recovery)
    );
    assert!(item.prefix().attributes().is_empty());
    assert!(item.prefix().visibility().is_none());
    let owner = module.source_ordered_items()[0];
    assert_item_slot_whole(&module, &parsed, owner);
    assert_item_owner_whole_recovery(&module, owner);
}

#[test]
fn nominal_member_preflight_accepts_exact_and_rejects_one_over() {
    let maximum = HirLimit::DeclarationMembers.maximum();
    assert!(preflight_nominal_members(maximum).is_ok());
    let Err(HirLowerFailure::Limit(error)) = preflight_nominal_members(maximum + 1) else {
        panic!("one-over nominal member inventory must fail before child lowering")
    };
    assert_eq!(error.limit(), HirLimit::DeclarationMembers);
    assert_eq!(error.observed(), maximum + 1);
    assert_eq!(error.maximum(), maximum);
}

#[test]
fn nominal_recovery_matrix_keeps_recognized_families_and_owner_whole_diagnostics() {
    let cases = [
        (
            "alias-missing-name",
            "type = Value\n",
            HirItemIssue::MissingName,
            "alias",
        ),
        (
            "alias-missing-assignment",
            "type Alias Value\n",
            HirItemIssue::Recovery,
            "alias",
        ),
        (
            "alias-missing-target",
            "type Alias =\n",
            HirItemIssue::MissingType,
            "alias",
        ),
        (
            "alias-missing-where-bound",
            "type Alias = Value where T:\n",
            HirItemIssue::Recovery,
            "alias",
        ),
        (
            "struct-missing-name",
            "struct {}\n",
            HirItemIssue::MissingName,
            "struct",
        ),
        (
            "struct-missing-body",
            "struct Record\n",
            HirItemIssue::MissingBody,
            "struct",
        ),
        (
            "struct-missing-field-colon",
            "struct Record { value Value }\n",
            HirItemIssue::InvalidMember,
            "struct",
        ),
        (
            "struct-missing-field-type",
            "struct Record { value: }\n",
            HirItemIssue::InvalidMember,
            "struct",
        ),
        (
            "struct-unclosed-body",
            "struct Record { value: Value\n",
            HirItemIssue::Recovery,
            "struct",
        ),
        (
            "enum-missing-name",
            "enum {}\n",
            HirItemIssue::MissingName,
            "enum",
        ),
        (
            "enum-missing-body",
            "enum Choice\n",
            HirItemIssue::MissingBody,
            "enum",
        ),
        (
            "enum-invalid-variant-name",
            "enum Choice { 42 }\n",
            HirItemIssue::InvalidMember,
            "enum",
        ),
        (
            "enum-invalid-payload",
            "enum Choice { Value ? }\n",
            HirItemIssue::InvalidMember,
            "enum",
        ),
        (
            "enum-unclosed-body",
            "enum Choice { Value Data\n",
            HirItemIssue::Recovery,
            "enum",
        ),
    ];

    for (case, source, expected_issue, family) in cases {
        let parsed = parse(
            &format!("arcweft-test://proof/final-hir-nominal-{case}"),
            source,
        );
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let tree = parsed.tree();
        let mut transaction = stage(&database, &parsed, &key);
        transaction
            .lower_attached_source_file_items(&tree)
            .unwrap_or_else(|error| panic!("{case}: lowering failed: {error:?}"));
        let module = transaction
            .finish(&mut database)
            .unwrap_or_else(|error| panic!("{case}: publication failed: {error:?}"))
            .into_module();
        let owner = module.source_ordered_items()[0];
        let item = resolve_item(&module, 0);
        assert!(
            matches!(
                (family, item.kind()),
                ("alias", HirItemKind::TypeAlias(_))
                    | ("struct", HirItemKind::Struct(_))
                    | ("enum", HirItemKind::Enum(_))
            ),
            "{case}: {:?}",
            item.kind(),
        );
        assert_eq!(
            item.state(),
            &HirItemPoisonState::Poisoned(expected_issue),
            "{case}",
        );
        let recovery = module
            .diagnostics()
            .iter()
            .filter_map(|diagnostic| match diagnostic {
                HirDiagnostic::Recovery(diagnostic)
                    if diagnostic.owner() == SyntheticOwner::Item(owner) =>
                {
                    Some(diagnostic)
                }
                HirDiagnostic::Syntax(_) | HirDiagnostic::Recovery(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(recovery.len(), 1, "{case}");
        assert_eq!(
            recovery[0].primary_role(),
            HirRecoveryPrimary::owner_whole(SyntheticOwner::Item(owner)),
            "{case}",
        );
        assert_eq!(
            recovery[0].primary(),
            module.slots().resolve(owner).unwrap().source_site(),
            "{case}",
        );
    }
}

#[test]
fn clean_signals_publish_retained_headers_types_and_slot_owned_whole_sources() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-signal-clean",
        concat!(
            "/// Current state\n",
            "pub signal @signal.current Current: Watch<Ref<Flow>>\n",
            "signal Events: Stream<GameEvent, EventError>\n",
            "signal Sampled: Sample<f32>\n",
        ),
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    assert_eq!(module.source_ordered_items().len(), 3);
    assert_eq!(
        module
            .arenas()
            .expressions()
            .try_iter(module.slots())
            .unwrap()
            .count(),
        0,
        "Signal declarations never fabricate initializer expressions"
    );

    for (ordinal, expected_name) in ["Current", "Events", "Sampled"].into_iter().enumerate() {
        let owner = module.source_ordered_items()[ordinal];
        let item = resolve_item(&module, ordinal);
        assert_eq!(item.state(), &HirItemPoisonState::Clean);
        assert!(item.members().is_empty());
        assert!(module.declaration_members().arena(owner).is_none());
        let HirItemKind::Signal(signal) = item.kind() else {
            panic!("final Signal item")
        };
        assert_eq!(signal.header().family(), DeclarationIdentityFamily::Signal);
        assert!(matches!(
            signal.header().name(),
            HirRetainedName::Resolved(name) if name.as_str() == expected_name
        ));
        assert!(
            !module
                .slots()
                .resolve(signal.observable_type())
                .unwrap()
                .is_poisoned()
        );
        assert_item_slot_whole(&module, &parsed, owner);
        assert_source_backed_child(&module, signal.observable_type());
    }

    let HirItemKind::Signal(explicit) = resolve_item(&module, 0).kind() else {
        panic!("first Signal")
    };
    assert!(matches!(
        explicit.header().public_id(),
        HirRetainedPublicId::Resolved {
            value,
            origin: HirPublicIdOrigin::Explicit,
        } if value.as_str() == "signal.current"
    ));
    let HirItemKind::Signal(derived) = resolve_item(&module, 1).kind() else {
        panic!("second Signal")
    };
    assert!(matches!(
        derived.header().public_id(),
        HirRetainedPublicId::Resolved {
            value,
            origin: HirPublicIdOrigin::DerivedFromName,
        } if value.as_str() == "signal.Events"
    ));
}

#[test]
fn signal_recovery_matrix_preserves_family_primary_and_zero_expression_owners() {
    let cases = [
        (
            "wrong-family-id",
            "signal @view.current Current: Watch<I32>\n",
            HirItemIssue::MalformedHeader,
        ),
        (
            "missing-name",
            "signal : Watch<I32>\n",
            HirItemIssue::MissingName,
        ),
        (
            "missing-colon",
            "signal Current Watch<I32>\n",
            HirItemIssue::Recovery,
        ),
        (
            "missing-type",
            "signal Current:\n",
            HirItemIssue::MissingType,
        ),
        (
            "malformed-type",
            "signal Broken: Stream<Event = source\n",
            HirItemIssue::Recovery,
        ),
        (
            "forbidden-initializer",
            "signal Current: Watch<I32> = source\n",
            HirItemIssue::Recovery,
        ),
    ];

    for (case, source, expected_issue) in cases {
        let parsed = parse(
            &format!("arcweft-test://proof/final-hir-signal-{case}"),
            source,
        );
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let module = lower(&mut database, &parsed, &key);
        let owner = module.source_ordered_items()[0];
        let item = resolve_item(&module, 0);
        assert!(matches!(item.kind(), HirItemKind::Signal(_)), "{case}");
        assert_eq!(
            item.state(),
            &HirItemPoisonState::Poisoned(expected_issue),
            "{case}"
        );
        assert_eq!(
            module
                .arenas()
                .expressions()
                .try_iter(module.slots())
                .unwrap()
                .count(),
            0,
            "{case}: forbidden/recovered Signal syntax is not an ExprId owner"
        );
        assert_item_slot_whole(&module, &parsed, owner);
        assert_item_owner_whole_recovery(&module, owner);
    }
}

#[test]
fn clean_actions_publish_one_item_owned_callable_scope_and_ordered_parameters() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-action-clean",
        concat!(
            "pub action feedback_submit(value: Feedback, count: Count);\n",
            "action Continue()\n",
        ),
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    assert_eq!(module.source_ordered_items().len(), 2);
    assert_eq!(
        module
            .arenas()
            .expressions()
            .try_iter(module.slots())
            .unwrap()
            .count(),
        0,
        "Action signatures own no expression or synthetic Unit payload"
    );

    let mut action_scopes = Vec::new();
    for (ordinal, expected_parameters) in [2_usize, 0].into_iter().enumerate() {
        let owner = module.source_ordered_items()[ordinal];
        let item = resolve_item(&module, ordinal);
        assert_eq!(item.state(), &HirItemPoisonState::Clean);
        assert!(item.members().is_empty());
        assert!(module.declaration_members().arena(owner).is_none());
        let HirItemKind::Action(action) = item.kind() else {
            panic!("final Action item")
        };
        assert_eq!(action.header().family(), DeclarationIdentityFamily::Action);
        assert_eq!(action.parameters().len(), expected_parameters);
        let callable_scope = action.callable_scope();
        action_scopes.push(callable_scope);
        let scope = module
            .arenas()
            .scopes()
            .resolve(module.slots(), callable_scope)
            .unwrap();
        assert_eq!(scope.kind(), HirScopeKind::Callable);
        assert_eq!(scope.parent(), Some(item.scope()));
        assert_eq!(scope.owner(), &HirScopeOwner::Item(owner));
        assert!(scope.children().is_empty());

        let item_metadata = module.slots().resolve(owner).unwrap();
        let scope_metadata = module.slots().resolve(callable_scope).unwrap();
        let (HirOrigin::Source(item_origin), HirOrigin::Source(scope_origin)) =
            (item_metadata.origin(), scope_metadata.origin())
        else {
            panic!("Action item and callable scope must share the attached item owner")
        };
        assert_eq!(scope_origin.syntax(), item_origin.syntax());
        assert_eq!(scope_metadata.source_site(), item_metadata.source_site());

        let flattened = action
            .parameters()
            .iter()
            .flat_map(|parameter| parameter.locals().iter().copied())
            .collect::<Vec<_>>();
        assert_eq!(scope.locals(), flattened);
        for parameter in action.parameters() {
            assert!(parameter.default().is_none());
            assert_source_backed_child(&module, parameter.pattern());
            assert_source_backed_child(&module, parameter.ty());
            let pattern = module
                .arenas()
                .patterns()
                .resolve(module.slots(), parameter.pattern())
                .unwrap();
            let ty = module
                .arenas()
                .types()
                .resolve(module.slots(), parameter.ty())
                .unwrap();
            assert_eq!(pattern.scope(), callable_scope);
            assert_eq!(ty.scope(), callable_scope);
            assert_eq!(parameter.locals().len(), 1);
            let local = module
                .arenas()
                .locals()
                .resolve(module.slots(), parameter.locals()[0])
                .unwrap();
            assert_eq!(local.scope(), callable_scope);
            assert_eq!(local.kind(), HirLocalKind::Parameter);
            assert_eq!(local.pattern(), Some(parameter.pattern()));
            assert!(local.annotation().is_none());
        }
        assert_item_slot_whole(&module, &parsed, owner);
    }

    let root = resolve_item(&module, 0).scope();
    let root_scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), root)
        .unwrap();
    assert_eq!(root_scope.children(), action_scopes);
}

#[test]
fn action_recovery_matrix_retains_family_and_never_lowers_forbidden_defaults() {
    let cases = [
        ("missing-group", "action Missing\n", HirItemIssue::Recovery),
        (
            "invalid-binding",
            "action Invalid((left, right): Pair)\n",
            HirItemIssue::InvalidMember,
        ),
        (
            "missing-colon",
            "action Untyped(value)\n",
            HirItemIssue::MissingType,
        ),
        (
            "missing-type",
            "action Empty(value:)\n",
            HirItemIssue::MissingType,
        ),
        (
            "malformed-type",
            "action Broken(value: @)\n",
            HirItemIssue::InvalidMember,
        ),
        (
            "forbidden-default",
            "action Defaulted(value: String = make(value))\n",
            HirItemIssue::InvalidMember,
        ),
        (
            "return-tail",
            "action Query() -> String\n",
            HirItemIssue::Recovery,
        ),
        (
            "body-tail",
            "action Run() { return }\n",
            HirItemIssue::Recovery,
        ),
        (
            "effect-tail",
            "action Tail() effects { ui.write }\n",
            HirItemIssue::Recovery,
        ),
    ];

    for (case, source, expected_issue) in cases {
        let parsed = parse(
            &format!("arcweft-test://proof/final-hir-action-{case}"),
            source,
        );
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let module = lower(&mut database, &parsed, &key);
        let owner = module.source_ordered_items()[0];
        let item = resolve_item(&module, 0);
        let HirItemKind::Action(action) = item.kind() else {
            panic!("{case}: malformed Action must retain its typed family")
        };
        assert_eq!(
            item.state(),
            &HirItemPoisonState::Poisoned(expected_issue),
            "{case}: {:?}",
            parsed.diagnostics(),
        );
        assert!(
            action
                .parameters()
                .iter()
                .all(|parameter| parameter.default().is_none())
        );
        assert_eq!(
            module
                .arenas()
                .expressions()
                .try_iter(module.slots())
                .unwrap()
                .count(),
            0,
            "{case}: forbidden Action syntax must not allocate ExprIds",
        );
        assert_item_slot_whole(&module, &parsed, owner);
        assert_item_owner_whole_recovery(&module, owner);
    }
}

#[derive(Clone, Copy)]
enum ActionLocalTamper {
    Name(&'static str),
    Generation(LocalGeneration),
    AnnotationFromParameterType,
    Mutable(bool),
}

fn assert_action_local_freeze_rejects(case: &str, tamper: ActionLocalTamper) {
    let parsed = parse(
        &format!("arcweft-test://proof/final-hir-action-local-{case}"),
        "action Submit(value: Value, value: Other)\n",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    transaction
        .lower_attached_source_file_items(&parsed.tree())
        .unwrap();
    let owner = transaction.source_ordered_items[0];
    let (local, parameter_type, payload) = {
        let (slots, arenas) = transaction.storage_mut();
        let (local, parameter_type) = {
            let item = arenas.items().resolve_staged(slots, owner).unwrap();
            let HirItemKind::Action(action) = item.kind() else {
                panic!("staged Action item")
            };
            let parameter = &action.parameters()[1];
            (parameter.locals()[0], parameter.ty())
        };
        let payload = arenas
            .locals()
            .resolve_staged(slots, local)
            .unwrap()
            .clone();
        (local, parameter_type, payload)
    };
    let mut name = payload.name().clone();
    let mut generation = payload.generation();
    let mut annotation = payload.annotation();
    let mut mutable = payload.is_mutable_binding();
    match tamper {
        ActionLocalTamper::Name(replacement) => {
            name = HirName::try_new(replacement.into()).unwrap();
        }
        ActionLocalTamper::Generation(replacement) => generation = replacement,
        ActionLocalTamper::AnnotationFromParameterType => annotation = Some(parameter_type),
        ActionLocalTamper::Mutable(replacement) => mutable = replacement,
    }
    let replacement = HirLocal::try_new(
        payload.scope(),
        payload.kind(),
        name,
        generation,
        payload.pattern(),
        annotation,
        mutable,
        payload.is_poisoned(),
    )
    .unwrap();
    let (slots, arenas) = transaction.storage_mut();
    arenas
        .locals()
        .revise_finalized(slots, local, replacement)
        .unwrap();
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&key).is_none());
}

#[test]
fn action_freeze_rejects_exact_parameter_local_payload_tampering() {
    assert_action_local_freeze_rejects("name", ActionLocalTamper::Name("renamed"));
    assert_action_local_freeze_rejects(
        "generation",
        ActionLocalTamper::Generation(LocalGeneration::FIRST),
    );
    assert_action_local_freeze_rejects(
        "annotation",
        ActionLocalTamper::AnnotationFromParameterType,
    );
    assert_action_local_freeze_rejects("mutability", ActionLocalTamper::Mutable(true));
}

#[test]
fn duplicate_action_parameter_names_remain_hir_clean_for_semantic_rejection() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-action-duplicate-parameter",
        "action Submit(value: First, value: Second)\n",
    );
    assert!(parsed.diagnostics().is_empty());
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let item = resolve_item(&module, 0);
    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    let HirItemKind::Action(action) = item.kind() else {
        panic!("final Action item")
    };
    let [first, second] = action.parameters() else {
        panic!("two retained parameters")
    };
    let first = module
        .arenas()
        .locals()
        .resolve(module.slots(), first.locals()[0])
        .unwrap();
    let second = module
        .arenas()
        .locals()
        .resolve(module.slots(), second.locals()[0])
        .unwrap();
    assert_eq!(first.name().as_str(), "value");
    assert_eq!(second.name().as_str(), "value");
    assert_ne!(first.generation(), second.generation());
}

#[test]
fn action_exact_fixed_parameter_budget_lowers_without_a_second_hir_limit() {
    let parameters = (0..256)
        .map(|ordinal| format!("p{ordinal}: T{ordinal}"))
        .collect::<Vec<_>>()
        .join(", ");
    let parsed = parse(
        "arcweft-test://proof/final-hir-action-fixed-parameter-limit",
        &format!("action Exact({parameters})\n"),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let HirItemKind::Action(action) = resolve_item(&module, 0).kind() else {
        panic!("final Action item")
    };
    assert_eq!(action.parameters().len(), 256);
    assert_eq!(
        module
            .arenas()
            .scopes()
            .resolve(module.slots(), action.callable_scope())
            .unwrap()
            .locals()
            .len(),
        256
    );
}

#[test]
fn action_freeze_rejects_parameter_order_scope_membership_and_scope_kind_tampering() {
    {
        let parsed = parse(
            "arcweft-test://proof/final-hir-action-parameter-order-tamper",
            "action Ordered(first: First, second: Second)\n",
        );
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let mut transaction = stage(&database, &parsed, &key);
        transaction
            .lower_attached_source_file_items(&parsed.tree())
            .unwrap();
        let owner = transaction.source_ordered_items[0];
        {
            let (slots, arenas) = transaction.storage_mut();
            let original = arenas.items().resolve_staged(slots, owner).unwrap().clone();
            let HirItemKind::Action(action) = original.kind() else {
                panic!("final Action item")
            };
            let mut parameters = action.parameters().to_vec();
            parameters.reverse();
            let action = HirActionDeclaration::try_new(
                action.header().clone(),
                action.callable_scope(),
                parameters.into_boxed_slice(),
            )
            .unwrap();
            let replacement = HirItem::try_new_with_state(
                owner,
                original.scope(),
                original.prefix().clone(),
                HirItemKind::Action(action),
                Box::new([]),
                *original.state(),
            )
            .unwrap();
            arenas
                .items()
                .revise_finalized(slots, owner, replacement)
                .unwrap();
        }
        assert!(matches!(
            transaction.finish(&mut database),
            Err(HirLowerFailure::Invariant(
                HirInvariantFailure::InvalidSourceIndex
            ))
        ));
        assert!(database.current(&key).is_none());
    }

    {
        let parsed = parse(
            "arcweft-test://proof/final-hir-action-root-membership-tamper",
            "action Ordered(value: Value)\n",
        );
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let mut transaction = stage(&database, &parsed, &key);
        let root_scope = transaction
            .lower_attached_source_file_items(&parsed.tree())
            .unwrap();
        {
            let (slots, arenas) = transaction.storage_mut();
            let original = arenas
                .scopes()
                .resolve_staged(slots, root_scope)
                .unwrap()
                .clone();
            let replacement = original
                .try_with_members(Box::new([]), original.locals().to_vec().into_boxed_slice())
                .unwrap();
            arenas
                .scopes()
                .revise_finalized(slots, root_scope, replacement)
                .unwrap();
        }
        assert!(matches!(
            transaction.finish(&mut database),
            Err(HirLowerFailure::Invariant(
                HirInvariantFailure::InvalidModuleArenaSnapshot
            ))
        ));
        assert!(database.current(&key).is_none());
    }

    {
        let parsed = parse(
            "arcweft-test://proof/final-hir-action-scope-kind-tamper",
            "action Ordered(value: Value)\n",
        );
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let mut transaction = stage(&database, &parsed, &key);
        transaction
            .lower_attached_source_file_items(&parsed.tree())
            .unwrap();
        let owner = transaction.source_ordered_items[0];
        let callable_scope = {
            let (slots, arenas) = transaction.storage_mut();
            let item = arenas.items().resolve_staged(slots, owner).unwrap();
            let HirItemKind::Action(action) = item.kind() else {
                panic!("final Action item")
            };
            action.callable_scope()
        };
        {
            let (slots, arenas) = transaction.storage_mut();
            let original = arenas
                .scopes()
                .resolve_staged(slots, callable_scope)
                .unwrap()
                .clone();
            let replacement = HirScope::try_new(
                callable_scope.module(),
                HirScopeKind::Block,
                original.parent(),
                *original.owner(),
                original.children().to_vec().into_boxed_slice(),
                original.locals().to_vec().into_boxed_slice(),
            )
            .unwrap();
            arenas
                .scopes()
                .revise_finalized(slots, callable_scope, replacement)
                .unwrap();
        }
        assert!(matches!(
            transaction.finish(&mut database),
            Err(HirLowerFailure::Invariant(
                HirInvariantFailure::InvalidSourceIndex
            ))
        ));
        assert!(database.current(&key).is_none());
    }
}

#[test]
fn clean_character_publishes_typed_header_member_expression_and_slot_whole() {
    let parsed = parse(
        "arcweft-test://proof/character-final-hir-clean",
        concat!(
            "character alice {\n",
            "    display_name = \"Alice\"\n",
            "}\n",
        ),
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let owner = module.source_ordered_items()[0];
    let item = resolve_item(&module, 0);
    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    let HirItemKind::Character(character) = item.kind() else {
        panic!("Character must lower directly into its final item family")
    };
    assert!(matches!(
        character.header().public_id(),
        HirRetainedPublicId::Resolved {
            value,
            origin: HirPublicIdOrigin::DerivedFromName,
        } if value.as_str() == "character.alice"
    ));
    assert!(matches!(
        character.header().name(),
        HirRetainedName::Resolved(name) if name.as_str() == "alice"
    ));
    assert!(matches!(
        character.surface_alias(),
        HirCharacterSurfaceAlias::Absent
    ));

    let member_id = character
        .display_name()
        .expect("display-name member identity");
    assert_eq!(item.members(), [member_id]);
    let member = module
        .declaration_members()
        .resolve(member_id)
        .expect("published Character member");
    assert_eq!(member.state(), HirDeclarationMemberPoisonState::Clean);
    let HirDeclarationMemberKind::CharacterDisplayName(display) = member.kind() else {
        panic!("typed Character display-name member")
    };
    assert_eq!(display.assignment(), HirCharacterAssignmentState::Present);
    assert!(!display.is_duplicate());
    let initializer = display.initializer().expect("typed initializer ExprId");
    let expression_metadata = module.slots().resolve(initializer).unwrap();
    assert!(!expression_metadata.is_poisoned());
    assert_source_backed_child(&module, initializer);
    assert_item_slot_whole(&module, &parsed, owner);
}

#[test]
fn character_recovery_matrix_keeps_typed_items_and_owner_whole_primaries() {
    let cases = [
        ("missing-name", "character {}\n", HirItemIssue::MissingName),
        (
            "missing-alias",
            "character Alice as {}\n",
            HirItemIssue::MissingName,
        ),
        (
            "missing-body",
            "character Alice\n",
            HirItemIssue::MissingBody,
        ),
        (
            "wrong-family-id",
            "character @view.alice Alice {}\n",
            HirItemIssue::MalformedHeader,
        ),
        (
            "unclosed-body",
            "character Alice {\n    display_name = \"Alice\"\n",
            HirItemIssue::Recovery,
        ),
    ];

    for (case, source, expected_issue) in cases {
        let parsed = parse(
            &format!("arcweft-test://proof/character-final-hir-{case}"),
            source,
        );
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let module = lower(&mut database, &parsed, &key);
        let owner = module.source_ordered_items()[0];
        let item = resolve_item(&module, 0);
        assert!(matches!(item.kind(), HirItemKind::Character(_)), "{case}");
        assert_eq!(
            item.state(),
            &HirItemPoisonState::Poisoned(expected_issue),
            "{case}",
        );
        assert_item_slot_whole(&module, &parsed, owner);
        assert_item_owner_whole_recovery(&module, owner);
    }
}

#[test]
fn character_member_recovery_preserves_all_ordinals_and_item_owner_whole_primary() {
    let parsed = parse(
        "arcweft-test://proof/character-final-hir-members",
        concat!(
            "character Alice {\n",
            "    display_name \"Alice\"\n",
            "    display_name =\n",
            "    voice = @res.voice\n",
            "}\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let owner = module.source_ordered_items()[0];
    let item = resolve_item(&module, 0);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember)
    );
    assert_eq!(
        item.members()
            .iter()
            .map(|member| member.ordinal())
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    let states = item
        .members()
        .iter()
        .map(|member| {
            module
                .declaration_members()
                .resolve(*member)
                .unwrap()
                .state()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        states,
        [
            HirDeclarationMemberPoisonState::Poisoned(
                crate::item::HirDeclarationMemberIssue::MissingAssignment,
            ),
            HirDeclarationMemberPoisonState::Poisoned(
                crate::item::HirDeclarationMemberIssue::Duplicate,
            ),
            HirDeclarationMemberPoisonState::Poisoned(
                crate::item::HirDeclarationMemberIssue::UnclassifiedSyntax,
            ),
        ]
    );
    assert_item_slot_whole(&module, &parsed, owner);
    assert_item_owner_whole_recovery(&module, owner);
}

#[test]
fn recovered_character_initializer_remains_a_typed_child_and_poisoned_member() {
    let parsed = parse(
        "arcweft-test://proof/character-final-hir-recovered-child",
        concat!("character Alice {\n", "    display_name = @\n", "}\n",),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let owner = module.source_ordered_items()[0];
    let item = resolve_item(&module, 0);
    let member_id = item.members()[0];
    let member = module.declaration_members().resolve(member_id).unwrap();
    assert_eq!(
        member.state(),
        HirDeclarationMemberPoisonState::Poisoned(
            crate::item::HirDeclarationMemberIssue::RecoveredChild,
        )
    );
    let HirDeclarationMemberKind::CharacterDisplayName(display) = member.kind() else {
        panic!("display-name member")
    };
    let initializer = display
        .initializer()
        .expect("recovered child remains typed");
    assert!(module.slots().resolve(initializer).unwrap().is_poisoned());
    assert_source_backed_child(&module, initializer);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember)
    );
    assert_item_slot_whole(&module, &parsed, owner);
    assert_item_owner_whole_recovery(&module, owner);
}

#[test]
fn character_prefix_uses_the_common_typed_owner_and_survives_source_freeze() {
    let parsed = parse(
        "arcweft-test://proof/character-final-hir-prefix-rollback",
        concat!(
            "/// Alice\n",
            "#[tool.fixture]\n",
            "pub character Alice {}\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let owner = module.source_ordered_items()[0];
    let item = resolve_item(&module, 0);

    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    assert_eq!(
        item.prefix()
            .documentation()
            .expect("Character documentation")
            .markdown(),
        "Alice"
    );
    assert_eq!(
        item.prefix().visibility(),
        Some(crate::item::HirVisibility::Public)
    );
    let [attribute] = item.prefix().attributes() else {
        panic!("one typed Character attribute")
    };
    assert_eq!(path_spellings(attribute.path()), ["tool", "fixture"]);
    assert!(attribute.arguments().is_empty());
    assert_item_slot_whole(&module, &parsed, owner);
}

#[test]
fn recovered_character_prefix_stays_in_the_character_family_without_partial_publication() {
    let parsed = parse(
        "arcweft-test://proof/character-final-hir-prefix-recovery",
        concat!("#[test.fixture]\n", "character Alice {}\n"),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let owner = module.source_ordered_items()[0];
    let item = resolve_item(&module, 0);

    assert!(matches!(item.kind(), HirItemKind::Character(_)));
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::Recovery)
    );
    assert!(item.prefix().attributes().is_empty());
    assert_item_slot_whole(&module, &parsed, owner);
    assert_item_owner_whole_recovery(&module, owner);
}

#[test]
fn character_member_preflight_accepts_exact_and_rejects_one_over() {
    assert!(preflight_character_members(HirLimit::DeclarationMembers.maximum()).is_ok());
    let Err(HirLowerFailure::Limit(error)) =
        preflight_character_members(HirLimit::DeclarationMembers.maximum() + 1)
    else {
        panic!("one-over declaration-member inventory must fail before child lowering")
    };
    assert_eq!(error.limit(), HirLimit::DeclarationMembers);
    assert_eq!(error.observed(), HirLimit::DeclarationMembers.maximum() + 1);
    assert_eq!(error.maximum(), HirLimit::DeclarationMembers.maximum());
}

#[test]
fn unclassified_item_publishes_one_poisoned_error_owner_with_slot_whole_source() {
    let parsed = parse("arcweft-test://proof/source-file-error-item", "???\n");
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    assert_eq!(module.source_ordered_items().len(), 1);

    let owner = module.source_ordered_items()[0];
    let item = resolve_item(&module, 0);
    assert!(matches!(item.kind(), HirItemKind::Error(_)));
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::UnclassifiedSyntax)
    );
    assert!(item.prefix().documentation().is_none());
    assert!(item.prefix().attributes().is_empty());
    assert!(item.prefix().visibility().is_none());
    assert!(item.members().is_empty());
    assert!(module.declaration_members().arena(owner).is_none());
    assert_item_slot_whole(&module, &parsed, owner);
    assert_item_owner_whole_recovery(&module, owner);
}

#[test]
fn late_module_late_use_and_unknown_items_remain_error_items_in_source_order() {
    let parsed = parse(
        "arcweft-test://proof/source-file-error-item-order",
        concat!(
            "use crate.alpha.value\n",
            "mod crate.late\n",
            "use crate.beta.value\n",
            "???\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    assert_eq!(module.source_ordered_items().len(), 4);
    assert!(matches!(
        resolve_item(&module, 0).kind(),
        HirItemKind::Use(_)
    ));

    for ordinal in 1..4 {
        let owner = module.source_ordered_items()[ordinal];
        let item = resolve_item(&module, ordinal);
        assert!(matches!(item.kind(), HirItemKind::Error(_)), "{ordinal}");
        assert_eq!(
            item.state(),
            &HirItemPoisonState::Poisoned(HirItemIssue::UnclassifiedSyntax),
            "{ordinal}",
        );
        assert_eq!(
            module
                .diagnostics()
                .iter()
                .filter(|diagnostic| matches!(
                    diagnostic,
                    HirDiagnostic::Recovery(diagnostic)
                        if diagnostic.owner() == SyntheticOwner::Item(owner)
                ))
                .count(),
            1,
            "{ordinal}",
        );
    }
}

#[test]
fn attached_headers_publish_one_root_scope_and_exact_authored_item_order() {
    let parsed = parse(
        "arcweft-test://proof/source-file-item-order",
        concat!(
            "mod crate.story\n",
            "pub(crate) use crate.library.value as local\n",
            "use self.widgets.*\n",
            "use parent.data.{alice, bob as narrator}\n",
        ),
    );
    assert!(parsed.diagnostics().is_empty());
    let tree = parsed.tree();
    let entries = tree
        .entries()
        .unwrap()
        .into_iter()
        .filter(|entry| !matches!(entry, SourceFileEntryNode::Attribute(_)))
        .collect::<Vec<_>>();
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    assert_eq!(module.source_ordered_items().len(), entries.len());
    let root_scope = resolve_item(&module, 0).scope();
    for (ordinal, (&id, entry)) in module
        .source_ordered_items()
        .iter()
        .zip(&entries)
        .enumerate()
    {
        let item = resolve_item(&module, ordinal);
        assert_eq!(item.scope(), root_scope);
        let metadata = module.slots().resolve(id).unwrap();
        assert_eq!(
            metadata.source_site(),
            &HirSourceSite::Span(entry.source_span())
        );
        let HirOrigin::Source(origin) = metadata.origin() else {
            panic!("top-level item must be source-backed")
        };
        assert_eq!(origin.syntax(), entry.id());
    }

    let HirItemKind::Module(declaration) = resolve_item(&module, 0).kind() else {
        panic!("module declaration")
    };
    assert_eq!(resolved_path(declaration.path()).root(), HirPathRoot::Crate);
    assert_eq!(path_spellings(resolved_path(declaration.path())), ["story"]);

    let HirItemKind::Use(direct) = resolve_item(&module, 1).kind() else {
        panic!("direct use")
    };
    assert_eq!(direct.bindings().len(), 1);
    assert_eq!(
        path_spellings(resolved_path(direct.bindings()[0].path())),
        ["library", "value"]
    );
    assert_eq!(direct.bindings()[0].alias().unwrap().as_str(), "local");

    let HirItemKind::Use(glob) = resolve_item(&module, 2).kind() else {
        panic!("glob use")
    };
    assert_eq!(glob.bindings().len(), 1);
    assert_eq!(
        resolved_path(glob.bindings()[0].path()).root(),
        HirPathRoot::SelfModule
    );
    assert_eq!(
        path_spellings(resolved_path(glob.bindings()[0].path())),
        ["widgets"]
    );

    let HirItemKind::Use(group) = resolve_item(&module, 3).kind() else {
        panic!("grouped use")
    };
    assert_eq!(group.bindings().len(), 2);
    assert_eq!(
        resolved_path(group.bindings()[0].path()).root(),
        HirPathRoot::Super { depth: 1 }
    );
    assert_eq!(
        path_spellings(resolved_path(group.bindings()[0].path())),
        ["data", "alice"]
    );
    assert_eq!(
        path_spellings(resolved_path(group.bindings()[1].path())),
        ["data", "bob"]
    );
    assert_eq!(group.bindings()[1].alias().unwrap().as_str(), "narrator");
}

#[test]
fn typed_segment_family_controls_projection_without_spelling_fallback() {
    let parsed = parse(
        "arcweft-test://proof/source-file-item-segments",
        concat!("use crate.members.self\n", "use crate.members.'scope\n",),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    let HirItemKind::Use(import) = resolve_item(&module, 0).kind() else {
        panic!("keyword-segment use")
    };
    assert!(matches!(
        &resolved_path(import.bindings()[0].path()).segments()[1],
        HirPathSegment::ProjectSymbol(symbol) if symbol.as_str() == "self"
    ));
    let recovered = resolve_item(&module, 1);
    let HirItemKind::Use(import) = recovered.kind() else {
        panic!("lifetime path must remain a typed Use")
    };
    assert!(import.bindings()[0].path().recovery().is_some());
    assert_eq!(
        recovered.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MalformedHeader)
    );
}

#[test]
fn attached_path_roots_preserve_crate_self_parent_and_consecutive_super_semantics() {
    let parsed = parse(
        "arcweft-test://proof/source-file-path-roots",
        concat!(
            "use local.members.value\n",
            "use crate.members.value\n",
            "use self.members.value\n",
            "use parent.members.value\n",
            "use super.super.members.value\n",
        ),
    );
    assert!(parsed.diagnostics().is_empty());
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    let roots = module
        .source_ordered_items()
        .iter()
        .enumerate()
        .map(|(ordinal, _)| {
            let HirItemKind::Use(import) = resolve_item(&module, ordinal).kind() else {
                panic!("use declaration")
            };
            resolved_path(import.bindings()[0].path()).root()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        roots,
        [
            HirPathRoot::ImplicitCrate,
            HirPathRoot::Crate,
            HirPathRoot::SelfModule,
            HirPathRoot::Super { depth: 1 },
            HirPathRoot::Super { depth: 2 },
        ]
    );
}

#[test]
fn malformed_module_and_use_families_remain_typed_with_one_poison_diagnostic() {
    let parsed = parse(
        "arcweft-test://proof/source-file-use-recovery",
        concat!(
            "mod crate\n",
            "use crate.{alice}\n",
            "use self.{bob}\n",
            "use super.{carol}\n",
            "use crate.members.{dave, 'scope}\n",
            "use crate.members.{erin as}\n",
            "use crate.members.{frank\n",
            "use crate\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    assert_eq!(module.source_ordered_items().len(), 8);
    let expected = [
        HirItemIssue::MissingName,
        HirItemIssue::MissingName,
        HirItemIssue::MissingName,
        HirItemIssue::MissingName,
        HirItemIssue::InvalidMember,
        HirItemIssue::MissingName,
        HirItemIssue::Recovery,
        HirItemIssue::MissingName,
    ];
    for (ordinal, expected_issue) in expected.into_iter().enumerate() {
        let item = resolve_item(&module, ordinal);
        match (ordinal, item.kind()) {
            (0, HirItemKind::Module(_)) | (1.., HirItemKind::Use(_)) => {}
            _ => panic!("malformed item {ordinal} must retain its recognized family"),
        }
        assert_eq!(item.state(), &HirItemPoisonState::Poisoned(expected_issue));
    }
    let recovery = module
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            HirDiagnostic::Recovery(diagnostic) => Some(diagnostic),
            HirDiagnostic::Syntax(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(recovery.len(), module.source_ordered_items().len());
    for &item in module.source_ordered_items() {
        let matches = recovery
            .iter()
            .filter(|diagnostic| diagnostic.owner() == SyntheticOwner::Item(item))
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1);
        assert_eq!(
            matches[0].primary_role(),
            HirRecoveryPrimary::owner_whole(SyntheticOwner::Item(item))
        );
        assert_eq!(
            matches[0].primary(),
            module.slots().resolve(item).unwrap().source_site()
        );
    }
}

#[test]
fn source_order_validation_rejects_reversed_duplicate_and_missing_source_items() {
    let parsed = parse(
        "arcweft-test://proof/source-order-validation",
        "use crate.alpha.value\nuse crate.beta.value\n",
    );
    let key = module_key(&parsed);

    let mut duplicate_database = HirDatabase::try_new().unwrap();
    let mut duplicate = stage(&duplicate_database, &parsed, &key);
    duplicate
        .lower_attached_source_file_items(&parsed.tree())
        .unwrap();
    duplicate
        .source_ordered_items
        .push(duplicate.source_ordered_items[0]);
    assert!(matches!(
        duplicate.finish(&mut duplicate_database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceOrderedItems
        ))
    ));
    assert!(duplicate_database.current(&key).is_none());

    let mut reversed_database = HirDatabase::try_new().unwrap();
    let mut reversed = stage(&reversed_database, &parsed, &key);
    reversed
        .lower_attached_source_file_items(&parsed.tree())
        .unwrap();
    reversed.source_ordered_items.reverse();
    assert!(matches!(
        reversed.finish(&mut reversed_database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceOrderedItems
        ))
    ));
    assert!(reversed_database.current(&key).is_none());

    let mut missing_database = HirDatabase::try_new().unwrap();
    let mut missing = stage(&missing_database, &parsed, &key);
    missing
        .lower_attached_source_file_items(&parsed.tree())
        .unwrap();
    missing.source_ordered_items.pop();
    assert!(matches!(
        missing.finish(&mut missing_database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceOrderedItems
        ))
    ));
    assert!(missing_database.current(&key).is_none());
}

#[test]
fn source_file_item_preflight_accepts_exact_and_rejects_one_over_before_allocation() {
    assert!(preflight_source_file_inventory(HirLimit::Items.maximum()).is_ok());
    let Err(HirLowerFailure::Limit(error)) =
        preflight_source_file_inventory(HirLimit::Items.maximum() + 1)
    else {
        panic!("one-over source-file item inventory must fail its Items preflight")
    };
    assert_eq!(error.limit(), HirLimit::Items);
    assert_eq!(error.observed(), HirLimit::Items.maximum() + 1);
    assert_eq!(error.maximum(), HirLimit::Items.maximum());
}

#[test]
fn attached_item_freeze_rejects_a_synthetic_item_without_publication() {
    let parsed = parse(
        "arcweft-test://proof/source-order-synthetic",
        "use crate.alpha.value\n",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    let root_scope = transaction
        .lower_attached_source_file_items(&parsed.tree())
        .unwrap();
    let root_site = HirSourceSite::Span(parsed.root_syntax().source_span().clone());
    let synthetic = {
        let (slots, arenas) = transaction.storage_mut();
        let reservation = arenas
            .items()
            .reserve_synthetic(
                slots,
                SyntheticKey::try_new(
                    SyntheticOwner::Scope(root_scope),
                    SyntheticRole::MissingRequiredTail,
                    0,
                )
                .unwrap(),
                root_site,
            )
            .unwrap();
        let owner = reservation.id();
        let path = HirPath::try_new(
            HirPathRoot::ImplicitCrate,
            Box::new([HirPathSegment::Identifier(
                HirName::try_new("synthetic".into()).unwrap(),
            )]),
        )
        .unwrap();
        let item = HirItem::try_new(
            owner,
            root_scope,
            HirItemPrefix::new(None, Box::new([]), None),
            HirItemKind::Module(HirModuleDeclaration::new(HirPathValue::Resolved(path))),
            Box::new([]),
        )
        .unwrap();
        arenas.items().finalize(slots, reservation, item).unwrap()
    };
    transaction.source_ordered_items.push(synthetic);

    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&key).is_none());
}

#[test]
fn incremental_reorder_preserves_item_ids_but_changes_only_the_source_order_owner() {
    let name = SourceName::path("proof/source-order-reorder.arcw");
    let document_id = "arcweft-test://proof/source-order-reorder";
    let initial_source = "use crate.alpha.value\nuse crate.beta.value\n";
    let reordered_source = "use crate.beta.value\nuse crate.alpha.value\n";
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(document_id, &name, initial_source),
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let key = module_key(&initial);
    let mut database = HirDatabase::try_new().unwrap();
    let first = lower(&mut database, &initial, &key);
    let initial_order = first.source_ordered_items().to_vec();

    let reordered = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial
                    .document()
                    .span(SourceRange::new(0, initial_source.len()))
                    .unwrap(),
                reordered_source,
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let second = lower(&mut database, &reordered, &key);
    assert_eq!(
        second.source_ordered_items(),
        [initial_order[1], initial_order[0]]
    );
    let raw_slot_order = second
        .arenas()
        .items()
        .try_iter(second.slots())
        .unwrap()
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    assert_eq!(raw_slot_order, initial_order);

    let deleted = syntax
        .reparse(
            &reordered,
            &[SourceEdit::new(
                reordered
                    .document()
                    .span(SourceRange::new(0, reordered_source.len()))
                    .unwrap(),
                "use crate.alpha.value\n",
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let third = lower(&mut database, &deleted, &key);
    assert_eq!(third.source_ordered_items(), [initial_order[0]]);
    assert!(
        third
            .arenas()
            .items()
            .resolve(third.slots(), initial_order[1])
            .is_err()
    );
    assert_eq!(first.source_ordered_items(), initial_order);
}

#[test]
fn incremental_action_insert_reorder_and_remove_preserve_callable_scope_ids_in_source_order() {
    let name = SourceName::path("proof/action-scope-order.arcw");
    let document_id = "arcweft-test://proof/action-scope-order";
    let initial_source = "action First(value: I32)\naction Second(value: I64)\n";
    let reordered_source =
        "action Second(value: I64)\naction Inserted()\naction First(value: I32)\n";
    let deleted_source = "action Inserted()\naction First(value: I32)\n";
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(document_id, &name, initial_source),
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let key = module_key(&initial);
    let mut database = HirDatabase::try_new().unwrap();
    let first = lower(&mut database, &initial, &key);
    let initial_items = first.source_ordered_items().to_vec();
    let first_scope = action_callable_scope(&first, 0);
    let second_scope = action_callable_scope(&first, 1);

    let reordered = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial
                    .document()
                    .span(SourceRange::new(0, initial_source.len()))
                    .unwrap(),
                reordered_source,
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let second = lower(&mut database, &reordered, &key);
    let inserted_item = second.source_ordered_items()[1];
    let inserted_scope = action_callable_scope(&second, 1);
    assert_eq!(
        second.source_ordered_items(),
        [initial_items[1], inserted_item, initial_items[0]]
    );
    assert_eq!(action_callable_scope(&second, 0), second_scope);
    assert_eq!(action_callable_scope(&second, 2), first_scope);
    let root_scope = resolve_item(&second, 0).scope();
    assert_eq!(
        second
            .arenas()
            .scopes()
            .resolve(second.slots(), root_scope)
            .unwrap()
            .children(),
        [second_scope, inserted_scope, first_scope]
    );

    let deleted = syntax
        .reparse(
            &reordered,
            &[SourceEdit::new(
                reordered
                    .document()
                    .span(SourceRange::new(0, reordered_source.len()))
                    .unwrap(),
                deleted_source,
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let third = lower(&mut database, &deleted, &key);
    assert_eq!(
        third.source_ordered_items(),
        [inserted_item, initial_items[0]]
    );
    let root_scope = resolve_item(&third, 0).scope();
    assert_eq!(
        third
            .arenas()
            .scopes()
            .resolve(third.slots(), root_scope)
            .unwrap()
            .children(),
        [inserted_scope, first_scope]
    );
    assert!(
        third
            .arenas()
            .scopes()
            .resolve(third.slots(), second_scope)
            .is_err()
    );
}

#[test]
fn foreign_and_stale_attached_roots_poison_the_transaction_without_publication() {
    let name = SourceName::path("proof/source-file-identity.arcw");
    let document_id = "arcweft-test://proof/source-file-identity";
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(document_id, &name, "use crate.alpha.value\n"),
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let revised = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial.document().span(SourceRange::new(0, 0)).unwrap(),
                " ",
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let key = module_key(&initial);
    let mut database = HirDatabase::try_new().unwrap();
    let mut stale = stage(&database, &initial, &key);
    assert!(matches!(
        stale.lower_attached_source_file_items(&revised.tree()),
        Err(HirLowerFailure::StaleSource { .. })
    ));
    assert!(stale.finish(&mut database).is_err());
    assert!(database.current(&key).is_none());

    let foreign = parse(
        "arcweft-test://proof/source-file-foreign",
        "use crate.alpha.value\n",
    );
    let mut foreign_transaction = stage(&database, &initial, &key);
    assert!(matches!(
        foreign_transaction.lower_attached_source_file_items(&foreign.tree()),
        Err(HirLowerFailure::WrongSyntaxDatabase { .. })
    ));
    assert!(foreign_transaction.finish(&mut database).is_err());
    assert!(database.current(&key).is_none());
}
