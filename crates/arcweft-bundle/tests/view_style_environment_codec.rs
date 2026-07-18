use arcweft_bundle::resource_codec::view::{
    ViewProgramStyleResources, ViewResourceBudget, ViewResourceMergeError,
    ViewStyleEnvironmentSourceError, ViewStyleEnvironmentSourceRole, ViewStyleResource,
};
use arcweft_bundle::resource_codec::{
    ProductSourceRef, SectionCodecError, SourceMapSection, SourceRangeRef, ValidatedViewProduct,
    ViewProductValidationError, ViewProductValidationLimits,
};
use arcweft_presentation::appearance::ColorScheme;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use arcweft_view::ViewElementKind;
use arcweft_view::style::{
    ViewEnvironmentClause, ViewEnvironmentCondition, ViewEnvironmentWrapperIndex,
    ViewEnvironmentWrapperSource, ViewPropertyKind, ViewRatioMilli, ViewSpecifiedValue,
    ViewStyleAssignOp, ViewStyleDeclaration, ViewStyleProgram, ViewStyleRule, ViewStyleSelector,
    ViewStyleSelectorSequence, ViewStyleSheet, ViewStyleSheetId, ViewStyleSourceId,
};

#[test]
fn environment_product_round_trips_canonical_json_cbor_msgpack() {
    let (_, one_wrapper) = fixture();
    let (_, nested) = nested_fixture(
        "round-trip.arcw",
        "view.style.round-trip",
        "style.round-trip",
    );
    let conditions = [one_wrapper, nested].map(|resource| {
        resource.program.sheets()[0].rules()[0]
            .environment()
            .expect("fixture guard")
            .clone()
    });

    for condition in conditions {
        let json = serde_json::to_vec(&condition).expect("JSON encodes");
        assert_eq!(
            serde_json::from_slice::<ViewEnvironmentCondition>(&json).expect("JSON decodes"),
            condition
        );

        #[cfg(feature = "format-cbor")]
        {
            let mut cbor = Vec::new();
            ciborium::into_writer(&condition, &mut cbor).expect("CBOR encodes");
            assert_eq!(
                ciborium::from_reader::<ViewEnvironmentCondition, _>(cbor.as_slice())
                    .expect("CBOR decodes"),
                condition
            );
        }

        #[cfg(feature = "format-messagepack")]
        {
            let messagepack = rmp_serde::to_vec_named(&condition).expect("MessagePack encodes");
            assert_eq!(
                rmp_serde::from_slice::<ViewEnvironmentCondition>(&messagepack)
                    .expect("MessagePack decodes"),
                condition
            );
        }
    }

    let old = br#"{"source":0,"clauses":[{"field":"color_scheme","comparison":"equal","value":"dark","source":1}]}"#;
    assert!(serde_json::from_slice::<ViewEnvironmentCondition>(old).is_err());
}

#[test]
fn environment_removed_and_unknown_wire_shapes_reject_in_every_enabled_format() {
    let condition = serde_json::to_value(
        fixture().1.program.sheets()[0].rules()[0]
            .environment()
            .expect("fixture guard"),
    )
    .expect("condition JSON");
    let old = serde_json::json!({
        "source": 0,
        "clauses": [{
            "field": "color_scheme",
            "comparison": "equal",
            "value": "dark",
            "source": 1
        }]
    });
    let mut unknown_condition = condition.clone();
    unknown_condition["unknown"] = serde_json::json!(true);
    let mut unknown_wrapper = condition.clone();
    unknown_wrapper["wrappers"][0]["unknown"] = serde_json::json!(true);
    let mut unknown_clause = condition;
    unknown_clause["clauses"][0]["unknown"] = serde_json::json!(true);

    for invalid in [old, unknown_condition, unknown_wrapper, unknown_clause] {
        assert_condition_wire_rejects(&invalid);
    }
}

#[test]
fn every_retained_environment_field_has_equivalent_wire_tamper_rejection() {
    let (document, resource) = fixture();
    let source_map = SourceMapSection::try_from_documents(&[&document]).expect("source map");
    let valid = serde_json::to_value(resource).expect("Style JSON");
    let mut candidates = Vec::new();

    let mut predicate = valid.clone();
    predicate["program"]["sheets"][0]["rules"][0]["environment"]["wrappers"][0]["predicate_source"] =
        serde_json::json!(3);
    candidates.push(predicate);

    let mut body = valid.clone();
    body["program"]["sheets"][0]["rules"][0]["environment"]["wrappers"][0]["body_source"] =
        serde_json::json!(0);
    candidates.push(body);

    let mut scope = valid.clone();
    scope["program"]["sheets"][0]["rules"][0]["environment"]["wrappers"][0]["scope_source"] =
        serde_json::json!(1);
    candidates.push(scope);

    let mut clause_source = valid.clone();
    clause_source["program"]["sheets"][0]["rules"][0]["environment"]["clauses"][0]["source"] =
        serde_json::json!(3);
    candidates.push(clause_source);

    let mut clause_wrapper = valid.clone();
    clause_wrapper["program"]["sheets"][0]["rules"][0]["environment"]["clauses"][0]["wrapper"] =
        serde_json::json!(1);
    candidates.push(clause_wrapper);

    let mut guarded_rule = valid;
    guarded_rule["program"]["sheets"][0]["rules"][0]["source"] = serde_json::json!(2);
    candidates.push(guarded_rule);

    for invalid in candidates {
        assert_style_wire_rejects(&invalid, &source_map);
    }
}

#[test]
fn nested_outer_and_inner_wrapper_fields_have_equivalent_wire_tamper_rejection() {
    let (document, resource) = nested_fixture(
        "nested-tamper.arcw",
        "view.style.nested-tamper",
        "style.nested-tamper",
    );
    let source_map = SourceMapSection::try_from_documents(&[&document]).expect("source map");
    let valid = serde_json::to_value(resource).expect("Style JSON");
    let mut candidates = Vec::new();

    for (wrapper, predicate, body, scope) in [(0_usize, 3, 0, 1), (1, 7, 4, 5)] {
        let mut invalid_predicate = valid.clone();
        invalid_predicate["program"]["sheets"][0]["rules"][0]["environment"]["wrappers"][wrapper]
            ["predicate_source"] = serde_json::json!(predicate);
        candidates.push(invalid_predicate);

        let mut invalid_body = valid.clone();
        invalid_body["program"]["sheets"][0]["rules"][0]["environment"]["wrappers"][wrapper]["body_source"] =
            serde_json::json!(body);
        candidates.push(invalid_body);

        let mut invalid_scope = valid.clone();
        invalid_scope["program"]["sheets"][0]["rules"][0]["environment"]["wrappers"][wrapper]["scope_source"] =
            serde_json::json!(scope);
        candidates.push(invalid_scope);
    }

    for invalid in candidates {
        assert_style_wire_rejects(&invalid, &source_map);
    }
}

#[test]
fn environment_sources_promote_only_inside_complete_product() {
    let (document, resource) = fixture();
    let source_map = SourceMapSection::try_from_documents(&[&document]).expect("source map");
    let bytes = resource.encode_canonical_section().expect("Style encodes");
    let decoded = ViewStyleResource::decode_canonical_section(&bytes).expect("Style decodes");
    assert_eq!(
        decoded
            .encode_canonical_section()
            .expect("decoded Style re-encodes"),
        bytes,
    );
    assert_eq!(decoded, resource);
    let product = promote(source_map, resource);

    assert!(product.program().is_none());
    let style = product.style().expect("validated Style");
    assert_eq!(style.program().sheets().len(), 1);
    assert_eq!(
        style.source_set_revision(),
        product.source_map().source_set_revision()
    );
}

#[test]
fn environment_complete_product_rejects_a_noncanonical_in_memory_candidate() {
    let (document, mut resource) = fixture();
    let source = resource.source_refs[0].clone();
    resource
        .source_map_refs
        .push(range(&resource.source_refs, &source, 0, 2));
    let source_map = SourceMapSection::try_from_documents(&[&document]).expect("source map");

    assert_eq!(
        promote_with_limits(source_map, resource, ViewProductValidationLimits::default(),)
            .expect_err("canonical encode/decode difference rejects before promotion"),
        ViewProductValidationError::NonCanonicalCandidate {
            resource: "ViewStyle",
        },
    );
}

#[test]
fn environment_wrapper_budget_counts_the_complete_nested_inventory() {
    let (_, resource) = nested_fixture("budget.arcw", "view.style.budget", "style.budget");
    let bytes = resource.encode_canonical_section().expect("Style encodes");
    let mut budget = ViewResourceBudget {
        environment_wrappers: 1,
        ..ViewResourceBudget::default()
    };
    assert!(ViewStyleResource::decode_canonical_section_with_budget(&bytes, budget).is_err());

    budget.environment_wrappers = 2;
    ViewStyleResource::decode_canonical_section_with_budget(&bytes, budget)
        .expect("exact wrapper budget accepts");
}

#[test]
fn environment_complete_product_enforces_missing_source_map_and_aggregate_work() {
    let (document, resource) = nested_fixture("work.arcw", "view.style.work", "style.work");
    assert_eq!(
        ValidatedViewProduct::try_new(
            None,
            None,
            Some(resource.clone()),
            ViewProductValidationLimits::default(),
        )
        .expect_err("source-bearing Style requires the product SourceMap"),
        ViewProductValidationError::MissingSourceMap,
    );
    let mut source_ids_without_tables = resource.clone();
    source_ids_without_tables.source_refs.clear();
    source_ids_without_tables.source_map_refs.clear();
    assert_eq!(
        ValidatedViewProduct::try_new(
            None,
            None,
            Some(source_ids_without_tables),
            ViewProductValidationLimits::default(),
        )
        .expect_err("Style source IDs alone still require the product SourceMap"),
        ViewProductValidationError::MissingSourceMap,
    );

    let source_map = SourceMapSection::try_from_documents(&[&document]).expect("source map");
    let exact = ViewProductValidationLimits {
        source_refs: 1,
        source_ranges: 10,
        validation_work: 23,
    };
    promote_with_limits(source_map.clone(), resource.clone(), exact)
        .expect("exact aggregate work budget accepts");
    let below = ViewProductValidationLimits {
        validation_work: 22,
        ..exact
    };
    assert!(matches!(
        promote_with_limits(source_map, resource.clone(), below),
        Err(ViewProductValidationError::BudgetExceeded {
            resource: "validation_work",
            actual: 23,
            limit: 22,
        })
    ));
    for (limits, resource_name, actual, limit) in [
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
                source_ranges: 9,
                ..exact
            },
            "source_ranges",
            10,
            9,
        ),
    ] {
        assert_eq!(
            promote_with_limits(
                SourceMapSection::try_from_documents(&[&document]).expect("source map"),
                resource.clone(),
                limits,
            )
            .expect_err("one-over aggregate budget rejects"),
            ViewProductValidationError::BudgetExceeded {
                resource: resource_name,
                actual,
                limit,
            },
        );
    }
}

#[test]
fn environment_complete_product_rejects_missing_stale_and_invalid_source_indices() {
    let (document, resource) = fixture();
    let other = source_document("missing.arcw", document.text());
    let other_map = SourceMapSection::try_from_documents(&[&other]).expect("other source map");
    assert!(matches!(
        promote_with_limits(
            other_map,
            resource.clone(),
            ViewProductValidationLimits::default(),
        ),
        Err(ViewProductValidationError::MissingSource { .. })
    ));

    let stale = source_document(document.identity().id().as_str(), "changed source text");
    let stale_map = SourceMapSection::try_from_documents(&[&stale]).expect("stale source map");
    assert!(matches!(
        promote_with_limits(
            stale_map,
            resource.clone(),
            ViewProductValidationLimits::default(),
        ),
        Err(ViewProductValidationError::StaleSource { .. })
    ));
    let mut stale_extent = serde_json::to_value(&resource).expect("Style JSON");
    let source_len = resource.source_refs[0].source_len();
    stale_extent["source_refs"][0]["source_len"] = serde_json::json!(source_len + 1);
    let stale_extent =
        serde_json::from_value::<ViewStyleResource>(stale_extent).expect("extent tamper");
    let exact_map = SourceMapSection::try_from_documents(&[&document]).expect("source map");
    assert!(matches!(
        promote_with_limits(
            exact_map,
            stale_extent,
            ViewProductValidationLimits::default(),
        ),
        Err(ViewProductValidationError::StaleSource { .. })
    ));

    let source_map = SourceMapSection::try_from_documents(&[&document]).expect("source map");
    let mut invalid_style_source = serde_json::to_value(&resource).expect("Style JSON");
    invalid_style_source["program"]["sheets"][0]["rules"][0]["environment"]["wrappers"][0]["predicate_source"] =
        serde_json::json!(99);
    let invalid_style_source =
        serde_json::from_value::<ViewStyleResource>(invalid_style_source).expect("typed tamper");
    assert_eq!(
        promote_with_limits(
            source_map.clone(),
            invalid_style_source,
            ViewProductValidationLimits::default(),
        )
        .expect_err("invalid Style source ID rejects"),
        ViewProductValidationError::View(SectionCodecError::NonCanonicalTable(
            "view_style_source_ids",
        )),
    );

    let mut invalid_product_source = serde_json::to_value(resource).expect("Style JSON");
    invalid_product_source["source_map_refs"][0]["source"] = serde_json::json!(99);
    let invalid_product_source =
        serde_json::from_value::<ViewStyleResource>(invalid_product_source).expect("typed tamper");
    assert_eq!(
        promote_with_limits(
            source_map,
            invalid_product_source,
            ViewProductValidationLimits::default(),
        )
        .expect_err("invalid product source index rejects"),
        ViewProductValidationError::InvalidSourceIndex {
            index: 99,
            count: 1
        },
    );
}

#[test]
fn environment_cross_source_relation_rejects_complete_product() {
    let (document, resource) = fixture();
    let other = source_document("other.arcw", "other source");
    let source_map =
        SourceMapSection::try_from_documents(&[&document, &other]).expect("two-source map");
    let other_ref = ProductSourceRef::from_document(
        source_map
            .documents()
            .find(|document| document.text() == "other source")
            .expect("other source document"),
    );
    let mut wrong_owner = resource.clone();
    wrong_owner.source_refs.push(other_ref.clone());
    wrong_owner.source_map_refs[0] = range(&wrong_owner.source_refs, &other_ref, 0, 1);
    assert!(matches!(
        ValidatedViewProduct::try_new(
            Some(source_map.clone()),
            None,
            Some(wrong_owner),
            ViewProductValidationLimits::default(),
        ),
        Err(ViewProductValidationError::StyleEnvironment(
            ViewStyleEnvironmentSourceError::WrongRuleOwner
        ))
    ));

    let mut cross_wrapper = resource.clone();
    cross_wrapper.source_refs.push(other_ref.clone());
    cross_wrapper.source_map_refs[1] = range(&cross_wrapper.source_refs, &other_ref, 0, 1);
    assert!(matches!(
        ValidatedViewProduct::try_new(
            Some(source_map.clone()),
            None,
            Some(cross_wrapper),
            ViewProductValidationLimits::default(),
        ),
        Err(ViewProductValidationError::StyleEnvironment(
            ViewStyleEnvironmentSourceError::CrossSourceRelation { .. }
        ))
    ));

    let mut cross_clause = resource;
    cross_clause.source_refs.push(other_ref.clone());
    cross_clause.source_map_refs[2] = range(&cross_clause.source_refs, &other_ref, 0, 1);
    assert!(matches!(
        ValidatedViewProduct::try_new(
            Some(source_map),
            None,
            Some(cross_clause),
            ViewProductValidationLimits::default(),
        ),
        Err(ViewProductValidationError::StyleEnvironment(
            ViewStyleEnvironmentSourceError::CrossSourceRelation {
                role: ViewStyleEnvironmentSourceRole::Clause { .. }
            }
        ))
    ));

    assert_nested_cross_source_roles_reject();
}

fn assert_nested_cross_source_roles_reject() {
    let (nested_document, nested) = nested_fixture(
        "nested-owner.arcw",
        "view.style.nested-owner",
        "style.nested-owner",
    );
    let nested_other = source_document("nested-other.arcw", "other source");
    let nested_source_map =
        SourceMapSection::try_from_documents(&[&nested_document, &nested_other])
            .expect("two-source nested map");
    let nested_other_ref = ProductSourceRef::from_document(
        nested_source_map
            .documents()
            .find(|document| document.text() == "other source")
            .expect("other nested source document"),
    );
    for (source_id, role) in [
        (
            5_usize,
            ViewStyleEnvironmentSourceRole::Predicate {
                wrapper: ViewEnvironmentWrapperIndex::new(1),
            },
        ),
        (
            7,
            ViewStyleEnvironmentSourceRole::Body {
                wrapper: ViewEnvironmentWrapperIndex::new(1),
            },
        ),
        (
            4,
            ViewStyleEnvironmentSourceRole::Scope {
                wrapper: ViewEnvironmentWrapperIndex::new(1),
            },
        ),
    ] {
        let mut candidate = nested.clone();
        candidate.source_refs.push(nested_other_ref.clone());
        candidate.source_map_refs[source_id] =
            range(&candidate.source_refs, &nested_other_ref, 0, 1);
        assert_eq!(
            ValidatedViewProduct::try_new(
                Some(nested_source_map.clone()),
                None,
                Some(candidate),
                ViewProductValidationLimits::default(),
            )
            .expect_err("each inner wrapper role must retain the outer source owner"),
            ViewProductValidationError::StyleEnvironment(
                ViewStyleEnvironmentSourceError::CrossSourceRelation { role },
            ),
        );
    }
}

#[test]
fn environment_source_range_out_of_bounds_or_utf8_boundary_rejects() {
    let (document, resource) = fixture();
    let source_map = SourceMapSection::try_from_documents(&[&document]).expect("source map");
    let source = resource.source_refs[0].clone();

    let mut out_of_bounds = resource.clone();
    out_of_bounds.source_map_refs[0] = range(
        &out_of_bounds.source_refs,
        &source,
        0,
        u32::try_from(document.text().len() + 1).expect("small fixture"),
    );
    assert!(matches!(
        ValidatedViewProduct::try_new(
            Some(source_map.clone()),
            None,
            Some(out_of_bounds),
            ViewProductValidationLimits::default(),
        ),
        Err(ViewProductValidationError::OutOfBoundsRange)
    ));

    let mut split_code_point = resource;
    let end = split_code_point.source_map_refs[0].end_byte();
    split_code_point.source_map_refs[0] = range(&split_code_point.source_refs, &source, 1, end);
    assert!(matches!(
        ValidatedViewProduct::try_new(
            Some(source_map),
            None,
            Some(split_code_point),
            ViewProductValidationLimits::default(),
        ),
        Err(ViewProductValidationError::NonUtf8Boundary)
    ));
}

#[test]
fn owning_predicate_must_contain_each_clause() {
    let (document, mut resource) = fixture();
    let source_map = SourceMapSection::try_from_documents(&[&document]).expect("source map");
    let source = resource.source_refs[0].clone();
    let predicate_end = resource.source_map_refs[1].end_byte();
    let clause_start = resource.source_map_refs[2].start_byte();
    resource.source_map_refs[2] = range(
        &resource.source_refs,
        &source,
        clause_start,
        predicate_end + 1,
    );

    assert!(matches!(
        ValidatedViewProduct::try_new(
            Some(source_map),
            None,
            Some(resource),
            ViewProductValidationLimits::default(),
        ),
        Err(ViewProductValidationError::StyleEnvironment(
            ViewStyleEnvironmentSourceError::ClauseNotContainedByPredicate { .. }
        ))
    ));
}

#[test]
fn environment_relation_edges_report_the_exact_failed_contract() {
    let (document, resource) = fixture();
    let source = resource.source_refs[0].clone();
    let source_map = SourceMapSection::try_from_documents(&[&document]).expect("source map");

    let mut predicate = resource.clone();
    let predicate_end = predicate.source_map_refs[1].end_byte();
    predicate.source_map_refs[1] = range(&predicate.source_refs, &source, 0, predicate_end);
    assert!(matches!(
        promote_with_limits(
            source_map.clone(),
            predicate,
            ViewProductValidationLimits::default()
        ),
        Err(ViewProductValidationError::StyleEnvironment(
            ViewStyleEnvironmentSourceError::PredicateNotContainedByScope { .. }
        ))
    ));

    let mut body = resource.clone();
    let body_end = body.source_map_refs[3].end_byte();
    body.source_map_refs[3] = range(&body.source_refs, &source, 0, body_end);
    assert!(matches!(
        promote_with_limits(
            source_map.clone(),
            body,
            ViewProductValidationLimits::default()
        ),
        Err(ViewProductValidationError::StyleEnvironment(
            ViewStyleEnvironmentSourceError::BodyNotContainedByScope { .. }
        ))
    ));

    let mut overlap = resource.clone();
    let predicate_start = overlap.source_map_refs[1].start_byte();
    let body_start = overlap.source_map_refs[3].start_byte();
    overlap.source_map_refs[1] = range(
        &overlap.source_refs,
        &source,
        predicate_start,
        body_start + 1,
    );
    assert!(matches!(
        promote_with_limits(
            source_map.clone(),
            overlap,
            ViewProductValidationLimits::default()
        ),
        Err(ViewProductValidationError::StyleEnvironment(
            ViewStyleEnvironmentSourceError::PredicateBodyOrder { .. }
        ))
    ));

    let mut guarded_rule = resource;
    let rule_start = guarded_rule.source_map_refs[4].start_byte();
    let body_end = guarded_rule.source_map_refs[3].end_byte();
    guarded_rule.source_map_refs[4] =
        range(&guarded_rule.source_refs, &source, rule_start, body_end + 1);
    assert!(matches!(
        promote_with_limits(
            source_map,
            guarded_rule,
            ViewProductValidationLimits::default()
        ),
        Err(ViewProductValidationError::StyleEnvironment(
            ViewStyleEnvironmentSourceError::GuardedRuleNotContained { .. }
        ))
    ));
}

#[test]
fn environment_empty_and_reversed_ranges_reject_for_every_retained_role() {
    let (document, resource) = fixture();
    let source_map = SourceMapSection::try_from_documents(&[&document]).expect("source map");
    let roles = [
        ViewStyleEnvironmentSourceRole::Predicate {
            wrapper: ViewEnvironmentWrapperIndex::new(0),
        },
        ViewStyleEnvironmentSourceRole::Body {
            wrapper: ViewEnvironmentWrapperIndex::new(0),
        },
        ViewStyleEnvironmentSourceRole::Scope {
            wrapper: ViewEnvironmentWrapperIndex::new(0),
        },
        ViewStyleEnvironmentSourceRole::Clause {
            wrapper: ViewEnvironmentWrapperIndex::new(0),
        },
        ViewStyleEnvironmentSourceRole::GuardedRule,
    ];

    let source_ids = [1_usize, 3, 0, 2, 4];
    for (source_id, role) in source_ids.into_iter().zip(roles) {
        let mut empty = resource.clone();
        let start = empty.source_map_refs[source_id].start_byte();
        let source = empty.source_refs[0].clone();
        empty.source_map_refs[source_id] = range(&empty.source_refs, &source, start, start);
        assert_eq!(
            promote_with_limits(
                source_map.clone(),
                empty,
                ViewProductValidationLimits::default(),
            )
            .expect_err("empty retained environment range rejects"),
            ViewProductValidationError::StyleEnvironment(
                ViewStyleEnvironmentSourceError::EmptyRange { role },
            ),
        );

        let mut reversed = resource.clone();
        let start = reversed.source_map_refs[source_id].start_byte();
        let end = reversed.source_map_refs[source_id].end_byte();
        let source = reversed.source_refs[0].clone();
        reversed.source_map_refs[source_id] = range(&reversed.source_refs, &source, end, start);
        assert_eq!(
            promote_with_limits(
                source_map.clone(),
                reversed,
                ViewProductValidationLimits::default(),
            )
            .expect_err("reversed retained environment range rejects"),
            ViewProductValidationError::ReversedRange,
        );
    }
}

#[test]
fn environment_integrated_equality_boundaries_are_accepted() {
    let (document, mut one_wrapper) = fixture();
    let source = one_wrapper.source_refs[0].clone();
    let predicate_start = one_wrapper.source_map_refs[1].start_byte();
    let body_start = one_wrapper.source_map_refs[3].start_byte();
    let body_end = one_wrapper.source_map_refs[3].end_byte();
    one_wrapper.source_map_refs[1] = range(
        &one_wrapper.source_refs,
        &source,
        predicate_start,
        body_start,
    );
    one_wrapper.source_map_refs[2] = one_wrapper.source_map_refs[1];
    one_wrapper.source_map_refs[4] = one_wrapper.source_map_refs[3];
    let source_map = SourceMapSection::try_from_documents(&[&document]).expect("source map");
    promote(source_map, one_wrapper);

    let (document, mut nested) = nested_fixture("equal.arcw", "view.style.equal", "style.equal");
    nested.source_map_refs[4] = nested.source_map_refs[3];
    let source_map = SourceMapSection::try_from_documents(&[&document]).expect("source map");
    promote(source_map, nested);

    assert!(predicate_start < body_start);
    assert!(body_start < body_end);
}

#[test]
fn environment_merge_remaps_every_nested_source_and_rejects_invalid_candidates_atomically() {
    let (_, left) = nested_fixture("left.arcw", "view.style.left", "style.left");
    let (_, right) = nested_fixture("right.arcw", "view.style.right", "style.right");
    let right_condition = right.program.sheets()[0].rules()[0]
        .environment()
        .expect("right condition");
    let expected = right_condition
        .wrappers()
        .iter()
        .flat_map(|wrapper| {
            [
                wrapper.predicate_source(),
                wrapper.body_source(),
                wrapper.scope_source(),
            ]
        })
        .chain(
            right_condition
                .clauses()
                .iter()
                .map(|clause| clause.source()),
        )
        .chain(std::iter::once(
            right.program.sheets()[0].rules()[0].source(),
        ))
        .chain(
            right.program.sheets()[0].rules()[0]
                .declarations()
                .iter()
                .map(ViewStyleDeclaration::source),
        )
        .map(|source| source_provenance(&right, source))
        .collect::<Vec<_>>();

    let merged = ViewProgramStyleResources::new(None, Some(left.clone()))
        .merge(ViewProgramStyleResources::new(None, Some(right.clone())))
        .expect("nested environments merge")
        .style
        .expect("merged Style");
    let right_sheet = merged
        .program
        .sheets()
        .iter()
        .find(|sheet| sheet.id().public_id().as_str() == "style.right")
        .expect("right sheet");
    let merged_condition = right_sheet.rules()[0]
        .environment()
        .expect("merged right condition");
    let actual = merged_condition
        .wrappers()
        .iter()
        .flat_map(|wrapper| {
            [
                wrapper.predicate_source(),
                wrapper.body_source(),
                wrapper.scope_source(),
            ]
        })
        .chain(
            merged_condition
                .clauses()
                .iter()
                .map(|clause| clause.source()),
        )
        .chain(std::iter::once(right_sheet.rules()[0].source()))
        .chain(
            right_sheet.rules()[0]
                .declarations()
                .iter()
                .map(ViewStyleDeclaration::source),
        )
        .map(|source| source_provenance(&merged, source))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);

    let mut invalid = right.clone();
    let source = invalid.source_refs[0].clone();
    let inner_scope_end = invalid.source_map_refs[4].end_byte();
    invalid.source_map_refs[4] = range(&invalid.source_refs, &source, 0, inner_scope_end);
    assert!(
        ViewProgramStyleResources::new(None, Some(left.clone()))
            .merge(ViewProgramStyleResources::new(None, Some(invalid)))
            .is_err()
    );
    left.encode_canonical_section()
        .expect("left input remains canonical");
    right
        .encode_canonical_section()
        .expect("right input remains canonical");
}

#[test]
fn environment_merge_accepts_exact_budgets_and_rejects_each_one_over_atomically() {
    let (_, left) = nested_fixture("budget-left.arcw", "view.style.left", "style.left");
    let (_, right) = nested_fixture("budget-right.arcw", "view.style.right", "style.right");
    let left_snapshot = left.clone();
    let right_snapshot = right.clone();
    let baseline = ViewProgramStyleResources::new(None, Some(left.clone()))
        .merge(ViewProgramStyleResources::new(None, Some(right.clone())))
        .expect("baseline merge")
        .style
        .expect("merged Style");
    let condition_count = baseline
        .program
        .sheets()
        .iter()
        .flat_map(ViewStyleSheet::rules)
        .filter(|rule| rule.environment().is_some())
        .count();
    let wrapper_count = baseline
        .program
        .sheets()
        .iter()
        .flat_map(ViewStyleSheet::rules)
        .filter_map(ViewStyleRule::environment)
        .map(|condition| condition.wrappers().len())
        .sum();
    let clause_count = baseline
        .program
        .sheets()
        .iter()
        .flat_map(ViewStyleSheet::rules)
        .filter_map(ViewStyleRule::environment)
        .map(|condition| condition.clauses().len())
        .sum();
    let exact = ViewResourceBudget {
        environment_conditions: condition_count,
        environment_wrappers: wrapper_count,
        environment_clauses: clause_count,
        source_map_refs: baseline.source_map_refs.len(),
        transcript_bytes: serde_json::to_vec(&baseline)
            .expect("merged Style transcript")
            .len(),
        ..ViewResourceBudget::default()
    };

    let accepted = ViewProgramStyleResources::new(None, Some(left.clone()))
        .merge_with_budget(
            ViewProgramStyleResources::new(None, Some(right.clone())),
            exact,
        )
        .expect("exact merge budgets accept")
        .style
        .expect("merged Style");
    assert_eq!(accepted, baseline);

    let failures = [
        (
            "view_style_environment_wrappers",
            ViewResourceBudget {
                environment_wrappers: exact.environment_wrappers - 1,
                ..exact
            },
        ),
        (
            "view_style_environment_clauses",
            ViewResourceBudget {
                environment_clauses: exact.environment_clauses - 1,
                ..exact
            },
        ),
        (
            "view_style_source_map_refs",
            ViewResourceBudget {
                source_map_refs: exact.source_map_refs - 1,
                ..exact
            },
        ),
        (
            "view_transcript_bytes",
            ViewResourceBudget {
                transcript_bytes: exact.transcript_bytes - 1,
                ..exact
            },
        ),
    ];
    for (budget_name, budget) in failures {
        let error = ViewProgramStyleResources::new(None, Some(left.clone()))
            .merge_with_budget(
                ViewProgramStyleResources::new(None, Some(right.clone())),
                budget,
            )
            .expect_err("one-over merged inventory rejects");
        assert_eq!(
            error,
            ViewResourceMergeError::Section(SectionCodecError::BudgetExceeded(budget_name)),
        );
        assert_eq!(left, left_snapshot);
        assert_eq!(right, right_snapshot);
    }
}

#[test]
fn environment_merge_preflight_rejects_each_local_containment_failure_on_either_side() {
    let (_, valid) = nested_fixture("valid.arcw", "view.style.valid", "style.valid");
    let source = valid.source_refs[0].clone();

    let mut predicate_outside_scope = valid.clone();
    let scope = predicate_outside_scope.source_map_refs[0];
    let predicate = predicate_outside_scope.source_map_refs[1];
    predicate_outside_scope.source_map_refs[0] = range(
        &predicate_outside_scope.source_refs,
        &source,
        predicate.start_byte() + 1,
        scope.end_byte(),
    );

    let mut nested_scope_outside_body = valid.clone();
    let inner_scope = nested_scope_outside_body.source_map_refs[4];
    nested_scope_outside_body.source_map_refs[4] = range(
        &nested_scope_outside_body.source_refs,
        &source,
        0,
        inner_scope.end_byte(),
    );

    let mut rule_outside_body = valid.clone();
    let rule = rule_outside_body.source_map_refs[8];
    rule_outside_body.source_map_refs[8] =
        range(&rule_outside_body.source_refs, &source, 0, rule.end_byte());

    let failures = [
        (
            predicate_outside_scope,
            ViewStyleEnvironmentSourceError::PredicateNotContainedByScope {
                wrapper: ViewEnvironmentWrapperIndex::new(0),
            },
        ),
        (
            nested_scope_outside_body,
            ViewStyleEnvironmentSourceError::NestedScopeNotContained {
                parent: ViewEnvironmentWrapperIndex::new(0),
                child: ViewEnvironmentWrapperIndex::new(1),
            },
        ),
        (
            rule_outside_body,
            ViewStyleEnvironmentSourceError::GuardedRuleNotContained {
                wrapper: ViewEnvironmentWrapperIndex::new(1),
            },
        ),
    ];

    for (invalid, expected) in failures {
        for (left, right) in [
            (invalid.clone(), valid.clone()),
            (valid.clone(), invalid.clone()),
        ] {
            assert_eq!(
                ViewProgramStyleResources::new(None, Some(left))
                    .merge(ViewProgramStyleResources::new(None, Some(right)))
                    .expect_err("invalid local containment rejects before merge"),
                ViewResourceMergeError::Section(SectionCodecError::ViewStyleEnvironmentSource(
                    expected.clone()
                )),
            );
        }
        valid
            .encode_canonical_section()
            .expect("valid input remains canonical");
        assert_eq!(invalid.source_refs, valid.source_refs);
    }
}

#[test]
fn merged_style_defers_missing_right_document_rejection_to_product_promotion() {
    let (left_document, left) = nested_fixture("left-only.arcw", "view.style.left", "style.left");
    let (_, right) = nested_fixture("missing-right.arcw", "view.style.right", "style.right");
    let missing = right.source_refs[0].id().clone();
    let merged = ViewProgramStyleResources::new(None, Some(left))
        .merge(ViewProgramStyleResources::new(None, Some(right)))
        .expect("merge does not own a SourceMap registry")
        .style
        .expect("merged Style");
    let source_map =
        SourceMapSection::try_from_documents(&[&left_document]).expect("left-only SourceMap");

    assert_eq!(
        ValidatedViewProduct::try_new(
            Some(source_map),
            None,
            Some(merged),
            ViewProductValidationLimits::default(),
        )
        .expect_err("complete product rejects the missing right document"),
        ViewProductValidationError::MissingSource { id: missing },
    );
}

fn promote(source_map: SourceMapSection, resource: ViewStyleResource) -> ValidatedViewProduct {
    ValidatedViewProduct::try_new(
        Some(source_map),
        None,
        Some(resource),
        ViewProductValidationLimits::default(),
    )
    .expect("complete Style product validates")
}

fn promote_with_limits(
    source_map: SourceMapSection,
    resource: ViewStyleResource,
    limits: ViewProductValidationLimits,
) -> Result<ValidatedViewProduct, ViewProductValidationError> {
    ValidatedViewProduct::try_new(Some(source_map), None, Some(resource), limits)
}

fn fixture() -> (SourceDocument, ViewStyleResource) {
    let text = "éwhen environment(color-scheme == dark) { Button { opacity = 1 } }";
    let document = source_document("main.arcw", text);
    let source_map = SourceMapSection::try_from_documents(&[&document]).expect("source map");
    let source = ProductSourceRef::from_document(source_map.documents().next().expect("source"));
    let source_refs = vec![source.clone()];
    let predicate_start = text.find('(').expect("predicate start");
    let predicate_end = text.find(')').expect("predicate end") + 1;
    let body_open = text.find('{').expect("body open");
    let body_close = text.rfind('}').expect("body close");
    let scope_start = text.find("when").expect("scope start");
    let scope_end = body_close + 1;
    let clause_start = predicate_start + 1;
    let clause_end = predicate_end - 1;
    let rule_text = "Button { opacity = 1 }";
    let rule_start = text.find(rule_text).expect("rule start");
    let rule_end = rule_start + rule_text.len();
    let declaration_text = "opacity = 1";
    let declaration_start = text.find(declaration_text).expect("declaration start");
    let declaration_end = declaration_start + declaration_text.len();

    let condition = ViewEnvironmentCondition::try_new(
        vec![ViewEnvironmentWrapperSource::new(
            ViewStyleSourceId::new(1),
            ViewStyleSourceId::new(3),
            ViewStyleSourceId::new(0),
        )],
        vec![ViewEnvironmentClause::color_scheme(
            ColorScheme::Dark,
            ViewEnvironmentWrapperIndex::new(0),
            ViewStyleSourceId::new(2),
        )],
    )
    .expect("checked condition");
    let selector = ViewStyleSelector::new(vec![
        ViewStyleSelectorSequence::new(None, Some(ViewElementKind::Button), None, Vec::new())
            .expect("selector sequence"),
    ])
    .expect("selector");
    let declaration = ViewStyleDeclaration::new(
        ViewPropertyKind::Opacity,
        ViewSpecifiedValue::Ratio {
            value: ViewRatioMilli::new(1_000).expect("ratio"),
        },
        ViewStyleAssignOp::Replace,
        ViewStyleSourceId::new(5),
    )
    .expect("declaration");
    let rule = ViewStyleRule::new(
        selector,
        Some(condition),
        vec![declaration],
        0,
        ViewStyleSourceId::new(4),
    )
    .expect("rule");
    let sheet_id = ViewStyleSheetId::try_new("style.adaptive").expect("sheet ID");
    let sheet = ViewStyleSheet::new(sheet_id, Vec::new(), vec![rule]).expect("sheet");
    let resource = ViewStyleResource {
        style_program_id: "view.style.program".to_owned(),
        program: ViewStyleProgram::try_new(vec![sheet], Vec::new()).expect("program"),
        source_refs,
        source_map_refs: vec![
            range_from_usize(&source, scope_start, scope_end),
            range_from_usize(&source, predicate_start, predicate_end),
            range_from_usize(&source, clause_start, clause_end),
            range_from_usize(&source, body_open + 1, body_close),
            range_from_usize(&source, rule_start, rule_end),
            range_from_usize(&source, declaration_start, declaration_end),
        ],
        adapter_requirements: Vec::new(),
    };
    (document, resource)
}

fn nested_fixture(
    document_id: &str,
    program_id: &str,
    sheet_id: &str,
) -> (SourceDocument, ViewStyleResource) {
    let text = "when environment(color-scheme == dark) { when environment(reduced-motion == true) { Button { opacity = 1 } } }";
    let document = source_document(document_id, text);
    let source_map = SourceMapSection::try_from_documents(&[&document]).expect("source map");
    let source = ProductSourceRef::from_document(source_map.documents().next().expect("source"));
    let source_refs = vec![source.clone()];
    let opens = text
        .match_indices('{')
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let closes = text
        .match_indices('}')
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let predicates = text
        .match_indices('(')
        .map(|(start, _)| {
            let end = text[start..].find(')').expect("predicate end") + start + 1;
            (start, end)
        })
        .collect::<Vec<_>>();
    let inner_scope_start = text[1..].find("when").expect("inner scope") + 1;
    let rule_start = text.find("Button").expect("rule start");
    let declaration_start = text.find("opacity").expect("declaration start");
    let declaration_end = declaration_start + "opacity = 1".len();
    let condition = ViewEnvironmentCondition::try_new(
        vec![
            ViewEnvironmentWrapperSource::new(
                ViewStyleSourceId::new(1),
                ViewStyleSourceId::new(3),
                ViewStyleSourceId::new(0),
            ),
            ViewEnvironmentWrapperSource::new(
                ViewStyleSourceId::new(5),
                ViewStyleSourceId::new(7),
                ViewStyleSourceId::new(4),
            ),
        ],
        vec![
            ViewEnvironmentClause::color_scheme(
                ColorScheme::Dark,
                ViewEnvironmentWrapperIndex::new(0),
                ViewStyleSourceId::new(2),
            ),
            ViewEnvironmentClause::reduced_motion(
                true,
                ViewEnvironmentWrapperIndex::new(1),
                ViewStyleSourceId::new(6),
            ),
        ],
    )
    .expect("nested condition");
    let selector = ViewStyleSelector::new(vec![
        ViewStyleSelectorSequence::new(None, Some(ViewElementKind::Button), None, Vec::new())
            .expect("selector sequence"),
    ])
    .expect("selector");
    let declaration = ViewStyleDeclaration::new(
        ViewPropertyKind::Opacity,
        ViewSpecifiedValue::Ratio {
            value: ViewRatioMilli::new(1_000).expect("ratio"),
        },
        ViewStyleAssignOp::Replace,
        ViewStyleSourceId::new(9),
    )
    .expect("declaration");
    let rule = ViewStyleRule::new(
        selector,
        Some(condition),
        vec![declaration],
        0,
        ViewStyleSourceId::new(8),
    )
    .expect("rule");
    let sheet = ViewStyleSheet::new(
        ViewStyleSheetId::try_new(sheet_id).expect("sheet ID"),
        Vec::new(),
        vec![rule],
    )
    .expect("sheet");
    (
        document,
        ViewStyleResource {
            style_program_id: program_id.to_owned(),
            program: ViewStyleProgram::try_new(vec![sheet], Vec::new()).expect("program"),
            source_refs,
            source_map_refs: vec![
                range_from_usize(&source, 0, closes[2] + 1),
                range_from_usize(&source, predicates[0].0, predicates[0].1),
                range_from_usize(&source, predicates[0].0 + 1, predicates[0].1 - 1),
                range_from_usize(&source, opens[0] + 1, closes[2]),
                range_from_usize(&source, inner_scope_start, closes[1] + 1),
                range_from_usize(&source, predicates[1].0, predicates[1].1),
                range_from_usize(&source, predicates[1].0 + 1, predicates[1].1 - 1),
                range_from_usize(&source, opens[1] + 1, closes[1]),
                range_from_usize(&source, rule_start, closes[0] + 1),
                range_from_usize(&source, declaration_start, declaration_end),
            ],
            adapter_requirements: Vec::new(),
        },
    )
}

fn source_provenance(
    resource: &ViewStyleResource,
    source: ViewStyleSourceId,
) -> (ProductSourceRef, u32, u32) {
    let range = resource.source_map_refs[source.value() as usize];
    (
        resource.source_refs[range.source().value() as usize].clone(),
        range.start_byte(),
        range.end_byte(),
    )
}

fn assert_condition_wire_rejects(invalid: &serde_json::Value) {
    assert!(serde_json::from_value::<ViewEnvironmentCondition>(invalid.clone()).is_err());

    #[cfg(feature = "format-cbor")]
    {
        let mut bytes = Vec::new();
        ciborium::into_writer(invalid, &mut bytes).expect("CBOR encodes");
        assert!(ciborium::from_reader::<ViewEnvironmentCondition, _>(bytes.as_slice()).is_err());
    }

    #[cfg(feature = "format-messagepack")]
    {
        let bytes = rmp_serde::to_vec_named(invalid).expect("MessagePack encodes");
        assert!(rmp_serde::from_slice::<ViewEnvironmentCondition>(&bytes).is_err());
    }
}

fn assert_style_wire_rejects(invalid: &serde_json::Value, source_map: &SourceMapSection) {
    let reject = |candidate: Result<ViewStyleResource, String>| {
        let Ok(candidate) = candidate else {
            return true;
        };
        promote_with_limits(
            source_map.clone(),
            candidate,
            ViewProductValidationLimits::default(),
        )
        .is_err()
    };
    assert!(reject(
        serde_json::from_value(invalid.clone()).map_err(|error| error.to_string())
    ));

    #[cfg(feature = "format-cbor")]
    {
        let mut bytes = Vec::new();
        ciborium::into_writer(invalid, &mut bytes).expect("CBOR encodes");
        assert!(reject(
            ciborium::from_reader(bytes.as_slice()).map_err(|error| error.to_string())
        ));
    }

    #[cfg(feature = "format-messagepack")]
    {
        let bytes = rmp_serde::to_vec_named(invalid).expect("MessagePack encodes");
        assert!(reject(
            rmp_serde::from_slice(&bytes).map_err(|error| error.to_string())
        ));
    }
}

fn source_document(id: &str, text: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new(id).expect("source ID"),
        SourceName::path(id),
        text,
    )
    .expect("source document")
}

fn range_from_usize(source: &ProductSourceRef, start: usize, end: usize) -> SourceRangeRef {
    let refs = vec![source.clone()];
    range(
        &refs,
        source,
        u32::try_from(start).expect("fixture range"),
        u32::try_from(end).expect("fixture range"),
    )
}

fn range(
    source_refs: &[ProductSourceRef],
    source: &ProductSourceRef,
    start: u32,
    end: u32,
) -> SourceRangeRef {
    SourceRangeRef::try_for_source(source_refs, source, start, end).expect("fixture source range")
}
