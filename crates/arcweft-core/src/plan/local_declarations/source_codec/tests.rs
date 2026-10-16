use super::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeEffectSet, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFunctionInputBindingSeed,
    RuntimeFunctionInputOrigin, RuntimeFunctionInputOwnershipRequirement,
    RuntimeFunctionInputSource, RuntimeFunctionInputTransfer, RuntimeFunctionParameterPassing,
    RuntimeFunctionSemanticRole, RuntimeFunctionSiteBodyKind, RuntimeFunctionSiteDeclarationSeed,
    RuntimeFunctionTypeContract, RuntimeLocalDeclarationSeed, RuntimeLocalPlacementError,
    RuntimePatternSeed, RuntimePatternSeedKind, RuntimePlan, RuntimePlanBuilder,
    RuntimePlanTypeProjection, RuntimePlanTypeSeed, RuntimeTypeBinder, RuntimeTypeScope,
};
use crate::runtime_id::RuntimeLocalDeclarationId;
use crate::value::RuntimeValue;

fn meter() -> TaskSemanticMeter {
    TaskSemanticMeter::new(u64::MAX, u64::MAX)
}
fn limits(types: usize) -> RuntimeLocalSourceLimits {
    RuntimeLocalSourceLimits {
        rows: 128,
        encoded_bytes: 65536,
        decoded_bytes: 65536,
        type_count: types,
    }
}
fn type_identity(value: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([value; 32])
}
fn literal(ty: RuntimeSemanticTypeId) -> RuntimeExprSeed {
    RuntimeExprSeed::new(ty, RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)))
}

fn make_plan(sources: &[RuntimeLocalDeclarationSource]) -> RuntimePlan {
    let mut builder = RuntimePlanBuilder::new();
    let ty = type_identity(1);
    let admitted = builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                ty,
                RuntimePlanTypeProjection::Bool,
            )],
            sources
                .iter()
                .map(|source| RuntimeLocalDeclarationSeed::new(*source, ty)),
        )
        .unwrap();
    let mut body = literal(ty);
    for local in admitted.local_ids().iter().rev() {
        body = RuntimeExprSeed::new(
            ty,
            RuntimeExprSeedKind::Let {
                binding: local.clone(),
                expr: Box::new(literal(ty)),
                body: Box::new(body),
            },
        );
    }
    builder
        .push_function_site_seed(
            RuntimeFunctionDefinitionIdentity::from_accepted_identity([9; 32]),
            RuntimeFunctionSemanticRole::Ordinary,
            [],
            body,
        )
        .unwrap();
    builder.finish().unwrap()
}

fn all_sources() -> Vec<RuntimeLocalDeclarationSource> {
    let mut sources = Vec::new();
    for kind in [
        RuntimeLocalBindingKind::Parameter,
        RuntimeLocalBindingKind::LetBinding,
        RuntimeLocalBindingKind::PatternBinding,
        RuntimeLocalBindingKind::ClosureParameter,
        RuntimeLocalBindingKind::LoopBinding,
        RuntimeLocalBindingKind::MatchBinding,
        RuntimeLocalBindingKind::PostconditionResult,
    ] {
        for mutable in [false, true] {
            for storage in [
                RuntimeLocalBindingStorage::Derived,
                RuntimeLocalBindingStorage::RetainedState,
            ] {
                let marker = u8::try_from(sources.len() + 1).unwrap();
                sources.push(RuntimeLocalDeclarationSource::Binding {
                    identity: [marker; 32],
                    declaration: RuntimeLocalBindingDeclaration::new(kind, mutable, storage),
                });
            }
        }
    }
    sources.extend([
        RuntimeLocalDeclarationSource::Parameter(
            RuntimeFunctionParameterIdentity::from_accepted_identity([200; 32]),
        ),
        RuntimeLocalDeclarationSource::EvaluatedResult(
            RuntimeFunctionDefinitionIdentity::from_accepted_identity([9; 32]),
        ),
        RuntimeLocalDeclarationSource::Generated(
            RuntimeGeneratedLocalOrigin::from_accepted_identity([202; 32]),
        ),
    ]);
    sources
}

fn place(builder: &mut RuntimeLocalDeclarationTableBuilder, plan: &RuntimePlan) {
    builder
        .admit_executable_placements(
            plan.function_sites(),
            plan.project_call_sites(),
            plan.pure_helpers(),
            plan.trait_methods(),
            plan.line_task_groups(),
            plan.stream_plans(),
        )
        .unwrap();
}

fn scoped_plan() -> RuntimePlan {
    let mut builder = RuntimePlanBuilder::new();
    let binder = RuntimeTypeBinder::new(1, 0, 0);
    let scope = RuntimeTypeScope::root().enter(binder).unwrap();
    let parameter = RuntimeFunctionParameterIdentity::from_accepted_identity([81; 32]);
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(
                    type_identity(1),
                    RuntimePlanTypeProjection::BoundType(scope.bound_type(0, 0).unwrap()),
                )
                .with_scope(scope),
                RuntimePlanTypeSeed::new(type_identity(2), RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    type_identity(3),
                    RuntimePlanTypeProjection::Function {
                        contract: RuntimeFunctionTypeContract::new(
                            binder,
                            crate::effect_row::EffectPredicate::unconstrained(),
                            crate::effect_row::EffectFormula::empty(),
                        ),
                        parameters: Box::new([type_identity(1)]),
                        result: type_identity(2),
                    },
                ),
            ],
            [RuntimeLocalDeclarationSeed::in_function(
                RuntimeLocalDeclarationSource::Parameter(parameter),
                type_identity(1),
                type_identity(3),
            )],
        )
        .unwrap();
    let local = admitted.local_ids()[0].clone();
    let site = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([41; 32]),
            role: RuntimeFunctionSemanticRole::Ordinary,
            function_type: Some(type_identity(3)),
            inputs: Box::new([RuntimeFunctionInputBindingSeed {
                transfer: RuntimeFunctionInputTransfer::Formal,
                origin: RuntimeFunctionInputOrigin::Parameter(parameter),
                source: RuntimeFunctionInputSource::Parameter {
                    position: 0,
                    passing: RuntimeFunctionParameterPassing::Affine,
                },
                input_local: local.clone(),
                pattern: RuntimePatternSeed::new(
                    type_identity(1),
                    RuntimePatternSeedKind::Bind {
                        mutable: false,
                        local,
                    },
                ),
                ownership: RuntimeFunctionInputOwnershipRequirement::default(),
                unrestricted_bindings: Box::new([]),
            }]),
            result: type_identity(2),
            body_kind: RuntimeFunctionSiteBodyKind::Expression,
            effects: RuntimeEffectSet::empty(),
        })
        .unwrap();
    builder
        .define_function_site_seed(&site, literal(type_identity(2)))
        .unwrap();
    builder.finish().unwrap()
}

#[test]
fn original_sources_round_trip_only_after_actual_executable_placement_admission() {
    let plan = make_plan(&all_sources());
    let limits = limits(plan.type_table().len());
    let bytes = plan
        .local_declarations()
        .encode_sources(limits, &mut meter())
        .unwrap();
    let mut bank = meter();
    let source =
        RuntimeLocalDeclarationTableBuilder::preflight_sources(&bytes, limits, &mut bank).unwrap();
    assert_eq!(source.row_count(), all_sources().len());
    let mut decoded = source.materialize(plan.type_table(), &mut bank).unwrap();
    assert_eq!(decoded.len(), plan.local_declarations().len());
    assert!(matches!(
        decoded.take_finish(),
        Err(RuntimeLocalPlacementError::Unowned { .. })
    ));
    assert_eq!(decoded.len(), plan.local_declarations().len());
    assert!(decoded.prepare_request_count(1).is_err());
    place(&mut decoded, &plan);
    let actual = decoded.take_finish().unwrap();
    assert_eq!(&actual, plan.local_declarations());
    assert_eq!(actual.encode_sources(limits, &mut meter()).unwrap(), bytes);
}

#[test]
fn original_generic_context_and_parameter_identity_are_retained() {
    let plan = scoped_plan();
    let limits = limits(plan.type_table().len());
    let bytes = plan
        .local_declarations()
        .encode_sources(limits, &mut meter())
        .unwrap();
    let mut bank = meter();
    let source =
        RuntimeLocalDeclarationTableBuilder::preflight_sources(&bytes, limits, &mut bank).unwrap();
    let mut decoded = source.materialize(plan.type_table(), &mut bank).unwrap();
    place(&mut decoded, &plan);
    assert_eq!(&decoded.take_finish().unwrap(), plan.local_declarations());
}

#[test]
fn type_and_local_frames_preflight_together_on_one_original_bank() {
    use crate::plan::type_table::source_codec::{RuntimeTypeSourceError, RuntimeTypeSourceLimits};
    let plan = scoped_plan();
    let type_limits = RuntimeTypeSourceLimits {
        rows: 128,
        elements: 65536,
        encoded_bytes: 65536,
        decoded_bytes: 65536,
        policy_depth: 128,
    };
    let type_bytes = plan
        .type_table()
        .encode_sources(type_limits, &mut meter())
        .unwrap();
    let local_bytes = plan
        .local_declarations()
        .encode_sources(limits(3), &mut meter())
        .unwrap();
    let work = 3 * u64::try_from(type_bytes.len() + local_bytes.len()).unwrap();
    let mut bank = TaskSemanticMeter::new(work, u64::MAX);
    let types =
        RuntimePlanTypeTable::preflight_sources(&type_bytes, type_limits, &mut bank).unwrap();
    let locals = RuntimeLocalDeclarationTableBuilder::preflight_sources(
        &local_bytes,
        limits(types.row_count()),
        &mut bank,
    )
    .unwrap();
    let decoded_types = types.materialize(&mut bank).unwrap();
    assert_eq!(&decoded_types, plan.type_table());
    let mut decoded_locals = locals.materialize(&decoded_types, &mut bank).unwrap();
    place(&mut decoded_locals, &plan);
    assert_eq!(
        &decoded_locals.take_finish().unwrap(),
        plan.local_declarations()
    );
    assert_eq!(
        bank.charge_work(1),
        Err(TaskSemanticEncodingError::SemanticWork)
    );
    let mut corrupt = local_bytes;
    corrupt[0] ^= 1;
    let mut bank = meter();
    let types =
        RuntimePlanTypeTable::preflight_sources(&type_bytes, type_limits, &mut bank).unwrap();
    assert!(matches!(
        RuntimeLocalDeclarationTableBuilder::preflight_sources(
            &corrupt,
            limits(types.row_count()),
            &mut bank
        ),
        Err(RuntimeLocalSourceError::Domain)
    ));
    assert_eq!(
        types.materialize(&mut bank),
        Err(RuntimeTypeSourceError::Resource(
            TaskSemanticEncodingError::OwnerRejected
        ))
    );
}

#[test]
fn canonical_version_one_wire_contains_source_properties_and_no_placement_proof() {
    let plan = make_plan(&[RuntimeLocalDeclarationSource::Binding {
        identity: [5; 32],
        declaration: RuntimeLocalBindingDeclaration::new(
            RuntimeLocalBindingKind::LetBinding,
            true,
            RuntimeLocalBindingStorage::RetainedState,
        ),
    }]);
    let bytes = plan
        .local_declarations()
        .encode_sources(limits(1), &mut meter())
        .unwrap();
    let mut expected = b"arcweft.runtime.local-declarations.v1\0".to_vec();
    expected.extend_from_slice(&[1, 1, 0, 0, 0]);
    expected.extend_from_slice(&[0, 0, 0, 0, 0]);
    expected.extend_from_slice(&[5; 32]);
    expected.extend_from_slice(&[1, 1, 1, 1, 0, 0, 0, 0]);
    assert_eq!(bytes, expected);
}

#[test]
fn all_truncations_and_unknown_tags_ordinals_and_types_fail_whole_frame_admission() {
    let plan = make_plan(&[RuntimeLocalDeclarationSource::Binding {
        identity: [5; 32],
        declaration: RuntimeLocalBindingDeclaration::new(
            RuntimeLocalBindingKind::LetBinding,
            true,
            RuntimeLocalBindingStorage::RetainedState,
        ),
    }]);
    let bytes = plan
        .local_declarations()
        .encode_sources(limits(1), &mut meter())
        .unwrap();
    for cut in 0..bytes.len() {
        assert!(
            RuntimeLocalDeclarationTableBuilder::preflight_sources(
                &bytes[..cut],
                limits(1),
                &mut meter()
            )
            .is_err(),
            "cut {cut}"
        );
    }
    let row = DOMAIN.len() + 5;
    for offset in [
        0,
        DOMAIN.len(),
        row,
        row + 4,
        row + 37,
        row + 38,
        row + 39,
        row + 44,
    ] {
        let mut corrupt = bytes.clone();
        corrupt[offset] = 255;
        assert!(
            RuntimeLocalDeclarationTableBuilder::preflight_sources(
                &corrupt,
                limits(1),
                &mut meter()
            )
            .is_err(),
            "offset {offset}"
        );
    }
    for id in [0, 2, u32::MAX] {
        let mut corrupt = bytes.clone();
        corrupt[row + 40..row + 44].copy_from_slice(&id.to_le_bytes());
        assert!(
            matches!(RuntimeLocalDeclarationTableBuilder::preflight_sources(&corrupt, limits(1), &mut meter()), Err(RuntimeLocalSourceError::Type { actual, .. }) if actual == id)
        );
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert!(matches!(
        RuntimeLocalDeclarationTableBuilder::preflight_sources(&trailing, limits(1), &mut meter()),
        Err(RuntimeLocalSourceError::Trailing)
    ));
}

#[test]
fn original_type_and_context_are_checked_before_pending_row_retention() {
    let plan = scoped_plan();
    let bytes = plan
        .local_declarations()
        .encode_sources(limits(3), &mut meter())
        .unwrap();
    let row = DOMAIN.len() + 5;
    for (offset, id) in [(row + 37, 2_u32), (row + 42, 2_u32)] {
        let mut corrupt = bytes.clone();
        corrupt[offset..offset + 4].copy_from_slice(&id.to_le_bytes());
        let mut bank = meter();
        let source =
            RuntimeLocalDeclarationTableBuilder::preflight_sources(&corrupt, limits(3), &mut bank)
                .unwrap();
        assert!(matches!(
            source.materialize(plan.type_table(), &mut bank),
            Err(RuntimeLocalSourceError::Context { .. })
        ));
        assert_eq!(bank.status(), Err(TaskSemanticEncodingError::OwnerRejected));
    }
    let mut no_context = bytes.clone();
    no_context[row + 41] = 0;
    no_context.drain(row + 42..row + 46);
    let mut bank = meter();
    let source =
        RuntimeLocalDeclarationTableBuilder::preflight_sources(&no_context, limits(3), &mut bank)
            .unwrap();
    assert!(matches!(
        source.materialize(plan.type_table(), &mut bank),
        Err(RuntimeLocalSourceError::Context { context: None, .. })
    ));
    let other = make_plan(&[]);
    let mut bank = meter();
    let source =
        RuntimeLocalDeclarationTableBuilder::preflight_sources(&bytes, limits(3), &mut bank)
            .unwrap();
    assert!(matches!(
        source.materialize(other.type_table(), &mut bank),
        Err(RuntimeLocalSourceError::TypeTableCount {
            expected: 3,
            actual: 1
        })
    ));
}

#[test]
fn duplicate_static_origin_still_preserves_distinct_original_local_slots() {
    let source = RuntimeLocalDeclarationSource::Generated(
        RuntimeGeneratedLocalOrigin::from_accepted_identity([4; 32]),
    );
    let plan = make_plan(&[source, source]);
    let bytes = plan
        .local_declarations()
        .encode_sources(limits(1), &mut meter())
        .unwrap();
    let mut bank = meter();
    let source =
        RuntimeLocalDeclarationTableBuilder::preflight_sources(&bytes, limits(1), &mut bank)
            .unwrap();
    let mut decoded = source.materialize(plan.type_table(), &mut bank).unwrap();
    assert!(
        decoded.contains(RuntimeLocalDeclarationId::from_accepted_ordinal(
            NonZeroU32::new(2).unwrap()
        ))
    );
    place(&mut decoded, &plan);
    assert_eq!(&decoded.take_finish().unwrap(), plan.local_declarations());
}

#[test]
fn exact_storage_and_shared_original_work_cover_all_source_rows() {
    let plan = make_plan(&all_sources());
    let prepared = plan
        .local_declarations()
        .prepare_sources_encoding(limits(1), &mut meter())
        .unwrap();
    let exact = RuntimeLocalSourceLimits {
        rows: u32::try_from(plan.local_declarations().len()).unwrap(),
        encoded_bytes: prepared.encoded_bytes(),
        decoded_bytes: prepared.storage_bytes(),
        type_count: 1,
    };
    let mut bytes = Vec::with_capacity(prepared.encoded_bytes());
    prepared.append_to(&mut bytes).unwrap();
    for short in [
        RuntimeLocalSourceLimits {
            rows: exact.rows - 1,
            ..exact
        },
        RuntimeLocalSourceLimits {
            encoded_bytes: exact.encoded_bytes - 1,
            ..exact
        },
        RuntimeLocalSourceLimits {
            decoded_bytes: exact.decoded_bytes - 1,
            ..exact
        },
    ] {
        assert!(
            plan.local_declarations()
                .encode_sources(short, &mut meter())
                .is_err()
        );
        assert!(
            RuntimeLocalDeclarationTableBuilder::preflight_sources(&bytes, short, &mut meter())
                .is_err()
        );
    }
    let work = 3 * u64::try_from(bytes.len()).unwrap();
    let mut bank = TaskSemanticMeter::new(work + 1, u64::MAX);
    bank.charge_work(1).unwrap();
    let source =
        RuntimeLocalDeclarationTableBuilder::preflight_sources(&bytes, exact, &mut bank).unwrap();
    assert_eq!(source.storage_bytes(), exact.decoded_bytes);
    source.materialize(plan.type_table(), &mut bank).unwrap();
    assert_eq!(
        bank.charge_work(1),
        Err(TaskSemanticEncodingError::SemanticWork)
    );
    let mut bank = TaskSemanticMeter::new(work, u64::MAX);
    plan.local_declarations()
        .encode_sources(exact, &mut bank)
        .unwrap();
    assert_eq!(
        bank.charge_work(1),
        Err(TaskSemanticEncodingError::SemanticWork)
    );
    let mut short = TaskSemanticMeter::new(work - 1, u64::MAX);
    assert!(matches!(
        RuntimeLocalDeclarationTableBuilder::preflight_sources(&bytes, exact, &mut short),
        Err(RuntimeLocalSourceError::Resource(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    assert!(matches!(
        RuntimeLocalDeclarationTableBuilder::preflight_sources(&[], exact, &mut short),
        Err(RuntimeLocalSourceError::Resource(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
}

#[test]
fn parent_append_is_capacity_checked_and_empty_source_invents_no_placement() {
    let plan = make_plan(&all_sources());
    let bytes = plan
        .local_declarations()
        .encode_sources(limits(1), &mut meter())
        .unwrap();
    let mut output = Vec::with_capacity(3 + bytes.len());
    output.extend_from_slice(&[4, 5, 6]);
    let pointer = output.as_ptr();
    plan.local_declarations()
        .prepare_sources_encoding(limits(1), &mut meter())
        .unwrap()
        .append_to(&mut output)
        .unwrap();
    assert_eq!(output.as_ptr(), pointer);
    assert_eq!(&output[..3], [4, 5, 6]);
    assert_eq!(&output[3..], bytes);
    let mut absent = Vec::new();
    assert!(matches!(
        plan.local_declarations()
            .prepare_sources_encoding(limits(1), &mut meter())
            .unwrap()
            .append_to(&mut absent),
        Err(RuntimeLocalSourceError::OutputCapacity { .. })
    ));
    assert_eq!(absent.capacity(), 0);
    let empty = RuntimePlanBuilder::new().finish().unwrap();
    let empty_limits = RuntimeLocalSourceLimits {
        rows: 0,
        decoded_bytes: 0,
        ..limits(0)
    };
    let bytes = empty
        .local_declarations()
        .encode_sources(empty_limits, &mut meter())
        .unwrap();
    let mut bank = meter();
    let source =
        RuntimeLocalDeclarationTableBuilder::preflight_sources(&bytes, empty_limits, &mut bank)
            .unwrap();
    assert_eq!(source.row_count(), 0);
    assert_eq!(source.storage_bytes(), 0);
    assert!(
        source
            .materialize(empty.type_table(), &mut bank)
            .unwrap()
            .take_finish()
            .unwrap()
            .is_empty()
    );
}
