use arcweft_bundle::container::BundleSectionKind;
use arcweft_bundle::resource_codec::*;
use std::collections::BTreeSet;

const TINY_TITLE: FieldId = FieldId(1);
const TINY_PUBLIC_ID: FieldId = FieldId(2);

#[test]
fn resource_codec_inventory_is_complete_and_bijective() {
    let mut tags = BTreeSet::new();
    let mut labels = BTreeSet::new();
    let mut magics = BTreeSet::new();
    let mut sections = BTreeSet::new();

    for codec in ProductSectionCodecKind::ALL {
        assert!(tags.insert(codec.encoded()), "duplicate tag for {codec:?}");
        assert!(
            labels.insert(codec.as_str()),
            "duplicate label for {codec:?}"
        );
        assert!(
            magics.insert(codec.magic()),
            "duplicate magic for {codec:?}"
        );
        assert!(
            sections.insert(codec.section_kind().encoded()),
            "duplicate section for {codec:?}"
        );
        assert_eq!(
            ProductSectionCodecKind::from_encoded(codec.encoded()),
            Some(codec)
        );
        assert_eq!(
            ProductSectionCodecKind::from_section_kind(codec.section_kind()),
            Some(codec)
        );
    }

    assert_eq!(
        ProductSectionCodecKind::RuntimeTypes.magic(),
        *b"AWRT\r\n\x1a\n"
    );
    assert_eq!(
        ProductSectionCodecKind::RuntimeTypes.section_kind(),
        BundleSectionKind::RuntimeTypes
    );
    assert_eq!(
        ProductSectionCodecKind::from_section_kind(BundleSectionKind::LocaleCatalog),
        None
    );
    assert_eq!(
        ProductSectionCodecKind::from_section_kind(BundleSectionKind::DebugSymbols),
        None
    );
    assert_eq!(ProductSectionCodecKind::from_encoded(14), None);
}

#[test]
fn resource_codec_header_validates_magic_schema_and_budgets() {
    let header = ProductSectionHeader::new(ProductSectionCodecKind::Entrypoints, 1, 1, 1);
    header
        .validate(8, SectionCodecBudget::default())
        .expect("header validates");

    let mut bad_magic = header.clone();
    bad_magic.magic = ProductSectionCodecKind::RuntimeTypes.magic();
    assert!(matches!(
        bad_magic.validate(8, SectionCodecBudget::default()),
        Err(SectionCodecError::BadMagic { .. })
    ));

    let mut bad_schema = header;
    bad_schema.schema_version = PRODUCT_SECTION_SCHEMA_VERSION + 1;
    assert_eq!(
        bad_schema.validate(8, SectionCodecBudget::default()),
        Err(SectionCodecError::UnsupportedSchema {
            actual: PRODUCT_SECTION_SCHEMA_VERSION + 1,
            expected: PRODUCT_SECTION_SCHEMA_VERSION,
        })
    );

    assert_eq!(
        ProductSectionHeader::new(ProductSectionCodecKind::Entrypoints, 2, 0, 0).validate(
            8,
            SectionCodecBudget {
                strings: 1,
                ..SectionCodecBudget::default()
            }
        ),
        Err(SectionCodecError::BudgetExceeded("strings"))
    );
}

#[test]
fn string_table_sorts_and_deduplicates_values() {
    let table = StringTable::new(["zeta".to_owned(), "alpha".to_owned(), "zeta".to_owned()])
        .expect("strings build");

    assert_eq!(table.values(), &["alpha".to_owned(), "zeta".to_owned()]);
    assert_eq!(table.get(StringId(1)), Ok("zeta"));
    assert_eq!(
        table.get(StringId(2)),
        Err(SectionCodecError::StringOutOfBounds(StringId(2)))
    );
}

#[test]
fn public_id_table_rejects_duplicates_without_deduplicating() {
    let error = PublicIdTable::new([
        "flow.main".to_owned(),
        "flow.other".to_owned(),
        "flow.main".to_owned(),
    ])
    .expect_err("duplicate public ids reject");

    assert_eq!(
        error,
        SectionCodecError::DuplicatePublicId("flow.main".to_owned())
    );
}

#[test]
fn public_id_table_enforces_count_and_byte_budgets() {
    assert_eq!(
        PublicIdTable::with_budget(
            ["flow.main".to_owned()],
            SectionCodecBudget {
                public_ids: 0,
                ..SectionCodecBudget::default()
            }
        ),
        Err(SectionCodecError::BudgetExceeded("public_ids"))
    );

    assert_eq!(
        PublicIdTable::with_budget(
            ["flow.main".to_owned()],
            SectionCodecBudget {
                string_bytes: 4,
                ..SectionCodecBudget::default()
            }
        ),
        Err(SectionCodecError::BudgetExceeded("string_bytes"))
    );
}

#[test]
fn common_wire_bytes_are_deterministic_for_canonical_table_ordering() {
    let first = tiny_fixture_envelope(
        [
            "title".to_owned(),
            "flow.main".to_owned(),
            "title".to_owned(),
        ],
        ["flow.main".to_owned()],
        vec![
            ResourceField::required(
                TINY_PUBLIC_ID,
                ResourceWireType::PublicIdRef,
                0_u32.to_le_bytes(),
            ),
            ResourceField::required(TINY_TITLE, ResourceWireType::StringRef, 1_u32.to_le_bytes()),
        ],
    );
    let second = tiny_fixture_envelope(
        ["flow.main".to_owned(), "title".to_owned()],
        ["flow.main".to_owned()],
        vec![
            ResourceField::required(TINY_TITLE, ResourceWireType::StringRef, 1_u32.to_le_bytes()),
            ResourceField::required(
                TINY_PUBLIC_ID,
                ResourceWireType::PublicIdRef,
                0_u32.to_le_bytes(),
            ),
        ],
    );

    assert_eq!(
        first.encode_canonical().expect("first encodes"),
        second.encode_canonical().expect("second encodes")
    );
}

#[test]
fn duplicate_table_entries_are_rejected_when_decoding_non_canonical_bytes() {
    let mut bytes = tiny_fixture_envelope(
        ["alfa".to_owned(), "beta".to_owned()],
        ["flow.main".to_owned()],
        tiny_fields(),
    )
    .encode_canonical()
    .expect("fixture encodes");

    let second_string_bytes = PRODUCT_SECTION_HEADER_LEN + 4 + 4 + 4;
    bytes[second_string_bytes..second_string_bytes + 4].copy_from_slice(b"alfa");
    assert_eq!(
        ProductResourceEnvelope::decode_with_registry(
            &bytes,
            ProductSectionCodecKind::RuntimeTypes,
            &tiny_registry(),
            SectionCodecBudget::default(),
        ),
        Err(SectionCodecError::DuplicateString("alfa".to_owned()))
    );
}

#[test]
fn unknown_optional_fields_skip_and_unknown_required_fields_reject() {
    let registry = tiny_registry();
    let optional_unknown =
        ResourceField::optional(FieldId(99), ResourceWireType::Bytes, b"new".to_vec());
    let mut fields = tiny_fields();
    fields.push(optional_unknown);
    let bytes = tiny_fixture_envelope(
        ["flow.main".to_owned(), "title".to_owned()],
        ["flow.main".to_owned()],
        fields,
    )
    .encode_canonical()
    .expect("fixture encodes");

    let decoded = ProductResourceEnvelope::decode_with_registry(
        &bytes,
        ProductSectionCodecKind::RuntimeTypes,
        &registry,
        SectionCodecBudget::default(),
    )
    .expect("unknown optional skips");
    assert_eq!(decoded.skipped_unknown_optional_fields, 1);
    assert_eq!(decoded.envelope.fields.len(), 2);

    let mut required_unknown_fields = tiny_fields();
    required_unknown_fields.push(ResourceField::required(
        FieldId(100),
        ResourceWireType::Bytes,
        b"new".to_vec(),
    ));
    let required_unknown = tiny_fixture_envelope(
        ["flow.main".to_owned(), "title".to_owned()],
        ["flow.main".to_owned()],
        required_unknown_fields,
    )
    .encode_canonical()
    .expect("fixture encodes");

    assert_eq!(
        ProductResourceEnvelope::decode_with_registry(
            &required_unknown,
            ProductSectionCodecKind::RuntimeTypes,
            &registry,
            SectionCodecBudget::default(),
        ),
        Err(SectionCodecError::UnknownRequiredField(FieldId(100)))
    );
}

#[test]
fn budgets_fail_for_byte_count_string_and_item_limits() {
    let bytes = tiny_fixture_envelope(
        ["flow.main".to_owned(), "title".to_owned()],
        ["flow.main".to_owned()],
        tiny_fields(),
    )
    .encode_canonical()
    .expect("fixture encodes");

    assert_eq!(
        ProductResourceEnvelope::decode_with_registry(
            &bytes,
            ProductSectionCodecKind::RuntimeTypes,
            &tiny_registry(),
            SectionCodecBudget {
                bytes: 1,
                ..SectionCodecBudget::default()
            },
        ),
        Err(SectionCodecError::BudgetExceeded("bytes"))
    );
    assert_eq!(
        ProductResourceEnvelope::decode_with_registry(
            &bytes,
            ProductSectionCodecKind::RuntimeTypes,
            &tiny_registry(),
            SectionCodecBudget {
                strings: 1,
                ..SectionCodecBudget::default()
            },
        ),
        Err(SectionCodecError::BudgetExceeded("strings"))
    );
    assert_eq!(
        ProductResourceEnvelope::decode_with_registry(
            &bytes,
            ProductSectionCodecKind::RuntimeTypes,
            &tiny_registry(),
            SectionCodecBudget {
                items: 1,
                ..SectionCodecBudget::default()
            },
        ),
        Err(SectionCodecError::BudgetExceeded("items"))
    );
}

#[test]
fn budgets_fail_for_depth_reference_and_fanout_limits() {
    let deep = ProductResourceEnvelope::with_budget(
        ProductSectionCodecKind::RuntimeTypes,
        StringTable::new(["title".to_owned(), "flow.main".to_owned()]).expect("strings"),
        PublicIdTable::new(["flow.main".to_owned()]).expect("public ids"),
        EnumRegistry::default(),
        [ResourceField::new(
            TINY_TITLE,
            FieldRequirement::Required,
            ResourceWireType::StringRef,
            2,
            0,
            0_u32.to_le_bytes(),
        )],
        1,
        SectionCodecBudget {
            depth: 1,
            ..SectionCodecBudget::default()
        },
    );
    assert_eq!(deep, Err(SectionCodecError::BudgetExceeded("depth")));

    let reference_heavy = ProductResourceEnvelope::with_budget(
        ProductSectionCodecKind::RuntimeTypes,
        StringTable::new(["title".to_owned(), "flow.main".to_owned()]).expect("strings"),
        PublicIdTable::new(["flow.main".to_owned()]).expect("public ids"),
        EnumRegistry::default(),
        [ResourceField::new(
            TINY_TITLE,
            FieldRequirement::Required,
            ResourceWireType::StringRef,
            0,
            2,
            0_u32.to_le_bytes(),
        )],
        1,
        SectionCodecBudget {
            references: 1,
            ..SectionCodecBudget::default()
        },
    );
    assert_eq!(
        reference_heavy,
        Err(SectionCodecError::BudgetExceeded("references"))
    );

    let fanout = ProductResourceEnvelope::with_budget(
        ProductSectionCodecKind::RuntimeTypes,
        StringTable::new(["title".to_owned(), "flow.main".to_owned()]).expect("strings"),
        PublicIdTable::new(["flow.main".to_owned()]).expect("public ids"),
        EnumRegistry::default(),
        tiny_fields(),
        1,
        SectionCodecBudget {
            table_fan_out: 1,
            ..SectionCodecBudget::default()
        },
    );
    assert_eq!(
        fanout,
        Err(SectionCodecError::BudgetExceeded("table_fan_out"))
    );
}

#[test]
fn canonical_digest_is_stable_for_equivalent_logical_resources() {
    let first = tiny_fixture_envelope(
        ["title".to_owned(), "flow.main".to_owned()],
        ["flow.main".to_owned()],
        tiny_fields(),
    );
    let mut reversed = tiny_fields();
    reversed.reverse();
    let second = tiny_fixture_envelope(
        [
            "flow.main".to_owned(),
            "title".to_owned(),
            "title".to_owned(),
        ],
        ["flow.main".to_owned()],
        reversed,
    );

    assert_eq!(
        first.canonical_digest().expect("first digests"),
        second.canonical_digest().expect("second digests")
    );
}

#[test]
fn inspection_json_round_trips_through_typed_owner_api_not_product_json_fallback() {
    let typed = TinyFixtureSection {
        title: "title".to_owned(),
        public_id: "flow.main".to_owned(),
    };
    let bytes = typed
        .to_envelope()
        .expect("typed envelope")
        .encode_canonical()
        .expect("encodes");
    let decoded = ProductResourceEnvelope::decode_with_registry(
        &bytes,
        ProductSectionCodecKind::RuntimeTypes,
        &tiny_registry(),
        SectionCodecBudget::default(),
    )
    .expect("decodes");
    let inspection_json = decoded
        .envelope
        .inspection_json_bytes()
        .expect("inspection exports");
    let inspection: ResourceInspection =
        serde_json::from_slice(&inspection_json).expect("inspection JSON is readable");

    assert_eq!(inspection.codec, ProductSectionCodecKind::RuntimeTypes);
    assert_eq!(
        TinyFixtureSection::from_envelope(&decoded.envelope).expect("typed imports"),
        typed
    );
}

#[derive(Debug, Eq, PartialEq)]
struct TinyFixtureSection {
    title: String,
    public_id: String,
}

impl TinyFixtureSection {
    fn to_envelope(&self) -> Result<ProductResourceEnvelope, SectionCodecError> {
        let strings = StringTable::new([self.title.clone(), self.public_id.clone()])?;
        let public_ids = PublicIdTable::new([self.public_id.clone()])?;
        let title = strings.id_for(&self.title).expect("title is interned");
        let public_id = public_ids
            .id_for(&self.public_id)
            .expect("public id is interned");
        ProductResourceEnvelope::new(
            ProductSectionCodecKind::RuntimeTypes,
            strings,
            public_ids,
            EnumRegistry::default(),
            [
                ResourceField::required(
                    TINY_TITLE,
                    ResourceWireType::StringRef,
                    title.0.to_le_bytes(),
                ),
                ResourceField::required(
                    TINY_PUBLIC_ID,
                    ResourceWireType::PublicIdRef,
                    public_id.0.to_le_bytes(),
                ),
            ],
            1,
        )
    }

    fn from_envelope(envelope: &ProductResourceEnvelope) -> Result<Self, SectionCodecError> {
        let title_ref = envelope
            .fields
            .iter()
            .find(|field| field.id == TINY_TITLE)
            .ok_or(SectionCodecError::MissingRequiredField(TINY_TITLE))?;
        let public_id_ref = envelope
            .fields
            .iter()
            .find(|field| field.id == TINY_PUBLIC_ID)
            .ok_or(SectionCodecError::MissingRequiredField(TINY_PUBLIC_ID))?;
        let title_id = StringId(u32::from_le_bytes(
            title_ref
                .payload
                .as_slice()
                .try_into()
                .map_err(|_| SectionCodecError::Truncated)?,
        ));
        let public_id = PublicIdRef(u32::from_le_bytes(
            public_id_ref
                .payload
                .as_slice()
                .try_into()
                .map_err(|_| SectionCodecError::Truncated)?,
        ));
        Ok(Self {
            title: envelope.strings.get(title_id)?.to_owned(),
            public_id: envelope.public_ids.get(public_id)?.to_owned(),
        })
    }
}

fn tiny_fixture_envelope(
    strings: impl IntoIterator<Item = String>,
    public_ids: impl IntoIterator<Item = String>,
    fields: Vec<ResourceField>,
) -> ProductResourceEnvelope {
    let strings = StringTable::new(strings).expect("strings");
    let public_ids = PublicIdTable::new(public_ids).expect("public ids");
    ProductResourceEnvelope::new(
        ProductSectionCodecKind::RuntimeTypes,
        strings,
        public_ids,
        EnumRegistry::default(),
        fields,
        1,
    )
    .expect("envelope")
}

fn tiny_fields() -> Vec<ResourceField> {
    vec![
        ResourceField::required(TINY_TITLE, ResourceWireType::StringRef, 1_u32.to_le_bytes()),
        ResourceField::required(
            TINY_PUBLIC_ID,
            ResourceWireType::PublicIdRef,
            0_u32.to_le_bytes(),
        ),
    ]
}

fn tiny_registry() -> FieldRegistry {
    FieldRegistry::new([
        FieldSpec::required(TINY_TITLE, ResourceWireType::StringRef),
        FieldSpec::required(TINY_PUBLIC_ID, ResourceWireType::PublicIdRef),
    ])
    .expect("registry")
}
