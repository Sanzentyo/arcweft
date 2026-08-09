use super::*;

use crate::expr::{HirPoisonState, HirRecoveryIssue};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};
use crate::pattern::{
    HirGenericPatternIssue, HirPatternError, HirPatternKind, HirPatternRecoveryIssue,
    HirPatternSequenceRestIssue,
};

fn assert_same_family_payload_substitution_rejected(
    document_id: &str,
    source: &str,
    substitute: &str,
) {
    let parsed = parsed_source(document_id, &[source, substitute]);
    let attached = attached_patterns(&parsed);
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    let owner = transaction
        .lower_attached_pattern(&attached[0], scope)
        .expect("source Pattern lowering");
    let substitute_owner = transaction
        .lower_attached_pattern(&attached[1], scope)
        .expect("substitute Pattern lowering");
    let scope_locals = [owner, substitute_owner]
        .into_iter()
        .flat_map(|owner| {
            transaction
                .pattern_locals
                .get(&owner)
                .expect("staged Pattern Local inventory")
                .iter()
                .copied()
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    transaction
        .close_scope_members(scope, scope_locals)
        .expect("close test module scope");
    let substitute_payload = {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .patterns()
            .resolve_staged(slots, substitute_owner)
            .expect("staged substitute Pattern")
            .clone()
    };
    {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .patterns()
            .revise_finalized(slots, owner, substitute_payload)
            .expect("test-only same-family payload substitution");
    }

    match transaction.finish(&mut database) {
        Err(HirLowerFailure::Invariant(HirInvariantFailure::InvalidSourceIndex)) => {}
        Err(error) => {
            panic!("same-family payload substitution for {document_id} returned {error:?}")
        }
        Ok(_) => panic!("same-family payload substitution for {document_id} published"),
    }
    assert!(
        database.current(&key).is_none(),
        "payload substitution published a partial module for {document_id}"
    );
}

#[test]
fn pattern_freeze_rejects_every_same_family_payload_substitution_atomically() {
    for (document_id, source, substitute) in [
        ("binding-payload", "alpha", "beta"),
        ("binding-root-identity", "same", "same"),
        ("mutable-binding-payload", "mut alpha", "mut beta"),
        ("literal-payload", "42", "43"),
        ("entity-reference-payload", "@flow.alpha", "@flow.beta"),
        ("variant-payload", "Choice.Ready", "Choice.Pending"),
        ("record-payload", "Point { left: _ }", "Other { right: _ }"),
        ("bracket-sequence-payload", "[_, ..tail]", "[_, ..other]"),
        ("whole-binding-payload", "whole (_, _)", "other (_, _)"),
        ("typed-binding-payload", "typed: Value", "other: Other"),
    ] {
        assert_same_family_payload_substitution_rejected(document_id, source, substitute);
    }
}

#[test]
fn zero_width_recovery_retains_exact_pattern_root_identity() {
    let parsed = parsed_source("zero-width-pattern-root", &["_ |", "_ |"]);
    let roots = attached_patterns(&parsed);
    let first_missing = attached_pattern_child(&roots[0], PatternNodeStep::Element(1));
    let second_missing = attached_pattern_child(&roots[1], PatternNodeStep::Element(1));

    let first_range = first_missing.whole_source_span().range();
    let second_range = second_missing.whole_source_span().range();
    assert_eq!(first_range.start(), first_range.end());
    assert_eq!(second_range.start(), second_range.end());
    assert_eq!(first_missing.root().expect("first Pattern root"), roots[0]);
    assert_eq!(
        second_missing.root().expect("second Pattern root"),
        roots[1]
    );
    assert_ne!(first_missing.root().expect("first Pattern root"), roots[1]);
    assert_ne!(
        second_missing.root().expect("second Pattern root"),
        roots[0]
    );
}

#[test]
fn pattern_freeze_rejects_error_issue_substitution_atomically() {
    let parsed = parsed_source("error-issue-payload", &["+"]);
    let attached = attached_patterns(&parsed);
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    let owner = transaction
        .lower_attached_pattern(&attached[0], scope)
        .expect("Error Pattern lowering");
    let replacement = HirPattern::try_new(
        HirPatternKind::Error(HirPatternError::new(
            HirGenericPatternIssue::TransactionalChildFailure,
        )),
        scope,
        HirPoisonState::Poisoned(HirRecoveryIssue::InvalidPattern(
            HirPatternRecoveryIssue::TransactionalChildFailure,
        )),
        &transaction,
    )
    .expect("same-family forged Error payload");
    {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .patterns()
            .revise_finalized(slots, owner, replacement)
            .expect("test-only Error payload substitution");
    }

    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&key).is_none());
}

#[test]
fn pattern_freeze_rejects_container_poison_substitution_atomically() {
    let parsed = parsed_source("container-poison-payload", &["[_, .., ..]"]);
    let attached = attached_patterns(&parsed);
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    let owner = transaction
        .lower_attached_pattern(&attached[0], scope)
        .expect("recovered BracketSequence Pattern lowering");
    let kind = {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .patterns()
            .resolve_staged(slots, owner)
            .expect("staged recovered BracketSequence Pattern")
            .kind()
            .clone()
    };
    let replacement = HirPattern::try_new(
        kind,
        scope,
        HirPoisonState::Poisoned(HirRecoveryIssue::InvalidPattern(
            HirPatternRecoveryIssue::SequenceRest(HirPatternSequenceRestIssue::MultipleRest {
                ordinal: 2,
            }),
        )),
        &transaction,
    )
    .expect("same-family forged BracketSequence poison");
    {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .patterns()
            .revise_finalized(slots, owner, replacement)
            .expect("test-only Pattern poison substitution");
    }

    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&key).is_none());
}
