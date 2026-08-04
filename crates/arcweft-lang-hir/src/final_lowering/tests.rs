use std::sync::Arc;

use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::incremental::{ParsedSource, SyntaxDatabase};
use arcweft_lang_syntax::text::MAX_RICH_TEXT_CONTENT_TAGS;
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceEdit, SourceName, SourceRange};

use crate::dialogue_application::{
    HirDialogueExpressionExpectation, HirDialogueTransactionContext,
    HirDialogueTransactionRequirement, HirRichTextCharge,
};
use crate::identity::ScopeId;
use crate::item::{HirFunctionBody, HirItemKind};
use crate::lower::{HirInvariantFailure, HirLowerFailure, HirModuleKey, LoweringRequest};
use crate::module::HirModuleStatus;
use crate::pattern::HirPatternResolver;
use crate::scope::{HirScope, HirScopeKind, HirScopeOwner};
use crate::source_index::HirSourceSite;
use crate::symbol::CallablePackageId;
use crate::type_ref::HirTypeResolver;

use super::{HirDatabase, StagedHirModuleTransaction};

fn parsed_revisions(document_id: &str) -> (ParsedSource, ParsedSource) {
    parsed_revisions_with_source(document_id, "")
}

fn parsed_revisions_with_source(document_id: &str, source: &str) -> (ParsedSource, ParsedSource) {
    let name = SourceName::path("proof/final-lowering-transaction.arcw");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(document_id).unwrap(),
            name.clone(),
            source,
        )
        .unwrap(),
    );
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let revised = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial
                    .document()
                    .span(SourceRange::new(source.len(), source.len()))
                    .unwrap(),
                " ",
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    (initial, revised)
}

fn key(parsed: &ParsedSource) -> HirModuleKey {
    HirModuleKey::new(
        CallablePackageId::try_new("proof-final-lowering-tests").unwrap(),
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

fn lower_attached_source(
    database: &mut HirDatabase,
    parsed: &ParsedSource,
    key: &HirModuleKey,
) -> crate::database::HirLowerOutput {
    database
        .lower_attached_source(LoweringRequest::try_new(key.clone(), parsed).unwrap())
        .unwrap()
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
            .unwrap(),
        )
        .unwrap()
}

#[test]
fn empty_attached_source_publishes_first_and_second_revisions_atomically() {
    let (initial, revised) = parsed_revisions("arcweft-test://proof/final-lowering-empty");
    let key = key(&initial);
    let mut database = HirDatabase::try_new().unwrap();

    let mut first_transaction = stage(&database, &initial, &key);
    allocate_module_scope(&mut first_transaction, &initial);
    let first = first_transaction.finish(&mut database).unwrap();
    let first_module = Arc::clone(first.module());
    assert_eq!(first_module.status(), HirModuleStatus::Clean);
    assert!(first.invalidations().is_empty());
    assert!(Arc::ptr_eq(&first_module, &database.current(&key).unwrap()));

    let mut second_transaction = stage(&database, &revised, &key);
    allocate_module_scope(&mut second_transaction, &revised);
    let second = second_transaction.finish(&mut database).unwrap();
    assert_eq!(second.module().module_id(), first_module.module_id());
    assert_eq!(
        second.module().snapshot_id().revision(),
        first_module
            .snapshot_id()
            .revision()
            .checked_next()
            .unwrap()
    );
    assert_eq!(
        second.module().provenance().syntax_snapshot(),
        revised.snapshot_id()
    );
    assert!(second.invalidations().is_empty());
    assert!(Arc::ptr_eq(
        second.module(),
        &database.current(&key).unwrap()
    ));
    assert!(Arc::ptr_eq(
        &first_module,
        &database.snapshot(first_module.snapshot_id()).unwrap()
    ));
}

#[test]
fn database_owned_entry_lowers_the_complete_attached_source_atomically() {
    let (initial, revised) = parsed_revisions_with_source(
        "arcweft-test://proof/final-lowering-owned-entry",
        "fn ready() {}",
    );
    let key = key(&initial);
    let mut database = HirDatabase::try_new().unwrap();

    let first = lower_attached_source(&mut database, &initial, &key);
    assert_eq!(first.module().source_ordered_items().len(), 1);
    assert!(Arc::ptr_eq(
        first.module(),
        &database.current(&key).expect("first revision is current")
    ));

    let second = lower_attached_source(&mut database, &revised, &key);
    assert_eq!(second.module().source_ordered_items().len(), 1);
    assert_eq!(
        second.module().snapshot_id().revision(),
        first
            .module()
            .snapshot_id()
            .revision()
            .checked_next()
            .unwrap()
    );
    assert!(Arc::ptr_eq(
        second.module(),
        &database.current(&key).expect("second revision is current")
    ));
}

#[test]
fn dropping_a_staged_revision_publishes_neither_database_state_nor_lifetime() {
    let (initial, revised) = parsed_revisions("arcweft-test://proof/final-lowering-drop");
    let key = key(&initial);
    let mut database = HirDatabase::try_new().unwrap();
    let mut initial_transaction = stage(&database, &initial, &key);
    allocate_module_scope(&mut initial_transaction, &initial);
    let accepted = initial_transaction
        .finish(&mut database)
        .unwrap()
        .into_module();

    let mut dropped = stage(&database, &revised, &key);
    let dropped_scope = allocate_module_scope(&mut dropped, &revised);
    drop(dropped);

    assert!(Arc::ptr_eq(&accepted, &database.current(&key).unwrap()));
    assert_eq!(accepted.slots().committed_slot_count(), 1);

    let mut replacement = stage(&database, &revised, &key);
    let replacement_scope = allocate_module_scope(&mut replacement, &revised);
    assert_eq!(replacement_scope, dropped_scope);
    let replacement = replacement.finish(&mut database).unwrap().into_module();
    assert_eq!(replacement.slots().committed_slot_count(), 1);
}

#[test]
fn forced_outer_failure_leaves_the_accepted_lifetime_ledger_unchanged() {
    let (initial, revised) = parsed_revisions("arcweft-test://proof/final-lowering-failure");
    let key = key(&initial);
    let mut database = HirDatabase::try_new().unwrap();
    let mut initial_transaction = stage(&database, &initial, &key);
    allocate_module_scope(&mut initial_transaction, &initial);
    let accepted = initial_transaction
        .finish(&mut database)
        .unwrap()
        .into_module();

    let mut failed = stage(&database, &revised, &key);
    let failed_scope = allocate_module_scope(&mut failed, &revised);
    failed.storage_mut().0.poison();
    assert!(matches!(
        failed.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSlotCommit
        ))
    ));
    assert!(Arc::ptr_eq(&accepted, &database.current(&key).unwrap()));
    assert_eq!(accepted.slots().committed_slot_count(), 1);

    let mut replacement = stage(&database, &revised, &key);
    let replacement_scope = allocate_module_scope(&mut replacement, &revised);
    assert_eq!(replacement_scope, failed_scope);
    let replacement = replacement.finish(&mut database).unwrap().into_module();
    assert_eq!(replacement.slots().committed_slot_count(), 1);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one transaction must prove both staged and retained resolver paths"
)]
fn transaction_resolvers_read_staged_and_retained_typed_payloads() {
    let (initial, revised) = parsed_revisions_with_source(
        "arcweft-test://proof/final-lowering-resolvers",
        "fn inspect(value: Never) -> Never { value }\n",
    );
    assert_eq!(initial.root_syntax().id(), revised.root_syntax().id());
    let key = key(&initial);
    let mut database = HirDatabase::try_new().unwrap();

    let mut first = stage(&database, &initial, &key);
    first
        .lower_attached_source_file_items(&initial.tree())
        .unwrap();
    let owner = first.staged_source_ordered_items()[0];
    let (callable_scope, ty, pattern, expression) = {
        let (slots, arenas) = first.storage_mut();
        let item = arenas.items().resolve_staged(slots, owner).unwrap();
        let HirItemKind::Function(function) = item.kind() else {
            panic!("fixture must lower to a Function")
        };
        let [group] = function.parameter_groups() else {
            panic!("fixture must retain one parameter group")
        };
        let [parameter] = group.parameters() else {
            panic!("fixture must retain one parameter")
        };
        let HirFunctionBody::Block { tail, .. } = function.body() else {
            panic!("fixture must retain a block body")
        };
        (
            function.callable_scope(),
            parameter.ty(),
            parameter.pattern(),
            *tail,
        )
    };

    assert!(HirTypeResolver::resolve_type(&first, callable_scope, ty).is_some());
    assert!(HirPatternResolver::resolve_pattern(&first, callable_scope, pattern).is_some());
    assert!(
        first
            .require(HirDialogueTransactionRequirement::Expression {
                id: expression,
                expected: HirDialogueExpressionExpectation::Any,
            })
            .is_ok()
    );
    assert!(
        first
            .require(HirDialogueTransactionRequirement::Expression {
                id: expression,
                expected: HirDialogueExpressionExpectation::Call,
            })
            .is_err()
    );
    assert!(
        first
            .require(HirDialogueTransactionRequirement::RichTextCharge(
                HirRichTextCharge::ContentTags {
                    observed: MAX_RICH_TEXT_CONTENT_TAGS,
                },
            ))
            .is_ok()
    );
    assert!(
        first
            .require(HirDialogueTransactionRequirement::RichTextCharge(
                HirRichTextCharge::ContentTags {
                    observed: MAX_RICH_TEXT_CONTENT_TAGS + 1,
                },
            ))
            .is_err()
    );
    let accepted = first.finish(&mut database).unwrap().into_module();

    let mut second = stage(&database, &revised, &key);
    assert_eq!(second.snapshot_id().module(), accepted.module_id());
    assert!(HirTypeResolver::resolve_type(&second, callable_scope, ty).is_some());
    assert!(HirPatternResolver::resolve_pattern(&second, callable_scope, pattern).is_some());
    assert!(
        second
            .require(HirDialogueTransactionRequirement::Expression {
                id: expression,
                expected: HirDialogueExpressionExpectation::Any,
            })
            .is_ok()
    );
    assert!(
        second
            .require(HirDialogueTransactionRequirement::Expression {
                id: expression,
                expected: HirDialogueExpressionExpectation::Call,
            })
            .is_err()
    );
    drop(second);
    assert!(Arc::ptr_eq(&accepted, &database.current(&key).unwrap()));
    assert_ne!(initial.document().identity(), revised.document().identity());
}
