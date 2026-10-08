use super::*;
use crate::entry::RuntimeDialogueContentTemplateDigest;
use crate::plan::*;
use crate::runtime_id::*;
use crate::value::RuntimeValue;

struct ContentFixture<'a> {
    padding: Option<u8>,
    value: bool,
    effect: bool,
    trigger: RuntimeDialogueContentEffectTrigger,
    template: u8,
    line: &'a str,
    reverse_effects: bool,
}

impl Default for ContentFixture<'_> {
    fn default() -> Self {
        Self {
            padding: None,
            value: true,
            effect: true,
            trigger: RuntimeDialogueContentEffectTrigger::Content,
            template: 21,
            line: "line.content_semantic",
            reverse_effects: false,
        }
    }
}

fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}

fn literal(value: bool) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        semantic(2),
        RuntimeExprSeedKind::Value(RuntimeValue::Bool(value)),
    )
}

fn capture(local: RuntimeLocalSeedId) -> RuntimeFunctionInputBindingSeed {
    RuntimeFunctionInputBindingSeed {
        transfer: RuntimeFunctionInputTransfer::Transferred(RuntimeFunctionCaptureMode::Copy),
        origin: RuntimeFunctionInputOrigin::Binding([4; 32]),
        source: RuntimeFunctionInputSource::Capture { position: 0 },
        input_local: local.clone(),
        pattern: RuntimePatternSeed::new(
            semantic(2),
            RuntimePatternSeedKind::Bind {
                mutable: false,
                local,
            },
        ),
        ownership: RuntimeFunctionInputOwnershipRequirement::Owned,
        unrestricted_bindings: Box::new([]),
    }
}

fn effect_function(
    builder: &mut RuntimePlanBuilder,
    local: RuntimeLocalSeedId,
    definition: u8,
) -> RuntimeFunctionSiteSeedId {
    let site = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([definition; 32]),
            role: RuntimeFunctionSemanticRole::Effect,
            function_type: None,
            inputs: Box::new([capture(local)]),
            result: semantic(1),
            body_kind: RuntimeFunctionSiteBodyKind::Executable,
            effects: RuntimeEffectSet::empty(),
        })
        .unwrap();
    builder
        .define_function_site_seed(
            &site,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([]),
            }),
        )
        .unwrap();
    site
}

fn content_plan(options: &ContentFixture<'_>) -> RuntimePlan {
    let mut builder = RuntimePlanBuilder::new();
    let types = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(semantic(1), RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(semantic(2), RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    semantic(3),
                    RuntimePlanTypeProjection::Function {
                        contract: RuntimeFunctionTypeContract::new(
                            RuntimeTypeBinder::new(0, 0, 0),
                            crate::effect_row::EffectPredicate::unconstrained(),
                            crate::effect_row::EffectFormula::empty(),
                        ),
                        parameters: Box::new([]),
                        result: semantic(1),
                    },
                ),
            ],
            [RuntimeLocalDeclarationSeed::new(
                crate::plan::RuntimeLocalDeclarationSource::Binding {
                    identity: [4; 32],
                    declaration: crate::plan::RuntimeLocalBindingDeclaration::new(
                        crate::plan::RuntimeLocalBindingKind::PatternBinding,
                        false,
                        crate::plan::RuntimeLocalBindingStorage::Derived,
                    ),
                },
                semantic(2),
            )],
        )
        .unwrap();
    let local = types.local_ids()[0].clone();
    if let Some(padding) = options.padding {
        builder
            .push_function_site_seed(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([padding; 32]),
                RuntimeFunctionSemanticRole::Ordinary,
                [],
                literal(false),
            )
            .unwrap();
    }
    let value = builder
        .push_function_site_seed(
            RuntimeFunctionDefinitionIdentity::from_accepted_identity([11; 32]),
            RuntimeFunctionSemanticRole::Dialogue,
            [capture(local.clone())],
            literal(true),
        )
        .unwrap();
    let fresh = builder
        .admit_type_batch(
            [],
            (0..2).map(|_| {
                RuntimeLocalDeclarationSeed::new(
                    fixture_binding_source([4; 32], false),
                    semantic(2),
                )
            }),
        )
        .unwrap();
    let mut effects = [
        effect_function(&mut builder, fresh.local_ids()[0].clone(), 12),
        effect_function(&mut builder, fresh.local_ids()[1].clone(), 13),
    ];
    if options.reverse_effects {
        effects.reverse();
    }
    admit_content(&mut builder, options, value, effects);
    builder.finish().unwrap()
}

#[test]
fn content_rows_enter_the_complete_executable_prefix_with_actual_effect_bodies() {
    use crate::plan::body_semantic::executable_rows::fixture_prefix;
    let first = content_plan(&ContentFixture::default());
    let changed = content_plan(&ContentFixture {
        value: false,
        ..ContentFixture::default()
    });
    assert_ne!(fixture_prefix(&first), fixture_prefix(&changed));
    let changed = content_plan(&ContentFixture {
        effect: false,
        ..ContentFixture::default()
    });
    assert_ne!(fixture_prefix(&first), fixture_prefix(&changed));
}

fn admit_content(
    builder: &mut RuntimePlanBuilder,
    options: &ContentFixture<'_>,
    value: RuntimeFunctionSiteSeedId,
    functions: [RuntimeFunctionSiteSeedId; 2],
) {
    let slot = RuntimeDialogueValueSlotId::from_zero_based(0).unwrap();
    builder
        .push_dialogue_content_seed(RuntimeDialogueContentPlanSeed {
            line: RuntimeLineId::from_runtime_line_value(options.line).unwrap(),
            template: RuntimeDialogueContentTemplateManifestSeed {
                id: RuntimeDialogueContentTemplateId::from_zero_based(0).unwrap(),
                digest: RuntimeDialogueContentTemplateDigest::from_bytes([options.template; 32]),
                slots: Box::new([RuntimeDialogueContentSlotSeed {
                    slot,
                    role: RuntimeDialogueValueRole::Interpolation,
                    semantic_type: semantic(2),
                }]),
                effects: (0..2)
                    .map(|index| RuntimeDialogueContentEffectSlotSeed {
                        site: RuntimeDialogueEffectSiteId::from_zero_based(index).unwrap(),
                        trigger: options.trigger,
                        capture_types: Box::new([semantic(2)]),
                    })
                    .collect(),
            },
            values: Box::new([RuntimeDialogueValueSiteSeed {
                slot,
                role: RuntimeDialogueValueRole::Interpolation,
                function: value,
                captures: Box::new([literal(options.value)]),
            }]),
            effect_sites: functions
                .into_iter()
                .enumerate()
                .map(|(index, function)| RuntimeDialogueEffectSiteSeed {
                    site: RuntimeDialogueEffectSiteId::from_zero_based(index).unwrap(),
                    function,
                    callable_type: semantic(3),
                    captures: Box::new([literal(options.effect)]),
                })
                .collect(),
            marks: Box::new(["mark".into()]),
            effect_site_count: RuntimeDialogueEffectSiteCount::try_from_len(2).unwrap(),
        })
        .unwrap();
}

fn row(
    plan: &RuntimePlan,
    work: u64,
    bytes: u64,
) -> (Result<blake3::Hash, RuntimeBodySemanticError>, (u64, u64)) {
    let mut meter = TaskSemanticMeter::new(work, bytes);
    let result = RuntimeBodySemanticContext::new(plan).dialogue_content_row_digest(&mut meter, 0);
    (result, meter.totals())
}

#[test]
fn content_row_commits_template_value_effect_trigger_callback_and_line_identity() {
    let first = content_plan(&ContentFixture::default());
    let expected = row(&first, 100_000, 1_000_000).0.unwrap();
    let padded = content_plan(&ContentFixture {
        padding: Some(99),
        ..ContentFixture::default()
    });
    assert_eq!(expected, row(&padded, 100_000, 1_000_000).0.unwrap());
    for options in [
        ContentFixture {
            template: 22,
            ..ContentFixture::default()
        },
        ContentFixture {
            line: "line.another",
            ..ContentFixture::default()
        },
        ContentFixture {
            value: false,
            ..ContentFixture::default()
        },
        ContentFixture {
            effect: false,
            ..ContentFixture::default()
        },
        ContentFixture {
            reverse_effects: true,
            ..ContentFixture::default()
        },
        ContentFixture {
            trigger: RuntimeDialogueContentEffectTrigger::Delay {
                duration: crate::time::LogicalDuration::from_nanos(1),
            },
            ..ContentFixture::default()
        },
    ] {
        let changed = content_plan(&options);
        assert_ne!(expected, row(&changed, 100_000, 1_000_000).0.unwrap());
    }
}

#[test]
fn content_line_link_excludes_debug_marks_and_delegates_body_to_the_line_owner() {
    let first = actual_line_plan(true, true, "cancel", "mark");
    let changed_body = actual_line_plan(false, false, "other_cancel", "renamed");
    let expected = row(&first.0, 100_000, 1_000_000).0.unwrap();
    assert_eq!(
        expected,
        row(&changed_body.0, 100_000, 1_000_000).0.unwrap()
    );
    let mut changed = first.0.clone();
    let old = &changed.line_task_groups()[0];
    changed.inventory.line_task_groups[0] = crate::line_task::LineTaskGroup::new(
        RuntimeFunctionDefinitionIdentity::from_accepted_identity([85; 32]),
        old.captures().into(),
        old.activation_exports().into(),
        old.activation_ops().into(),
        old.result_type(),
        old.handle_sites().into(),
        old.root(),
        old.nodes().into(),
        old.cancel_rules().into(),
        old.cleanup().clone(),
    );
    changed.verify().unwrap();
    assert_ne!(expected, row(&changed, 100_000, 1_000_000).0.unwrap());
}

#[test]
fn content_row_obeys_exact_shared_limits_and_missing_owner_before_bytes() {
    let plan = content_plan(&ContentFixture::default());
    let (expected, (work, bytes)) = row(&plan, 100_000, 1_000_000);
    assert_eq!(expected.unwrap(), row(&plan, work, bytes).0.unwrap());
    for (work, bytes, error) in [
        (work - 1, bytes, TaskSemanticEncodingError::SemanticWork),
        (work, bytes - 1, TaskSemanticEncodingError::TranscriptBytes),
    ] {
        assert!(
            matches!(row(&plan, work, bytes).0, Err(RuntimeBodySemanticError::Encoding(actual)) if actual == error)
        );
    }
    let context = RuntimeBodySemanticContext::new(&plan);
    let mut missing = TaskSemanticMeter::new(100_000, 1_000_000);
    assert!(matches!(
        context.dialogue_content_row_digest(&mut missing, 1),
        Err(RuntimeBodySemanticError::MissingRow {
            table: "dialogue content",
            ordinal: 1
        })
    ));
    assert_eq!(missing.totals(), (0, 0));
    assert_eq!(
        missing.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
    let mut poison = TaskSemanticMeter::new(0, 1_000_000);
    poison.charge_work(1).unwrap_err();
    assert!(matches!(
        context.dialogue_content_row_digest(&mut poison, 99),
        Err(RuntimeBodySemanticError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    assert_eq!(poison.totals(), (0, 0));
}
