use arcweft_bundle::resource_codec::view::{
    ViewDefinitionRef, ViewExportedPart, ViewOwnedPartRef, ViewPartExportSourceRef,
    ViewProgramInstruction, ViewSemanticTarget,
};
use arcweft_bundle::resource_codec::{
    ProductSourceRef, SourceMapSection, SourceRangeRef, ValidatedViewProduct,
    ViewDefinitionResource, ViewInstructionSpan, ViewProductValidationError,
    ViewProductValidationLimits, ViewProgramResource,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use arcweft_view::{ViewPartLocalName, ViewPartName};

#[test]
fn complete_product_accepts_exact_revision_and_exposes_only_validated_views() {
    let (exact_map, program) = ranged_program("main.arcw", "hello");

    let product = ValidatedViewProduct::try_new(
        Some(exact_map.clone()),
        Some(program),
        None,
        ViewProductValidationLimits::default(),
    )
    .expect("exact source revision validates");
    let program = product.program().expect("validated program");

    assert_eq!(program.program_id().as_str(), "view.program.validation");
    assert_eq!(
        program.source_set_revision(),
        exact_map.source_set_revision()
    );
    assert_ne!(program.accepted_revision().as_bytes(), &[0; 32]);
    assert_eq!(program.definitions().len(), 0);
}

#[test]
fn complete_product_rejects_missing_unknown_and_stale_sources() {
    let (exact_map, program) = ranged_program("main.arcw", "hello");
    assert_eq!(
        ValidatedViewProduct::try_new(
            None,
            Some(program.clone()),
            None,
            ViewProductValidationLimits::default(),
        )
        .expect_err("source-bearing program requires SourceMap"),
        ViewProductValidationError::MissingSourceMap,
    );

    let other_map = source_map(&[("other.arcw", "hello")]);
    assert!(matches!(
        ValidatedViewProduct::try_new(
            Some(other_map),
            Some(program.clone()),
            None,
            ViewProductValidationLimits::default(),
        )
        .expect_err("unknown product source rejects"),
        ViewProductValidationError::MissingSource { .. }
    ));

    let stale_map = source_map(&[("main.arcw", "changed")]);
    assert!(matches!(
        ValidatedViewProduct::try_new(
            Some(stale_map),
            Some(program),
            None,
            ViewProductValidationLimits::default(),
        )
        .expect_err("same logical source with another revision rejects"),
        ViewProductValidationError::StaleSource { .. }
    ));

    assert_eq!(exact_map.documents().len(), 1);
}

#[test]
fn complete_product_rejects_invalid_reversed_out_of_bounds_and_non_utf8_ranges() {
    let (source_map, program) = ranged_program("main.arcw", "hello");
    let mut wire = serde_json::to_value(&program).expect("program serializes");
    wire["semantic_targets"][0]["source"]["source"] = serde_json::json!(7);
    let invalid_index = serde_json::from_value(wire).expect("raw candidate decodes");
    assert_eq!(
        validate(source_map.clone(), invalid_index).expect_err("invalid index rejects"),
        ViewProductValidationError::InvalidSourceIndex { index: 7, count: 1 },
    );

    let source = program.source_refs[0].clone();
    let mut reversed = program.clone();
    reversed.semantic_targets[0].source = Some(source_range(&reversed.source_refs, &source, 2, 1));
    assert_eq!(
        validate(source_map.clone(), reversed).expect_err("reversed range rejects"),
        ViewProductValidationError::ReversedRange,
    );

    let mut out_of_bounds = program;
    out_of_bounds.semantic_targets[0].source =
        Some(source_range(&out_of_bounds.source_refs, &source, 0, 6));
    assert_eq!(
        validate(source_map, out_of_bounds).expect_err("out-of-bounds range rejects"),
        ViewProductValidationError::OutOfBoundsRange,
    );

    let (unicode_map, mut unicode_program) = ranged_program("unicode.arcw", "é");
    let unicode_source = unicode_program.source_refs[0].clone();
    unicode_program.semantic_targets[0].source = Some(source_range(
        &unicode_program.source_refs,
        &unicode_source,
        1,
        2,
    ));
    assert_eq!(
        validate(unicode_map, unicode_program).expect_err("mid-codepoint range rejects"),
        ViewProductValidationError::NonUtf8Boundary,
    );
}

#[test]
fn complete_product_rejects_cross_source_and_uncontained_export_ranges() {
    let source_map = source_map(&[
        ("left.arcw", &"l".repeat(64)),
        ("right.arcw", &"r".repeat(64)),
    ]);
    let source_refs = source_map
        .documents()
        .map(ProductSourceRef::from_document)
        .collect::<Vec<_>>();
    let left = source_refs
        .iter()
        .find(|source| {
            source_map
                .get(source.id())
                .is_some_and(|document| document.document_id().as_str() == "left.arcw")
        })
        .expect("left source")
        .clone();
    let right = source_refs
        .iter()
        .find(|source| {
            source_map
                .get(source.id())
                .is_some_and(|document| document.document_id().as_str() == "right.arcw")
        })
        .expect("right source")
        .clone();

    let mut cross_source = exported_program(&source_refs, &left);
    cross_source.exported_parts[0].source.public_name = source_range(&source_refs, &right, 24, 31);
    assert!(matches!(
        validate(source_map.clone(), cross_source).expect_err("cross-source export rejects"),
        ViewProductValidationError::CrossSource { .. }
    ));

    let mut outside = exported_program(&source_refs, &left);
    outside.exported_parts[0].source.local_name = source_range(&source_refs, &left, 33, 34);
    assert_eq!(
        validate(source_map, outside).expect_err("uncontained export operand rejects"),
        ViewProductValidationError::RangeNotContained,
    );
}

#[test]
fn complete_product_enforces_exact_candidate_first_limits() {
    let (source_map, program) = ranged_program("main.arcw", "hello");
    let exact = ViewProductValidationLimits {
        source_refs: 1,
        source_ranges: 1,
        validation_work: 2,
    };
    validate_with_limits(source_map.clone(), program.clone(), exact)
        .expect("exact source limits accept");

    for (limits, resource, actual, limit) in [
        (
            ViewProductValidationLimits {
                source_refs: 0,
                ..exact
            },
            "source_refs",
            1,
            0,
        ),
        (
            ViewProductValidationLimits {
                source_ranges: 0,
                ..exact
            },
            "source_ranges",
            1,
            0,
        ),
        (
            ViewProductValidationLimits {
                validation_work: 1,
                ..exact
            },
            "validation_work",
            2,
            1,
        ),
    ] {
        assert_eq!(
            validate_with_limits(source_map.clone(), program.clone(), limits)
                .expect_err("one-over limit rejects"),
            ViewProductValidationError::BudgetExceeded {
                resource,
                actual,
                limit,
            }
        );
    }
}

#[test]
fn accepted_revision_excludes_source_identity_text_and_ranges() {
    let (first_map, first_program) = ranged_program("first.arcw", "hello");
    let (second_map, second_program) = ranged_program("renamed.arcw", "longer source text");

    let first = validate(first_map, first_program).expect("first product validates");
    let second = validate(second_map, second_program).expect("second product validates");
    let first = first.program().expect("first program");
    let second = second.program().expect("second program");

    assert_eq!(first.accepted_revision(), second.accepted_revision());
    assert_ne!(first.source_set_revision(), second.source_set_revision());
}

#[test]
fn accepted_revision_is_independent_of_definition_table_order() {
    let instructions = vec![
        ViewProgramInstruction::EmitCustom {
            element: "first".to_owned(),
            styles: Vec::new(),
            part: None,
            source: None,
        },
        ViewProgramInstruction::EmitCustom {
            element: "second".to_owned(),
            styles: Vec::new(),
            part: None,
            source: None,
        },
    ];
    let first_definition = definition("view.First", 0, 1);
    let second_definition = definition("view.Second", 1, 2);
    let first = ViewProgramResource {
        definitions: vec![first_definition.clone(), second_definition.clone()],
        instructions: instructions.clone(),
        ..ViewProgramResource::default()
    };
    let second = ViewProgramResource {
        definitions: vec![second_definition, first_definition],
        instructions,
        ..ViewProgramResource::default()
    };

    let first = validate_without_sources(first);
    let second = validate_without_sources(second);

    assert_eq!(first, second);
}

#[test]
fn accepted_revision_changes_with_typed_semantics() {
    let (source_map, program) = ranged_program("main.arcw", "hello");
    let mut changed = program.clone();
    changed.semantic_targets[0].target = "target.changed".to_owned();

    let original = validate(source_map.clone(), program).expect("original validates");
    let changed = validate(source_map, changed).expect("semantic change validates");

    assert_ne!(
        original
            .program()
            .expect("original program")
            .accepted_revision(),
        changed
            .program()
            .expect("changed program")
            .accepted_revision(),
    );
}

fn ranged_program(label: &str, text: &str) -> (SourceMapSection, ViewProgramResource) {
    let source_map = source_map(&[(label, text)]);
    let source_refs = source_map
        .documents()
        .map(ProductSourceRef::from_document)
        .collect::<Vec<_>>();
    let source = source_refs[0].clone();
    let end = u32::try_from(text.len()).expect("test source length");
    let program = ViewProgramResource {
        program_id: arcweft_view::ViewProgramId::try_new("view.program.validation").unwrap(),
        source_refs: source_refs.clone(),
        semantic_targets: vec![ViewSemanticTarget {
            public_id: "target.validation".to_owned(),
            target: "target.validation".to_owned(),
            view: None,
            label_text_source: None,
            source: Some(source_range(&source_refs, &source, 0, end)),
        }],
        ..ViewProgramResource::default()
    };
    (source_map, program)
}

fn exported_program(
    source_refs: &[ProductSourceRef],
    source: &ProductSourceRef,
) -> ViewProgramResource {
    ViewProgramResource {
        program_id: arcweft_view::ViewProgramId::try_new("view.program.export-validation").unwrap(),
        source_refs: source_refs.to_owned(),
        definitions: vec![ViewDefinitionResource {
            public_id: arcweft_bundle::resource_codec::view::ViewDefinitionRef::try_new(
                "view.Validation",
            )
            .unwrap(),
            body: ViewInstructionSpan::new(0, 1),
            styles: Vec::new(),
            parameters: Vec::new(),
            state_schema_hash: 0,
        }],
        instructions: vec![ViewProgramInstruction::EmitCustom {
            element: "validation".to_owned(),
            styles: Vec::new(),
            part: Some(ViewPartLocalName::try_new("part.local").expect("local part")),
            source: None,
        }],
        exported_parts: vec![ViewExportedPart {
            target: ViewOwnedPartRef::new(
                ViewDefinitionRef::try_new("view.Validation").expect("owner"),
                ViewPartLocalName::try_new("part.local").expect("local part"),
            ),
            public_name: ViewPartName::try_new("part.public").expect("public part"),
            source: ViewPartExportSourceRef {
                declaration: source_range(source_refs, source, 0, 32),
                local_name: source_range(source_refs, source, 12, 20),
                public_name: source_range(source_refs, source, 24, 31),
            },
        }],
        ..ViewProgramResource::default()
    }
}

fn definition(public_id: &str, start: u32, end: u32) -> ViewDefinitionResource {
    ViewDefinitionResource {
        public_id: ViewDefinitionRef::try_new(public_id).expect("definition ID"),
        body: ViewInstructionSpan::new(start, end),
        styles: Vec::new(),
        parameters: Vec::new(),
        state_schema_hash: 0,
    }
}

fn validate_without_sources(
    program: ViewProgramResource,
) -> arcweft_view::AcceptedViewProgramRevision {
    ValidatedViewProduct::try_new(
        None,
        Some(program),
        None,
        ViewProductValidationLimits::default(),
    )
    .expect("source-free program validates")
    .program()
    .expect("validated program")
    .accepted_revision()
}

fn source_map(entries: &[(&str, &str)]) -> SourceMapSection {
    let documents = entries
        .iter()
        .map(|(label, text)| {
            SourceDocument::try_new(
                SourceDocumentId::try_new(*label).expect("source ID"),
                SourceName::path(*label),
                *text,
            )
            .expect("source document")
        })
        .collect::<Vec<_>>();
    SourceMapSection::try_from_documents(&documents.iter().collect::<Vec<_>>()).expect("source map")
}

fn source_range(
    source_refs: &[ProductSourceRef],
    source: &ProductSourceRef,
    start_byte: u32,
    end_byte: u32,
) -> SourceRangeRef {
    SourceRangeRef::try_for_source(source_refs, source, start_byte, end_byte).expect("source range")
}

fn validate(
    source_map: SourceMapSection,
    program: ViewProgramResource,
) -> Result<ValidatedViewProduct, ViewProductValidationError> {
    validate_with_limits(source_map, program, ViewProductValidationLimits::default())
}

fn validate_with_limits(
    source_map: SourceMapSection,
    program: ViewProgramResource,
    limits: ViewProductValidationLimits,
) -> Result<ValidatedViewProduct, ViewProductValidationError> {
    ValidatedViewProduct::try_new(Some(source_map), Some(program), None, limits)
}
