use arcweft_lang_hir::lower::lower_document_to_hir;
use arcweft_lang_syntax::{
    ast::pattern::Pattern,
    expr::LifetimeScopeKind,
    parser::{ParseOptions, parse_document_with_source},
    reference::BorrowKind,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::{TypeChecker, TypeExpressionId};
use crate::{effects::EffectId, env::TypeCheckEnv, types::TypeKind};

fn with_checker(check: impl FnOnce(&mut TypeChecker<'_>)) {
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new("registered-candidate-transaction").expect("document id"),
        SourceName::Memory,
        "fn main() -> Unit { () }",
    )
    .expect("source document");
    let document = std::sync::Arc::new(document);
    let parsed =
        parse_document_with_source(std::sync::Arc::clone(&document), ParseOptions::default());
    assert!(parsed.errors().is_empty(), "transaction fixture parses");
    let module = lower_document_to_hir(&document, parsed.typed_tree()).expect("fixture lowers");
    let environment = TypeCheckEnv::standard();
    check(&mut TypeChecker::new(&environment, &module));
}

#[test]
fn rejected_closure_capture_and_effect_callable_are_fully_rolled_back() {
    with_checker(|checker| {
        checker.push_closure_capture_frame(TypeExpressionId::from_index(1), Vec::<String>::new());
        let checkpoint = checker.checkpoint_registered_candidate();

        checker.record_closure_capture("captured", &TypeKind::I32);
        let (callable, _, _) =
            checker.enter_closure_effect_callable(TypeExpressionId::from_index(2), None);
        checker.effect_collector.record_effect(
            EffectId::parse("test.observe").expect("effect id"),
            crate::effect_model::EffectSite::new("provisional closure"),
        );
        assert!(
            checker
                .effect_collector
                .inferred_effect_row(&callable)
                .is_some()
        );
        assert!(
            checker.closure_capture_stack[0]
                .captures
                .contains_key("captured")
        );

        checker.rollback_registered_candidate(checkpoint);

        assert!(
            checker
                .effect_collector
                .inferred_effect_row(&callable)
                .is_none()
        );
        assert!(checker.closure_capture_stack[0].captures.is_empty());
        assert!(checker.closure_captures.is_empty());
        assert!(checker.errors.is_empty());
    });
}

#[test]
fn block_local_presentation_default_and_borrow_state_do_not_leak() {
    with_checker(|checker| {
        let semantic_binding = checker.next_semantic_binding;
        let checkpoint = checker.checkpoint_registered_candidate();
        checker.with_local_mutation_scope(|checker| {
            checker.bind_local("temporary".to_owned(), TypeKind::I32);
            checker.set_active_presentation_default("character", "temporary".to_owned());
            checker.register_borrow_bindings(
                &Pattern::Ident("borrowed".to_owned()),
                &TypeKind::BorrowRef {
                    kind: BorrowKind::Shared,
                    lifetime: Some(LifetimeScopeKind::Flow),
                    inner: Box::new(TypeKind::I32),
                },
            );
        });
        assert!(!checker.locals.contains_key("temporary"));
        assert!(
            checker
                .active_presentation_defaults
                .contains_key("character")
        );
        assert!(checker.borrow_local_lifetimes.contains_key("borrowed"));

        checker.rollback_registered_candidate(checkpoint);

        assert!(!checker.locals.contains_key("temporary"));
        assert!(checker.active_presentation_defaults.is_empty());
        assert!(checker.borrow_local_lifetimes.is_empty());
        assert!(checker.active_borrow_lifetimes.is_empty());
        assert_eq!(checker.active_borrow_total, 0);
        assert_eq!(checker.next_semantic_binding, semantic_binding);
    });
}

#[test]
fn nested_candidate_rollback_preserves_outer_state_until_outer_rollback() {
    with_checker(|checker| {
        let outer = checker.checkpoint_registered_candidate();
        checker.set_active_presentation_default("background", "outer".to_owned());
        let (outer_callable, _, _) =
            checker.enter_closure_effect_callable(TypeExpressionId::from_index(3), None);

        let inner = checker.checkpoint_registered_candidate();
        checker.set_active_presentation_default("character", "inner".to_owned());
        let (inner_callable, _, _) =
            checker.enter_closure_effect_callable(TypeExpressionId::from_index(4), None);
        checker.rollback_registered_candidate(inner);

        assert_eq!(
            checker
                .active_presentation_defaults
                .get("background")
                .map(String::as_str),
            Some("outer")
        );
        assert!(
            !checker
                .active_presentation_defaults
                .contains_key("character")
        );
        assert!(
            checker
                .effect_collector
                .inferred_effect_row(&outer_callable)
                .is_some()
        );
        assert!(
            checker
                .effect_collector
                .inferred_effect_row(&inner_callable)
                .is_none()
        );

        checker.rollback_registered_candidate(outer);
        assert!(checker.active_presentation_defaults.is_empty());
        assert!(
            checker
                .effect_collector
                .inferred_effect_row(&outer_callable)
                .is_none()
        );
    });
}

#[test]
fn committed_effect_state_survives_the_candidate_transaction() {
    with_checker(|checker| {
        let checkpoint = checker.checkpoint_registered_candidate();
        let (callable, _, _) =
            checker.enter_closure_effect_callable(TypeExpressionId::from_index(5), None);
        checker.effect_collector.record_effect(
            EffectId::parse("test.commit").expect("effect id"),
            crate::effect_model::EffectSite::new("selected candidate"),
        );

        checker.commit_registered_candidate(&checkpoint);

        assert!(
            checker
                .effect_collector
                .inferred_effect_row(&callable)
                .is_some(),
            "the selected candidate keeps its committed effect row"
        );
    });
}
