mod free_families;
mod method_families;

use std::sync::atomic::AtomicBool;

use arcweft_source::SourceRange;

use crate::{
    callable::{
        CallPoison, CallTargetFact, CallTargetFacts, CallableFamily, MigrationAuthorityClass,
        MigrationCompletionDisposition, PRODUCTION_CALLABLE_LIMITS, ResolverWork,
    },
    checker::module::analyze_registered_project_types_for_call_facts,
    env::TypeCheckEnv,
    test_support::character_project::{
        one_character_facts, register, root_project_source, sample_manifest,
    },
};

use super::{
    SignatureFixture, SignatureQueryOutcome, selected_overload_publication, unique_offset,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExpectedDisposition {
    Selected,
    RejectedCandidates,
    SelectedPoisoned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MigrationCaseKind {
    Accepted,
    RejectedOrPoisoned,
    CleanRecovery,
}

#[derive(Clone, Copy, Debug)]
struct FamilyCase<'a> {
    call: &'a str,
    cursor: &'a str,
    family: CallableFamily,
    disposition: ExpectedDisposition,
    kind: MigrationCaseKind,
    argument_expression_checks: usize,
}

impl<'a> FamilyCase<'a> {
    const fn accepted(
        call: &'a str,
        cursor: &'a str,
        family: CallableFamily,
        argument_expression_checks: usize,
    ) -> Self {
        Self {
            call,
            cursor,
            family,
            disposition: ExpectedDisposition::Selected,
            kind: MigrationCaseKind::Accepted,
            argument_expression_checks,
        }
    }

    const fn rejected_candidates(
        call: &'a str,
        cursor: &'a str,
        family: CallableFamily,
        argument_expression_checks: usize,
    ) -> Self {
        Self {
            call,
            cursor,
            family,
            disposition: ExpectedDisposition::RejectedCandidates,
            kind: MigrationCaseKind::RejectedOrPoisoned,
            argument_expression_checks,
        }
    }

    const fn rejected(
        call: &'a str,
        cursor: &'a str,
        family: CallableFamily,
        argument_expression_checks: usize,
    ) -> Self {
        Self::rejected_candidates(call, cursor, family, argument_expression_checks)
    }

    const fn selected_poisoned(
        call: &'a str,
        cursor: &'a str,
        family: CallableFamily,
        argument_expression_checks: usize,
    ) -> Self {
        Self {
            call,
            cursor,
            family,
            disposition: ExpectedDisposition::SelectedPoisoned,
            kind: MigrationCaseKind::RejectedOrPoisoned,
            argument_expression_checks,
        }
    }

    const fn clean_recovery(
        call: &'a str,
        cursor: &'a str,
        family: CallableFamily,
        argument_expression_checks: usize,
    ) -> Self {
        Self {
            call,
            cursor,
            family,
            disposition: ExpectedDisposition::Selected,
            kind: MigrationCaseKind::CleanRecovery,
            argument_expression_checks,
        }
    }
}

fn fixture_with_environment(source: &str, environment: TypeCheckEnv) -> SignatureFixture {
    let (document, project, world_id) = root_project_source("signature-migration-family", source);
    let facts = one_character_facts(&document, world_id, &sample_manifest("layers/body.png"));
    let world = register(&project, &facts, environment, None)
        .expect("migration-family fixture registers one accepted world");
    SignatureFixture {
        document,
        project,
        world,
    }
}

fn assert_migration_case_kind(case: FamilyCase<'_>) {
    let evidence = case.family.migration_evidence();
    match (evidence.current(), case.kind) {
        (
            MigrationAuthorityClass::RejectingSchema,
            MigrationCaseKind::Accepted | MigrationCaseKind::RejectedOrPoisoned,
        )
        | (
            MigrationAuthorityClass::IntentionallyUnchecked,
            MigrationCaseKind::Accepted | MigrationCaseKind::CleanRecovery,
        ) => {}
        (authority, kind) => panic!(
            "{:?} has migration authority {authority:?}, incompatible with {kind:?}",
            case.family
        ),
    }
}

fn assert_clean_recovery_evidence(facts: &CallTargetFacts, case: FamilyCase<'_>) {
    if case.kind != MigrationCaseKind::CleanRecovery {
        return;
    }
    assert!(
        matches!(facts.target(), CallTargetFact::Selected { .. }),
        "{:?} clean recovery must retain a selected target",
        case.family
    );
    assert_eq!(facts.poison(), CallPoison::Clean);
    assert!(facts.diagnostics().is_empty());
    let slots = facts
        .arguments()
        .iter()
        .flat_map(crate::callable::CheckedCallArgumentFact::slots)
        .collect::<Vec<_>>();
    assert_eq!(
        slots.len(),
        case.argument_expression_checks,
        "{:?} clean recovery must retain every authored recovery slot",
        case.family
    );
    assert!(slots.iter().all(|slot| {
        slot.inferred().is_none() && slot.expected().is_none() && slot.poison() == CallPoison::Clean
    }));
}

fn assert_family_case(fixture: &SignatureFixture, case: FamilyCase<'_>) {
    assert_migration_case_kind(case);
    let call_start = unique_offset(fixture.document.text(), case.call);
    let call_span = fixture
        .document
        .span(SourceRange::new(call_start, call_start + case.call.len()))
        .expect("family case has an exact accepted call span");
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let focused = analyze_registered_project_types_for_call_facts(
        &fixture.project.linked_module(),
        &fixture.world,
        call_span,
        &cancellation,
        &mut work,
    )
    .expect("family case completes focused registered analysis");
    let stats = &focused.report().stats;
    assert_eq!(
        stats.registered_call_expressions, 1,
        "{:?} must contain exactly one authored call expression",
        case.family
    );
    assert_eq!(
        stats.shared_resolver_invocations, 1,
        "{:?} must enter the shared resolver exactly once",
        case.family
    );
    assert_eq!(
        stats.old_dispatch_calls, 0,
        "{:?} must not enter an old successful dispatcher",
        case.family
    );
    assert_eq!(
        stats.registered_argument_expression_checks, case.argument_expression_checks,
        "{:?} must check each authored or recovery argument expression exactly once",
        case.family
    );

    let facts = focused
        .focused_call_target_facts()
        .expect("family case retains focused call facts");
    let checker_primary = match (case.disposition, facts.target(), facts.poison()) {
        (
            ExpectedDisposition::Selected,
            CallTargetFact::Selected { selected, .. },
            CallPoison::Clean,
        )
        | (
            ExpectedDisposition::SelectedPoisoned,
            CallTargetFact::Selected { selected, .. },
            CallPoison::Rejected,
        ) => selected.as_ref(),
        (
            ExpectedDisposition::RejectedCandidates,
            CallTargetFact::Rejected { candidates },
            CallPoison::Rejected,
        ) => candidates
            .first()
            .expect("a rejected family retains at least one bounded candidate"),
        (expected, actual, poison) => panic!(
            "{:?} expected {expected:?}, got {actual:?} with {poison:?}",
            case.family
        ),
    };
    assert_eq!(checker_primary.id().family(), case.family);
    assert!(
        case.family
            .migration_validator_matches(checker_primary.schema().validator()),
        "{:?} observed an incompatible schema validator {:?}",
        case.family,
        checker_primary.schema().validator()
    );

    assert_clean_recovery_evidence(facts, case);

    let SignatureQueryOutcome::Help(help) = fixture
        .query_in(case.call, case.cursor)
        .expect("family case completes the production signature query")
    else {
        panic!("{:?} must project semantic signature help", case.family)
    };
    let signature_primary = help
        .signatures()
        .get(help.active_signature().get())
        .expect("active signature index is validated")
        .candidate();
    assert_eq!(signature_primary, checker_primary.id());
    assert_eq!(signature_primary.family(), case.family);
}

#[test]
fn post_capacity_migration_evidence_is_exhaustive() {
    let mut rejecting = 0usize;
    let mut intentionally_unchecked = 0usize;
    let mut pending_authority = 0usize;
    let mut credited = 0usize;
    let mut pending_completion_authority = 0usize;
    let mut pending_removal = 0usize;

    for family in CallableFamily::ALL {
        let evidence = family.migration_evidence();
        match evidence.current() {
            MigrationAuthorityClass::RejectingSchema => rejecting += 1,
            MigrationAuthorityClass::IntentionallyUnchecked => intentionally_unchecked += 1,
            MigrationAuthorityClass::PendingAuthority => pending_authority += 1,
        }
        match evidence.final_completion() {
            MigrationCompletionDisposition::Credited => credited += 1,
            MigrationCompletionDisposition::PendingAuthority => {
                pending_completion_authority += 1;
            }
            MigrationCompletionDisposition::PendingRemoval => pending_removal += 1,
        }
    }

    assert_eq!(CallableFamily::ALL.len(), 23);
    assert_eq!(
        (rejecting, intentionally_unchecked, pending_authority),
        (18, 4, 1)
    );
    assert_eq!(
        (credited, pending_completion_authority, pending_removal),
        (21, 1, 1)
    );
    assert_eq!(rejecting + intentionally_unchecked, 22);
    assert_eq!((rejecting + intentionally_unchecked) * 2, 44);
    assert_eq!(credited * 2, 42);
    assert_eq!(
        CallableFamily::CapacityMethod
            .migration_evidence()
            .current(),
        MigrationAuthorityClass::IntentionallyUnchecked
    );
    assert_eq!(
        CallableFamily::Speaker
            .migration_evidence()
            .final_completion(),
        MigrationCompletionDisposition::PendingRemoval
    );
    assert_eq!(
        CallableFamily::Dialogue.migration_evidence().current(),
        MigrationAuthorityClass::PendingAuthority
    );
    assert_eq!(
        CallableFamily::Dialogue
            .migration_evidence()
            .final_completion(),
        MigrationCompletionDisposition::PendingAuthority
    );
    assert_ne!(pending_completion_authority + pending_removal, 0);
}

#[test]
fn shared_dispatch_counters_commit_once_and_match_signature_primary() {
    let call = "selected_overload(1i32)";
    let fixture = SignatureFixture::with_publication(
        &format!("fn main() -> Unit {{\n    {call}\n    ()\n}}\n"),
        selected_overload_publication(),
    );
    let call_start = unique_offset(fixture.document.text(), call);
    let call_span = fixture
        .document
        .span(SourceRange::new(call_start, call_start + call.len()))
        .expect("exact selected call span");
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let focused = analyze_registered_project_types_for_call_facts(
        &fixture.project.linked_module(),
        &fixture.world,
        call_span,
        &cancellation,
        &mut work,
    )
    .expect("focused registered analysis");
    let stats = &focused.report().stats;
    assert_eq!(stats.registered_call_expressions, 1);
    assert_eq!(stats.shared_resolver_invocations, 1);
    assert_eq!(stats.old_dispatch_calls, 0);
    assert_eq!(stats.registered_argument_expression_checks, 1);

    let facts = focused
        .focused_call_target_facts()
        .expect("selected call facts");
    let CallTargetFact::Selected { selected, .. } = facts.target() else {
        panic!("accepted overload must retain one selected primary")
    };
    let SignatureQueryOutcome::Help(help) = fixture
        .query_in(call, "1i32")
        .expect("selected signature query")
    else {
        panic!("selected overload must project signature help")
    };
    assert_eq!(
        selected.id(),
        help.signatures()[help.active_signature().get()].candidate()
    );
}

#[test]
fn rejected_shared_dispatch_checks_each_recovery_argument_once() {
    let call = "selected_overload(true)";
    let fixture = SignatureFixture::with_publication(
        &format!("fn main() -> Unit {{\n    {call}\n    ()\n}}\n"),
        selected_overload_publication(),
    );
    let call_start = unique_offset(fixture.document.text(), call);
    let call_span = fixture
        .document
        .span(SourceRange::new(call_start, call_start + call.len()))
        .expect("exact rejected call span");
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let focused = analyze_registered_project_types_for_call_facts(
        &fixture.project.linked_module(),
        &fixture.world,
        call_span,
        &cancellation,
        &mut work,
    )
    .expect("focused rejected registered analysis");
    let stats = &focused.report().stats;
    assert_eq!(stats.registered_call_expressions, 1);
    assert_eq!(stats.shared_resolver_invocations, 1);
    assert_eq!(stats.old_dispatch_calls, 0);
    assert_eq!(stats.registered_argument_expression_checks, 1);
    assert!(matches!(
        focused
            .focused_call_target_facts()
            .expect("rejected call facts")
            .target(),
        CallTargetFact::Rejected { .. }
    ));
}
