use arcweft_bundle::BundleSource;
use arcweft_bundle::resource_codec::view::{ViewStyleEnvironmentSourceError, ViewStyleResource};
use arcweft_bundle::resource_codec::{
    PublicIdRef, SectionCodecError, SourceMapIndex, SourceMapSourceId, SourceRangeRef,
};
use arcweft_presentation::appearance::ColorScheme;
use arcweft_view::ViewElementKind;
use arcweft_view::style::{
    ViewEnvironmentClause, ViewEnvironmentCondition, ViewPropertyKind, ViewRatioMilli,
    ViewSpecifiedValue, ViewStyleAssignOp, ViewStyleDeclaration, ViewStyleProgram, ViewStyleRule,
    ViewStyleSelector, ViewStyleSelectorSequence, ViewStyleSheet, ViewStyleSheetId,
    ViewStyleSourceId,
};

#[test]
fn environment_product_round_trips_canonical_json_cbor_msgpack() {
    let (_, _, resource) = fixture();
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
fn environment_sources_resolve_in_final_source_index() {
    let (source, source_id, resource) = fixture();
    let index = SourceMapIndex::from_source(&source).expect("source index");

    resource
        .validate_environment_sources(&index, &source_id)
        .expect("condition, clause, and rule ranges resolve");
    let bytes = resource.encode_canonical_section().expect("Style encodes");
    let decoded = ViewStyleResource::decode_canonical_section(&bytes).expect("Style decodes");
    decoded
        .validate_environment_sources(&index, &source_id)
        .expect("decoded ranges retain final source identities");
}

#[test]
fn environment_source_owner_mismatch_rejects_complete_product() {
    let (source, source_id, mut resource) = fixture();
    let table = resource.public_id_table().expect("public IDs");
    resource.source_map_refs[1].source = table
        .id_for(&resource.style_program_id)
        .expect("program identity");
    let index = SourceMapIndex::from_source(&source).expect("source index");

    assert!(matches!(
        resource.validate_environment_sources(&index, &source_id),
        Err(SectionCodecError::ViewStyleEnvironmentSource(
            ViewStyleEnvironmentSourceError::WrongOwner
        ))
    ));
}

#[test]
fn environment_source_range_out_of_bounds_or_utf8_boundary_rejects() {
    let (source, source_id, resource) = fixture();
    let index = SourceMapIndex::from_source(&source).expect("source index");

    let mut out_of_bounds = resource.clone();
    out_of_bounds.source_map_refs[0].end_byte =
        u32::try_from(source.text.len() + 1).expect("small fixture");
    assert!(matches!(
        out_of_bounds.validate_environment_sources(&index, &source_id),
        Err(SectionCodecError::ViewStyleEnvironmentSource(
            ViewStyleEnvironmentSourceError::SourceOutOfBounds
        ))
    ));

    let mut split_code_point = resource;
    split_code_point.source_map_refs[0].start_byte = 1;
    assert!(matches!(
        split_code_point.validate_environment_sources(&index, &source_id),
        Err(SectionCodecError::ViewStyleEnvironmentSource(
            ViewStyleEnvironmentSourceError::InvalidUtf8Boundary
        ))
    ));
}

#[test]
fn condition_must_contain_clause_ranges() {
    let (source, source_id, mut resource) = fixture();
    let condition_end = resource.source_map_refs[0].end_byte;
    resource.source_map_refs[1].end_byte = condition_end + 1;
    let index = SourceMapIndex::from_source(&source).expect("source index");

    assert!(matches!(
        resource.validate_environment_sources(&index, &source_id),
        Err(SectionCodecError::ViewStyleEnvironmentSource(
            ViewStyleEnvironmentSourceError::ClauseNotContained
        ))
    ));
}

fn fixture() -> (BundleSource, SourceMapSourceId, ViewStyleResource) {
    let text = "éwhen environment(color-scheme == dark) { Button { opacity = 1 } }";
    let source = BundleSource {
        label: "main.arcw".to_owned(),
        text: text.to_owned(),
    };
    let source_id = SourceMapSourceId::try_new(source.label.clone()).expect("source identity");
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
    let sheet = ViewStyleSheet::new(sheet_id.clone(), Vec::new(), vec![rule]).expect("sheet");
    let mut resource = ViewStyleResource {
        style_program_id: "view.style.program".to_owned(),
        program: ViewStyleProgram::try_new(vec![sheet], Vec::new()).expect("program"),
        source_map_refs: vec![
            range(condition_start, condition_end),
            range(clause_start, clause_end),
            range(rule_start, rule_end),
            range(declaration_start, declaration_end),
        ],
        adapter_requirements: Vec::new(),
    };
    let table = resource.public_id_table().expect("public IDs");
    let owner = table
        .id_for(sheet_id.public_id().as_str())
        .expect("sheet owner");
    for range in &mut resource.source_map_refs {
        range.source = owner;
    }
    (source, source_id, resource)
}

fn range(start: usize, end: usize) -> SourceRangeRef {
    SourceRangeRef {
        source: PublicIdRef::default(),
        start_byte: u32::try_from(start).expect("fixture range"),
        end_byte: u32::try_from(end).expect("fixture range"),
    }
}
