use arcweft_bundle::resource_codec::view::{ViewStyleEnvironmentSourceError, ViewStyleResource};
use arcweft_bundle::resource_codec::{
    ProductSourceRef, SectionCodecError, SourceMapSection, SourceRangeRef,
};
use arcweft_presentation::appearance::ColorScheme;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use arcweft_view::ViewElementKind;
use arcweft_view::style::{
    ViewEnvironmentClause, ViewEnvironmentCondition, ViewPropertyKind, ViewRatioMilli,
    ViewSpecifiedValue, ViewStyleAssignOp, ViewStyleDeclaration, ViewStyleProgram, ViewStyleRule,
    ViewStyleSelector, ViewStyleSelectorSequence, ViewStyleSheet, ViewStyleSheetId,
    ViewStyleSourceId,
};

#[test]
fn environment_product_round_trips_canonical_json_cbor_msgpack() {
    let (_, resource) = fixture();
    let condition = resource.program.sheets()[0].rules()[0]
        .environment()
        .expect("fixture guard");

    let json = serde_json::to_vec(condition).expect("JSON encodes");
    assert_eq!(
        serde_json::from_slice::<ViewEnvironmentCondition>(&json).expect("JSON decodes"),
        condition.clone()
    );

    #[cfg(feature = "format-cbor")]
    {
        let mut cbor = Vec::new();
        ciborium::into_writer(condition, &mut cbor).expect("CBOR encodes");
        assert_eq!(
            ciborium::from_reader::<ViewEnvironmentCondition, _>(cbor.as_slice())
                .expect("CBOR decodes"),
            condition.clone()
        );
    }

    #[cfg(feature = "format-messagepack")]
    {
        let messagepack = rmp_serde::to_vec_named(condition).expect("MessagePack encodes");
        assert_eq!(
            rmp_serde::from_slice::<ViewEnvironmentCondition>(&messagepack)
                .expect("MessagePack decodes"),
            condition.clone()
        );
    }
}

#[test]
fn environment_sources_resolve_in_complete_source_map() {
    let (source_map, resource) = fixture();

    resource
        .validate_environment_sources(&source_map)
        .expect("condition, clause, and rule ranges resolve");
    let bytes = resource.encode_canonical_section().expect("Style encodes");
    let decoded = ViewStyleResource::decode_canonical_section(&bytes).expect("Style decodes");
    decoded
        .validate_environment_sources(&source_map)
        .expect("decoded ranges retain final source identities");
}

#[test]
fn environment_cross_source_relation_rejects_complete_product() {
    let (source_map, mut resource) = fixture();
    let other = source_document("other.arcw", "other source");
    let other_map = SourceMapSection::try_from_documents(&[&other]).expect("other source map");
    let other_ref = ProductSourceRef::from_document(
        other_map.documents().next().expect("other source document"),
    );
    resource.source_refs.push(other_ref.clone());
    resource.source_map_refs[1] = range(&resource.source_refs, &other_ref, 0, 1);

    assert!(matches!(
        resource.validate_environment_sources(&source_map),
        Err(SectionCodecError::ViewStyleEnvironmentSource(
            ViewStyleEnvironmentSourceError::WrongOwner
        ))
    ));
}

#[test]
fn environment_source_range_out_of_bounds_or_utf8_boundary_rejects() {
    let (source_map, resource) = fixture();
    let source = resource.source_refs[0].clone();

    let mut out_of_bounds = resource.clone();
    out_of_bounds.source_map_refs[0] = range(
        &out_of_bounds.source_refs,
        &source,
        0,
        u32::try_from(source_map.documents().next().expect("source").text().len() + 1)
            .expect("small fixture"),
    );
    assert!(matches!(
        out_of_bounds.validate_environment_sources(&source_map),
        Err(SectionCodecError::ViewStyleEnvironmentSource(
            ViewStyleEnvironmentSourceError::SourceOutOfBounds
        ))
    ));

    let mut split_code_point = resource;
    let end = split_code_point.source_map_refs[0].end_byte();
    split_code_point.source_map_refs[0] = range(&split_code_point.source_refs, &source, 1, end);
    assert!(matches!(
        split_code_point.validate_environment_sources(&source_map),
        Err(SectionCodecError::ViewStyleEnvironmentSource(
            ViewStyleEnvironmentSourceError::InvalidUtf8Boundary
        ))
    ));
}

#[test]
fn condition_must_contain_clause_ranges() {
    let (source_map, mut resource) = fixture();
    let source = resource.source_refs[0].clone();
    let condition_end = resource.source_map_refs[0].end_byte();
    let clause_start = resource.source_map_refs[1].start_byte();
    resource.source_map_refs[1] = range(
        &resource.source_refs,
        &source,
        clause_start,
        condition_end + 1,
    );

    assert!(matches!(
        resource.validate_environment_sources(&source_map),
        Err(SectionCodecError::ViewStyleEnvironmentSource(
            ViewStyleEnvironmentSourceError::ClauseNotContained
        ))
    ));
}

fn fixture() -> (SourceMapSection, ViewStyleResource) {
    let text = "éwhen environment(color-scheme == dark) { Button { opacity = 1 } }";
    let document = source_document("main.arcw", text);
    let source_map = SourceMapSection::try_from_documents(&[&document]).expect("source map");
    let source = ProductSourceRef::from_document(source_map.documents().next().expect("source"));
    let source_refs = vec![source.clone()];
    let condition_start = text.find('(').expect("condition start");
    let condition_end = text.find(')').expect("condition end") + 1;
    let clause_start = condition_start + 1;
    let clause_end = condition_end - 1;
    let rule_start = text.find("Button").expect("rule start");
    let rule_end = text.len() - 2;
    let declaration_start = text.find("opacity").expect("declaration start");
    let declaration_end = rule_end - 2;

    let condition = ViewEnvironmentCondition::try_new(
        ViewStyleSourceId::new(0),
        vec![ViewEnvironmentClause::color_scheme(
            ColorScheme::Dark,
            ViewStyleSourceId::new(1),
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
        ViewStyleSourceId::new(3),
    )
    .expect("declaration");
    let rule = ViewStyleRule::new(
        selector,
        Some(condition),
        vec![declaration],
        0,
        ViewStyleSourceId::new(2),
    )
    .expect("rule");
    let sheet_id = ViewStyleSheetId::try_new("style.adaptive").expect("sheet ID");
    let sheet = ViewStyleSheet::new(sheet_id, Vec::new(), vec![rule]).expect("sheet");
    let resource = ViewStyleResource {
        style_program_id: "view.style.program".to_owned(),
        program: ViewStyleProgram::try_new(vec![sheet], Vec::new()).expect("program"),
        source_refs,
        source_map_refs: vec![
            range_from_usize(&source, &source_map, condition_start, condition_end),
            range_from_usize(&source, &source_map, clause_start, clause_end),
            range_from_usize(&source, &source_map, rule_start, rule_end),
            range_from_usize(&source, &source_map, declaration_start, declaration_end),
        ],
        adapter_requirements: Vec::new(),
    };
    (source_map, resource)
}

fn source_document(id: &str, text: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new(id).expect("source ID"),
        SourceName::path(id),
        text,
    )
    .expect("source document")
}

fn range_from_usize(
    source: &ProductSourceRef,
    source_map: &SourceMapSection,
    start: usize,
    end: usize,
) -> SourceRangeRef {
    let refs = vec![source.clone()];
    debug_assert_eq!(source_map.documents().count(), 1);
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
