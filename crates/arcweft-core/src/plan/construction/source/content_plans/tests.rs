use super::super::callable_states::tests as graph;
use super::super::content_declarations::{
    PreparedRuntimeContentDeclarationSources, RuntimeContentDeclarationSourceLimits,
};
use super::super::expression::{
    RuntimeExpressionSourceHandles,
    tests::{decode, limits as expression_limits},
};
use super::super::pattern::tests::{meter, resources};
use super::*;
use crate::plan::*;

pub(in crate::plan::construction::source) fn limits(
    plan: &RuntimePlan,
) -> RuntimeContentPlanSourceLimits {
    RuntimeContentPlanSourceLimits {
        rows: 100,
        nodes: 100_000,
        elements: 1_000_000,
        encoded_bytes: 1 << 24,
        decoded_bytes: 1 << 28,
        function_count: plan.function_sites().len(),
        group_count: plan.line_task_groups().len(),
        expression: expression_limits(plan),
    }
}
fn fixture() -> RuntimePlan {
    crate::plan::body_semantic::content_root_builder_fixture()
        .0
        .finish()
        .unwrap()
}
fn group_seed() -> RuntimeLineTaskGroupSeed {
    RuntimeLineTaskGroupSeed {
        definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([229; 32]),
        activation_ops: vec![],
        result_type: crate::pattern::RuntimeSemanticTypeId::from_bytes([1; 32]),
        handle_sites: Box::new([]),
        root: RuntimeLineTaskNodeSeed::Action(vec![RuntimeFlowOpSeed::Noop]),
        cancel_rules: Box::new([]),
        cleanup_completed: vec![],
        cleanup_cancelled: vec![],
        cleanup_failed: vec![],
        cleanup_policy: crate::line_task::LineCleanupPolicy::default(),
    }
}
fn grouped_fixture() -> RuntimePlan {
    let (mut builder, content) = crate::plan::body_semantic::content_root_builder_fixture();
    // Keep the original content factory's actual handle; it is not reconstructed
    // from a line/mark label or a source metadata proof.
    let group = builder.push_line_task_group_seed(group_seed()).unwrap();
    builder
        .attach_line_task_group_seed(&content, &group)
        .unwrap();
    builder.finish().unwrap()
}
pub(in crate::plan::construction::source) fn encode(plan: &RuntimePlan) -> Vec<u8> {
    plan.encode_content_plan_sources(limits(plan), &resources(), &mut meter())
        .unwrap()
        .into_bytes()
}
pub(in crate::plan::construction::source) fn source(
    plan: &RuntimePlan,
    bank: TaskSemanticMeter,
) -> (
    super::super::RuntimePlanSourceConstruction,
    Box<[RuntimeFunctionSiteSeedId]>,
    Box<[RuntimeCallableStateSeedId]>,
) {
    let (mut source, functions, states) =
        graph::source_with_meter(plan, &graph::encode(plan), bank);
    let lim = RuntimeContentDeclarationSourceLimits {
        templates: 100,
        attempts: 100,
        elements: 100_000,
        encoded_bytes: 1 << 22,
        decoded_bytes: 1 << 24,
        type_count: plan.type_table().len(),
    };
    let encoded = plan
        .prepare_content_declaration_sources_encoding(lim, &mut source.meter)
        .unwrap();
    let mut bytes = Vec::with_capacity(encoded.encoded_bytes());
    encoded.append_to(&mut bytes).unwrap();
    let attempts =
        PreparedRuntimeContentDeclarationSources::preflight(&bytes, lim, &mut source.meter)
            .unwrap()
            .reserve(&mut source)
            .unwrap();
    assert!(attempts.is_empty());
    (source, functions, states)
}
fn handles(states: &[RuntimeCallableStateSeedId]) -> RuntimeExpressionSourceHandles<'_> {
    RuntimeExpressionSourceHandles {
        states,
        specializations: &[],
        helpers: &[],
        methods: &[],
        attempts: &[],
    }
}
pub(in crate::plan::construction::source) fn bodies(
    plan: &RuntimePlan,
    source: &mut super::super::RuntimePlanSourceConstruction,
    functions: &[RuntimeFunctionSiteSeedId],
    states: &[RuntimeCallableStateSeedId],
    contents: &[RuntimeDialogueContentPlanSeedId],
) {
    for (function, handle) in plan.function_sites().iter().zip(functions) {
        let body = match function.body() {
            RuntimeFunctionSiteBody::Expression(expr) => RuntimeFunctionSiteBodySeed::Expression(
                decode(expr, plan, source, &handles(states)),
            ),
            RuntimeFunctionSiteBody::Executable(body) => {
                let limits = super::super::flow::tests::limits(plan);
                let encoded = plan
                    .encode_flow_source(body.ops(), limits, &resources(), &mut source.meter)
                    .unwrap();
                let prepared = super::super::flow::PreparedRuntimeFlowSource::preflight(
                    encoded.bytes(),
                    limits,
                    &resources(),
                    &mut source.meter,
                )
                .unwrap();
                let expressions = handles(states);
                let flow_handles = super::super::flow::RuntimeFlowSourceHandles {
                    expression: &expressions,
                    contents,
                    defers: &[],
                    tasks: &[],
                };
                let ops = prepared
                    .materialize(source, &flow_handles, &mut resources())
                    .unwrap();
                RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                    effects: body.effects().clone(),
                    ops: ops.into_boxed_slice(),
                })
            }
        };
        source
            .builder
            .define_function_site_seed(handle, body)
            .unwrap();
    }
}
pub(in crate::plan::construction::source) fn finish(
    source: super::super::RuntimePlanSourceConstruction,
) -> RuntimePlan {
    let seal = RuntimeTaskPlanSealLimits::default();
    source
        .prepare_image(seal)
        .unwrap()
        .seal(seal, None)
        .unwrap()
        .into_plan()
}
#[test]
fn content_plan_sources_restore_original_values_effect_states_captures_marks_and_semantics() {
    let plan = fixture();
    let bytes = encode(&plan);
    let encoded = plan
        .encode_content_plan_sources(limits(&plan), &resources(), &mut meter())
        .unwrap();
    let (mut source, functions, states) = source(&plan, meter());
    let prepared = PreparedRuntimeContentPlanSources::preflight(
        &bytes,
        limits(&plan),
        &resources(),
        &mut source.meter,
    )
    .unwrap();
    assert_eq!(prepared.storage_bytes(), encoded.storage_bytes());
    assert_eq!(prepared.elements(), encoded.elements());
    assert_eq!(prepared.node_count(), encoded.node_count());
    let content = prepared
        .materialize(&mut source, &functions, &handles(&states), &mut resources())
        .unwrap();
    bodies(&plan, &mut source, &functions, &states, &content);
    prepared
        .bind_line_groups(&mut source, &content, &[])
        .unwrap();
    let restored = finish(source);
    assert_eq!(restored.dialogue_content(), plan.dialogue_content());
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(restored.callable_states(), plan.callable_states());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    assert_eq!(encode(&restored), bytes);
}

#[test]
fn empty_content_plan_sources_round_trip_with_zero_payload_caps_and_a_real_empty_owner() {
    let plan = RuntimePlanBuilder::new().finish().unwrap();
    let bytes = encode(&plan);
    let exact = RuntimeContentPlanSourceLimits {
        rows: 0,
        nodes: 0,
        elements: 0,
        decoded_bytes: 0,
        encoded_bytes: bytes.len(),
        ..limits(&plan)
    };
    let (mut source, functions, states) = source(&plan, meter());
    let prepared = PreparedRuntimeContentPlanSources::preflight(
        &bytes,
        exact,
        &resources(),
        &mut source.meter,
    )
    .unwrap();
    let content = prepared
        .materialize(&mut source, &functions, &handles(&states), &mut resources())
        .unwrap();
    prepared
        .bind_line_groups(&mut source, &content, &[])
        .unwrap();
    assert_eq!(finish(source), plan);
}
#[test]
fn content_plan_sources_bind_original_line_group_links_only_through_actual_group_factories() {
    let plan = grouped_fixture();
    let bytes = encode(&plan);
    let (mut source, functions, states) = source(&plan, meter());
    let prepared = PreparedRuntimeContentPlanSources::preflight(
        &bytes,
        limits(&plan),
        &resources(),
        &mut source.meter,
    )
    .unwrap();
    let content = prepared
        .materialize(&mut source, &functions, &handles(&states), &mut resources())
        .unwrap();
    bodies(&plan, &mut source, &functions, &states, &content);
    // Line graph/operation transport is separate; supply the exact genuine
    // typed group seed and let its existing owner build the original graph.
    let group = source
        .builder
        .push_line_task_group_seed(group_seed())
        .unwrap();
    prepared
        .bind_line_groups(&mut source, &content, &[group])
        .unwrap();
    let restored = finish(source);
    assert_eq!(restored.dialogue_content(), plan.dialogue_content());
    assert_eq!(restored.line_task_groups(), plan.line_task_groups());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    assert_eq!(encode(&restored), bytes);
}
#[test]
fn content_plan_sources_share_exact_aggregate_caps_and_reject_one_less() {
    let plan = fixture();
    let bytes = encode(&plan);
    let prepared = PreparedRuntimeContentPlanSources::preflight(
        &bytes,
        limits(&plan),
        &resources(),
        &mut meter(),
    )
    .unwrap();
    let exact = RuntimeContentPlanSourceLimits {
        rows: 1,
        nodes: prepared.node_count() as u32,
        elements: prepared.elements() as u32,
        encoded_bytes: bytes.len(),
        decoded_bytes: prepared.storage_bytes(),
        ..limits(&plan)
    };
    PreparedRuntimeContentPlanSources::preflight(&bytes, exact, &resources(), &mut meter())
        .unwrap();
    plan.encode_content_plan_sources(exact, &resources(), &mut meter())
        .unwrap();
    for short in [
        RuntimeContentPlanSourceLimits { rows: 0, ..exact },
        RuntimeContentPlanSourceLimits {
            nodes: exact.nodes - 1,
            ..exact
        },
        RuntimeContentPlanSourceLimits {
            elements: exact.elements - 1,
            ..exact
        },
        RuntimeContentPlanSourceLimits {
            encoded_bytes: bytes.len() - 1,
            ..exact
        },
        RuntimeContentPlanSourceLimits {
            decoded_bytes: exact.decoded_bytes - 1,
            ..exact
        },
    ] {
        let mut bank = meter();
        assert!(
            PreparedRuntimeContentPlanSources::preflight(&bytes, short, &resources(), &mut bank)
                .is_err()
        );
        assert!(bank.status().is_err());
        assert!(
            plan.encode_content_plan_sources(short, &resources(), &mut meter())
                .is_err()
        );
    }
}
#[test]
fn content_plan_sources_reject_all_truncated_prefixes_bad_domain_version_refs_flags_and_trailing_bytes()
 {
    let plan = fixture();
    let bytes = encode(&plan);
    for end in 0..bytes.len() {
        assert!(
            PreparedRuntimeContentPlanSources::preflight(
                &bytes[..end],
                limits(&plan),
                &resources(),
                &mut meter()
            )
            .is_err(),
            "prefix {end}"
        );
    }
    for (offset, byte) in [(0, 0), (DOMAIN.len(), 2), (bytes.len() - 1, 2)] {
        let mut bad = bytes.clone();
        bad[offset] = byte;
        assert!(
            PreparedRuntimeContentPlanSources::preflight(
                &bad,
                limits(&plan),
                &resources(),
                &mut meter()
            )
            .is_err()
        );
    }
    let mut cursor = cursor::Cursor::new(&bytes, limits(&plan));
    cursor.header().unwrap();
    let row = cursor.row().unwrap();
    let value = row.values.as_ptr() as usize - bytes.as_ptr() as usize;
    let effect = row.effects.as_ptr() as usize - bytes.as_ptr() as usize;
    let mark = row.marks.as_ptr() as usize - bytes.as_ptr() as usize;
    let digest = plan.dialogue_content_templates().rows()[0].digest();
    let template = bytes
        .windows(32)
        .position(|p| p == digest.as_bytes())
        .unwrap()
        - 4;
    for offset in [template, value, value + 5, effect + 4, mark] {
        let mut bad = bytes.clone();
        bad[offset] = 0;
        assert!(
            PreparedRuntimeContentPlanSources::preflight(
                &bad,
                limits(&plan),
                &resources(),
                &mut meter()
            )
            .is_err()
        );
    }
    let mut bad_role = bytes.clone();
    bad_role[value + 4] = 3;
    assert!(
        PreparedRuntimeContentPlanSources::preflight(
            &bad_role,
            limits(&plan),
            &resources(),
            &mut meter()
        )
        .is_err()
    );
    let mut bad = bytes;
    bad.push(0);
    assert!(matches!(
        PreparedRuntimeContentPlanSources::preflight(
            &bad,
            limits(&plan),
            &resources(),
            &mut meter()
        ),
        Err(Error::Trailing { .. })
    ));
}
#[test]
fn content_plan_sources_late_duplicate_cannot_publish_an_earlier_valid_application() {
    let plan = fixture();
    let bytes = encode(&plan);
    let header = DOMAIN.len() + 1;
    let mut bad = bytes.clone();
    bad[header..header + 4].copy_from_slice(&2u32.to_le_bytes());
    bad.extend_from_slice(&bytes[header + 4..]);
    let (mut source, functions, states) = source(&plan, meter());
    let prepared = PreparedRuntimeContentPlanSources::preflight(
        &bad,
        limits(&plan),
        &resources(),
        &mut source.meter,
    )
    .unwrap();
    assert!(matches!(
        prepared.materialize(&mut source, &functions, &handles(&states), &mut resources()),
        Err(Error::Construction(RuntimePlanBuildError::DialogueContent(
            RuntimeDialogueContentPlanTableError::DuplicateApplication { .. }
        )))
    ));
    assert_eq!(source.builder.dialogue_content.len(), 1);
    assert!(source.meter.status().is_err());
    assert!(
        source
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
}
#[test]
fn content_plan_sources_reject_foreign_content_and_group_handles_before_attaching_any_row() {
    let plan = grouped_fixture();
    let bytes = encode(&plan);
    for foreign_group in [false, true] {
        let (mut source, functions, states) = source(&plan, meter());
        let prepared = PreparedRuntimeContentPlanSources::preflight(
            &bytes,
            limits(&plan),
            &resources(),
            &mut source.meter,
        )
        .unwrap();
        let content = prepared
            .materialize(&mut source, &functions, &handles(&states), &mut resources())
            .unwrap();
        let group = source
            .builder
            .push_line_task_group_seed(group_seed())
            .unwrap();
        let (mut other, other_functions, other_states) = self::source(&plan, meter());
        let other_content = prepared
            .materialize(
                &mut other,
                &other_functions,
                &handles(&other_states),
                &mut resources(),
            )
            .unwrap();
        let other_group = other
            .builder
            .push_line_task_group_seed(group_seed())
            .unwrap();
        let result = if foreign_group {
            prepared.bind_line_groups(&mut source, &content, &[other_group])
        } else {
            prepared.bind_line_groups(&mut source, &other_content, &[group])
        };
        assert!(matches!(result, Err(Error::Handle { .. })));
        let id = content[0].resolve(&source.builder.issuer).unwrap();
        assert!(
            source
                .builder
                .dialogue_content
                .get(id)
                .unwrap()
                .line_task_group()
                .is_none()
        );
        assert!(source.meter.status().is_err());
    }
}
#[test]
fn content_plan_sources_authenticate_all_function_and_state_handles_before_retaining_rows() {
    let plan = fixture();
    let bytes = encode(&plan);
    for kind in 0..3 {
        let (mut source, mut functions, mut states) = source(&plan, meter());
        let (_, other_functions, other_states) = self::source(&plan, meter());
        match kind {
            0 => functions.swap(0, 1),
            1 => functions = other_functions,
            _ => states = other_states,
        }
        let prepared = PreparedRuntimeContentPlanSources::preflight(
            &bytes,
            limits(&plan),
            &resources(),
            &mut source.meter,
        )
        .unwrap();
        assert!(
            prepared
                .materialize(&mut source, &functions, &handles(&states), &mut resources())
                .is_err()
        );
        assert_eq!(source.builder.dialogue_content.len(), 0);
        assert!(source.meter.status().is_err());
    }
}
#[test]
fn content_plan_sources_reject_conflicting_original_template_identity_before_capture_retention() {
    let plan = fixture();
    let mut bytes = encode(&plan);
    let digest = plan.dialogue_content_templates().rows()[0].digest();
    let at = bytes
        .windows(32)
        .position(|p| p == digest.as_bytes())
        .unwrap();
    bytes[at] ^= 1;
    let (mut source, functions, states) = source(&plan, meter());
    let prepared = PreparedRuntimeContentPlanSources::preflight(
        &bytes,
        limits(&plan),
        &resources(),
        &mut source.meter,
    )
    .unwrap();
    assert!(matches!(
        prepared.materialize(&mut source, &functions, &handles(&states), &mut resources()),
        Err(Error::Construction(
            RuntimePlanBuildError::DialogueTemplateManifest(_)
        ))
    ));
    assert_eq!(source.builder.dialogue_content.len(), 0);
    assert!(
        source
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
}
#[test]
fn content_plan_sources_preserve_original_work_bank_output_prefix_and_missing_body_rejection() {
    let plan = fixture();
    let bytes = encode(&plan);
    let (trial, _, _) = source(&plan, meter());
    let used = trial.meter.totals().0;
    let (mut constrained, _, _) = source(&plan, TaskSemanticMeter::new(used + 1, u64::MAX));
    assert!(matches!(
        PreparedRuntimeContentPlanSources::preflight(
            &bytes,
            limits(&plan),
            &resources(),
            &mut constrained.meter
        ),
        Err(Error::Resource(TaskSemanticEncodingError::SemanticWork))
    ));
    assert!(
        constrained
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
    let encoded = plan
        .encode_content_plan_sources(limits(&plan), &resources(), &mut meter())
        .unwrap();
    let mut output = Vec::with_capacity(encoded.bytes().len());
    output.push(29);
    assert!(matches!(
        encoded.append_to(&mut output),
        Err(Error::OutputCapacity { .. })
    ));
    assert_eq!(output, [29]);
    let (mut source, functions, states) = source(&plan, meter());
    PreparedRuntimeContentPlanSources::preflight(
        &bytes,
        limits(&plan),
        &resources(),
        &mut source.meter,
    )
    .unwrap()
    .materialize(&mut source, &functions, &handles(&states), &mut resources())
    .unwrap();
    assert!(
        source
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
}
