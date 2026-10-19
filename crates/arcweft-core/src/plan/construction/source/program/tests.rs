use super::super::pattern::tests::{meter, resources};
use super::*;
use crate::engine::Engine;
use crate::plan::{RuntimePlan, RuntimePlanBuilder, RuntimeTaskPlanSealLimits};
use crate::step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions};
use crate::value::RuntimeValue;
use std::sync::Arc;

fn limits() -> RuntimeProgramSourceLimits {
    RuntimeProgramSourceLimits {
        rows: 100_000,
        nodes: 100_000,
        elements: 1_000_000,
        depth: 128,
        policy_depth: 128,
        encoded_bytes: 1 << 24,
        decoded_bytes: 1 << 28,
        opaque_producer_bytes: 1 << 16,
    }
}
fn decode(
    bytes: &[u8],
    limits: RuntimeProgramSourceLimits,
) -> Result<(RuntimePlan, Footprint), Error> {
    let mut bank = meter();
    let mut values = resources();
    let prepared = PreparedRuntimeProgramSource::preflight(bytes, limits, &values, &mut bank)?;
    let original = prepared.materialize(bank, &mut values)?;
    assert!(
        original.tasks.is_empty(),
        "these original program fixtures have no Task definitions"
    );
    let footprint = original.footprint;
    let seal = RuntimeTaskPlanSealLimits::default();
    let plan = original
        .source
        .prepare_image(seal)?
        .seal(seal, None)?
        .into_plan();
    Ok((plan, footprint))
}
fn fixtures() -> [RuntimePlan; 7] {
    [
        RuntimePlanBuilder::new().finish().unwrap(),
        super::super::pure_callables::tests::fixture(),
        super::super::invocations::tests::fixture(),
        super::super::callable_specializations::tests::fixture(),
        super::super::line_groups::tests::fixture(
            crate::line_task::LineCleanupPolicy::default(),
            crate::line_task::ChildCancelPolicy::CancelAndJoin,
        ),
        super::super::line_groups::tests::handles::fixture(),
        super::super::streams::tests::fixture(),
    ]
}
#[test]
fn program_source_rebuilds_complete_original_inventory_without_an_external_inventory_then_common_seals()
 {
    for plan in fixtures() {
        let encoded = plan
            .encode_program_source(limits(), &resources(), &mut meter())
            .unwrap();
        let (restored, footprint) = decode(encoded.bytes(), limits()).unwrap();
        assert_eq!(footprint.nodes, encoded.node_count());
        assert_eq!(footprint.elements, encoded.elements());
        assert_eq!(footprint.storage, encoded.storage_bytes());
        assert_eq!(restored.inventory, plan.inventory);
        assert_eq!(
            restored.executable_semantic_digest(),
            plan.executable_semantic_digest()
        );
        assert_eq!(
            restored
                .encode_program_source(limits(), &resources(), &mut meter())
                .unwrap()
                .bytes(),
            encoded.bytes()
        );
    }
}
#[test]
fn program_source_shares_exact_aggregate_caps_and_preserves_original_work_failure() {
    let plan = super::super::invocations::tests::fixture();
    let encoded = plan
        .encode_program_source(limits(), &resources(), &mut meter())
        .unwrap();
    let exact = RuntimeProgramSourceLimits {
        nodes: u32::try_from(encoded.node_count()).unwrap(),
        elements: u32::try_from(encoded.elements()).unwrap(),
        encoded_bytes: encoded.bytes().len(),
        decoded_bytes: encoded.storage_bytes(),
        ..limits()
    };
    decode(encoded.bytes(), exact).unwrap();
    assert_eq!(
        plan.encode_program_source(exact, &resources(), &mut meter())
            .unwrap()
            .bytes(),
        encoded.bytes()
    );
    for short in [
        RuntimeProgramSourceLimits {
            nodes: exact.nodes - 1,
            ..exact
        },
        RuntimeProgramSourceLimits {
            elements: exact.elements - 1,
            ..exact
        },
        RuntimeProgramSourceLimits {
            encoded_bytes: exact.encoded_bytes - 1,
            ..exact
        },
        RuntimeProgramSourceLimits {
            decoded_bytes: exact.decoded_bytes - 1,
            ..exact
        },
    ] {
        assert!(decode(encoded.bytes(), short).is_err());
        assert!(
            plan.encode_program_source(short, &resources(), &mut meter())
                .is_err()
        );
    }
    let mut bank = TaskSemanticMeter::new(0, 0);
    let first =
        PreparedRuntimeProgramSource::preflight(encoded.bytes(), exact, &resources(), &mut bank)
            .err()
            .unwrap();
    let second =
        PreparedRuntimeProgramSource::preflight(encoded.bytes(), exact, &resources(), &mut bank)
            .err()
            .unwrap();
    assert!(matches!(
        (first, second),
        (
            Error::Resource(TaskSemanticEncodingError::SemanticWork),
            Error::Resource(TaskSemanticEncodingError::SemanticWork)
        )
    ));
}
#[test]
fn program_source_rejects_all_truncations_bad_children_versions_namespaces_and_trailing_data() {
    let plan = super::super::pure_callables::tests::fixture();
    let encoded = plan
        .encode_program_source(limits(), &resources(), &mut meter())
        .unwrap();
    for end in 0..encoded.bytes().len() {
        assert!(
            decode(&encoded.bytes()[..end], limits()).is_err(),
            "prefix {end}"
        );
    }
    for offset in [0, DOMAIN.len(), DOMAIN.len() + 1 + 3 * 4] {
        let mut bytes = encoded.bytes().to_vec();
        bytes[offset] = 0;
        assert!(decode(&bytes, limits()).is_err(), "offset {offset}");
    }
    let frames = Frames::read(encoded.bytes(), limits()).unwrap();
    for section in Section::ALL {
        let child = frames.get(section);
        let offset = child.as_ptr() as usize - encoded.bytes().as_ptr() as usize;
        let mut bytes = encoded.bytes().to_vec();
        bytes[offset] = 0;
        assert!(
            decode(&bytes, limits()).is_err(),
            "section {}",
            section.index()
        );
    }
    let mut bytes = encoded.bytes().to_vec();
    bytes.push(0);
    assert!(matches!(
        decode(&bytes, limits()),
        Err(Error::Trailing { .. })
    ));
}
#[test]
fn program_source_restored_original_trait_and_forward_helper_graph_executes_in_native() {
    let plan = super::super::pure_callables::tests::fixture();
    let encoded = plan
        .encode_program_source(limits(), &resources(), &mut meter())
        .unwrap();
    for input in [false, true] {
        let (restored, _) = decode(encoded.bytes(), limits()).unwrap();
        let program = restored.pure_programs()[0].program();
        let mut engine = Engine::for_program_invocation(
            Arc::new(restored),
            program,
            vec![RuntimeValue::Bool(input)],
        )
        .unwrap();
        let report = engine.step(
            RuntimeStepInput::default(),
            RuntimeStepOptions {
                mode: RuntimeStepMode::Drain,
                budget: RuntimeStepBudget { max_ops: 64 },
                ..RuntimeStepOptions::default()
            },
        );
        assert!(
            report.output.diagnostics.is_empty(),
            "{:?}",
            report.output.diagnostics
        );
        let (actual, result) = engine.take_program_result().unwrap().unwrap();
        assert_eq!(actual, program);
        assert_eq!(result, RuntimeValue::Bool(input));
    }
}

#[test]
fn program_source_preserves_project_site_ids_when_bodies_were_defined_in_another_order() {
    let plan = super::super::project_calls::tests::differently_defined_body_fixture();
    let encoded = plan
        .encode_program_source(limits(), &resources(), &mut meter())
        .unwrap();
    let (restored, _) = decode(encoded.bytes(), limits()).unwrap();
    assert_eq!(restored.project_call_sites(), plan.project_call_sites());
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(restored.local_declarations(), plan.local_declarations());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    assert_eq!(
        restored
            .encode_program_source(limits(), &resources(), &mut meter())
            .unwrap()
            .bytes(),
        encoded.bytes()
    );
}

#[test]
fn program_source_counts_task_reservations_in_original_work_bank_and_keeps_them_unsealed() {
    let plan = RuntimePlanBuilder::new().finish().unwrap();
    let mut bytes = plan
        .encode_program_source(limits(), &resources(), &mut meter())
        .unwrap()
        .bytes()
        .to_vec();
    let restore = |bytes: &[u8], work| {
        let mut bank = TaskSemanticMeter::new(work, u64::MAX);
        let mut values = resources();
        PreparedRuntimeProgramSource::preflight(bytes, limits(), &values, &mut bank)?
            .materialize(bank, &mut values)
    };
    let baseline = restore(&bytes, u64::MAX).unwrap().source.meter.totals().0;
    // An untrusted namespace can request slots without supplying definitions.
    let offset = DOMAIN.len() + 1 + 12 * 4;
    bytes[offset..offset + 4].copy_from_slice(&17_u32.to_le_bytes());
    let original = restore(&bytes, u64::MAX).unwrap();
    assert_eq!(original.tasks.len(), 17);
    for (ordinal, task) in original.tasks.iter().enumerate() {
        assert_eq!(task.ordinal() as usize, ordinal);
        assert!(task.same_candidate(&original.tasks[0]));
        assert!(task.sealed_plan().is_none());
    }
    let work = original.source.meter.totals().0;
    // Reservation and the body/Line namespace authentications each visit the
    // genuine slots under this same bank, even when all body tables are empty.
    assert_eq!(work, baseline + 3 * 17);
    assert!(matches!(
        original
            .source
            .prepare_image(RuntimeTaskPlanSealLimits::default()),
        Err(RuntimeTaskPlanSealError::IncompleteDefinition { ordinal: 0 })
    ));
    assert!(
        original
            .tasks
            .iter()
            .all(|task| task.sealed_plan().is_none())
    );
    restore(&bytes, work).unwrap();
    assert!(matches!(
        restore(&bytes, work - 1),
        Err(Error::Resource(TaskSemanticEncodingError::SemanticWork))
    ));
}
