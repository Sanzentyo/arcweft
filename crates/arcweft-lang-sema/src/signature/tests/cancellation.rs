use std::{
    cell::{Cell, RefCell},
    sync::atomic::AtomicBool,
};

use arcweft_source::SourceRange;

use crate::{
    callable::{
        CallTargetFactError, CallableParameterPassing, CallableParameterPresence,
        CallableParameterType, CallableQueryLimitError, PRODUCTION_CALLABLE_LIMITS,
        PRODUCTION_SIGNATURE_LIMITS, ResolveCallError, ResolverWork, SignatureQueryStep,
        SignatureQueryStepControl, SignatureQueryWorkMeter,
    },
    checker::{
        CandidateEvaluationPass, CandidateExpectedType, FocusedCallSite,
        PhysicalArgumentEvaluationKind,
        module::{
            FocusedCallTypeCheckReport, SignatureFocusedAnalysis,
            analyze_registered_project_types_for_signature_call,
        },
    },
    types::TypeKind,
};

use super::{
    SignatureFixture, SignatureQueryControl, SignatureQueryError, ambiguous_publication,
    fixed_literal_spread_schema, one_parameter_schema, publication, selected_overload_publication,
    two_positional_parameter_schema, unique_offset,
};

struct DeadlineAfterStep {
    step: SignatureQueryStep,
    prior_occurrences: Cell<usize>,
}

impl DeadlineAfterStep {
    const fn new(step: SignatureQueryStep, prior_occurrences: usize) -> Self {
        Self {
            step,
            prior_occurrences: Cell::new(prior_occurrences),
        }
    }
}

impl SignatureQueryStepControl for DeadlineAfterStep {
    fn check_signature_query_step(&self, step: SignatureQueryStep) -> Result<(), ResolveCallError> {
        if step != self.step {
            return Ok(());
        }
        let prior_occurrences = self.prior_occurrences.get();
        if prior_occurrences == 0 {
            return Err(ResolveCallError::DeadlineExceeded);
        }
        self.prior_occurrences.set(prior_occurrences - 1);
        Ok(())
    }
}

#[derive(Default)]
struct RecordedQuerySteps {
    steps: RefCell<Vec<SignatureQueryStep>>,
}

impl SignatureQueryStepControl for RecordedQuerySteps {
    fn check_signature_query_step(&self, step: SignatureQueryStep) -> Result<(), ResolveCallError> {
        self.steps.borrow_mut().push(step);
        Ok(())
    }
}

fn associated_capacity_site(
    fixture: &SignatureFixture,
    source: &str,
    cancellation: &AtomicBool,
    signature_work: &mut SignatureQueryWorkMeter,
) -> FocusedCallSite {
    let call = "Vec<i32>.with_capacity(1usize, 2usize)";
    let call_start = source.find(call).expect("associated call spelling");
    let cursor = call_start + call.find("2usize").expect("associated cursor argument");
    let control = SignatureQueryControl::new(cancellation, None);
    let linked = fixture.project.linked_module();
    let site = super::super::surface::select_signature_surface(
        &linked,
        &fixture.document,
        cursor,
        control,
        signature_work,
    )
    .expect("associated cancellation surface selection")
    .site
    .expect("associated cancellation focused site");
    let expected = fixture
        .document
        .span(SourceRange::new(call_start, call_start + call.len()))
        .expect("associated cancellation call span");
    assert_eq!(site.call(), &expected);
    site
}

#[test]
fn associated_cancellation_before_commit_is_atomic() {
    const SOURCE: &str = r"
fn main() -> Vec<i32> {
    Vec<i32>.with_capacity(1usize, 2usize)
}
";
    let fixture = SignatureFixture::new(SOURCE);
    let linked = fixture.project.linked_module();

    let uncancelled = AtomicBool::new(false);
    let mut discovery_signature_work = SignatureQueryWorkMeter::new(PRODUCTION_SIGNATURE_LIMITS);
    let discovery_site = associated_capacity_site(
        &fixture,
        SOURCE,
        &uncancelled,
        &mut discovery_signature_work,
    );
    let discovery = RecordedQuerySteps::default();
    let mut discovery_resolver_work =
        ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let discovery_report =
        analyze_registered_project_types_for_signature_call(SignatureFocusedAnalysis {
            module: &linked,
            registered: &fixture.world,
            site: discovery_site,
            cancellation: &uncancelled,
            work: &mut discovery_resolver_work,
            signature_work: &mut discovery_signature_work,
            signature_control: &discovery,
        })
        .expect("associated step discovery succeeds");
    discovery_report
        .focused_call_target_facts()
        .expect("associated step discovery commits one target");

    let steps = discovery.steps.borrow().clone();
    assert!(!steps.is_empty());
    let mut occurrences = Vec::with_capacity(steps.len());
    for (index, step) in steps.iter().copied().enumerate() {
        let prior = steps[..index]
            .iter()
            .filter(|observed| **observed == step)
            .count();
        occurrences.push((step, prior));
    }

    for (step, prior) in occurrences {
        let cancellation = AtomicBool::new(false);
        let prior = Cell::new(prior);
        let control = SignatureQueryControl::new(&cancellation, None)
            .with_cancellation_step_after(step, &prior);
        let mut signature_work = SignatureQueryWorkMeter::new(PRODUCTION_SIGNATURE_LIMITS);
        let site = associated_capacity_site(&fixture, SOURCE, &cancellation, &mut signature_work);
        let mut resolver_work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
        let report =
            analyze_registered_project_types_for_signature_call(SignatureFocusedAnalysis {
                module: &linked,
                registered: &fixture.world,
                site,
                cancellation: &cancellation,
                work: &mut resolver_work,
                signature_work: &mut signature_work,
                signature_control: &control,
            })
            .expect("terminal cancellation remains in the focused report");

        assert!(cancellation.load(std::sync::atomic::Ordering::Acquire));
        assert!(matches!(
            report.focused_call_target_facts(),
            Err(CallTargetFactError::Resolve { reason, .. })
                if matches!(reason.as_ref(), ResolveCallError::Cancelled)
        ));
        assert_eq!(report.report().retained_call_target_facts().count(), 0);
        assert_eq!(
            report.report().stats.registered_argument_expression_checks,
            0
        );
        assert_eq!(
            report.report().retained_argument_inference_facts().count(),
            0
        );
    }
}

#[test]
fn cancellation_between_candidates_discards_partial_signature_help() {
    let fixture = SignatureFixture::with_publication(
        r"
fn main() -> Unit {
    let value = ambiguous_value(1i32)
    ()
}
",
        ambiguous_publication(),
    );
    let cancelled = AtomicBool::new(false);
    let first_candidate_may_finish = Cell::new(1);
    let control = SignatureQueryControl::new(&cancelled, None).with_cancellation_step_after(
        SignatureQueryStep::CandidateProbe,
        &first_candidate_may_finish,
    );

    assert_eq!(
        fixture.query_with_control(unique_offset(fixture.document.text(), "1i32"), control),
        Err(SignatureQueryError::Cancelled),
    );
    assert!(cancelled.load(std::sync::atomic::Ordering::Acquire));
}

#[test]
fn cancellation_before_selected_commit_discards_partial_signature_help() {
    let fixture = SignatureFixture::with_publication(
        r"
fn main() -> Unit {
    let value = selected_overload(1i32)
    ()
}
",
        selected_overload_publication(),
    );
    let cancelled = AtomicBool::new(false);
    let cancel_first_commit = Cell::new(0);
    let control = SignatureQueryControl::new(&cancelled, None)
        .with_cancellation_step_after(SignatureQueryStep::SelectedReplay, &cancel_first_commit);

    assert_eq!(
        fixture.query_with_control(unique_offset(fixture.document.text(), "1i32"), control),
        Err(SignatureQueryError::Cancelled),
    );
    assert!(cancelled.load(std::sync::atomic::Ordering::Acquire));
}

#[test]
fn cancellation_before_first_argument_slot_records_no_physical_or_retained_fact() {
    let report = candidate_argument_cancelled_report(0);

    assert!(matches!(
        report.focused_call_target_facts(),
        Err(CallTargetFactError::Resolve { reason, .. })
            if matches!(reason.as_ref(), crate::callable::ResolveCallError::Cancelled)
    ));
    assert!(
        report
            .report()
            .physical_candidate_argument_evaluations()
            .is_empty()
    );
    assert_eq!(
        report.report().retained_argument_inference_facts().count(),
        0
    );
}

#[test]
fn cancellation_after_one_argument_slot_keeps_only_the_completed_physical_prefix() {
    let report = candidate_argument_cancelled_report(1);

    assert!(matches!(
        report.focused_call_target_facts(),
        Err(CallTargetFactError::Resolve { reason, .. })
            if matches!(reason.as_ref(), crate::callable::ResolveCallError::Cancelled)
    ));
    let [evaluation] = report.report().physical_candidate_argument_evaluations() else {
        panic!("exactly one admitted argument slot must be retained as physical evidence")
    };
    assert_eq!(evaluation.pass, CandidateEvaluationPass::DirectCommitted);
    assert_eq!(evaluation.argument.get(), 0);
    assert_eq!(evaluation.slot.get(), 0);
    assert_eq!(evaluation.kind, PhysicalArgumentEvaluationKind::Authored);
    assert_eq!(
        evaluation.expected,
        CandidateExpectedType::Exact(TypeKind::I32)
    );
    assert_eq!(
        report.report().retained_argument_inference_facts().count(),
        0
    );
}

#[test]
fn deadline_after_one_overload_probe_rolls_back_all_semantic_candidate_state() {
    let source = r"
fn main() -> Unit {
    let value = work_choice(1i32)
    ()
}
";
    let fixture = SignatureFixture::with_publication(source, selected_replay_work_publication());
    let cancelled = AtomicBool::new(false);
    let surface_control = SignatureQueryControl::new(&cancelled, None);
    let deadline = DeadlineAfterStep::new(SignatureQueryStep::CandidateArgumentProbe, 1);
    let linked = fixture.project.linked_module();
    let mut signature_work = SignatureQueryWorkMeter::new(PRODUCTION_SIGNATURE_LIMITS);
    let site = super::super::surface::select_signature_surface(
        &linked,
        &fixture.document,
        unique_offset(source, "1i32"),
        surface_control,
        &mut signature_work,
    )
    .expect("overload-probe surface selection")
    .site
    .expect("focused overload-probe call");
    let mut resolver_work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let report = analyze_registered_project_types_for_signature_call(SignatureFocusedAnalysis {
        module: &linked,
        registered: &fixture.world,
        site,
        cancellation: &cancelled,
        work: &mut resolver_work,
        signature_work: &mut signature_work,
        signature_control: &deadline,
    })
    .expect("terminal overload-probe failure remains in the focused report");

    assert!(matches!(
        report.focused_call_target_facts(),
        Err(CallTargetFactError::Resolve { reason, .. })
            if matches!(reason.as_ref(), ResolveCallError::DeadlineExceeded)
    ));
    let [evaluation] = report.report().physical_candidate_argument_evaluations() else {
        panic!("only the completed first overload probe must remain as physical evidence")
    };
    assert_eq!(evaluation.pass, CandidateEvaluationPass::Probe);
    assert_eq!(
        evaluation.expected,
        CandidateExpectedType::Exact(TypeKind::String)
    );
    assert_eq!(
        report.report().retained_argument_inference_facts().count(),
        0
    );
    assert_eq!(
        report.report().stats.registered_argument_expression_checks,
        0
    );
    assert!(report.report().diagnostics.is_empty());
}

#[test]
fn deadline_between_fixed_spread_slots_keeps_only_the_completed_physical_prefix() {
    let control = DeadlineAfterStep::new(SignatureQueryStep::CandidateArgumentProbe, 1);
    let (report, _) =
        fixed_literal_spread_report(&control, PRODUCTION_CALLABLE_LIMITS.max_query_work());

    assert!(matches!(
        report.focused_call_target_facts(),
        Err(CallTargetFactError::Resolve { reason, .. })
            if matches!(reason.as_ref(), ResolveCallError::DeadlineExceeded)
    ));
    assert_completed_fixed_spread_prefix(&report);
}

#[test]
fn work_failure_between_fixed_spread_slots_keeps_only_the_completed_physical_prefix() {
    let cancelled = AtomicBool::new(false);
    let control = SignatureQueryControl::new(&cancelled, None);
    let (_, accepted_work) =
        fixed_literal_spread_report(&control, PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let one_slot_limit = accepted_work
        .checked_sub(2)
        .expect("the accepted two-slot query charges two operations per slot");
    let (report, consumed) = fixed_literal_spread_report(&control, one_slot_limit);

    let Err(CallTargetFactError::Resolve { reason, .. }) = report.focused_call_target_facts()
    else {
        panic!("fixed-spread work failure must remain the typed terminal error")
    };
    let ResolveCallError::Work(CallableQueryLimitError::Work {
        requested,
        consumed: failed_at,
        limit,
    }) = reason.as_ref()
    else {
        panic!("fixed-spread query must fail at the exact callable work boundary")
    };
    assert_eq!(*requested, 1);
    assert_eq!(*failed_at, one_slot_limit);
    assert_eq!(*limit, one_slot_limit);
    assert_eq!(consumed, one_slot_limit);
    assert_completed_fixed_spread_prefix(&report);
}

#[test]
fn selected_replay_physical_evidence_does_not_duplicate_candidate_work() {
    let source = r"
fn main() -> Unit {
    let value = work_choice(1i32)
    ()
}
";
    let fixture = SignatureFixture::with_publication(source, selected_replay_work_publication());
    let cancelled = AtomicBool::new(false);
    let control = SignatureQueryControl::new(&cancelled, None);
    let linked = fixture.project.linked_module();
    let mut signature_work = SignatureQueryWorkMeter::new(PRODUCTION_SIGNATURE_LIMITS);
    let site = super::super::surface::select_signature_surface(
        &linked,
        &fixture.document,
        unique_offset(source, "1i32"),
        control,
        &mut signature_work,
    )
    .expect("selected-replay surface selection")
    .site
    .expect("focused selected-replay call");
    let mut resolver_work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let report = analyze_registered_project_types_for_signature_call(SignatureFocusedAnalysis {
        module: &linked,
        registered: &fixture.world,
        site,
        cancellation: &cancelled,
        work: &mut resolver_work,
        signature_work: &mut signature_work,
        signature_control: &control,
    })
    .expect("selected-replay focused semantic report");

    assert_eq!(
        report
            .report()
            .physical_candidate_argument_evaluations()
            .iter()
            .map(|evaluation| evaluation.pass)
            .collect::<Vec<_>>(),
        vec![
            CandidateEvaluationPass::Probe,
            CandidateEvaluationPass::Probe,
            CandidateEvaluationPass::SelectedReplay,
        ]
    );
    assert_eq!(
        report
            .report()
            .physical_candidate_argument_evaluations()
            .iter()
            .map(|evaluation| evaluation.expected.clone())
            .collect::<Vec<_>>(),
        vec![
            CandidateExpectedType::Exact(TypeKind::String),
            CandidateExpectedType::Exact(TypeKind::I32),
            CandidateExpectedType::Exact(TypeKind::I32),
        ]
    );
    let resolution = signature_work.report().resolution();
    assert_eq!(resolution.argument_bindings(), 2);
    assert_eq!(resolution.specificity_checks(), 2);
    assert_eq!(
        report.report().retained_argument_inference_facts().count(),
        1
    );
}

fn selected_replay_work_publication() -> super::TestPublication {
    publication(
        "adapter.signature-selected-replay-work",
        "work_choice",
        [
            one_parameter_schema(
                CallableParameterType::Exact(TypeKind::String),
                CallableParameterPassing::PositionalOrNamed,
                CallableParameterPresence::Required,
                TypeKind::String,
            ),
            one_parameter_schema(
                CallableParameterType::Exact(TypeKind::I32),
                CallableParameterPassing::PositionalOrNamed,
                CallableParameterPresence::Required,
                TypeKind::Bool,
            ),
        ],
    )
}

fn candidate_argument_cancelled_report(
    prior_argument_admissions: usize,
) -> FocusedCallTypeCheckReport {
    let source = r"
fn main() -> Unit {
    let value = two_values(1i32, 2i32)
    ()
}
";
    let fixture = SignatureFixture::with_publication(
        source,
        publication(
            "adapter.signature-argument-cancellation",
            "two_values",
            [two_positional_parameter_schema(TypeKind::String)],
        ),
    );
    let cancelled = AtomicBool::new(false);
    let prior_argument_admissions = Cell::new(prior_argument_admissions);
    let control = SignatureQueryControl::new(&cancelled, None).with_cancellation_step_after(
        SignatureQueryStep::CandidateArgumentProbe,
        &prior_argument_admissions,
    );
    let linked = fixture.project.linked_module();
    let mut signature_work = SignatureQueryWorkMeter::new(PRODUCTION_SIGNATURE_LIMITS);
    let site = super::super::surface::select_signature_surface(
        &linked,
        &fixture.document,
        unique_offset(source, "2i32"),
        control,
        &mut signature_work,
    )
    .expect("surface selection succeeds before candidate cancellation")
    .site
    .expect("focused two-argument call surface");
    let mut resolver_work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let report = analyze_registered_project_types_for_signature_call(SignatureFocusedAnalysis {
        module: &linked,
        registered: &fixture.world,
        site,
        cancellation: &cancelled,
        work: &mut resolver_work,
        signature_work: &mut signature_work,
        signature_control: &control,
    })
    .expect("terminal query failure remains in the focused fact report");
    assert!(cancelled.load(std::sync::atomic::Ordering::Acquire));
    assert!(
        !report
            .report()
            .physical_candidate_argument_evaluations_overflowed()
    );
    report
}

fn fixed_literal_spread_report(
    signature_control: &dyn SignatureQueryStepControl,
    resolver_work_limit: u64,
) -> (FocusedCallTypeCheckReport, u64) {
    let source = r"
fn main() -> Unit {
    let value = spread_values([1i32, 2i32]...)
    ()
}
";
    let fixture = SignatureFixture::with_publication(
        source,
        publication(
            "adapter.signature-fixed-spread-terminal",
            "spread_values",
            [fixed_literal_spread_schema(2, TypeKind::String)],
        ),
    );
    let cancelled = AtomicBool::new(false);
    let surface_control = SignatureQueryControl::new(&cancelled, None);
    let linked = fixture.project.linked_module();
    let mut signature_work = SignatureQueryWorkMeter::new(PRODUCTION_SIGNATURE_LIMITS);
    let site = super::super::surface::select_signature_surface(
        &linked,
        &fixture.document,
        unique_offset(source, "2i32"),
        surface_control,
        &mut signature_work,
    )
    .expect("fixed-spread surface selection")
    .site
    .expect("focused fixed-spread call");
    let mut resolver_work = ResolverWork::new(resolver_work_limit);
    let report = analyze_registered_project_types_for_signature_call(SignatureFocusedAnalysis {
        module: &linked,
        registered: &fixture.world,
        site,
        cancellation: &cancelled,
        work: &mut resolver_work,
        signature_work: &mut signature_work,
        signature_control,
    })
    .expect("fixed-spread semantic result remains in the focused report");
    (report, resolver_work.consumed())
}

fn assert_completed_fixed_spread_prefix(report: &FocusedCallTypeCheckReport) {
    let evaluations = report.report().physical_candidate_argument_evaluations();
    let [evaluation] = evaluations else {
        panic!(
            "only the first admitted fixed-spread slot must remain as physical evidence, got {evaluations:?}"
        )
    };
    assert_eq!(evaluation.pass, CandidateEvaluationPass::DirectCommitted);
    assert_eq!(evaluation.argument.get(), 0);
    assert_eq!(evaluation.slot.get(), 0);
    assert_eq!(
        evaluation.kind,
        PhysicalArgumentEvaluationKind::FixedLiteralSpread
    );
    assert_eq!(
        evaluation.expected,
        CandidateExpectedType::Exact(TypeKind::I32)
    );
    assert_eq!(
        report.report().retained_argument_inference_facts().count(),
        0
    );
    assert_eq!(
        report.report().stats.registered_argument_expression_checks,
        0,
        "candidate TypeCheckStats must roll back while physical work survives"
    );
    assert!(
        !report
            .report()
            .physical_candidate_argument_evaluations_overflowed()
    );
}
