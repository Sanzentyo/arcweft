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
        let (row, _) = compiled
            .runtime_plan()
            .plan
            .task_plan(binding.coordinate())
            .unwrap();
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
            .1
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
