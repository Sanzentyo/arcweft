use super::*;
use crate::plan::RuntimeFunctionParameterPassing;
use crate::plan::body_semantic::entry_fixtures::controller_plan;
use crate::task::semantic::TaskSemanticEncodingError;

fn run(
    plan: &super::super::RuntimePlanInventory,
    owner: &RuntimeTaskPlanCoordinateOwner,
    limits: RuntimeTaskPlanSealLimits,
) -> (Result<blake3::Hash, RuntimeBodySemanticError>, (u64, u64)) {
    let mut meter = TaskSemanticMeter::new(limits.max_semantic_work, limits.max_transcript_bytes);
    let result =
        RuntimeExecutableSemanticRows::new(plan, owner, limits, &mut meter).and_then(|mut rows| {
            let mut encoder = TaskSemanticEncoder::new(b"test-executable-prefix.v1\0", &mut meter);
            rows.write_tables(&mut encoder, &mut |_| panic!("fixture has no task edges"))?;
            encoder.finish().map_err(Into::into)
        });
    (result, meter.totals())
}

#[test]
fn shared_function_is_walked_once_for_callable_flow_and_function_rows() {
    let (plan, owner, function) =
        controller_plan(false, true, RuntimeFunctionParameterPassing::Value, "input");
    let limits = RuntimeTaskPlanSealLimits::default();
    let mut meter = TaskSemanticMeter::new(limits.max_semantic_work, limits.max_transcript_bytes);
    let mut rows = RuntimeExecutableSemanticRows::new(&plan, &owner, limits, &mut meter).unwrap();
    let mut no_tasks = |_: super::super::flow::RuntimeBodyTaskSource<'_>| panic!("no task edge");
    let ordinal = function.get().get() as usize - 1;
    rows.complete(
        ExecutableTable::Functions,
        ordinal,
        &mut meter,
        &mut no_tasks,
    )
    .unwrap();
    let proof = rows.functions[ordinal].as_ref().unwrap();
    // Compute the exact cost of E7/E8/E9 from the completed F owner, without
    // its body. The global traversal must have precisely that same cost.
    let mut expected_meter =
        TaskSemanticMeter::new(limits.max_semantic_work, limits.max_transcript_bytes);
    let callable = rows
        .context
        .callable_executable_row_digest(
            &mut expected_meter,
            0,
            ExecutableCallableCodeSemantic::ControllerFlow(proof),
        )
        .unwrap();
    let executable = rows
        .context
        .flow_executable_row_digest(&mut expected_meter, 0, proof)
        .unwrap();
    let flow = proof
        .executable_flow_row_digest(&rows.context, &mut expected_meter, 0)
        .unwrap();
    let before = meter.totals();
    assert_eq!(
        callable,
        rows.complete(
            ExecutableTable::CallableExecutables,
            0,
            &mut meter,
            &mut no_tasks
        )
        .unwrap()
    );
    assert_eq!(
        executable,
        rows.complete(
            ExecutableTable::FlowExecutables,
            0,
            &mut meter,
            &mut no_tasks
        )
        .unwrap()
    );
    assert_eq!(
        flow,
        rows.complete(ExecutableTable::Flows, 0, &mut meter, &mut no_tasks)
            .unwrap()
    );
    let after = meter.totals();
    assert_eq!(
        (after.0 - before.0, after.1 - before.1),
        expected_meter.totals()
    );
    let done = meter.totals();
    assert_eq!(
        flow,
        rows.complete(ExecutableTable::Flows, 0, &mut meter, &mut no_tasks)
            .unwrap()
    );
    assert_eq!(meter.totals(), done);
}

fn expected_controller_prefix(
    plan: &super::super::RuntimePlanInventory,
    owner: &RuntimeTaskPlanCoordinateOwner,
    function: RuntimeFunctionSiteId,
) -> blake3::Hash {
    let limits = RuntimeTaskPlanSealLimits::default();
    let context = RuntimeBodySemanticContext::new(plan);
    let mut meter = TaskSemanticMeter::new(limits.max_semantic_work, limits.max_transcript_bytes);
    let producer = context
        .producer_function(
            &mut meter,
            function,
            owner,
            &mut |_| panic!("no task edge"),
            limits,
        )
        .unwrap();
    let ty = plan.type_table().declarations().next().unwrap();
    let local = RuntimeLocalDeclarationId::from_accepted_ordinal(NonZeroU32::MIN);
    let local_digest = context.local_row_digest(&mut meter, local).unwrap();
    let payloads = [
        (
            0,
            1,
            Some((
                ty.projection().executable_semantic_kind(),
                ty.executable_semantic_row_digest(&context, &mut meter)
                    .unwrap(),
            )),
        ),
        (1, 1, Some((0, local_digest))),
        (2, 0, None),
        (3, 0, None),
        (
            4,
            1,
            Some((
                crate::plan::RuntimeFunctionSemanticRole::Flow.semantic_tag(),
                producer
                    .executable_row_digest(&context, &mut meter)
                    .unwrap(),
            )),
        ),
        (5, 0, None),
        (
            6,
            1,
            Some((
                plan.entries()[0].kind.canonical_tag(),
                context.entry_row_digest(&mut meter, 0).unwrap(),
            )),
        ),
        (
            7,
            1,
            Some((
                2,
                context
                    .callable_executable_row_digest(
                        &mut meter,
                        0,
                        ExecutableCallableCodeSemantic::ControllerFlow(&producer),
                    )
                    .unwrap(),
            )),
        ),
        (
            8,
            1,
            Some((
                0,
                context
                    .flow_executable_row_digest(&mut meter, 0, &producer)
                    .unwrap(),
            )),
        ),
        (
            9,
            1,
            Some((
                0,
                producer
                    .executable_flow_row_digest(&context, &mut meter, 0)
                    .unwrap(),
            )),
        ),
        (10, 0, None),
        (11, 0, None),
        (12, 0, None),
        (13, 0, None),
    ];
    let mut bytes = b"test-executable-prefix.v1\0".to_vec();
    for (tag, count, payload) in payloads {
        bytes.push(tag);
        bytes.extend_from_slice(&u32::to_le_bytes(count));
        if let Some((kind, digest)) = payload {
            bytes.extend_from_slice(&0_u32.to_le_bytes());
            bytes.push(kind);
            bytes.extend_from_slice(digest.as_bytes());
        }
    }
    blake3::hash(&bytes)
}

#[test]
fn executable_prefix_has_exact_source_order_and_every_empty_table_boundary() {
    let (plan, owner, function) =
        controller_plan(false, true, RuntimeFunctionParameterPassing::Value, "input");
    let limits = RuntimeTaskPlanSealLimits::default();
    let expected = expected_controller_prefix(&plan, &owner, function);
    assert_eq!(expected, run(&plan, &owner, limits).0.unwrap());
    let renamed = controller_plan(
        false,
        true,
        RuntimeFunctionParameterPassing::Value,
        "display_only",
    );
    assert_eq!(expected, run(&renamed.0, &renamed.1, limits).0.unwrap());
    let changed = controller_plan(
        false,
        false,
        RuntimeFunctionParameterPassing::Value,
        "input",
    );
    assert_ne!(expected, run(&changed.0, &changed.1, limits).0.unwrap());
    let padded = controller_plan(true, true, RuntimeFunctionParameterPassing::Value, "input");
    // Whole executable inventories commit every row, even unreachable padding.
    assert_ne!(expected, run(&padded.0, &padded.1, limits).0.unwrap());
}

#[test]
fn global_prefix_shares_exact_limits_and_charges_reused_digest_atoms() {
    let (plan, owner, _) =
        controller_plan(false, true, RuntimeFunctionParameterPassing::Value, "input");
    let limits = RuntimeTaskPlanSealLimits::default();
    let (expected, (work, bytes)) = run(&plan, &owner, limits);
    let exact = RuntimeTaskPlanSealLimits {
        max_semantic_work: work,
        max_transcript_bytes: bytes,
        ..limits
    };
    assert_eq!(expected.unwrap(), run(&plan, &owner, exact).0.unwrap());
    for (limits, error) in [
        (
            RuntimeTaskPlanSealLimits {
                max_semantic_work: work - 1,
                ..exact
            },
            TaskSemanticEncodingError::SemanticWork,
        ),
        (
            RuntimeTaskPlanSealLimits {
                max_transcript_bytes: bytes - 1,
                ..exact
            },
            TaskSemanticEncodingError::TranscriptBytes,
        ),
    ] {
        assert!(
            matches!(run(&plan, &owner, limits).0, Err(RuntimeBodySemanticError::Encoding(actual)) if actual == error)
        );
    }
    let mut meter = TaskSemanticMeter::new(work * 2, bytes * 2);
    let mut rows = RuntimeExecutableSemanticRows::new(&plan, &owner, limits, &mut meter).unwrap();
    let emit = |rows: &mut RuntimeExecutableSemanticRows<'_, '_>, meter: &mut TaskSemanticMeter| {
        let mut encoder = TaskSemanticEncoder::new(b"test-executable-prefix.v1\0", meter);
        rows.write_tables(&mut encoder, &mut |_| panic!("no task edge"))
            .unwrap();
        encoder.finish().unwrap()
    };
    let first = emit(&mut rows, &mut meter);
    let before = meter.totals();
    assert_eq!(first, emit(&mut rows, &mut meter));
    let after = meter.totals();
    // Exact bytes in the outer prefix still count on reuse; child transcripts
    // no longer count. Every emitted digest contributes all 32 bytes.
    assert_eq!(
        after.1 - before.1,
        b"test-executable-prefix.v1\0".len() as u64 + 14 * 5 + 7 * 37
    );
    assert!(after.0 - before.0 < before.0);
}

#[test]
fn row_count_preflight_and_visiting_failure_poison_the_whole_pass() {
    let (plan, owner, _) =
        controller_plan(false, true, RuntimeFunctionParameterPassing::Value, "input");
    let limits = RuntimeTaskPlanSealLimits::default();
    let rejected = RuntimeTaskPlanSealLimits {
        max_executable_rows: 6,
        ..limits
    };
    let (result, totals) = run(&plan, &owner, rejected);
    assert!(matches!(
        result,
        Err(RuntimeBodySemanticError::ExecutableRows {
            actual: 7,
            maximum: 6
        })
    ));
    assert_eq!(totals, (0, 0));
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let mut rows = RuntimeExecutableSemanticRows::new(&plan, &owner, limits, &mut meter).unwrap();
    rows.rows[4][0] = RowState::Visiting;
    assert!(matches!(
        rows.complete(ExecutableTable::Flows, 0, &mut meter, &mut |_| panic!(
            "no task edge"
        )),
        Err(RuntimeBodySemanticError::ExecutableCycle {
            table: 4,
            ordinal: 0
        })
    ));
    assert_eq!(
        meter.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
    assert!(matches!(
        rows.complete(ExecutableTable::Types, 0, &mut meter, &mut |_| panic!(
            "no task edge"
        )),
        Err(RuntimeBodySemanticError::Encoding(
            TaskSemanticEncodingError::OwnerRejected
        ))
    ));
    assert!(matches!(rows.rows[0][0], RowState::Unvisited));
}

#[test]
fn actual_line_rows_join_the_same_global_prefix_meter() {
    let (plan, owner) = super::super::tests::actual_line_plan(true, true, "cancel", "mark");
    let limits = RuntimeTaskPlanSealLimits::default();
    let (expected, (work, bytes)) = run(&plan, &owner, limits);
    let expected = expected.unwrap();
    assert_eq!(
        expected,
        run(
            &plan,
            &owner,
            RuntimeTaskPlanSealLimits {
                max_semantic_work: work,
                max_transcript_bytes: bytes,
                ..limits
            }
        )
        .0
        .unwrap()
    );
    let changed = super::super::tests::actual_line_plan(false, true, "cancel", "mark");
    assert_ne!(expected, run(&changed.0, &changed.1, limits).0.unwrap());
}
