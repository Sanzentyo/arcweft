use super::{
    CompiledProject, ValidatedViewProduct, ViewProductValidationLimits, ViewProgramInstruction,
    project_view_fixture,
};
use arcweft_bundle::resource_codec::view::ViewTaskPlanBindingSource;
use arcweft_core::pattern::{MatchSourceData, MatchSourceMaterializationBudget};
use arcweft_core::plan::RuntimeTaskPlanBuildCoordinate;
use arcweft_core::task::ViewTaskPlanValidationError;

fn unbound_product(compiled: &CompiledProject) -> ValidatedViewProduct {
    let original = compiled.view_product().product();
    ValidatedViewProduct::try_new(
        Some(original.source_map().clone()),
        original.program().map(|program| program.resource().clone()),
        original.style().map(|style| style.resource().clone()),
        ViewProductValidationLimits::default(),
    )
    .unwrap()
}

struct SourceByteCount;
impl arcweft_core::pattern::MatchSourceSink for SourceByteCount {
    type Error = arcweft_core::pattern::MatchSourceEncodingError;
    fn write_bytes(&mut self, _: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn exact_binding_limits(compiled: &CompiledProject) -> ViewProductValidationLimits {
    use arcweft_core::pattern::{CanonicalMatchSource, MatchSourceWriter};
    let program = compiled.view_product().product().program().unwrap();
    let mut storage = MatchSourceMaterializationBudget::new(u64::MAX);
    let mut writer = MatchSourceWriter::new(SourceByteCount);
    for instruction in &program.resource().instructions {
        if let ViewProgramInstruction::Match { program, .. } = instruction {
            program.source.preflight_source(&mut storage).unwrap();
            writer.extend_from_slice(arcweft_view::ViewMatchSiteId::DOMAIN);
            program
                .source
                .coordinate()
                .write_source(&mut writer)
                .unwrap();
            program.source.write_semantic(&mut writer).unwrap();
            writer.extend_from_slice(arcweft_view::CheckedViewMatchAdmissionDigest::DOMAIN);
            writer.extend_from_slice(&[0; 32]);
        }
    }
    let bytes = writer.byte_len();
    writer.finish(Ok(())).unwrap();
    ViewProductValidationLimits {
        validation_work: u64::try_from(
            program.task_plan_bindings().count() * 2 + program.resource().instructions.len(),
        )
        .unwrap()
            + bytes,
        match_source_bytes: storage.used_bytes(),
        ..ViewProductValidationLimits::default()
    }
}

#[test]
fn view_binding_limits_cover_all_source_bytes_and_failure_leaves_the_slot_unbound() {
    use arcweft_core::pattern::{CanonicalMatchSource, MatchSourceWriter};
    let compiled = compile();
    let (expected, sources) = binding_sources(&compiled);
    let program = compiled.view_product().product().program().unwrap();
    let mut storage = MatchSourceMaterializationBudget::new(u64::MAX);
    let mut writer = MatchSourceWriter::new(SourceByteCount);
    for instruction in &program.resource().instructions {
        if let ViewProgramInstruction::Match { program, .. } = instruction {
            program.source.preflight_source(&mut storage).unwrap();
            writer.extend_from_slice(arcweft_view::ViewMatchSiteId::DOMAIN);
            program
                .source
                .coordinate()
                .write_source(&mut writer)
                .unwrap();
            program.source.write_semantic(&mut writer).unwrap();
            writer.extend_from_slice(arcweft_view::CheckedViewMatchAdmissionDigest::DOMAIN);
            writer.extend_from_slice(&[0; 32]);
        }
    }
    let bytes = writer.byte_len();
    writer.finish(Ok(())).unwrap();
    let work =
        u64::try_from(expected.len() + sources.len() + program.resource().instructions.len())
            .unwrap()
            + bytes;
    let limits = ViewProductValidationLimits {
        validation_work: work,
        match_source_bytes: storage.used_bytes(),
        ..ViewProductValidationLimits::default()
    };
    for below in [
        ViewProductValidationLimits {
            validation_work: work - 1,
            ..limits
        },
        ViewProductValidationLimits {
            match_source_bytes: storage.used_bytes() - 1,
            ..limits
        },
    ] {
        let product = unbound_product(&compiled);
        assert!(matches!(
            product.bind_task_plans(&expected, sources.clone(), below),
            Err(ViewTaskPlanValidationError::WorkLimitExceeded)
        ));
        assert_eq!(product.program().unwrap().task_plan_bindings().count(), 0);
        product
            .bind_task_plans(&expected, sources.clone(), limits)
            .unwrap();
        assert_eq!(product.program().unwrap().task_plan_bindings().count(), 2);
    }
}

fn binding_sources(
    compiled: &CompiledProject,
) -> (
    Vec<RuntimeTaskPlanBuildCoordinate>,
    Vec<ViewTaskPlanBindingSource>,
) {
    let program = compiled.view_product().product().program().unwrap();
    let coordinates = program
        .task_plan_bindings()
        .map(|binding| binding.coordinate().clone())
        .collect();
    let sources = program
        .task_plan_bindings()
        .map(|binding| {
            ViewTaskPlanBindingSource::new(
                binding.program_id().clone(),
                binding.accepted_revision(),
                binding.site(),
                binding.admission(),
                binding.coordinate().clone(),
            )
        })
        .collect();
    (coordinates, sources)
}

#[test]
fn view_task_bindings_resolve_only_in_the_original_completed_core_plan() {
    let compiled = compile();
    let foreign = compile();
    let program = compiled.view_product().product().program().unwrap();
    let bindings = program.task_plan_bindings().collect::<Vec<_>>();
    assert_eq!(bindings.len(), 2);
    for binding in bindings {
        assert!(std::ptr::eq(
            program.task_plan_binding(binding.coordinate()).unwrap(),
            binding
        ));
        let row = compiled
            .runtime_plan()
            .plan
            .task_plan(binding.coordinate())
            .unwrap()
            .definition();
        assert_eq!(
            row.family(),
            arcweft_core::task::NeedProducerFamily::ViewMatchSubscription
        );
        assert_eq!(row.class(), &arcweft_core::task::TaskClass::LocalView);
        assert_eq!(
            row.binding().kind(),
            arcweft_core::plan::RuntimeTaskSemanticBindingKind::View
        );
        assert_eq!(
            row.request_template().source(),
            arcweft_core::plan::RuntimeTaskRequestSource::FunctionRoot
        );
        assert!(
            foreign
                .runtime_plan()
                .plan
                .task_plan(binding.coordinate())
                .is_none()
        );
    }
}

#[test]
fn decoded_view_source_reseals_exact_original_bindings_in_a_fresh_namespace() {
    use arcweft_core::plan::{
        RuntimeTaskPlanSealError, RuntimeTaskSourceArtifact, RuntimeTaskSourceArtifactLimits,
    };

    let compiled = compile();
    let plan = &compiled.runtime_plan().plan;
    let view = compiled.view_product().product().program().unwrap();
    let limits = RuntimeTaskSourceArtifactLimits::default();
    let artifact = plan.task_source_artifact(limits).unwrap();
    let bytes = serde_json::to_vec(&artifact).unwrap();
    let artifact: RuntimeTaskSourceArtifact = serde_json::from_slice(&bytes).unwrap();

    assert!(matches!(
        artifact.admit(limits, None).unwrap_err().seal_error(),
        Some(RuntimeTaskPlanSealError::MissingViewAuthority { .. })
    ));
    // The construction authority remains tied to its original coordinates.
    assert!(matches!(
        artifact.admit(limits, Some(view)).unwrap_err().seal_error(),
        Some(RuntimeTaskPlanSealError::ViewAuthority {
            source: ViewTaskPlanValidationError::MissingBinding { .. },
            ..
        })
    ));

    let authority = view
        .task_source_authority(ViewProductValidationLimits::default())
        .unwrap();
    let decoded = artifact.admit(limits, Some(&authority)).unwrap();
    assert_eq!(
        plan.executable_semantic_digest(),
        decoded.executable_semantic_digest()
    );
    let keys = |plan: &arcweft_core::plan::RuntimePlan| {
        (0..plan.task_plans().len())
            .map(|index| {
                plan.task_plans()
                    .by_index(arcweft_core::plan::RuntimeTaskPlanIndex(
                        u32::try_from(index).unwrap(),
                    ))
                    .unwrap()
                    .key()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(keys(plan), keys(&decoded));
    for binding in view.task_plan_bindings() {
        assert!(plan.task_plan(binding.coordinate()).is_some());
        assert!(decoded.task_plan(binding.coordinate()).is_none());
    }
    // The same accepted original bindings can check another complete decode;
    // neither successful admission mutates the original construction authority.
    artifact.admit(limits, Some(&authority)).unwrap();

    let resource = arcweft_bundle::resource_codec::ViewProgramResource::decode_canonical_section(
        &view.resource().encode_canonical_section().unwrap(),
    )
    .unwrap();
    let decoded_product = ValidatedViewProduct::try_new(
        Some(compiled.view_product().product().source_map().clone()),
        Some(resource),
        compiled
            .view_product()
            .product()
            .style()
            .map(|style| style.resource().clone()),
        ViewProductValidationLimits::default(),
    )
    .unwrap();
    let decoded_view = decoded_product.program().unwrap();
    assert_eq!(decoded_view.task_plan_bindings().count(), 0);
    assert_eq!(decoded_view.accepted_revision(), view.accepted_revision());
    let decoded_authority = decoded_view
        .task_source_authority(ViewProductValidationLimits::default())
        .unwrap();
    artifact.admit(limits, Some(&decoded_authority)).unwrap();
}

#[test]
fn decoded_view_source_binding_index_is_bounded_and_rejects_a_changed_product() {
    let compiled = compile();
    let view = compiled.view_product().product().program().unwrap();
    let exact = exact_binding_limits(&compiled);
    assert!(matches!(
        view.task_source_authority(ViewProductValidationLimits {
            validation_work: exact.validation_work - 1,
            ..exact
        }),
        Err(ViewTaskPlanValidationError::WorkLimitExceeded)
    ));
    view.task_source_authority(exact).unwrap();

    let changed = project_view_fixture(
        "view Main(enabled: bool = true) { match enabled { true => Text(\"changed\"), false => Text(\"no\") }; }",
        "arcweft-test://retained-match-source",
    )
    .compile()
    .unwrap();
    let authority = changed
        .view_product()
        .product()
        .program()
        .unwrap()
        .task_source_authority(ViewProductValidationLimits::default())
        .unwrap();
    let limits = arcweft_core::plan::RuntimeTaskSourceArtifactLimits::default();
    let artifact = compiled
        .runtime_plan()
        .plan
        .task_source_artifact(limits)
        .unwrap();
    assert!(matches!(
        artifact
            .admit(limits, Some(&authority))
            .unwrap_err()
            .seal_error(),
        Some(
            arcweft_core::plan::RuntimeTaskPlanSealError::ViewAuthority {
                source: ViewTaskPlanValidationError::MissingOriginalBinding,
                ..
            }
        )
    ));
}

#[test]
fn view_source_assertions_reject_missing_duplicate_and_foreign_admissions() {
    let compiled = compile();
    let product = compiled.view_product().product();
    let view = product.program().unwrap();
    let original = serde_json::to_value(view.resource()).unwrap();
    for forgery in 0..4 {
        let mut candidate = original.clone();
        let rows = candidate["task_plan_bindings"].as_array_mut().unwrap();
        assert_eq!(rows.len(), 2);
        match forgery {
            0 => {
                rows.pop();
            }
            1 => {
                rows[0]["expected"] = rows[1]["expected"].clone();
            }
            2 => {
                rows[0]["admission"] = rows[1]["admission"].clone();
            }
            _ => {
                rows.clear();
            }
        }
        let resource = serde_json::from_value(candidate).unwrap();
        let candidate = ValidatedViewProduct::try_new(
            Some(product.source_map().clone()),
            Some(resource),
            product.style().map(|style| style.resource().clone()),
            ViewProductValidationLimits::default(),
        )
        .unwrap();
        assert!(
            candidate
                .program()
                .unwrap()
                .task_source_authority(ViewProductValidationLimits::default())
                .is_err(),
            "forgery {forgery}"
        );
    }
    assert!(matches!(
        product.seal_task_plan_bindings(ViewProductValidationLimits::default()),
        Err(ViewTaskPlanValidationError::AuthorityAlreadyBound)
    ));
}

#[test]
fn admitted_view_awbc_keeps_original_source_through_owned_restore() {
    use arcweft_core::awbc::product_step::AwbcProductStepExecutor;
    use arcweft_core::value::RuntimeValue;
    use std::sync::Arc;

    let compiled = super::project_view_fixture_with_entry(
        "entry cli @entry.main { goto @flow.main } flow main() -> String { return \"done\" } view Main(enabled: bool = true) { match enabled { true => Text(\"yes\"), false => Text(\"no\") }; }",
        "arcweft-test://view-source-owned-restore",
    ).compile().unwrap();
    let code = arcweft_runtime_plan::awbc_lower::AwbcLowerer::new(
        &compiled.runtime_plan().plan,
        &compiled.runtime_plan().dialogue_content_catalog,
        "main.arcw",
    )
    .lower()
    .unwrap()
    .program;
    let code = Arc::new(
        arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
            &code.encode_canonical().unwrap(),
            arcweft_core::awbc::codec::AwbcDecodeBudget::default(),
        )
        .unwrap(),
    );
    let admitted = super::admit_view_awbc(&compiled, Arc::clone(&code));
    let program = matches(&compiled)[0].execution.program;
    let mut executor = AwbcProductStepExecutor::for_admitted_program_invocation(
        admitted,
        program,
        vec![RuntimeValue::Bool(true)],
        arcweft_core::task::GenerationId::new(0),
        64,
    )
    .unwrap();
    let saved = executor.snapshot().unwrap();
    executor = executor
        .restore_snapshot_owned(saved)
        .unwrap_or_else(|(_, error)| panic!("{error}"));
    let output = executor.step(Default::default(), Default::default());
    assert!(output.output.diagnostics.is_empty(), "{output:?}");
    let saved = executor.snapshot().unwrap();
    executor = executor
        .restore_snapshot_owned(saved)
        .unwrap_or_else(|(_, error)| panic!("{error}"));
    let (_, result) = executor.take_program_result().unwrap().unwrap();
    assert_eq!(
        result,
        RuntimeValue::Tuple(vec![RuntimeValue::u32(0), RuntimeValue::Unit])
    );
    assert!(Arc::ptr_eq(&executor.program_arc(), &code));
}

#[test]
fn view_binding_join_rejects_missing_extra_duplicate_stale_and_foreign_coordinates() {
    let compiled = compile();
    let foreign = compile();
    let (expected, sources) = binding_sources(&compiled);
    let program = compiled.view_product().product().program().unwrap();
    let bindings = program.task_plan_bindings().collect::<Vec<_>>();
    let validate = |coordinates: &[RuntimeTaskPlanBuildCoordinate], sources| {
        let product = unbound_product(&compiled);
        product.bind_task_plans(coordinates, sources, ViewProductValidationLimits::default())
    };
    validate(&expected, sources.clone()).unwrap();
    assert!(matches!(
        compiled.view_product().product().bind_task_plans(
            &expected,
            sources.clone(),
            ViewProductValidationLimits::default()
        ),
        Err(ViewTaskPlanValidationError::AuthorityAlreadyBound)
    ));
    assert!(matches!(
        validate(&expected, sources[..1].to_vec()),
        Err(ViewTaskPlanValidationError::BindingSetMismatch)
    ));
    let mut extra = sources.clone();
    extra.push(sources[0].clone());
    assert!(matches!(
        validate(&expected, extra),
        Err(ViewTaskPlanValidationError::BindingSetMismatch)
    ));
    let duplicate = ViewTaskPlanBindingSource::new(
        bindings[1].program_id().clone(),
        bindings[1].accepted_revision(),
        bindings[1].site(),
        bindings[1].admission(),
        expected[0].clone(),
    );
    assert!(matches!(
        validate(&expected, vec![sources[0].clone(), duplicate]),
        Err(ViewTaskPlanValidationError::DuplicateBinding)
    ));
    let mismatched = ViewTaskPlanBindingSource::new(
        bindings[0].program_id().clone(),
        bindings[0].accepted_revision(),
        bindings[0].site(),
        bindings[1].admission(),
        expected[0].clone(),
    );
    assert!(matches!(
        validate(&expected, vec![mismatched, sources[1].clone()]),
        Err(ViewTaskPlanValidationError::AdmissionMismatch)
    ));
    let (foreign_coordinates, _) = binding_sources(&foreign);
    assert!(matches!(
        validate(
            &[expected[0].clone(), foreign_coordinates[1].clone()],
            sources
        ),
        Err(ViewTaskPlanValidationError::CoordinateOwnerMismatch)
    ));
    let replacement = project_view_fixture(
        r#"view Main(enabled: bool = true) {
        match enabled { true => Text("changed"), false => Text("other") };
    }"#,
        "arcweft-test://retained-match-source",
    )
    .compile()
    .unwrap();
    let revision = replacement
        .view_product()
        .product()
        .program()
        .unwrap()
        .accepted_revision();
    assert_ne!(revision, bindings[0].accepted_revision());
    let (expected, mut stale) = binding_sources(&compiled);
    stale[0] = ViewTaskPlanBindingSource::new(
        bindings[0].program_id().clone(),
        revision,
        bindings[0].site(),
        bindings[0].admission(),
        expected[0].clone(),
    );
    assert!(matches!(
        validate(&expected, stale),
        Err(ViewTaskPlanValidationError::StaleAuthority)
    ));
}

#[test]
fn view_task_key_ignores_revision_only_replacement_and_tracks_exact_match_admission() {
    let source = r#"view Main(enabled: bool = true) {
        match enabled { true => Text("yes"), false => Text("no") };
        Text("outside")
    }
    view DecorationA() {}"#;
    let compile = |text: &str| {
        project_view_fixture(text, "arcweft-test://view-task-revision")
            .compile()
            .unwrap()
    };
    let original = compile(source);
    let replacement = compile(&source.replace("DecorationA", "DecorationB"));
    let widget_changed = compile(&source.replace("Text(\"outside\")", "Button(\"outside\")"));
    let changed = compile(&source.replace("Text(\"yes\")", "Text(\"changed\")"));
    let key = |compiled: &CompiledProject| {
        let program = compiled.view_product().product().program().unwrap();
        let binding = program.task_plan_bindings().next().unwrap();
        compiled
            .runtime_plan()
            .plan
            .task_plan(binding.coordinate())
            .unwrap()
            .key()
    };
    assert_ne!(
        original
            .view_product()
            .product()
            .program()
            .unwrap()
            .accepted_revision(),
        replacement
            .view_product()
            .product()
            .program()
            .unwrap()
            .accepted_revision()
    );
    assert_eq!(
        original.runtime_plan().plan.executable_semantic_digest(),
        replacement.runtime_plan().plan.executable_semantic_digest(),
        "revision-only fixture must preserve the complete Core prefix"
    );
    let original_program = original.view_product().product().program().unwrap();
    let replaced_program = replacement.view_product().product().program().unwrap();
    assert_eq!(original_program.program_id(), replaced_program.program_id());
    let original_binding = original_program.task_plan_bindings().next().unwrap();
    let replaced_binding = replaced_program.task_plan_bindings().next().unwrap();
    assert_eq!(original_binding.site(), replaced_binding.site());
    assert_eq!(original_binding.admission(), replaced_binding.admission());
    assert_eq!(key(&original), key(&replacement));
    assert_ne!(key(&original), key(&changed));
    assert_ne!(
        original.runtime_plan().plan.executable_semantic_digest(),
        widget_changed
            .runtime_plan()
            .plan
            .executable_semantic_digest()
    );
    assert_ne!(key(&original), key(&widget_changed));
}

fn compile() -> CompiledProject {
    project_view_fixture(
        r#"
        fn choose(value: bool) -> bool { value }
        view Main(value: (bool, String), enabled: bool = true) {
            match value {
                (true, label) when choose(enabled) => Text(label),
                (true, _) => Text("fallback"),
                (false, label) => Button(label)
            };
            match enabled { true => Text("yes"), false => Text("no") };
        }
    "#,
        "arcweft-test://retained-match-source",
    )
    .compile()
    .unwrap()
}

fn matches(compiled: &CompiledProject) -> Vec<&arcweft_view::ViewMatchProgram> {
    compiled
        .view_product()
        .product()
        .program()
        .unwrap()
        .resource()
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            ViewProgramInstruction::Match { program, .. } => Some(program),
            _ => None,
        })
        .collect()
}

#[test]
fn retained_definitions_round_trip_and_match_only_their_original_checked_owner() {
    let compiled = compile();
    let selections = matches(&compiled);
    let rows = compiled
        .view_product()
        .match_catalog_rows()
        .collect::<Vec<_>>();
    assert_eq!(selections.len(), 2);
    assert_eq!(rows.len(), 2);
    for (selection, row) in selections.iter().zip(&rows) {
        let bytes = serde_json::to_vec(&selection.source).unwrap();
        let restored: MatchSourceData = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(&restored, selection.source.as_ref());
        row.validate_retained_source(&restored).unwrap();
        assert!(selection.outputs_are_canonical());
    }
    assert!(
        rows[0]
            .validate_retained_source(&selections[1].source)
            .is_err()
    );
    assert!(
        rows[1]
            .validate_retained_source(&selections[0].source)
            .is_err()
    );
}

#[test]
fn every_retained_semantic_field_and_original_coordinate_is_revalidated() {
    let compiled = compile();
    let selections = matches(&compiled);
    let row = compiled.view_product().match_catalog_rows().next().unwrap();
    let original = serde_json::to_value(&selections[0].source).unwrap();
    let result_tag = original["arms"][0]["result"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap();
    let result_path = format!("/arms/0/result/{result_tag}/0");
    for path in [
        "/coordinate/0",
        "/scrutinee/0",
        "/scrutinee_type/0",
        "/arms/0/coordinate/owner/0",
        "/arms/0/coordinate/ordinal",
        "/arms/0/pattern/0",
        "/arms/0/guard/Dynamic/0",
        result_path.as_str(),
        "/arms/0/bindings/0/coordinate/0",
        "/arms/0/bindings/0/ty/0",
        "/coverage/domain_digest/0",
    ] {
        let mut candidate = original.clone();
        let value = candidate
            .pointer_mut(path)
            .unwrap_or_else(|| panic!("missing {path}"));
        *value = serde_json::json!(value.as_u64().unwrap() ^ 1);
        let candidate: MatchSourceData = serde_json::from_value(candidate).unwrap();
        assert!(
            row.validate_retained_source(&candidate).is_err(),
            "accepted {path}"
        );
    }
    let mut candidate = original;
    candidate["coverage"]["exhaustive"] = serde_json::json!(false);
    let candidate: MatchSourceData = serde_json::from_value(candidate).unwrap();
    assert!(row.validate_retained_source(&candidate).is_err());
}

#[test]
fn retained_match_source_is_required_and_rejects_unknown_definition_fields() {
    let compiled = compile();
    let selections = matches(&compiled);
    let mut instruction = serde_json::to_value(selections[0]).unwrap();
    instruction.as_object_mut().unwrap().remove("source");
    assert!(serde_json::from_value::<arcweft_view::ViewMatchProgram>(instruction).is_err());
    let mut source = serde_json::to_value(&selections[0].source).unwrap();
    source["legacy_digest"] = serde_json::json!(true);
    assert!(serde_json::from_value::<MatchSourceData>(source).is_err());
}

#[test]
fn complete_product_preflights_all_retained_sources_with_one_exact_allowance() {
    let compiled = compile();
    let selections = matches(&compiled);
    let mut count = MatchSourceMaterializationBudget::new(u64::MAX);
    for selection in selections {
        selection.source.preflight_source(&mut count).unwrap();
    }
    let bytes = count.used_bytes();
    assert!(bytes > 0);
    let product = compiled.view_product().product();
    let validate = |limit| {
        ValidatedViewProduct::try_new(
            Some(product.source_map().clone()),
            product.program().map(|program| program.resource().clone()),
            product.style().map(|style| style.resource().clone()),
            ViewProductValidationLimits {
                match_source_bytes: limit,
                ..ViewProductValidationLimits::default()
            },
        )
    };
    validate(bytes).unwrap();
    assert!(matches!(
        validate(bytes - 1),
        Err(arcweft_bundle::resource_codec::ViewProductValidationError::MatchSource(_))
    ));
}
